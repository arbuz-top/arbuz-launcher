//! CurseForge (ТЗ §10.2).

use super::{push_link, str_arr, str_of, strip_html, CatalogItem, ProjectDetails, SearchArgs, SearchResult};
use crate::content::{self, ContentMeta, Entry};
use crate::error::{err, Res};
use crate::http::{download_all, Dl};
use crate::models::{Instance, Loader};
use crate::state::AppState;
use serde_json::Value;
use tauri::AppHandle;

const API: &str = "https://api.curseforge.com/v1";
/// Публичный прокси CurseForge API: тот же формат ответа, но без ключа.
/// Нужен, чтобы CurseForge работал сразу после установки, без настройки.
const API_PROXY: &str = "https://api.curse.tools/v1";

/// Базовый адрес API: со своим ключом — официальный, без него — прокси.
fn base_for(key: &str) -> &'static str {
    if key.trim().is_empty() {
        API_PROXY
    } else {
        API
    }
}

/// Базовый адрес API и ключ: со своим ключом — официальный API, без него — прокси.
pub(crate) async fn endpoint(state: &AppState) -> (String, Option<String>) {
    let k = state.config.lock().await.curseforge_api_key.trim().to_string();
    let base = base_for(&k);
    if k.is_empty() {
        (base.to_string(), None)
    } else {
        (base.to_string(), Some(k))
    }
}

/// GET к CurseForge; ключ отправляется только когда он задан.
pub(crate) fn get(state: &AppState, url: &str, key: &Option<String>) -> reqwest::RequestBuilder {
    let rb = state.http.get(url).header("Accept", "application/json");
    match key {
        Some(k) => rb.header("x-api-key", k),
        None => rb,
    }
}

/// POST к CurseForge; ключ отправляется только когда он задан.
pub(crate) fn post(state: &AppState, url: &str, key: &Option<String>) -> reqwest::RequestBuilder {
    let rb = state.http.post(url).header("Accept", "application/json");
    match key {
        Some(k) => rb.header("x-api-key", k),
        None => rb,
    }
}

fn class_id(kind: &str) -> u32 {
    match kind {
        "mod" => 6,
        "rp" => 12,
        "pk" => 4471,
        "sh" => 6552,
        "dp" => 6945,
        _ => 6,
    }
}

fn sort_field(sort: &str) -> u32 {
    match sort {
        "newest" => 11,
        "updated" => 3,
        _ => 6, // downloads
    }
}

/// Название загрузчика в терминах CurseForge (`gameVersions`).
fn loader_name(l: Loader) -> Option<&'static str> {
    match l {
        Loader::Forge => Some("Forge"),
        Loader::Fabric => Some("Fabric"),
        Loader::Quilt => Some("Quilt"),
        Loader::Neoforge => Some("NeoForge"),
        Loader::Vanilla => None,
    }
}

fn loader_type(l: Loader) -> Option<u32> {
    match l {
        Loader::Forge => Some(1),
        Loader::Fabric => Some(4),
        Loader::Quilt => Some(5),
        Loader::Neoforge => Some(6),
        Loader::Vanilla => None,
    }
}

/// `modLoaderType` передаём только для `mod`/`pk` и только при одном загрузчике.
fn search_loader_type(a: &SearchArgs) -> Option<u32> {
    if a.kind != "mod" && a.kind != "pk" {
        return None;
    }
    if a.loaders.len() != 1 {
        return None;
    }
    match a.loaders[0].to_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn install_loader_type(kind: &str, l: Loader) -> Option<u32> {
    if kind != "mod" && kind != "pk" {
        return None;
    }
    loader_type(l)
}

/// `modLoaderType` по slug загрузчика (`fabric`, `forge`, …).
fn slug_loader_type(kind: &str, slug: &str) -> Option<u32> {
    if kind != "mod" && kind != "pk" {
        return None;
    }
    match slug.to_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Есть ли у проекта файл под версию игры и загрузчик профиля.
pub async fn has_file(
    state: &AppState,
    project_id: &str,
    kind: &str,
    loader: &str,
    mc: &str,
) -> Res<bool> {
    let (api, key) = endpoint(state).await;
    let mut url = format!("{api}/mods/{project_id}/files?pageSize=1");
    if !super::any_version(kind) && !mc.is_empty() {
        url.push_str(&format!("&gameVersion={}", urlencoding::encode(mc)));
    }
    if let Some(t) = slug_loader_type(kind, loader) {
        url.push_str(&format!("&modLoaderType={t}"));
    }
    let v: Value = get(state, &url, &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(v.get("data")
        .and_then(|x| x.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false))
}

fn str_list(v: &Value, key: &str, sub: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|c| c.get(sub).and_then(|x| x.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Ссылка на файл, когда API не отдаёт `downloadUrl`.
pub(crate) fn forgecdn_url(id: i64, name: &str) -> String {
    format!("https://edge.forgecdn.net/files/{}/{}/{}", id / 1000, id % 1000, name)
}

/// Домены CDN, с которых CurseForge отдаёт файлы.
const CDN_HOSTS: [&str; 3] = [
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "media.forgecdn.net",
];

/// Ссылка ведёт на известный CDN CurseForge (только https).
pub(crate) fn is_cdn_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host);
    CDN_HOSTS.iter().any(|h| host.eq_ignore_ascii_case(h))
}

pub async fn search(state: &AppState, a: &SearchArgs) -> Res<SearchResult> {
    let (api, key) = endpoint(state).await;
    let mut url = format!(
        "{api}/mods/search?gameId=432&classId={}&searchFilter={}&sortField={}&sortOrder=desc&pageSize=20&index={}",
        class_id(&a.kind),
        urlencoding::encode(&a.query),
        sort_field(&a.sort),
        a.offset
    );
    if let Some(v) = &a.mc_version {
        if !v.is_empty() && !super::any_version(&a.kind) {
            url.push_str(&format!("&gameVersion={}", urlencoding::encode(v)));
        }
    }
    if let Some(t) = search_loader_type(a) {
        url.push_str(&format!("&modLoaderType={t}"));
    }

    let v: Value = get(state, &url, &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let total = v
        .pointer("/pagination/totalCount")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    let mut hits = Vec::new();
    if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
        for m in arr {
            hits.push(CatalogItem {
                id: m
                    .get("id")
                    .and_then(|x| x.as_i64())
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                title: str_of(m, "name"),
                author: m
                    .pointer("/authors/0/name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                description: str_of(m, "summary"),
                downloads: m.get("downloadCount").and_then(|x| x.as_u64()).unwrap_or(0),
                loaders: Vec::new(),
                categories: str_list(m, "categories", "name"),
                icon_url: m
                    .pointer("/logo/thumbnailUrl")
                    .and_then(|x| x.as_str())
                    .map(str::to_string),
                source: "curseforge".into(),
                kind: a.kind.clone(),
                installed: false,
            });
        }
    }
    Ok(SearchResult { hits, total })
}

/// Установка файла проекта. Зависимости CurseForge не обрабатываются (ТЗ §10.2).
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    inst: &Instance,
    project_id: &str,
    kind: &str,
    dir: &str,
    meta: &mut ContentMeta,
) -> Res<()> {
    let (api, key) = endpoint(state).await;
    let mut url = format!("{api}/mods/{project_id}/files?pageSize=1");
    // Текстуры и шейдеры не привязаны к версии игры — берём свежий файл.
    if !inst.mc_version.is_empty() && !super::any_version(kind) {
        url.push_str(&format!("&gameVersion={}", urlencoding::encode(&inst.mc_version)));
    }
    if let Some(t) = install_loader_type(kind, inst.loader) {
        url.push_str(&format!("&modLoaderType={t}"));
    }

    let v: Value = get(state, &url, &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let Some(f) = v.get("data").and_then(|x| x.as_array()).and_then(|a| a.first()) else {
        return err("У проекта нет файлов, подходящих профилю");
    };

    // Проверяем совместимость файла с профилем: API мог отдать файл другого
    // загрузчика или другой версии игры, если фильтр не сработал.
    let games = str_arr(f, "gameVersions");
    if !super::any_version(kind)
        && !inst.mc_version.is_empty()
        && !games.iter().any(|g| g == &inst.mc_version)
    {
        return err(format!(
            "У проекта нет файла для Minecraft {}",
            inst.mc_version
        ));
    }
    if kind == "mod" || kind == "pk" {
        if let Some(name) = loader_name(inst.loader) {
            if !games.iter().any(|g| g == name) {
                return err(format!("У проекта нет файла для {name}"));
            }
        }
    }

    let file_id = f.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
    let filename = str_of(f, "fileName");
    if filename.is_empty() {
        return err("У файла нет имени");
    }
    // algo == 1 — это sha1.
    let sha1 = f
        .get("hashes")
        .and_then(|x| x.as_array())
        .and_then(|a| a.iter().find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1)))
        .and_then(|h| h.get("value").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string();
    let file_url = match f.get("downloadUrl").and_then(|x| x.as_str()) {
        // Со своим ключом ответ приходит напрямую от CurseForge. Через публичный
        // прокси ссылку принимаем только на известный CDN — иначе прокси мог бы
        // увести загрузку на произвольный хост вместе со своим же хешем.
        Some(u) if !u.is_empty() && (key.is_some() || is_cdn_url(u)) => u.to_string(),
        _ => forgecdn_url(file_id, &filename),
    };
    let size = f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);

    let rel = format!("{dir}/{filename}");
    let path = content::safe_join(&crate::paths::instance_minecraft_dir(&inst.id), &rel)?;
    let dl = Dl {
        url: file_url,
        path,
        sha1: if sha1.is_empty() { None } else { Some(sha1.clone()) },
        sha256: None,
        size,
    };
    download_all(
        app,
        &state.http,
        &super::content_progress_id(&inst.id),
        vec![dl],
        0.0,
        100.0,
    )
    .await?;

    meta.files.insert(
        rel,
        Entry {
            source: "curseforge".into(),
            project_id: project_id.to_string(),
            version_id: file_id.to_string(),
            sha1,
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_key_uses_public_proxy() {
        assert_eq!(base_for(""), API_PROXY);
        assert_eq!(base_for("   "), API_PROXY);
    }

    #[test]
    fn with_key_uses_official_api() {
        assert_eq!(base_for("$2a$10$abc"), API);
    }

    #[test]
    fn loader_names_match_curseforge_tags() {
        assert_eq!(loader_name(Loader::Fabric), Some("Fabric"));
        assert_eq!(loader_name(Loader::Forge), Some("Forge"));
        assert_eq!(loader_name(Loader::Neoforge), Some("NeoForge"));
        assert_eq!(loader_name(Loader::Quilt), Some("Quilt"));
        // У ванильного профиля загрузчика нет — фильтровать не по чему.
        assert_eq!(loader_name(Loader::Vanilla), None);
    }

    #[test]
    fn only_known_cdn_hosts_are_trusted() {
        assert!(is_cdn_url("https://edge.forgecdn.net/files/123/456/x.jar"));
        assert!(is_cdn_url("https://mediafilez.forgecdn.net/files/1/2/x.jar"));
        assert!(is_cdn_url("https://media.forgecdn.net/files/1/2/x.jar"));
        assert!(!is_cdn_url("http://edge.forgecdn.net/files/1/2/x.jar"));
        assert!(!is_cdn_url("https://evil.example/x.jar"));
        assert!(!is_cdn_url("https://edge.forgecdn.net.evil.example/x.jar"));
    }
}

/// Подробности проекта для страницы мода.
pub async fn project(state: &AppState, id: &str) -> Res<ProjectDetails> {
    let (base, key) = endpoint(state).await;
    let v: Value = get(state, &format!("{base}/mods/{id}"), &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let d = &v["data"];
    let mut links = Vec::new();
    push_link(&mut links, "Исходный код", d["links"]["sourceUrl"].as_str());
    push_link(&mut links, "Багрепорты", d["links"]["issuesUrl"].as_str());
    push_link(&mut links, "Вики", d["links"]["wikiUrl"].as_str());
    // Полное описание приходит отдельным запросом и в виде HTML.
    let body = match get(state, &format!("{base}/mods/{id}/description"), &key).send().await {
        Ok(r) => match r.json::<Value>().await {
            Ok(j) => strip_html(j.get("data").and_then(|x| x.as_str()).unwrap_or("")),
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    };
    Ok(ProjectDetails {
        id: id.to_string(),
        source: "curseforge".to_string(),
        title: str_of(d, "name"),
        description: str_of(d, "summary"),
        body,
        downloads: d.get("downloadCount").and_then(|x| x.as_u64()).unwrap_or(0),
        icon_url: d["logo"]["url"]
            .as_str()
            .or_else(|| d["logo"]["thumbnailUrl"].as_str())
            .map(str::to_string),
        links,
    })
}