use crate::error::{err, Res};
use crate::models::{Account, Instance};
use crate::paths;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Счётчик временных файлов: две параллельные записи одного JSON не должны
/// делить один `.tmp` (иначе одна перезапись утащит чужой снимок).
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Читает JSON. Если файла нет — возвращает `T::default()`.
pub async fn read_json<T: DeserializeOwned + Default>(path: &Path) -> Res<T> {
    match tokio::fs::read_to_string(path).await {
        Ok(s) => Ok(serde_json::from_str(&s)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

/// Атомарная запись JSON: во временный файл рядом с целевым, затем `rename`.
pub async fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Res<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("data");
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{name}.{}.{seq}.tmp", std::process::id()));
    let data = serde_json::to_string_pretty(value)?;
    if let Err(e) = tokio::fs::write(&tmp, data).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    // На Windows rename может кратковременно упереться в антивирус или
    // индексатор, который держит целевой файл, — пара повторов это лечит.
    let mut last = None;
    for attempt in 0..4u64 {
        match tokio::fs::rename(&tmp, path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(20 * (attempt + 1))).await;
            }
        }
    }
    let _ = tokio::fs::remove_file(&tmp).await;
    match last {
        Some(e) => Err(e.into()),
        None => err("Не удалось сохранить файл"),
    }
}

/// Читает `instances/<id>/instance.json`. Нет файла или битый JSON — «Профиль не найден».
pub async fn read_instance(id: &str) -> Res<Instance> {
    let path = paths::instance_json(id);
    let raw = match tokio::fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return err("Профиль не найден"),
    };
    match serde_json::from_str::<Instance>(&raw) {
        Ok(inst) => Ok(inst),
        Err(_) => err("Профиль не найден"),
    }
}

/// Пишет `instances/<id>/instance.json`.
pub async fn write_instance(inst: &Instance) -> Res<()> {
    write_json_atomic(&paths::instance_json(&inst.id), inst).await
}

/// Читает `accounts.json` (без секретов).
pub async fn read_accounts() -> Res<Vec<Account>> {
    read_json(&paths::accounts_file()).await
}

/// Пишет `accounts.json`.
pub async fn write_accounts(list: &[Account]) -> Res<()> {
    write_json_atomic(&paths::accounts_file(), &list.to_vec()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Параллельные записи не должны делить временный файл и оставлять мусор.
    #[tokio::test]
    async fn concurrent_writes_leave_valid_json_and_no_tmp() {
        let dir = std::env::temp_dir().join(format!("arbuz-store-{}", std::process::id()));
        if let Err(e) = tokio::fs::create_dir_all(&dir).await {
            panic!("temp dir: {e}");
        }
        let path = dir.join("test.json");
        let mut tasks = Vec::new();
        for i in 0..16u32 {
            let p = path.clone();
            tasks.push(tokio::spawn(async move {
                write_json_atomic(&p, &vec![i.to_string()]).await
            }));
        }
        for t in tasks {
            match t.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("write failed: {e}"),
                Err(e) => panic!("join failed: {e}"),
            }
        }
        let back: Vec<String> = match read_json(&path).await {
            Ok(v) => v,
            Err(e) => panic!("read back: {e}"),
        };
        assert_eq!(back.len(), 1);

        let mut rd = match tokio::fs::read_dir(&dir).await {
            Ok(d) => d,
            Err(e) => panic!("read dir: {e}"),
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.ends_with(".tmp"), "leftover temp file: {name}");
        }
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
