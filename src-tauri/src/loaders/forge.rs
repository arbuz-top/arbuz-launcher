//! Forge — ТЗ §7.6.

use crate::error::*;
use crate::loaders::{install_with_installer, LoaderVersion};
use crate::models::Instance;
use crate::state::AppState;
use tauri::AppHandle;

const MAVEN: &str = "https://maven.minecraftforge.net";

/// Версии Forge для указанной версии Minecraft (от новых к старым).
pub async fn versions(http: &reqwest::Client, mc: &str) -> Res<Vec<LoaderVersion>> {
    let xml = http
        .get(format!("{MAVEN}/net/minecraftforge/forge/maven-metadata.xml"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let prefix = format!("{mc}-");
    let mut out: Vec<String> = xml
        .split("<version>")
        .skip(1)
        .filter_map(|p| p.split_once("</version>").map(|(v, _)| v.trim().to_string()))
        .filter(|v| v.starts_with(&prefix))
        .collect();

    // Числовая сортировка: `47.10.0` новее `47.2.0`.
    out.sort_by_key(|v| numeric_key(v));
    out.reverse();

    Ok(out
        .into_iter()
        .map(|version| LoaderVersion {
            version,
            stable: true,
        })
        .collect())
}

/// Ключ сортировки — все числа версии по порядку.
fn numeric_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|x| !x.is_empty())
        .filter_map(|x| x.parse().ok())
        .collect()
}

/// Установка Forge: installer в `runtimes/installers/`, id профиля — новая
/// папка в `shared/versions/`.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    version: &str,
    lo: f32,
    hi: f32,
) -> Res<String> {
    let short = version
        .strip_prefix(&format!("{}-", inst.mc_version))
        .unwrap_or(version);
    // У 1.7.10 версия выглядит как `10.13.4.1614-1.7.10` — в имени папки только первая часть.
    let tag = short.split('-').next().unwrap_or(short).to_string();

    install_with_installer(
        app,
        state,
        id,
        inst,
        format!("{MAVEN}/net/minecraftforge/forge/{version}/forge-{version}-installer.jar"),
        format!("forge-{version}-installer.jar"),
        tag,
        "Forge",
        lo,
        hi,
    )
    .await
}
