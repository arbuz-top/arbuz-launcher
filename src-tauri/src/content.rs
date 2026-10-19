//! Контент профиля и миры (ТЗ §9).
//!
//! Сканируются `mods/`, `resourcepacks/`, `shaderpacks/` и
//! `saves/*/datapacks/`. Файл считается выключенным, если его имя заканчивается
//! на `.disabled`. Имя и версия мода читаются из jar, у ресурспаков и шейдеров
//! берётся имя файла без расширения.
//!
//! Миры читаются из `saves/*/level.dat` — это gzip-NBT.

use crate::error::{err, Res};
use crate::{paths, store};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Запись `content.json` об одном установленном через каталог файле.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub sha1: String,
}

/// Что установлено через каталог: относительный путь -> запись.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentMeta {
    pub files: BTreeMap<String, Entry>,
    /// Файлы, помеченные пользователем как «изолируемые»: их наличие включает
    /// песочницу при запуске игры (см. mojang::launch).
    pub isolated: BTreeSet<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    /// Относительный путь от `minecraft/` без суффикса `.disabled`.
    pub file: String,
    pub name: String,
    pub version: String,
    /// mod | rp | sh | dp
    pub kind: String,
    pub size_bytes: u64,
    pub enabled: bool,
    pub sha1: String,
    pub project_id: Option<String>,
    /// Источник, из которого файл поставлен через каталог.
    pub source: Option<String>,
    /// Абсолютный путь к иконке в кэше (для `convertFileSrc`).
    pub icon: Option<String>,
    /// Иконка проекта на CDN, если своей в архиве нет.
    pub icon_url: Option<String>,
    /// Помечен ли файл как изолируемый (песочница при запуске).
    pub isolated: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// Имя папки внутри `saves/`.
    pub dir: String,
    pub name: String,
    /// Выживание | Креатив | Приключение | Наблюдатель | Хардкор
    pub mode: String,
    pub last_played_ms: i64,
}

// ---------------------------------------------------------------------------
// Метаданные jar-файла
// ---------------------------------------------------------------------------

/// Имя и версия контента из jar. Если метаданных нет — имя файла без расширения.
pub fn read_meta(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let Ok(f) = std::fs::File::open(path) else {
        return (stem, String::new());
    };
    let Ok(mut z) = zip::ZipArchive::new(f) else {
        return (stem, String::new());
    };
    if let Ok(e) = z.by_name("fabric.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(e) {
            return (
                v["name"].as_str().or(v["id"].as_str()).unwrap_or(&stem).into(),
                v["version"].as_str().unwrap_or("").into(),
            );
        }
    }
    if let Ok(e) = z.by_name("quilt.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(e) {
            let m = &v["quilt_loader"];
            return (
                m["metadata"]["name"].as_str().or(m["id"].as_str()).unwrap_or(&stem).into(),
                m["version"].as_str().unwrap_or("").into(),
            );
        }
    }
    for n in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Ok(mut e) = z.by_name(n) {
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            if let Ok(t) = toml::from_str::<toml::Value>(&s) {
                if let Some(m) = t.get("mods").and_then(|m| m.as_array()).and_then(|a| a.first()) {
                    let v = m.get("version").and_then(|x| x.as_str()).unwrap_or("");
                    return (
                        m.get("displayName").and_then(|x| x.as_str()).unwrap_or(&stem).into(),
                        if v.starts_with("${") { String::new() } else { v.into() },
                    );
                }
            }
        }
    }
    (stem, String::new())
}

// ---------------------------------------------------------------------------
// Иконки контента
// ---------------------------------------------------------------------------

/// Иконки крупнее этого размера не показываем: это уже не иконка.
const MAX_ICON_BYTES: u64 = 256 * 1024;

/// Путь к иконке внутри архива.
///
/// Моды указывают её в `fabric.mod.json` / `quilt.mod.json` / `mods.toml`,
/// ресурспаки и шейдеры кладут `pack.png` в корень.
fn icon_entry(z: &mut zip::ZipArchive<std::fs::File>) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();

    if let Ok(e) = z.by_name("fabric.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(e) {
            candidates.extend(icon_from_json(&v["icon"]));
        }
    }
    if let Ok(e) = z.by_name("quilt.mod.json") {
        if let Ok(v) = serde_json::from_reader::<_, serde_json::Value>(e) {
            candidates.extend(icon_from_json(&v["quilt_loader"]["metadata"]["icon"]));
        }
    }
    for n in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Ok(mut e) = z.by_name(n) {
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            if let Ok(t) = toml::from_str::<toml::Value>(&s) {
                if let Some(p) = t
                    .get("mods")
                    .and_then(|m| m.as_array())
                    .and_then(|a| a.first())
                    .and_then(|m| m.get("logoFile"))
                    .and_then(|x| x.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    candidates.push(p.to_string());
                }
            }
        }
    }
    // У ресурспаков и шейдеров иконка лежит в корне архива.
    candidates.push("pack.png".to_string());

    // Берём первый путь, который в архиве действительно есть.
    candidates.into_iter().find(|n| z.by_name(n).is_ok())
}

/// Поле `icon`: строка с путём или объект «размер → путь».
fn icon_from_json(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    let mut best: Option<(u64, &str)> = None;
    for (k, val) in v.as_object()? {
        let Ok(size) = k.parse::<u64>() else {
            continue;
        };
        let Some(p) = val.as_str() else {
            continue;
        };
        if best.map(|(s, _)| size > s).unwrap_or(true) {
            best = Some((size, p));
        }
    }
    best.map(|(_, p)| p.to_string())
}

/// PNG-иконка из архива. `None`, если её нет или она не PNG.
fn read_icon(path: &Path) -> Option<Vec<u8>> {
    let f = std::fs::File::open(path).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    let name = icon_entry(&mut z)?;
    let mut e = z.by_name(&name).ok()?;
    let size = e.size();
    if size < 8 || size > MAX_ICON_BYTES {
        return None;
    }
    let mut buf = Vec::with_capacity(size as usize);
    e.read_to_end(&mut buf).ok()?;
    if &buf[..4] != b"\x89PNG" {
        return None;
    }
    Some(buf)
}

/// Имя файла в кэше иконок: зависит от содержимого, поэтому обновление
/// мода автоматически даёт новую иконку.
fn icon_key(rel: &str, size: u64, mtime_ms: u128) -> String {
    let mut h = Sha1::new();
    h.update(rel.as_bytes());
    h.update(b"\0");
    h.update(size.to_string().as_bytes());
    h.update(b"\0");
    h.update(mtime_ms.to_string().as_bytes());
    let full = hex::encode(h.finalize());
    full[..20].to_string()
}

/// Распаковывает иконку в кэш и возвращает путь к файлу.
fn cached_icon(path: &Path, rel: &str, size: u64, mtime_ms: u128) -> Option<String> {
    let dir = paths::icons_cache_dir();
    let file = dir.join(format!("{}.png", icon_key(rel, size, mtime_ms)));
    if !file.is_file() {
        let bytes = read_icon(path)?;
        std::fs::create_dir_all(&dir).ok()?;
        // Пишем через временный файл, чтобы не оставить обрезанную иконку.
        let tmp = file.with_extension("part");
        std::fs::write(&tmp, &bytes).ok()?;
        std::fs::rename(&tmp, &file).ok()?;
    }
    Some(file.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Пути
// ---------------------------------------------------------------------------

/// Относительный путь в виде `mods/x.jar` (прямые слэши) — идентификатор файла
/// для фронтенда, одинаковый на любой платформе.
pub(crate) fn rel_slash(p: &Path) -> String {
    p.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Компонент относительного пути небезопасен: `..`, диск/ADS (`:`), NUL, а
/// также компоненты, которые Windows нормализует в `.`/`..` (завершающие
/// пробелы и точки). Без этой проверки `".. "` проходит как обычное имя, а
/// Win32 затем схлопывает его в `..` и уводит запись за пределы каталога.
pub(crate) fn bad_component(part: &str) -> bool {
    if part.contains(':') || part.contains('\0') {
        return true;
    }
    let trimmed = part.trim_end_matches([' ', '.']);
    trimmed.is_empty() || trimmed == "." || trimmed == ".."
}

/// Относительный путь из компонентов архива/индекса, безопасный для `join`.
/// `None`, если путь пустой или содержит небезопасный компонент.
pub(crate) fn safe_rel(raw: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut any = false;
    for part in raw.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if bad_component(part) {
            return None;
        }
        out.push(part);
        any = true;
    }
    if any {
        Some(out)
    } else {
        None
    }
}

/// Относительный путь от `gameDir` в абсолютный. Отвергает выход за пределы
/// папки профиля: `..`, абсолютные пути и префиксы диска.
pub(crate) fn safe_join(game: &Path, rel: &str) -> Res<PathBuf> {
    match safe_rel(rel) {
        Some(r) => Ok(game.join(r)),
        None => err("Недопустимый путь файла"),
    }
}

fn with_disabled(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".disabled");
    PathBuf::from(s)
}

fn strip_disabled(s: &str) -> &str {
    s.strip_suffix(".disabled").unwrap_or(s)
}

fn is_disabled(name: &str) -> bool {
    name.ends_with(".disabled")
}

/// Расширения, которые считаются контентом указанного вида.
fn is_content(kind: &str, name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    match kind {
        "mod" => n.ends_with(".jar"),
        "rp" | "sh" => n.ends_with(".zip"),
        "dp" => n.ends_with(".zip") || n.ends_with(".jar"),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Сканирование
// ---------------------------------------------------------------------------

fn push_item(
    path: &Path,
    kind: &str,
    game: &Path,
    meta: &ContentMeta,
    out: &mut Vec<ContentItem>,
) {
    let Some(raw) = path.file_name().and_then(|s| s.to_str()) else {
        return;
    };
    let (base, enabled) = if is_disabled(raw) {
        (strip_disabled(raw), false)
    } else {
        (raw, true)
    };
    if !is_content(kind, base) {
        return;
    }
    let Ok(rel) = path.strip_prefix(game) else {
        return;
    };
    // `file` всегда без суффикса `.disabled` — это стабильный идентификатор,
    // по которому работают включение, выключение и удаление.
    let file = rel_slash(&rel.with_file_name(base));
    let meta_info = std::fs::metadata(path).ok();
    let size_bytes = meta_info.as_ref().map(|m| m.len()).unwrap_or(0);
    let (name, version) = if kind == "mod" {
        read_meta(path)
    } else {
        (base.to_string(), String::new())
    };
    // Иконку достаём только для архивов, которые её поддерживают.
    let icon = match kind {
        "mod" | "rp" | "sh" => {
            let mtime_ms = meta_info
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0);
            cached_icon(path, &file, size_bytes, mtime_ms)
        }
        _ => None,
    };
    let entry = meta.files.get(&file);
    let isolated = meta.isolated.contains(&file);
    out.push(ContentItem {
        file,
        name,
        version,
        kind: kind.to_string(),
        size_bytes,
        enabled,
        sha1: entry.map(|e| e.sha1.clone()).unwrap_or_default(),
        project_id: entry.map(|e| e.project_id.clone()),
        source: entry.map(|e| e.source.clone()),
        icon,
        icon_url: None,
        isolated,
    });
}

/// Обходит одну папку и добавляет найденный контент вида `kind`.
fn scan_dir(dir: &Path, kind: &str, game: &Path, meta: &ContentMeta, out: &mut Vec<ContentItem>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_file() {
            push_item(&path, kind, game, meta, out);
        }
    }
}

/// Датапаки лежат внутри миров: `saves/<world>/datapacks/`.
fn scan_datapacks(game: &Path, meta: &ContentMeta, out: &mut Vec<ContentItem>) {
    let Ok(rd) = std::fs::read_dir(game.join("saves")) else {
        return;
    };
    for entry in rd.flatten() {
        let dir = entry.path();
        if dir.is_dir() {
            scan_dir(&dir.join("datapacks"), "dp", game, meta, out);
        }
    }
}

fn scan(game: &Path, meta: &ContentMeta) -> Res<Vec<ContentItem>> {
    let mut out: Vec<ContentItem> = Vec::new();
    scan_dir(&game.join("mods"), "mod", game, meta, &mut out);
    scan_dir(&game.join("resourcepacks"), "rp", game, meta, &mut out);
    scan_dir(&game.join("shaderpacks"), "sh", game, meta, &mut out);
    scan_datapacks(game, meta, &mut out);
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(out)
}

async fn read_content_meta(id: &str) -> ContentMeta {
    store::read_json::<ContentMeta>(&paths::instance_content_json(id))
        .await
        .unwrap_or_default()
}

/// Читает `content.json` профиля (что установлено через каталог).
pub async fn load_meta(id: &str) -> ContentMeta {
    read_content_meta(id).await
}

/// Пишет `content.json` профиля.
pub async fn save_meta(id: &str, meta: &ContentMeta) -> Res<()> {
    store::write_json_atomic(&paths::instance_content_json(id), meta).await
}

/// Список установленного контента профиля.
pub async fn list(id: &str) -> Res<Vec<ContentItem>> {
    let game = paths::instance_minecraft_dir(id);
    let meta = read_content_meta(id).await;
    tokio::task::spawn_blocking(move || scan(&game, &meta)).await?
}

// ---------------------------------------------------------------------------
// Включение, выключение, удаление
// ---------------------------------------------------------------------------

/// Помечает файл как изолируемый или снимает пометку.
pub async fn set_isolated(id: &str, file: &str, isolated: bool) -> Res<()> {
    let key = strip_disabled(file).to_string();
    let mut meta = load_meta(id).await;
    if isolated {
        meta.isolated.insert(key);
    } else {
        meta.isolated.remove(&key);
    }
    save_meta(id, &meta).await
}

/// Включает или выключает файл: `enabled=false` → `X` становится `X.disabled`,
/// `enabled=true` → суффикс `.disabled` убирается.
pub async fn toggle(id: &str, file: &str, enabled: bool) -> Res<()> {
    let game = paths::instance_minecraft_dir(id);
    let on = safe_join(&game, strip_disabled(file))?;
    let off = with_disabled(&on);

    tokio::task::spawn_blocking(move || -> Res<()> {
        if enabled {
            if off.is_file() {
                std::fs::rename(&off, &on)?;
            } else if !on.is_file() {
                return err("Файл не найден");
            }
        } else if on.is_file() {
            std::fs::rename(&on, &off)?;
        } else if !off.is_file() {
            return err("Файл не найден");
        }
        Ok(())
    })
    .await?
}

/// Удаляет файл контента (и его `.disabled`-версию, если она есть).
pub async fn remove(id: &str, file: &str) -> Res<()> {
    let game = paths::instance_minecraft_dir(id);
    let base = safe_join(&game, strip_disabled(file))?;
    let rel = rel_slash(&base);

    tokio::task::spawn_blocking(move || -> Res<()> {
        let mut found = false;
        for p in [base.clone(), with_disabled(&base)] {
            if p.is_file() {
                std::fs::remove_file(&p)?;
                found = true;
            }
        }
        if !found {
            return err("Файл не найден");
        }
        Ok(())
    })
    .await??;

    let mut meta = read_content_meta(id).await;
    meta.files.remove(&rel);
    save_meta(id, &meta).await
}

// ---------------------------------------------------------------------------
// Миры
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LevelDat {
    #[serde(rename = "Data")]
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "LevelName", default)]
    name: String,
    #[serde(rename = "GameType", default)]
    game_type: i32,
    #[serde(rename = "LastPlayed", default)]
    last_played: i64,
    #[serde(rename = "hardcore", default)]
    hardcore: i8,
}

fn mode_label(game_type: i32, hardcore: i8) -> &'static str {
    if hardcore != 0 {
        return "Хардкор";
    }
    match game_type {
        1 => "Креатив",
        2 => "Приключение",
        3 => "Наблюдатель",
        _ => "Выживание",
    }
}

/// Предел распакованного `level.dat`: защита от gzip-бомбы (миры приходят в
/// том числе из `overrides/` публичных сборок).
const MAX_LEVEL_DAT: u64 = 64 * 1024 * 1024;

/// Читает один `level.dat` (gzip-NBT).
fn read_level(dir: &Path, level: &Path) -> Option<World> {
    let f = std::fs::File::open(level).ok()?;
    let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
    let mut buf = Vec::new();
    gz.take(MAX_LEVEL_DAT).read_to_end(&mut buf).ok()?;
    if buf.len() as u64 >= MAX_LEVEL_DAT {
        return None;
    }
    let lvl: LevelDat = fastnbt::from_bytes(&buf).ok()?;

    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let name = if lvl.data.name.trim().is_empty() {
        dir_name.clone()
    } else {
        lvl.data.name
    };
    Some(World {
        dir: dir_name,
        name,
        mode: mode_label(lvl.data.game_type, lvl.data.hardcore).to_string(),
        last_played_ms: lvl.data.last_played,
    })
}

fn read_worlds(game: &Path) -> Vec<World> {
    let mut out: Vec<World> = Vec::new();
    let Ok(rd) = std::fs::read_dir(game.join("saves")) else {
        return out;
    };
    for entry in rd.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let level = dir.join("level.dat");
        if !level.is_file() {
            continue;
        }
        if let Some(w) = read_level(&dir, &level) {
            out.push(w);
        }
    }
    out.sort_by(|a, b| b.last_played_ms.cmp(&a.last_played_ms));
    out
}

/// Миры профиля, свежие сверху.
pub async fn worlds(id: &str) -> Res<Vec<World>> {
    let game = paths::instance_minecraft_dir(id);
    Ok(tokio::task::spawn_blocking(move || read_worlds(&game)).await?)
}

// ---------------------------------------------------------------------------
// Скриншоты
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub file: String,
    pub size: u64,
    pub modified: i64,
}

/// Скриншоты профиля: `minecraft/screenshots/*.png|jpg|jpeg`, свежие сверху.
pub async fn screenshots(id: &str) -> Res<Vec<Shot>> {
    let dir = paths::instance_minecraft_dir(id).join("screenshots");
    let mut out: Vec<Shot> = Vec::new();
    let mut rd = match tokio::fs::read_dir(&dir).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
            continue;
        }
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        out.push(Shot {
            file: name.to_string(),
            size: meta.len(),
            modified,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.file.cmp(&b.file)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_escape() {
        let game = Path::new("C:/data/instances/x/minecraft");
        assert!(safe_join(game, "../other/file.jar").is_err());
        assert!(safe_join(game, "mods/../../file.jar").is_err());
        assert!(safe_join(game, "C:/Windows/system32").is_err());
        assert!(safe_join(game, "mods/a.jar:stream").is_err());
        assert!(safe_join(game, "").is_err());
    }

    #[test]
    fn safe_join_keeps_normal_path() {
        let game = Path::new("C:/data/instances/x/minecraft");
        let p = safe_join(game, "mods/sodium.jar").unwrap_or_default();
        let rel = p.strip_prefix(game).unwrap_or(Path::new(""));
        assert_eq!(rel_slash(rel), "mods/sodium.jar");
    }

    #[test]
    fn safe_join_rejects_windows_normalized_components() {
        let game = Path::new("C:/data/instances/x/minecraft");
        // Windows схлопывает завершающие пробелы и точки, поэтому ".. " == "..".
        assert!(safe_join(game, ".. /.. /evil.jar").is_err());
        assert!(safe_join(game, "mods/.. /evil.jar").is_err());
        assert!(safe_join(game, ".../evil.jar").is_err());
        assert!(safe_join(game, "mods/sodium.jar").is_ok());
    }

    #[test]
    fn bad_component_matches_windows_normalization() {
        assert!(bad_component(".."));
        assert!(bad_component(".. "));
        assert!(bad_component(".. ."));
        assert!(bad_component("..."));
        assert!(bad_component("a:b"));
        assert!(bad_component("x\0y"));
        assert!(!bad_component("mods"));
        assert!(!bad_component("sodium-1.0.jar"));
    }

    #[test]
    fn safe_rel_neutralizes_absolute_and_unc() {
        let slash = |p: PathBuf| p.to_string_lossy().replace('\\', "/");
        // Абсолютные и UNC-пути становятся относительными внутри каталога.
        assert_eq!(safe_rel("/etc/passwd").map(slash), Some("etc/passwd".to_string()));
        assert_eq!(
            safe_rel("\\\\server\\share\\x.jar").map(slash),
            Some("server/share/x.jar".to_string())
        );
        // Диск и ADS отвергаются.
        assert!(safe_rel("C:/Windows/x").is_none());
        assert!(safe_rel("a.jar:stream").is_none());
        assert!(safe_rel("").is_none());
    }

    #[test]
    fn disabled_suffix_round_trip() {
        let p = Path::new("mods/a.jar");
        let d = with_disabled(p);
        assert_eq!(d.file_name().and_then(|s| s.to_str()), Some("a.jar.disabled"));
        assert_eq!(strip_disabled("a.jar.disabled"), "a.jar");
        assert_eq!(strip_disabled("a.jar"), "a.jar");
    }

    #[test]
    fn content_kinds_by_extension() {
        assert!(is_content("mod", "sodium.jar"));
        assert!(!is_content("mod", "sodium.zip"));
        assert!(is_content("rp", "faithful.zip"));
        assert!(is_content("sh", "bsl.zip"));
        assert!(is_content("dp", "tweak.zip"));
        assert!(is_content("dp", "tweak.jar"));
    }

    #[test]
    fn mode_labels() {
        assert_eq!(mode_label(0, 0), "Выживание");
        assert_eq!(mode_label(1, 0), "Креатив");
        assert_eq!(mode_label(2, 0), "Приключение");
        assert_eq!(mode_label(3, 0), "Наблюдатель");
        assert_eq!(mode_label(0, 1), "Хардкор");
    }

    #[test]
    fn icon_from_string() {
        let v = serde_json::json!("assets/icon.png");
        assert_eq!(icon_from_json(&v).as_deref(), Some("assets/icon.png"));
    }

    #[test]
    fn icon_from_size_map_picks_largest() {
        let v = serde_json::json!({ "32": "a.png", "128": "b.png", "64": "c.png" });
        assert_eq!(icon_from_json(&v).as_deref(), Some("b.png"));
    }

    #[test]
    fn icon_absent_is_none() {
        assert!(icon_from_json(&serde_json::Value::Null).is_none());
        assert!(icon_from_json(&serde_json::json!({})).is_none());
        assert!(icon_from_json(&serde_json::json!({ "not-a-size": "a.png" })).is_none());
    }

    #[test]
    fn icon_key_changes_with_content() {
        assert_eq!(icon_key("mods/a.jar", 10, 1), icon_key("mods/a.jar", 10, 1));
        assert_ne!(icon_key("mods/a.jar", 10, 1), icon_key("mods/a.jar", 11, 1));
        assert_ne!(icon_key("mods/a.jar", 10, 1), icon_key("mods/a.jar", 10, 2));
        assert_ne!(icon_key("mods/a.jar", 10, 1), icon_key("mods/b.jar", 10, 1));
    }

    /// Небольшой PNG: важна только сигнатура, картинку никто не рисует.
    const PNG_STUB: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x00";

    /// Собирает zip во временном файле.
    fn write_zip(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        use std::io::Write as _;
        let path = std::env::temp_dir().join(name);
        let Ok(f) = std::fs::File::create(&path) else {
            panic!("не удалось создать {path:?}");
        };
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (n, data) in files {
            if let Err(e) = w.start_file(*n, opts) {
                panic!("start_file: {e}");
            }
            if let Err(e) = w.write_all(data) {
                panic!("write: {e}");
            }
        }
        if let Err(e) = w.finish() {
            panic!("finish: {e}");
        }
        path
    }

    #[test]
    fn icon_from_fabric_mod_json() {
        let path = write_zip(
            "arbuz_icon_fabric.zip",
            &[
                ("fabric.mod.json", br#"{"id":"x","icon":"assets/x.png"}"#),
                ("assets/x.png", PNG_STUB),
            ],
        );
        assert_eq!(read_icon(&path).as_deref(), Some(PNG_STUB));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn icon_falls_back_to_pack_png() {
        let path = write_zip(
            "arbuz_icon_pack.zip",
            &[("pack.mcmeta", b"{}"), ("pack.png", PNG_STUB)],
        );
        assert_eq!(read_icon(&path).as_deref(), Some(PNG_STUB));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn icon_absent_in_archive_is_none() {
        let path = write_zip("arbuz_icon_none.zip", &[("readme.txt", b"no icon here")]);
        assert!(read_icon(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }
}
