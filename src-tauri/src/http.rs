use crate::error::*;
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{io::AsyncWriteExt, sync::Semaphore, task::JoinSet};

/// Верхний предел одной загрузки: защита от бесконечного/гигантского ответа.
const MAX_DOWNLOAD_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct Dl {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub size: u64,
}

#[derive(Clone, serde::Serialize)]
pub struct Progress {
    pub id: String,
    pub pct: f32,
}

/// Проверяет ссылку загрузки: только https и без локальных/приватных адресов.
/// Ссылки приходят из удалённых JSON (манифесты, каталоги, ответы прокси),
/// поэтому их нельзя передавать в `reqwest` как есть.
pub fn validate_url(url: &str) -> Res<()> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| AppError::Msg("Ссылка загрузки должна быть https".into()))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim_matches(['[', ']']);
    if host.is_empty() {
        return err("Некорректная ссылка загрузки");
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let local = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_broadcast()
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
        };
        if local {
            return err("Недопустимый адрес загрузки");
        }
    }
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return err("Недопустимый адрес загрузки");
    }
    Ok(())
}

pub async fn sha1_file(p: &Path) -> Res<String> {
    let p = p.to_path_buf();
    tokio::task::spawn_blocking(move || -> Res<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(p)?;
        let mut h = Sha1::new();
        let mut b = vec![0u8; 65536];
        loop {
            let n = f.read(&mut b)?;
            if n == 0 {
                break;
            }
            h.update(&b[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await?
}

async fn sha256_file(p: &Path) -> Res<String> {    let p = p.to_path_buf();
    tokio::task::spawn_blocking(move || -> Res<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(p)?;
        let mut h = Sha256::new();
        let mut b = vec![0u8; 65536];
        loop {
            let n = f.read(&mut b)?;
            if n == 0 {
                break;
            }
            h.update(&b[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await?
}

/// SHA-1, опубликованный рядом с файлом (Maven `.sha1`). `None`, если источника
/// нет или ответ не похож на sha1 — тогда вызывающий решает, что делать.
pub async fn fetch_sha1(c: &reqwest::Client, url: &str) -> Option<String> {
    let r = c
        .get(format!("{url}.sha1"))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let text = r.text().await.ok()?;
    let hex = text.split_whitespace().next()?.trim();
    if hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(hex.to_ascii_lowercase())
    } else {
        None
    }
}

async fn fetch(c: &reqwest::Client, d: &Dl, on: &(dyn Fn(u64) + Send + Sync)) -> Res<()> {
    validate_url(&d.url)?;
    let r = c.get(&d.url).send().await?.error_for_status()?;
    let part = d.path.with_extension("part");
    let mut f = tokio::fs::File::create(&part).await?;
    let mut h1 = d.sha1.is_some().then(Sha1::new);
    let mut h2 = d.sha256.is_some().then(Sha256::new);
    let mut got: u64 = 0;
    let mut s = r.bytes_stream();
    while let Some(ch) = s.next().await {
        let ch = ch?;
        got = got.saturating_add(ch.len() as u64);
        if got > MAX_DOWNLOAD_BYTES {
            drop(f);
            let _ = tokio::fs::remove_file(&part).await;
            return err("Файл слишком большой");
        }
        if let Some(h) = h1.as_mut() {
            h.update(&ch);
        }
        if let Some(h) = h2.as_mut() {
            h.update(&ch);
        }
        f.write_all(&ch).await?;
        on(ch.len() as u64);
    }
    f.flush().await?;
    drop(f);
    if let Some(want) = &d.sha1 {
        let got = h1.map(|h| hex::encode(h.finalize())).unwrap_or_default();
        if !got.eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            return err("Контрольная сумма файла не совпала");
        }
    }
    if let Some(want) = &d.sha256 {
        let got = h2.map(|h| hex::encode(h.finalize())).unwrap_or_default();
        if !got.eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            return err("Контрольная сумма файла не совпала");
        }
    }
    tokio::fs::rename(&part, &d.path).await?;
    Ok(())
}

async fn one(c: &reqwest::Client, d: &Dl, on: &(dyn Fn(u64) + Send + Sync)) -> Res<()> {
    if let Ok(m) = tokio::fs::metadata(&d.path).await {
        let fresh = if let Some(s) = &d.sha1 {
            sha1_file(&d.path).await?.eq_ignore_ascii_case(s)
        } else if let Some(s) = &d.sha256 {
            sha256_file(&d.path).await?.eq_ignore_ascii_case(s)
        } else {
            // Без хеша доверяем только совпадению непустого размера.
            d.size > 0 && m.len() == d.size
        };
        if fresh {
            on(d.size);
            return Ok(());
        }
    }
    if let Some(p) = d.path.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    let mut last = AppError::Msg("Ошибка загрузки".into());
    for a in 0..3u64 {
        match fetch(c, d, on).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e;
                tokio::time::sleep(Duration::from_millis(400 * (a + 1))).await;
            }
        }
    }
    Err(last)
}

/// Скачивание одиночного файла без AppHandle и без отчёта о прогрессе.
pub async fn download_file(c: &reqwest::Client, d: &Dl) -> Res<()> {
    one(c, d, &|_| {}).await
}

/// pct прогресса отображается в диапазоне lo..hi
pub async fn download_all(
    app: &AppHandle,
    c: &reqwest::Client,
    id: &str,
    items: Vec<Dl>,
    lo: f32,
    hi: f32,
) -> Res<()> {
    let total: u64 = items.iter().map(|d| d.size.max(1)).sum::<u64>().max(1);
    let done = Arc::new(AtomicU64::new(0));
    let last_emit = Arc::new(AtomicU64::new(0));
    let sem = Arc::new(Semaphore::new(16));
    let mut set = JoinSet::new();
    for d in items {
        let (c, sem, done, last_emit, app, id) = (
            c.clone(),
            sem.clone(),
            done.clone(),
            last_emit.clone(),
            app.clone(),
            id.to_string(),
        );
        set.spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| AppError::Msg(e.to_string()))?;
            let cb = |n: u64| {
                let v = done.fetch_add(n, Ordering::Relaxed) + n;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|x| x.as_millis() as u64)
                    .unwrap_or(0);
                if now.saturating_sub(last_emit.load(Ordering::Relaxed)) > 100 {
                    last_emit.store(now, Ordering::Relaxed);
                    let _ = app.emit(
                        "progress",
                        Progress {
                            id: id.clone(),
                            pct: lo + (hi - lo) * (v as f32 / total as f32).min(1.0),
                        },
                    );
                }
            };
            one(&c, &d, &cb).await
        });
    }
    while let Some(r) = set.join_next().await {
        r??;
    }
    let _ = app.emit(
        "progress",
        Progress {
            id: id.to_string(),
            pct: hi,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_url_allows_https_hosts() {
        assert!(validate_url("https://cdn.modrinth.com/data/x.jar").is_ok());
        assert!(validate_url("https://edge.forgecdn.net/files/1/2/x.jar").is_ok());
        assert!(validate_url("https://example.com:8443/x").is_ok());
    }

    #[test]
    fn validate_url_rejects_plain_http_and_other_schemes() {
        assert!(validate_url("http://example.com/x.jar").is_err());
        assert!(validate_url("ftp://example.com/x").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("/etc/passwd").is_err());
    }

    #[test]
    fn validate_url_rejects_local_addresses() {
        assert!(validate_url("https://127.0.0.1/x").is_err());
        assert!(validate_url("https://localhost/x").is_err());
        assert!(validate_url("https://10.0.0.5/x").is_err());
        assert!(validate_url("https://192.168.1.10/x").is_err());
        assert!(validate_url("https://169.254.169.254/latest/meta-data").is_err());
    }
}
