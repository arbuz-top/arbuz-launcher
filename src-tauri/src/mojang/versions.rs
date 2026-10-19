use crate::error::*;
use crate::paths;
use std::collections::HashMap;
use std::time::Instant;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const CACHE_TTL_SECS: u64 = 3600;

/// Версия Minecraft для фронтенда.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McVersion {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

/// Запись манифеста: версия + url её version.json (для скачивания).
#[derive(Clone)]
pub(crate) struct Entry {
    pub v: McVersion,
    pub url: String,
}

static CACHE: once_cell::sync::Lazy<tokio::sync::Mutex<Option<(Instant, Vec<Entry>)>>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));

#[derive(serde::Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
    release_time: String,
}

/// Список версий из манифеста Mojang с кэшем в памяти на 1 час.
pub async fn entries(http: &reqwest::Client) -> Res<Vec<Entry>> {
    {
        let guard = CACHE.lock().await;
        if let Some((at, list)) = guard.as_ref() {
            if at.elapsed().as_secs() < CACHE_TTL_SECS {
                return Ok(list.clone());
            }
        }
    }

    let text = http
        .get(MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let manifest: Manifest = serde_json::from_str(&text)?;

    let list: Vec<Entry> = manifest
        .versions
        .into_iter()
        .map(|v| Entry {
            v: McVersion {
                id: v.id,
                kind: v.kind,
                release_time: v.release_time,
            },
            url: v.url,
        })
        .collect();

    let mut guard = CACHE.lock().await;
    *guard = Some((Instant::now(), list.clone()));
    Ok(list)
}

/// Список версий для команды `list_mc_versions`: свои версии сверху.
pub async fn list(http: &reqwest::Client) -> Res<Vec<McVersion>> {
    let mut out: Vec<McVersion> = custom_ids()
        .await
        .into_iter()
        .map(|id| McVersion {
            id,
            kind: "custom".into(),
            release_time: String::new(),
        })
        .collect();
    for v in entries(http).await?.into_iter().map(|e| e.v) {
        if !out.iter().any(|c| c.id == v.id) {
            out.push(v);
        }
    }
    Ok(out)
}

// ---- Пользовательские версии ----

/// Файл со списком id своих версий (дополняет манифест Mojang).
fn custom_file() -> std::path::PathBuf {
    paths::root().join("custom_versions.json")
}

/// id своих версий.
pub async fn custom_ids() -> Vec<String> {
    tokio::fs::read_to_string(custom_file())
        .await
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .unwrap_or_default()
}

async fn save_custom_ids(ids: &[String]) -> Res<()> {
    let p = custom_file();
    if let Some(dir) = p.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&p, serde_json::to_vec(ids)?).await?;
    Ok(())
}

/// Допустимый id своей версии: без пробелов и разделителей пути.
pub fn valid_custom_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
}

/// Добавляет свою версию: свой `version.json` и, по желанию, client jar.
pub async fn add_custom(
    id: &str,
    base: &str,
    json_path: Option<&str>,
    jar_path: Option<&str>,
) -> Res<String> {
    let id = id.trim();
    if !valid_custom_id(id) {
        return err("Некорректный id версии: буквы, цифры, . _ - +");
    }
    let path = paths::version_json(id);
    if tokio::fs::metadata(&path).await.is_ok() {
        return err("Версия с таким id уже существует");
    }

    let text = match json_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw_path) => {
            let raw = tokio::fs::read_to_string(raw_path).await?;
            let mut v: serde_json::Value = serde_json::from_str(&raw)?;
            let obj = v
                .as_object_mut()
                .ok_or_else(|| AppError::Msg("version.json должен быть объектом".into()))?;
            obj.insert("id".into(), serde_json::Value::String(id.into()));
            if !obj.contains_key("inheritsFrom") {
                let base = base.trim();
                if !base.is_empty() {
                    obj.insert("inheritsFrom".into(), serde_json::Value::String(base.into()));
                }
            }
            serde_json::to_string_pretty(&v)?
        }
        None => {
            let base = base.trim();
            if base.is_empty() {
                return err("Укажите базовую версию или version.json");
            }
            serde_json::json!({ "id": id, "inheritsFrom": base }).to_string()
        }
    };

    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&path, text).await?;

    if let Some(jar) = jar_path.map(str::trim).filter(|s| !s.is_empty()) {
        tokio::fs::copy(jar, paths::version_jar(id)).await?;
    }

    let mut ids = custom_ids().await;
    if !ids.iter().any(|x| x == id) {
        ids.push(id.to_string());
        save_custom_ids(&ids).await?;
    }
    Ok(id.to_string())
}

/// Удаляет свою версию вместе с папкой.
pub async fn remove_custom(id: &str) -> Res<()> {
    if !valid_custom_id(id) {
        return err("Некорректный id версии");
    }
    let mut ids = custom_ids().await;
    ids.retain(|x| x != id);
    save_custom_ids(&ids).await?;
    let dir = paths::version_dir(id);
    if tokio::fs::metadata(&dir).await.is_ok() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}

// ---- Структуры version.json ----

#[derive(serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: Option<String>,
    pub assets: Option<String>,
    pub asset_index: Option<AssetIndex>,
    pub downloads: Option<Downloads>,
    pub libraries: Vec<Library>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersion>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: Option<String>,
    pub size: u64,
    pub url: String,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct Downloads {
    pub client: Option<ClientDownload>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct ClientDownload {
    pub url: String,
    pub sha1: Option<String>,
    pub size: u64,
}

#[derive(serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub rules: Vec<crate::mojang::rules::Rule>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
    pub url: Option<String>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: u64,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct Extract {
    pub exclude: Vec<String>,
}

#[derive(serde::Deserialize, Clone, Default)]
pub struct Arguments {
    pub game: Vec<crate::mojang::rules::Arg>,
    pub jvm: Vec<crate::mojang::rules::Arg>,
}

/// Читает version.json из `shared/versions/<id>/<id>.json`, а при отсутствии —
/// находит url в манифесте, скачивает и сохраняет файл как есть.
pub async fn fetch_json(http: &reqwest::Client, id: &str) -> Res<VersionJson> {
    let path = paths::version_json(id);
    if tokio::fs::metadata(&path).await.is_ok() {
        let text = tokio::fs::read_to_string(&path).await?;
        return Ok(serde_json::from_str(&text)?);
    }

    let url = entries(http)
        .await?
        .into_iter()
        .find(|e| e.v.id == id)
        .map(|e| e.url)
        .ok_or_else(|| AppError::Msg(format!("Версия {id} не найдена в манифесте Mojang")))?;

    let text = http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &text).await?;

    Ok(serde_json::from_str(&text)?)
}

/// Рекурсивно разрешает `inheritsFrom` и сливает родителя с потомком.
pub async fn merged(http: &reqwest::Client, id: &str) -> Res<VersionJson> {
    let child = fetch_json(http, id).await?;
    let Some(parent_id) = child.inherits_from.clone() else {
        return Ok(child);
    };
    let parent = Box::pin(merged(http, &parent_id)).await?;
    Ok(merge(parent, child))
}

/// Слияние по правилам ТЗ: id и приоритет полей у child, библиотеки/аргументы склеиваются.
fn merge(parent: VersionJson, mut child: VersionJson) -> VersionJson {
    let mut libraries = child.libraries;
    libraries.extend(parent.libraries);
    child.libraries = libraries;

    child.arguments = match (parent.arguments, child.arguments) {
        (Some(p), Some(c)) => {
            let mut jvm = p.jvm;
            jvm.extend(c.jvm);
            let mut game = p.game;
            game.extend(c.game);
            Some(Arguments { game, jvm })
        }
        (Some(p), None) => Some(p),
        (None, Some(c)) => Some(c),
        (None, None) => None,
    };

    if child.main_class.is_none() {
        child.main_class = parent.main_class;
    }
    if child.assets.is_none() {
        child.assets = parent.assets;
    }
    if child.asset_index.is_none() {
        child.asset_index = parent.asset_index;
    }
    if child.downloads.is_none() {
        child.downloads = parent.downloads;
    }
    if child.java_version.is_none() {
        child.java_version = parent.java_version;
    }
    if child.minecraft_arguments.is_none() {
        child.minecraft_arguments = parent.minecraft_arguments;
    }

    child
}
