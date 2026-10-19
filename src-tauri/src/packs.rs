//! Установка сборок: `.mrpack` (Modrinth) и CurseForge `.zip` (ТЗ §11).
//!
//! Тип сборки определяется по содержимому архива: есть `modrinth.index.json`
//! — это mrpack, есть `manifest.json` — CurseForge. Профиль создаётся без
//! скачивания Minecraft: игра докачивается при первом нажатии «Играть».

use crate::catalog::curseforge;
use crate::catalog::PACK_ID;
use crate::content;
use crate::error::{err, Res};
use crate::http::{download_all, Dl, Progress};
use crate::models::{default_java, Instance, Loader};
use crate::paths;
use crate::state::AppState;
use crate::store;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

const BIOMES: [&str; 5] = ["plains", "taiga", "desert", "nether", "snow"];
const API_MR: &str = "https://api.modrinth.com/v2";

/// Лимиты распаковки сборки: защита от zip-bomb (один файл и суммарно).
const MAX_UNPACK_FILE: u64 = 512 * 1024 * 1024;
const MAX_UNPACK_TOTAL: u64 = 8 * 1024 * 1024 * 1024;
/// Предел размера `modrinth.index.json`/`manifest.json` при чтении из архива.
const MAX_INDEX_JSON: u64 = 32 * 1024 * 1024;

/// Число профилей на диске — для выбора биома по кругу.
async fn count_instances() -> usize {
    let mut n = 0usize;
    if let Ok(mut rd) = tokio::fs::read_dir(paths::instances_dir()).await {
        while let Ok(Some(_)) = rd.next_entry().await {
            n += 1;
        }
    }
    n
}

/// Создаёт профиль под сборку и готовит стандартные папки.
async fn create_profile(
    name: &str,
    mc: &str,
    loader: Loader,
    loader_version: Option<String>,
) -> Res<Instance> {
    let count = count_instances().await;
    let id = Uuid::new_v4().simple().to_string();
    let instance = Instance {
        id: id.clone(),
        name: name.to_string(),
        mc_version: mc.to_string(),
        loader,
        loader_version,
        biome: BIOMES[count % BIOMES.len()].to_string(),
        icon: None,
        icon_preset: None,
        wallpaper: None,
        ram_min_mb: 1024,
        ram_max_mb: 4096,
        java_major: default_java(mc),
        java_path: None,
        jvm_args: String::new(),
        width: 1280,
        height: 720,
        fullscreen: false,
        playtime_sec: 0,
        playtime_days: BTreeMap::new(),
        last_played: None,
        created: chrono::Utc::now().timestamp(),
    };
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    let game = paths::instance_minecraft_dir(&id);
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(game.join(sub)).await?;
    }
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

// ---------------------------------------------------------------------------
// Zip-хелперы
// ---------------------------------------------------------------------------

/// Читает JSON-файл из архива; `None`, если файла в архиве нет.
/// Размер ограничен, чтобы запись-бомба не съела память.
fn zip_json(path: &Path, name: &str) -> Res<Option<Value>> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let out = match z.by_name(name) {
        Ok(e) => {
            if e.size() > MAX_INDEX_JSON {
                return err("Индекс сборки слишком большой");
            }
            let mut buf = Vec::new();
            e.take(MAX_INDEX_JSON).read_to_end(&mut buf)?;
            Some(serde_json::from_slice(&buf)?)
        }
        Err(zip::result::ZipError::FileNotFound) => None,
        Err(e) => return Err(e.into()),
    };
    Ok(out)
}

/// Распаковывает содержимое папок `prefixes` архива в `dest`.
/// Пути с `..`, префиксом диска или Windows-нормализуемыми компонентами
/// пропускаются; объём распаковки ограничен (защита от zip-bomb).
fn extract_dirs(path: &Path, prefixes: &[&str], dest: &Path) -> Res<()> {
    let f = std::fs::File::open(path)?;
    let mut z = zip::ZipArchive::new(f)?;
    let mut total: u64 = 0;
    for i in 0..z.len() {
        let e = z.by_index(i)?;
        let raw = e.name().replace('\\', "/");

        let mut rest: Option<String> = None;
        for p in prefixes {
            let pre = format!("{p}/");
            if let Some(r) = raw.strip_prefix(pre.as_str()) {
                rest = Some(r.to_string());
                break;
            }
        }
        let Some(rest) = rest else {
            continue;
        };
        let Some(rel) = content::safe_rel(&rest) else {
            continue;
        };
        let out = dest.join(rel);

        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        let declared = e.size();
        if declared > MAX_UNPACK_FILE || total.saturating_add(declared) > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = std::fs::File::create(&out)?;
        let copied = std::io::copy(&mut e.take(MAX_UNPACK_FILE), &mut w)?;
        total = total.saturating_add(copied);
        if total > MAX_UNPACK_TOTAL {
            return err("Содержимое сборки слишком большое");
        }
    }
    Ok(())
}

/// Загрузчик сборки из `dependencies` mrpack-индекса.
fn loader_from_deps(deps: &Value) -> (Loader, Option<String>) {
    for (key, loader) in [
        ("fabric-loader", Loader::Fabric),
        ("quilt-loader", Loader::Quilt),
        ("forge", Loader::Forge),
        ("neoforge", Loader::Neoforge),
    ] {
        if let Some(v) = deps.get(key).and_then(|x| x.as_str()) {
            if !v.is_empty() {
                return (loader, Some(v.to_string()));
            }
        }
    }
    (Loader::Vanilla, None)
}

/// Для Forge номер версии записывается как `{mc}-{версия}` (ТЗ §11).
fn forge_version(mc: &str, loader: Loader, v: Option<String>) -> Option<String> {
    match (loader, v) {
        (Loader::Forge, Some(v)) => Some(format!("{mc}-{v}")),
        (_, v) => v,
    }
}

// ---------------------------------------------------------------------------
// Импорт архива сборки
// ---------------------------------------------------------------------------

/// Определяет тип сборки по содержимому архива и импортирует её.
pub async fn import_pack(app: &AppHandle, state: &AppState, path: &str) -> Res<Instance> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return err("Файл сборки не найден");
    }

    let probe = p.clone();
    let index = tokio::task::spawn_blocking(move || zip_json(&probe, "modrinth.index.json")).await??;
    if let Some(index) = index {
        return import_mrpack(app, state, &p, index).await;
    }

    let probe2 = p.clone();
    let manifest = tokio::task::spawn_blocking(move || zip_json(&probe2, "manifest.json")).await??;
    match manifest {
        Some(m) => import_curseforge(app, state, &p, m).await,
        None => err("Это не сборка Modrinth (.mrpack) и не CurseForge (.zip)"),
    }
}

/// Импорт Modrinth `.mrpack`.
async fn import_mrpack(app: &AppHandle, state: &AppState, zip: &Path, index: Value) -> Res<Instance> {
    let name = index
        .get("name")
        .and_then(|x| x.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Сборка")
        .to_string();
    let deps = index.get("dependencies").cloned().unwrap_or(Value::Null);
    let mc = deps.get("minecraft").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if mc.is_empty() {
        return err("В сборке не указана версия Minecraft");
    }
    let (loader, lv) = loader_from_deps(&deps);
    let inst = create_profile(&name, &mc, loader, forge_version(&mc, loader, lv)).await?;
    let game = paths::instance_minecraft_dir(&inst.id);

    let mut dls: Vec<Dl> = Vec::new();
    if let Some(files) = index.get("files").and_then(|x| x.as_array()) {
        for f in files {
            if f.pointer("/env/client").and_then(|x| x.as_str()) == Some("unsupported") {
                continue;
            }
            let Some(rel) = f.get("path").and_then(|x| x.as_str()) else {
                continue;
            };
            let Some(url) = f.pointer("/downloads/0").and_then(|x| x.as_str()) else {
                continue;
            };
            if url.is_empty() {
                continue;
            }
            let sha1 = f
                .pointer("/hashes/sha1")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let size = f.get("fileSize").and_then(|x| x.as_u64()).unwrap_or(0);
            dls.push(Dl {
                url: url.to_string(),
                path: content::safe_join(&game, rel)?,
                sha1,
                sha256: None,
                size,
            });
        }
    }
    if !dls.is_empty() {
        download_all(app, &state.http, PACK_ID, dls, 0.0, 90.0).await?;
    }

    let zip2 = zip.to_path_buf();
    let dest = game.clone();
    tokio::task::spawn_blocking(move || {
        extract_dirs(&zip2, &["overrides", "client-overrides"], &dest)
    })
    .await??;

    let _ = app.emit("progress", Progress { id: PACK_ID.into(), pct: 100.0 });
    Ok(inst)
}

/// Импорт CurseForge `.zip` по `manifest.json`.
async fn import_curseforge(
    app: &AppHandle,
    state: &AppState,
    zip: &Path,
    manifest: Value,
) -> Res<Instance> {
    let name = manifest
        .get("name")
        .and_then(|x| x.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Сборка")
        .to_string();
    let mc = manifest
        .pointer("/minecraft/version")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if mc.is_empty() {
        return err("В сборке не указана версия Minecraft");
    }

    // `id` загрузчика вида "forge-47.2.0": префикс до первого дефиса.
    let mut loader = Loader::Vanilla;
    let mut lv: Option<String> = None;
    if let Some(mls) = manifest.pointer("/minecraft/modLoaders").and_then(|x| x.as_array()) {
        let pick = mls
            .iter()
            .find(|m| m.get("primary").and_then(|x| x.as_bool()).unwrap_or(false))
            .or_else(|| mls.first());
        if let Some(m) = pick {
            if let Some(id) = m.get("id").and_then(|x| x.as_str()) {
                let (pre, ver) = id.split_once('-').unwrap_or((id, ""));
                loader = match pre {
                    "forge" => Loader::Forge,
                    "fabric" => Loader::Fabric,
                    "quilt" => Loader::Quilt,
                    "neoforge" => Loader::Neoforge,
                    _ => Loader::Vanilla,
                };
                lv = if ver.is_empty() { None } else { Some(ver.to_string()) };
            }
        }
    }

    let inst = create_profile(&name, &mc, loader, forge_version(&mc, loader, lv)).await?;
    let game = paths::instance_minecraft_dir(&inst.id);

    let mut ids: Vec<i64> = Vec::new();
    if let Some(files) = manifest.get("files").and_then(|x| x.as_array()) {
        for f in files {
            if let Some(id) = f.get("fileID").and_then(|x| x.as_i64()) {
                ids.push(id);
            }
        }
    }

    if !ids.is_empty() {
        let (api, key) = curseforge::endpoint(state).await;
        let v: Value = curseforge::post(state, &format!("{api}/mods/files"), &key)
            .json(&serde_json::json!({ "fileIds": ids }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut dls: Vec<Dl> = Vec::new();
        if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
            for f in arr {
                let fid = f.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
                let fname = f.get("fileName").and_then(|x| x.as_str()).unwrap_or("");
                if fname.is_empty() {
                    continue;
                }
                let url = match f.get("downloadUrl").and_then(|x| x.as_str()) {
                    Some(u) if !u.is_empty() && (key.is_some() || curseforge::is_cdn_url(u)) => {
                        u.to_string()
                    }
                    _ => curseforge::forgecdn_url(fid, fname),
                };
                let sha1 = f
                    .get("hashes")
                    .and_then(|x| x.as_array())
                    .and_then(|a| {
                        a.iter()
                            .find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1))
                    })
                    .and_then(|h| h.get("value").and_then(|x| x.as_str()))
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let size = f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);
                dls.push(Dl {
                    url,
                    path: content::safe_join(&game, &format!("mods/{fname}"))?,
                    sha1,
                    sha256: None,
                    size,
                });
            }
        }
        if !dls.is_empty() {
            download_all(app, &state.http, PACK_ID, dls, 0.0, 90.0).await?;
        }
    }

    let prefix = manifest
        .get("overrides")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("overrides")
        .to_string();
    let zip2 = zip.to_path_buf();
    let dest = game.clone();
    tokio::task::spawn_blocking(move || extract_dirs(&zip2, &[prefix.as_str()], &dest)).await??;

    let _ = app.emit("progress", Progress { id: PACK_ID.into(), pct: 100.0 });
    Ok(inst)
}

/// Версия сборки, доступная к установке (ТЗ §11).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackVersion {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    /// ISO-8601.
    pub date: String,
}

/// Список версий проекта в каталоге.
pub async fn list_versions(state: &AppState, source: &str, project_id: &str) -> Res<Vec<PackVersion>> {
    match source {
        "modrinth" => modrinth_versions(state, project_id).await,
        "curseforge" => curseforge_versions(state, project_id).await,
        _ => err("Неизвестный источник"),
    }
}

/// Есть ли в версии файл `.mrpack`.
fn has_mrpack(v: &Value) -> bool {
    v.get("files")
        .and_then(|x| x.as_array())
        .map(|fs| {
            fs.iter().any(|f| {
                f.get("filename")
                    .and_then(|x| x.as_str())
                    .map(|n| n.ends_with(".mrpack"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Версии Modrinth, у которых есть `.mrpack`.
async fn modrinth_versions(state: &AppState, project_id: &str) -> Res<Vec<PackVersion>> {
    let v: Value = state
        .http
        .get(format!("{API_MR}/project/{project_id}/version"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out = Vec::new();
    if let Some(arr) = v.as_array() {
        for ver in arr {
            if !has_mrpack(ver) {
                continue;
            }
            let first = |key: &str| {
                ver.get(key)
                    .and_then(|x| x.as_array())
                    .and_then(|a| a.first())
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            out.push(PackVersion {
                id: ver.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                name: ver.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                mc_version: first("game_versions"),
                loader: first("loaders"),
                date: ver
                    .get("date_published")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(out)
}

/// Версии CurseForge.
async fn curseforge_versions(state: &AppState, project_id: &str) -> Res<Vec<PackVersion>> {
    let (api, key) = curseforge::endpoint(state).await;
    let v: Value = curseforge::get(state, &format!("{api}/mods/{project_id}/files?pageSize=50"), &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out = Vec::new();
    if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
        for f in arr {
            // `gameVersions` — смесь версий игры и названий загрузчиков.
            let gv = f.get("gameVersions").and_then(|x| x.as_array());
            let names: Vec<&str> = gv
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            let mc = names
                .iter()
                .find(|s| s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
                .copied()
                .unwrap_or("");
            let loader = names
                .iter()
                .find(|s| matches!(**s, "Forge" | "Fabric" | "Quilt" | "NeoForge"))
                .copied()
                .unwrap_or("");
            out.push(PackVersion {
                id: f.get("id").and_then(|x| x.as_i64()).map(|x| x.to_string()).unwrap_or_default(),
                name: f.get("displayName").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                mc_version: mc.to_string(),
                loader: loader.to_string(),
                date: f.get("fileDate").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Установка сборки из каталога
// ---------------------------------------------------------------------------

/// Файл `.mrpack` выбранной версии (или последней, если версия не задана).
async fn modrinth_pack(
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
) -> Res<(String, Option<String>, u64, String)> {
    let v: Value = state
        .http
        .get(format!("{API_MR}/project/{project_id}/version"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let empty = Vec::new();
    let arr = v.as_array().unwrap_or(&empty);
    for ver in arr {
        if let Some(want) = version_id {
            if ver.get("id").and_then(|x| x.as_str()) != Some(want) {
                continue;
            }
        }
        let Some(files) = ver.get("files").and_then(|x| x.as_array()) else {
            continue;
        };
        for f in files {
            let fname = f.get("filename").and_then(|x| x.as_str()).unwrap_or("");
            if !fname.ends_with(".mrpack") {
                continue;
            }
            let url = f.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
            if url.is_empty() {
                continue;
            }
            let sha1 = f
                .pointer("/hashes/sha1")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let size = f.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
            return Ok((url, sha1, size, fname.to_string()));
        }
    }
    err("У выбранной версии нет файла .mrpack")
}

/// Файл сборки CurseForge: конкретная версия или последняя.
async fn curseforge_pack(
    state: &AppState,
    project_id: &str,
    version_id: Option<&str>,
) -> Res<(String, Option<String>, u64, String)> {
    let (api, key) = curseforge::endpoint(state).await;
    let url = match version_id {
        Some(id) => format!("{api}/mods/{project_id}/files/{id}"),
        None => format!("{api}/mods/{project_id}/files?pageSize=1"),
    };
    let v: Value = curseforge::get(state, &url, &key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let f = match version_id {
        Some(_) => v.get("data").cloned().unwrap_or(Value::Null),
        None => v
            .get("data")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null),
    };
    if f.is_null() {
        return err("У проекта нет файлов");
    }

    let fid = f.get("id").and_then(|x| x.as_i64()).unwrap_or(0);
    let fname = f.get("fileName").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if fname.is_empty() {
        return err("У файла нет имени");
    }
    let url = match f.get("downloadUrl").and_then(|x| x.as_str()) {
        Some(u) if !u.is_empty() && (key.is_some() || curseforge::is_cdn_url(u)) => u.to_string(),
        _ => curseforge::forgecdn_url(fid, &fname),
    };
    let sha1 = f
        .get("hashes")
        .and_then(|x| x.as_array())
        .and_then(|a| a.iter().find(|h| h.get("algo").and_then(|x| x.as_u64()) == Some(1)))
        .and_then(|h| h.get("value").and_then(|x| x.as_str()))
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let size = f.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);
    Ok((url, sha1, size, fname))
}

/// Скачивает выбранную (или последнюю) версию сборки и импортирует её.
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    source: &str,
    project_id: &str,
    version_id: Option<String>,
) -> Res<Instance> {
    let (url, sha1, size, filename) = match source {
        "modrinth" => modrinth_pack(state, project_id, version_id.as_deref()).await?,
        "curseforge" => curseforge_pack(state, project_id, version_id.as_deref()).await?,
        _ => return err("Неизвестный источник"),
    };

    let tmp = paths::tmp_dir();
    tokio::fs::create_dir_all(&tmp).await?;
    // Имя файла приходит из ответа каталога — берём только basename.
    let fname = Path::new(&filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if fname.is_empty() || content::bad_component(fname) {
        return err("Недопустимое имя файла сборки");
    }
    let file = tmp.join(fname);
    download_all(
        app,
        &state.http,
        PACK_ID,
        vec![Dl {
            url,
            path: file.clone(),
            sha1,
            sha256: None,
            size,
        }],
        0.0,
        100.0,
    )
    .await?;

    let result = import_pack(app, state, &file.to_string_lossy()).await;
    let _ = tokio::fs::remove_file(&file).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_rel_rejects_escape() {
        assert!(content::safe_rel("../evil.jar").is_none());
        assert!(content::safe_rel("mods/../../evil.jar").is_none());
        assert!(content::safe_rel("C:/Windows/x.dll").is_none());
        assert!(content::safe_rel("a.jar:stream").is_none());
        assert!(content::safe_rel("").is_none());
        assert!(content::safe_rel("./").is_none());
        // Windows схлопывает завершающие пробелы и точки: ".. " == "..".
        assert!(content::safe_rel(".. /evil.jar").is_none());
        assert!(content::safe_rel("mods/.. /evil.jar").is_none());
    }

    #[test]
    fn zip_rel_keeps_normal_path() {
        assert_eq!(
            content::safe_rel("mods/sodium.jar"),
            Some(PathBuf::from("mods").join("sodium.jar"))
        );
        assert_eq!(
            content::safe_rel("./config/a.toml"),
            Some(PathBuf::from("config").join("a.toml"))
        );
    }

    #[test]
    fn loader_deps_mapping() {
        let deps = serde_json::json!({ "minecraft": "1.20.1", "fabric-loader": "0.15.0" });
        let (loader, v) = loader_from_deps(&deps);
        assert_eq!(loader, Loader::Fabric);
        assert_eq!(v.as_deref(), Some("0.15.0"));
        assert_eq!(forge_version("1.20.1", Loader::Fabric, v), Some("0.15.0".into()));
    }

    #[test]
    fn forge_loader_version_gets_mc_prefix() {
        let deps = serde_json::json!({ "minecraft": "1.20.1", "forge": "47.2.0" });
        let (loader, v) = loader_from_deps(&deps);
        assert_eq!(loader, Loader::Forge);
        assert_eq!(forge_version("1.20.1", loader, v).as_deref(), Some("1.20.1-47.2.0"));
    }
}
