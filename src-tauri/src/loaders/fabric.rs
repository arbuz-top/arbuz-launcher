//! Fabric — ТЗ §7.6.

use crate::error::*;
use crate::loaders::LoaderVersion;

const META: &str = "https://meta.fabricmc.net/v2/versions/loader";

#[derive(serde::Deserialize)]
struct Entry {
    loader: Info,
}

#[derive(serde::Deserialize)]
struct Info {
    version: String,
    #[serde(default)]
    stable: bool,
}

/// Версии загрузчика Fabric для указанной версии Minecraft (от новых к старым).
pub async fn versions(http: &reqwest::Client, mc: &str) -> Res<Vec<LoaderVersion>> {
    let list: Vec<Entry> = http
        .get(format!("{META}/{mc}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(list
        .into_iter()
        .map(|e| LoaderVersion {
            version: e.loader.version,
            stable: e.loader.stable,
        })
        .collect())
}

/// Скачивает `profile/json` и сохраняет его в `shared/versions/<id>/<id>.json`.
pub async fn install(http: &reqwest::Client, mc: &str, loader: &str) -> Res<String> {
    let text = http
        .get(format!("{META}/{mc}/{loader}/profile/json"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    crate::loaders::save_profile(&text).await
}
