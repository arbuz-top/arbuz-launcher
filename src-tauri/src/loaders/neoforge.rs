//! NeoForge — ТЗ §7.6.

use crate::error::*;
use crate::loaders::{install_with_installer, LoaderVersion};
use crate::models::Instance;
use crate::state::AppState;
use tauri::AppHandle;

const MAVEN: &str = "https://maven.neoforged.net";

#[derive(serde::Deserialize)]
struct Versions {
    versions: Vec<String>,
}

/// Версии NeoForge для указанной версии Minecraft (от новых к старым).
pub async fn versions(http: &reqwest::Client, mc: &str) -> Res<Vec<LoaderVersion>> {
    let list: Versions = http
        .get(format!(
            "{MAVEN}/api/maven/versions/releases/net/neoforged/neoforge"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out: Vec<String> = list
        .versions
        .into_iter()
        .filter(|v| !v.to_lowercase().contains("beta"))
        .filter(|v| mc_of(v).as_deref() == Some(mc))
        .collect();

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

/// Все части версии — числа. Суффиксы (`21.1.31-beta`) дают `None`.
fn parts(v: &str) -> Option<Vec<u64>> {
    v.split('.').map(|x| x.parse::<u64>().ok()).collect()
}

/// Ключ сортировки — все числа версии по порядку.
fn numeric_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|x| !x.is_empty())
        .filter_map(|x| x.parse().ok())
        .collect()
}

/// Minecraft, которому соответствует версия NeoForge.
/// Старая нумерация: `A.B.C` → `1.A.B` (при `B = 0` → `1.A`).
/// Годовая: `A.B.C.build` → `A.B.C` (при `C = 0` → `A.B`).
fn mc_of(neo: &str) -> Option<String> {
    let p = parts(neo)?;
    match p.len() {
        3 => Some(if p[1] == 0 {
            format!("1.{}", p[0])
        } else {
            format!("1.{}.{}", p[0], p[1])
        }),
        4 => Some(if p[2] == 0 {
            format!("{}.{}", p[0], p[1])
        } else {
            format!("{}.{}.{}", p[0], p[1], p[2])
        }),
        _ => None,
    }
}

/// Установка NeoForge тем же installer'ом, что и Forge.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    id: &str,
    inst: &Instance,
    version: &str,
    lo: f32,
    hi: f32,
) -> Res<String> {
    install_with_installer(
        app,
        state,
        id,
        inst,
        format!("{MAVEN}/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"),
        format!("neoforge-{version}-installer.jar"),
        version.to_string(),
        "NeoForge",
        lo,
        hi,
    )
    .await
}
