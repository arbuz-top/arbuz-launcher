//! Вкладка «Сборка»: манифест модпака, редактор конфигов, проверка и экспорт.
//!
//! Метаданные модов читаются прямо из jar-архивов (`fabric.mod.json`,
//! `quilt.mod.json`, `META-INF/mods.toml`), поэтому конфиги можно привязать к
//! конкретному моду без интернета и без внешних индексов.

use crate::error::{err, Res};
use crate::models::{default_java, Instance, Loader};
use crate::paths;
use crate::store;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

/// Больше этого файл конфигурации не открываем: это уже не конфиг.
const MAX_CFG: u64 = 2 * 1024 * 1024;

/// Где ищем конфиги.
const CFG_DIRS: [&str; 2] = ["config", "defaultconfigs"];

/// Расширения, которые считаем текстовыми конфигами.
const CFG_EXT: [&str; 12] = [
    "toml", "json", "json5", "yaml", "yml", "cfg", "conf", "properties", "txt", "ini", "hocon",
    "xml",
];

/// Зависимости, которые поставляет сама игра или загрузчик.
const BUILTIN_DEPS: [&str; 12] = [
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "fabric_loader",
    "quilt_loader",
    "quiltloader",
    "forge",
    "neoforge",
    "javafml",
    "lowcodefml",
    "mclanguage",
];

// ---------------------------------------------------------------------------
// Модели
// ---------------------------------------------------------------------------

/// Манифест сборки (`instances/<id>/pack.json`).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct PackManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub summary: String,
    pub description: String,
    /// Имя файла иконки внутри папки профиля (необязательно).
    pub icon: Option<String>,
}

impl Default for PackManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "1.0.0".into(),
            author: String::new(),
            summary: String::new(),
            description: String::new(),
            icon: None,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub id: String,
    /// Имя профиля (может отличаться от имени сборки).
    pub name: String,
    pub manifest: PackManifest,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    /// Включённые моды.
    pub mods: usize,
    /// Все jar-файлы, включая выключенные.
    pub mods_total: usize,
    pub configs: usize,
    pub size_bytes: u64,
    /// Абсолютный путь к иконке профиля/сборки.
    pub icon: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEntry {
    /// Путь от `minecraft/`, например `config/create-common.toml`.
    pub path: String,
    pub size: u64,
    /// Расширение без точки: toml | json | yaml …
    pub kind: String,
    pub mod_id: Option<String>,
    pub mod_name: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDoc {
    pub path: String,
    pub content: String,
    pub size: u64,
    /// Есть ли резервная копия (файл сохранялся через лаунчер).
    pub has_backup: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackIssue {
    /// error | warn | info
    pub severity: String,
    pub title: String,
    pub detail: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub files: usize,
    pub bytes: u64,
}

/// Метаданные установленного мода.
#[derive(Clone, Debug)]
struct ModMeta {
    id: String,
    name: String,
    /// Путь от `minecraft/`: `mods/foo.jar`.
    file: String,
    enabled: bool,
    /// id зависимостей.
    depends: Vec<String>,
    /// id, которые мод предоставляет (модули Fabric API и т.п.).
    provides: Vec<String>,
}

// ---------------------------------------------------------------------------
// Кэш метаданных jar: вкладка открывается часто, а zip-архивы читать дорого.
// ---------------------------------------------------------------------------

type CachedMod = (u64, u64, ModMeta);

static MOD_CACHE: Lazy<Mutex<HashMap<PathBuf, CachedMod>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn stamp(path: &Path) -> (u64, u64) {
    match std::fs::metadata(path) {
        Ok(m) => (
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            m.len(),
        ),
        Err(_) => (0, 0),
    }
}

// ---------------------------------------------------------------------------
// Загрузка профиля
// ---------------------------------------------------------------------------

async fn load(id: &str) -> Res<Instance> {
    paths::validate_instance_id(id)?;
    store::read_instance(id).await
}

fn loader_key(l: Loader) -> &'static str {
    match l {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Quilt => "quilt",
    }
}

// ---------------------------------------------------------------------------
// Манифест
// ---------------------------------------------------------------------------

fn cut(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

fn sanitize_manifest(m: &mut PackManifest) {
    let name = cut(&m.name, 100);
    let version = cut(&m.version, 40);
    let author = cut(&m.author, 100);
    let summary = cut(&m.summary, 300);
    let description: String = m.description.trim().chars().take(20_000).collect();
    m.name = name;
    m.version = if version.is_empty() { "1.0.0".into() } else { version };
    m.author = author;
    m.summary = summary;
    m.description = description;
    m.icon = m
        .icon
        .take()
        .and_then(|s| Path::new(s.trim()).file_name().map(|n| n.to_string_lossy().to_string()))
        .filter(|s| !s.is_empty());
}

async fn read_manifest(inst: &Instance) -> PackManifest {
    let path = paths::instance_dir(&inst.id).join("pack.json");
    let mut m: PackManifest = store::read_json(&path).await.unwrap_or_default();
    if m.name.trim().is_empty() {
        m.name = inst.name.clone();
    }
    sanitize_manifest(&mut m);
    m
}

/// Сохраняет манифест и возвращает свежую информацию о сборке.
pub async fn save(id: &str, mut manifest: PackManifest) -> Res<PackInfo> {
    let inst = load(id).await?;
    sanitize_manifest(&mut manifest);
    if manifest.name.is_empty() {
        manifest.name = inst.name.clone();
    }
    store::write_json_atomic(&paths::instance_dir(&inst.id).join("pack.json"), &manifest).await?;
    info(id).await
}

// ---------------------------------------------------------------------------
// Информация о сборке
// ---------------------------------------------------------------------------

pub async fn info(id: &str) -> Res<PackInfo> {
    let inst = load(id).await?;
    let manifest = read_manifest(&inst).await;
    let mods = scan_mods(&inst);
    let configs = collect_configs(&inst, &mods);
    let game = paths::instance_minecraft_dir(&inst.id);

    let mut size = 0u64;
    for d in ["mods", "config", "defaultconfigs", "resourcepacks", "shaderpacks"] {
        size += dir_size(&game.join(d));
    }

    let icon = manifest
        .icon
        .clone()
        .or_else(|| inst.icon.clone())
        .map(|f| paths::instance_icon(&inst.id, &f))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string());

    Ok(PackInfo {
        id: inst.id.clone(),
        name: inst.name.clone(),
        mc_version: inst.mc_version.clone(),
        loader: loader_key(inst.loader).into(),
        loader_version: detect_loader_version(&inst),
        mods: mods.iter().filter(|m| m.enabled).count(),
        mods_total: mods.len(),
        configs: configs.len(),
        size_bytes: size,
        manifest,
        icon,
    })
}

fn dir_size(root: &Path) -> u64 {
    if !root.is_dir() {
        return 0;
    }
    walkdir::WalkDir::new(root)
        .max_depth(10)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Версия загрузчика: поле профиля, иначе шапка `logs/latest.log`.
fn detect_loader_version(inst: &Instance) -> Option<String> {
    if let Some(v) = &inst.loader_version {
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    let log = paths::instance_minecraft_dir(&inst.id)
        .join("logs")
        .join("latest.log");
    let raw = std::fs::read(&log).ok()?;
    let head = String::from_utf8_lossy(&raw[..raw.len().min(8192)]).to_string();
    let patterns: &[&str] = match inst.loader {
        Loader::Fabric => &[r"Fabric Loader[^\d]{0,10}([\w.\-+]+)"],
        Loader::Quilt => &[r"Quilt Loader[^\d]{0,10}([\w.\-+]+)"],
        Loader::Forge => &[r"Forge[^\d]{0,10}([\w.\-+]+)"],
        Loader::Neoforge => &[r"NeoForge[^\d]{0,10}([\w.\-+]+)"],
        Loader::Vanilla => &[],
    };
    for p in patterns {
        let Ok(re) = regex::Regex::new(p) else {
            continue;
        };
        if let Some(c) = re.captures(&head) {
            if let Some(m) = c.get(1) {
                return Some(m.as_str().to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Моды и их метаданные
// ---------------------------------------------------------------------------

fn scan_mods(inst: &Instance) -> Vec<ModMeta> {
    let dir = paths::instance_minecraft_dir(&inst.id).join("mods");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let Some(raw) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let (base, enabled) = match raw.strip_suffix(".disabled") {
            Some(b) => (b, false),
            None => (raw, true),
        };
        if !base.to_ascii_lowercase().ends_with(".jar") {
            continue;
        }
        let rel = format!("mods/{base}");
        out.push(mod_meta(&p, &rel, enabled));
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

fn mod_meta(path: &Path, rel: &str, enabled: bool) -> ModMeta {
    let (mtime, len) = stamp(path);
    if let Some((mt, ln, cached)) = MOD_CACHE
        .lock()
        .ok()
        .and_then(|c| c.get(path).cloned())
    {
        if mt == mtime && ln == len {
            let mut m = cached;
            m.file = rel.to_string();
            m.enabled = enabled;
            return m;
        }
    }
    let meta = read_mod_meta(path, rel, enabled);
    if let Ok(mut c) = MOD_CACHE.lock() {
        if c.len() > 4000 {
            c.clear();
        }
        c.insert(path.to_path_buf(), (mtime, len, meta.clone()));
    }
    meta
}

fn clean_id(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len());
    let mut dash = false;
    for ch in stem.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn read_mod_meta(path: &Path, rel: &str, enabled: bool) -> ModMeta {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut m = ModMeta {
        id: clean_id(&stem),
        name: stem.clone(),
        file: rel.to_string(),
        enabled,
        depends: Vec::new(),
        provides: Vec::new(),
    };
    let Ok(f) = std::fs::File::open(path) else {
        return m;
    };
    let Ok(mut z) = zip::ZipArchive::new(f) else {
        return m;
    };

    // Fabric.
    if let Ok(e) = z.by_name("fabric.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, Value>(e) {
            if let Some(id) = v["id"].as_str() {
                m.id = id.to_string();
            }
            if let Some(n) = v["name"].as_str() {
                if !n.trim().is_empty() {
                    m.name = n.to_string();
                }
            }
            if let Some(d) = v["depends"].as_object() {
                m.depends = d.keys().cloned().collect();
            }
            if let Some(p) = v["provides"].as_array() {
                m.provides = p
                    .iter()
                    .filter_map(|x| x.as_str())
                    .map(str::to_string)
                    .collect();
            }
        }
    }

    // Quilt.
    if let Ok(e) = z.by_name("quilt.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, Value>(e) {
            let q = &v["quilt_loader"];
            if let Some(id) = q["id"].as_str() {
                m.id = id.to_string();
            }
            if let Some(n) = q["metadata"]["name"].as_str() {
                if !n.trim().is_empty() {
                    m.name = n.to_string();
                }
            }
            if let Some(arr) = q["depends"].as_array() {
                for d in arr {
                    if let Some(s) = d.as_str() {
                        m.depends.push(s.to_string());
                    } else if let Some(s) = d["id"].as_str() {
                        m.depends.push(s.to_string());
                    }
                }
            }
            if let Some(arr) = q["provides"].as_array() {
                for d in arr {
                    if let Some(s) = d.as_str() {
                        m.provides.push(s.to_string());
                    } else if let Some(s) = d["id"].as_str() {
                        m.provides.push(s.to_string());
                    }
                }
            }
        }
    }

    // Forge / NeoForge.
    for name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        let Ok(mut e) = z.by_name(name) else {
            continue;
        };
        let mut s = String::new();
        if e.read_to_string(&mut s).is_err() {
            continue;
        }
        let Ok(t) = toml::from_str::<toml::Value>(&s) else {
            continue;
        };
        if let Some(mm) = t
            .get("mods")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
        {
            if let Some(id) = mm.get("modId").and_then(|x| x.as_str()) {
                m.id = id.to_string();
            }
            if let Some(n) = mm.get("displayName").and_then(|x| x.as_str()) {
                if !n.trim().is_empty() {
                    m.name = n.to_string();
                }
            }
        }
        if let Some(deps) = t.get("dependencies").and_then(|x| x.as_table()) {
            for (_, arr) in deps {
                let Some(list) = arr.as_array() else {
                    continue;
                };
                for d in list {
                    if d.get("mandatory").and_then(|x| x.as_bool()) == Some(false) {
                        continue;
                    }
                    if let Some(id) = d.get("modId").and_then(|x| x.as_str()) {
                        m.depends.push(id.to_string());
                    }
                }
            }
        }
        break;
    }

    m
}

// ---------------------------------------------------------------------------
// Конфиги
// ---------------------------------------------------------------------------

fn collect_configs(inst: &Instance, mods: &[ModMeta]) -> Vec<ConfigEntry> {
    let game = paths::instance_minecraft_dir(&inst.id);
    let mut out: Vec<ConfigEntry> = Vec::new();
    for base in CFG_DIRS {
        let root = game.join(base);
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .max_depth(6)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let low = name.to_ascii_lowercase();
            if name.starts_with('.') || low.ends_with(".bak") || low.ends_with(".tmp") {
                continue;
            }
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !CFG_EXT.contains(&ext.as_str()) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size > MAX_CFG {
                continue;
            }
            let Ok(rel) = p.strip_prefix(&game) else {
                continue;
            };
            let rel = crate::content::rel_slash(rel);
            let (mod_id, mod_name) = match_mod(&rel, mods);
            out.push(ConfigEntry {
                path: rel,
                size,
                kind: ext,
                mod_id,
                mod_name,
            });
            if out.len() >= 4000 {
                break;
            }
        }
    }
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    out
}

/// Нормализует строку для мягкого сравнения: `Iris Shaders` = `iris-shaders`.
fn norm_path(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Ищет мод, которому принадлежит файл конфигурации.
fn match_mod(rel: &str, mods: &[ModMeta]) -> (Option<String>, Option<String>) {
    let low = rel.replace('\\', "/").to_ascii_lowercase();
    let npath = norm_path(&low);
    let segs: Vec<&str> = low.split('/').collect();
    let file = segs.last().copied().unwrap_or("");
    let stem = file.rsplit_once('.').map(|(a, _)| a).unwrap_or(file);
    let nstem = norm_path(stem);
    let dirs = &segs[..segs.len().saturating_sub(1)];

    // 1. Папка внутри config/ совпадает с id мода.
    for m in mods {
        let idn = norm_path(&m.id);
        if idn.len() < 3 {
            continue;
        }
        if dirs.iter().any(|s| norm_path(s) == idn) {
            return (Some(m.id.clone()), Some(m.name.clone()));
        }
    }

    // 2. Имя файла начинается с id: `create-common`, `create_common`, `create.toml`.
    for m in mods {
        let idn = norm_path(&m.id);
        if idn.len() < 3 {
            continue;
        }
        let sep = stem.starts_with(&format!("{}-", m.id))
            || stem.starts_with(&format!("{}_", m.id))
            || stem.starts_with(&format!("{}.", m.id));
        if nstem == idn || sep {
            return (Some(m.id.clone()), Some(m.name.clone()));
        }
    }

    // 3. id входит в имя файла — берём самый длинный (меньше ложных срабатываний).
    let mut best: Option<&ModMeta> = None;
    for m in mods {
        let idn = norm_path(&m.id);
        if idn.len() < 3 || !nstem.contains(&idn) {
            continue;
        }
        if best
            .map(|b| norm_path(&b.id).len() < idn.len())
            .unwrap_or(true)
        {
            best = Some(m);
        }
    }
    if let Some(m) = best {
        return (Some(m.id.clone()), Some(m.name.clone()));
    }

    // 4. Отображаемое имя мода в пути.
    let mut best: Option<&ModMeta> = None;
    for m in mods {
        let n = norm_path(&m.name);
        if n.len() < 5 || n == norm_path(&m.id) || !npath.contains(&n) {
            continue;
        }
        if best.map(|b| b.name.len() < m.name.len()).unwrap_or(true) {
            best = Some(m);
        }
    }
    if let Some(m) = best {
        return (Some(m.id.clone()), Some(m.name.clone()));
    }
    (None, None)
}

/// Список конфигов профиля.
pub async fn configs(id: &str) -> Res<Vec<ConfigEntry>> {
    let inst = load(id).await?;
    let mods = scan_mods(&inst);
    Ok(collect_configs(&inst, &mods))
}

fn cfg_path(id: &str, rel: &str) -> Res<PathBuf> {
    let low = rel.replace('\\', "/").to_ascii_lowercase();
    if !CFG_DIRS.iter().any(|d| low.starts_with(&format!("{d}/"))) {
        return err("Редактировать можно только файлы из config/");
    }
    let game = paths::instance_minecraft_dir(id);
    let path = crate::content::safe_join(&game, rel)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !CFG_EXT.contains(&ext.as_str()) {
        return err("Это не текстовый файл конфигурации");
    }
    Ok(path)
}

fn bak(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bak");
    PathBuf::from(s)
}

async fn read_doc(id: &str, rel: &str) -> Res<ConfigDoc> {
    let path = cfg_path(id, rel)?;
    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(_) => return err("Файл конфигурации не найден"),
    };
    if meta.len() > MAX_CFG {
        return err("Файл слишком большой для редактора");
    }
    let bytes = tokio::fs::read(&path).await?;
    let content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return err("Файл не является текстом UTF-8"),
    };
    Ok(ConfigDoc {
        path: rel.to_string(),
        size: meta.len(),
        content,
        has_backup: bak(&path).is_file(),
    })
}

pub async fn read_config(id: &str, rel: &str) -> Res<ConfigDoc> {
    load(id).await?;
    read_doc(id, rel).await
}

async fn write_text_atomic(path: &Path, data: &[u8]) -> Res<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config");
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    if let Err(e) = tokio::fs::write(&tmp, data).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    let mut last: Option<std::io::Error> = None;
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

pub async fn save_config(id: &str, rel: &str, content: &str) -> Res<ConfigDoc> {
    load(id).await?;
    let path = cfg_path(id, rel)?;
    if content.len() as u64 > MAX_CFG {
        return err("Файл слишком большой для редактора");
    }
    if tokio::fs::metadata(&path).await.is_ok() {
        let _ = tokio::fs::copy(&path, bak(&path)).await;
    }
    write_text_atomic(&path, content.as_bytes()).await?;
    read_doc(id, rel).await
}

pub async fn restore_config(id: &str, rel: &str) -> Res<ConfigDoc> {
    load(id).await?;
    let path = cfg_path(id, rel)?;
    let data = match tokio::fs::read(bak(&path)).await {
        Ok(d) => d,
        Err(_) => return err("Резервная копия не найдена"),
    };
    write_text_atomic(&path, &data).await?;
    read_doc(id, rel).await
}

// ---------------------------------------------------------------------------
// Проверка сборки
// ---------------------------------------------------------------------------

fn norm_id(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

pub async fn validate(id: &str) -> Res<Vec<PackIssue>> {
    let inst = load(id).await?;
    let mods = scan_mods(&inst);
    let mut out: Vec<PackIssue> = Vec::new();

    // Дубликаты: два jar-файла с одним mod id.
    let mut by_id: HashMap<String, Vec<&ModMeta>> = HashMap::new();
    for m in &mods {
        by_id.entry(m.id.to_ascii_lowercase()).or_default().push(m);
    }
    for (mid, list) in by_id.iter() {
        if list.len() > 1 {
            let files = list
                .iter()
                .map(|m| m.file.clone())
                .collect::<Vec<_>>()
                .join(", ");
            out.push(PackIssue {
                severity: "error".into(),
                title: format!("Дубликат мода «{mid}»"),
                detail: format!("Установлено несколько файлов: {files}. Оставьте один."),
            });
        }
    }

    let enabled: Vec<&ModMeta> = mods.iter().filter(|m| m.enabled).collect();
    let mut have: HashSet<String> = HashSet::new();
    for m in &enabled {
        have.insert(m.id.to_ascii_lowercase());
        for p in &m.provides {
            have.insert(p.to_ascii_lowercase());
        }
    }
    let all: HashSet<String> = mods.iter().map(|m| m.id.to_ascii_lowercase()).collect();
    let have_fabric_api = have.contains("fabric-api")
        || have.contains("fabric_api")
        || have.contains("fabricapi");
    let have_qfapi = have.contains("qfapi")
        || have.contains("quilted_fabric_api")
        || have.contains("quilted-fabric-api");

    for m in &enabled {
        for dep in &m.depends {
            let d = dep.to_ascii_lowercase();
            if d.is_empty() || BUILTIN_DEPS.contains(&d.as_str()) || have.contains(&d) {
                continue;
            }
            // Fabric API поставляет модули вида fabric-rendering-v1.
            if (d.starts_with("fabric-") || d.starts_with("fabric_")) && have_fabric_api {
                continue;
            }
            if (d.starts_with("quilted-") || d.starts_with("quilt-")) && (have_qfapi || have_fabric_api) {
                continue;
            }
            // Мягкое сравнение id: cloth-config2 ~ cloth-config.
            let dn = norm_id(&d);
            let soft = dn.len() >= 4
                && enabled.iter().any(|x| {
                    let xn = norm_id(&x.id);
                    xn.len() >= 4 && (xn == dn || xn.contains(&dn) || dn.contains(&xn))
                });
            if soft {
                continue;
            }

            if all.contains(&d) {
                out.push(PackIssue {
                    severity: "warn".into(),
                    title: format!("Зависимость отключена: {d}"),
                    detail: format!("Мод «{}» требует «{d}», но этот мод выключен.", m.name),
                });
            } else {
                out.push(PackIssue {
                    severity: "error".into(),
                    title: format!("Не хватает зависимости: {d}"),
                    detail: format!(
                        "Моду «{}» нужен «{d}». Добавьте его на вкладке «Контент».",
                        m.name
                    ),
                });
            }
        }
    }

    if enabled.is_empty() {
        out.push(PackIssue {
            severity: "info".into(),
            title: "В сборке нет модов".into(),
            detail: "Добавьте моды на вкладке «Контент» — иначе это обычный профиль.".into(),
        });
    }

    let need = default_java(&inst.mc_version);
    if need > inst.java_major {
        out.push(PackIssue {
            severity: "warn".into(),
            title: format!("Нужна Java {need}"),
            detail: format!(
                "Для Minecraft {} нужна Java {need}, а в профиле выбрана Java {}.",
                inst.mc_version, inst.java_major
            ),
        });
    }

    if inst.loader != Loader::Vanilla && detect_loader_version(&inst).is_none() {
        out.push(PackIssue {
            severity: "info".into(),
            title: "Версия загрузчика не определена".into(),
            detail: "При экспорте в .mrpack её не получится записать в манифест — укажите её в настройках загрузчика.".into(),
        });
    }

    out.truncate(300);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Экспорт
// ---------------------------------------------------------------------------

fn collect_files(game: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for base in ["mods", "config", "defaultconfigs", "resourcepacks", "shaderpacks"] {
        let root = game.join(base);
        if !root.is_dir() {
            continue;
        }
        for e in walkdir::WalkDir::new(&root)
            .max_depth(10)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !e.file_type().is_file() {
                continue;
            }
            let p = e.path();
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let low = name.to_ascii_lowercase();
            if name.starts_with('.')
                || low.ends_with(".bak")
                || low.ends_with(".disabled")
                || low.ends_with(".tmp")
            {
                continue;
            }
            let Ok(rel) = p.strip_prefix(game) else {
                continue;
            };
            out.push((crate::content::rel_slash(rel), p.to_path_buf()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn write_pack(
    inst: &Instance,
    manifest: &PackManifest,
    loader_ver: Option<String>,
    dest: &Path,
    fmt: &str,
) -> Res<ExportResult> {
    let game = paths::instance_minecraft_dir(&inst.id);
    let files = collect_files(&game);
    if files.is_empty() {
        return err("В папке сборки нет файлов для экспорта");
    }

    let name = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("pack");
    let tmp = dest.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let f = std::fs::File::create(&tmp)?;
    let mut zw = zip::ZipWriter::new(f);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut bytes = 0u64;
    let mut count = 0usize;

    if fmt == "mrpack" {
        let mut deps = serde_json::Map::new();
        deps.insert("minecraft".into(), json!(inst.mc_version));
        if let Some(v) = &loader_ver {
            let key = match inst.loader {
                Loader::Fabric => "fabric-loader",
                Loader::Quilt => "quilt-loader",
                Loader::Forge => "forge",
                Loader::Neoforge => "neoforge",
                Loader::Vanilla => "",
            };
            if !key.is_empty() {
                deps.insert(key.into(), json!(v));
            }
        }
        let summary = if manifest.summary.trim().is_empty() {
            manifest
                .description
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(300)
                .collect::<String>()
        } else {
            manifest.summary.clone()
        };
        let index = json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": manifest.version,
            "name": manifest.name,
            "summary": summary,
            "files": [],
            "dependencies": deps,
        });
        let data = serde_json::to_vec_pretty(&index)?;
        zw.start_file("modrinth.index.json", deflated)?;
        zw.write_all(&data)?;
        bytes += data.len() as u64;
        count += 1;
    } else {
        let data = serde_json::to_vec_pretty(manifest)?;
        zw.start_file("pack.json", deflated)?;
        zw.write_all(&data)?;
        bytes += data.len() as u64;
        count += 1;
    }

    for (rel, path) in &files {
        let zname = if fmt == "mrpack" {
            format!("overrides/{rel}")
        } else {
            rel.clone()
        };
        let opts = if rel.ends_with(".jar") || rel.ends_with(".zip") {
            stored
        } else {
            deflated
        };
        zw.start_file(&zname, opts)?;
        let data = std::fs::read(path)?;
        bytes += data.len() as u64;
        zw.write_all(&data)?;
        count += 1;
    }

    zw.finish()?;
    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(ExportResult {
        path: dest.to_string_lossy().to_string(),
        files: count,
        bytes,
    })
}

/// Экспорт сборки: `format` = `mrpack` | `zip`.
pub async fn export(id: &str, dest: &str, format: &str) -> Res<ExportResult> {
    let inst = load(id).await?;
    let fmt = format.to_ascii_lowercase();
    if fmt != "mrpack" && fmt != "zip" {
        return err("Неизвестный формат экспорта");
    }
    let raw = dest.trim();
    if raw.is_empty() {
        return err("Не указан путь для сохранения");
    }
    let mut path = PathBuf::from(raw);
    if !path.is_absolute() {
        return err("Укажите полный путь для сохранения");
    }
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(&fmt))
        .unwrap_or(false);
    if !ext_ok {
        path.set_extension(&fmt);
    }
    if let Some(parent) = path.parent() {
        if !parent.is_dir() {
            return err("Папка для сохранения не существует");
        }
    }

    let manifest = read_manifest(&inst).await;
    let loader_ver = detect_loader_version(&inst);
    tokio::task::spawn_blocking(move || {
        write_pack(&inst, &manifest, loader_ver, &path, &fmt)
    })
    .await?
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, name: &str) -> ModMeta {
        ModMeta {
            id: id.into(),
            name: name.into(),
            file: format!("mods/{id}.jar"),
            enabled: true,
            depends: Vec::new(),
            provides: Vec::new(),
        }
    }

    #[test]
    fn clean_id_is_stable() {
        assert_eq!(clean_id("Sodium Fabric 0.5.8"), "sodium-fabric-0-5-8");
        assert_eq!(clean_id("create_1.20.1"), "create_1-20-1");
    }

    #[test]
    fn matches_directory_and_prefix() {
        let mods = vec![m("create", "Create"), m("sodium", "Sodium")];
        assert_eq!(
            match_mod("config/create/server.toml", &mods).0.as_deref(),
            Some("create")
        );
        assert_eq!(
            match_mod("config/create-common.toml", &mods).0.as_deref(),
            Some("create")
        );
        assert_eq!(
            match_mod("config/sodium-options.json", &mods).0.as_deref(),
            Some("sodium")
        );
        assert_eq!(match_mod("config/forge-client.toml", &mods).0, None);
    }

    #[test]
    fn matches_display_name() {
        let mods = vec![m("iris", "Iris Shaders")];
        assert_eq!(
            match_mod("config/iris shaders/settings.toml", &mods).0.as_deref(),
            Some("iris")
        );
    }
}
