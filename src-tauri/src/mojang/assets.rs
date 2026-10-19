use crate::error::*;
use crate::http::{download_all, Dl};
use crate::paths;
use std::collections::HashMap;

const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(serde::Deserialize)]
struct IndexFile {
    objects: HashMap<String, AssetObj>,
}

#[derive(serde::Deserialize)]
struct AssetObj {
    hash: String,
    size: u64,
}

/// Скачивает индекс ассетов и строит план загрузки всех объектов.
pub async fn plan(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
    id: &str,
    idx: &crate::mojang::versions::AssetIndex,
    lo: f32,
) -> Res<Vec<Dl>> {
    let index_path = paths::assets_indexes_dir().join(format!("{}.json", idx.id));

    download_all(
        app,
        http,
        id,
        vec![Dl {
            url: idx.url.clone(),
            path: index_path.clone(),
            sha1: idx.sha1.clone(),
            sha256: None,
            size: idx.size,
        }],
        lo,
        lo + 1.0,
    )
    .await?;

    let parsed = tokio::task::spawn_blocking(move || -> Res<IndexFile> {
        let text = std::fs::read_to_string(&index_path)?;
        Ok(serde_json::from_str(&text)?)
    })
    .await??;

    let objects_dir = paths::assets_objects_dir();
    let mut items = Vec::with_capacity(parsed.objects.len());
    for o in parsed.objects.into_values() {
        // Хеш из индекса формирует путь — принимаем только корректный sha1.
        if o.hash.len() != 40 || !o.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let prefix = o.hash[..2].to_string();
        let url = format!("{RESOURCES_URL}/{prefix}/{}", o.hash);
        let path = objects_dir.join(&prefix).join(&o.hash);
        items.push(Dl {
            url,
            path,
            sha1: Some(o.hash.clone()),
            sha256: None,
            size: o.size,
        });
    }

    Ok(items)
}
