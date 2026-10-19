//! Установка, обновление и удаление Quarry.
//!
//! Всё, что трогает систему, живёт здесь: файлы в папке пользователя, ярлыки,
//! запись в списке установленных программ и очистка после удаления.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const MARKER: &str = ".quarry-install";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Quarry";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Готовый лаунчер, собранный перед сборкой установщика.
static LAUNCHER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src-tauri/target/release/quarry.exe"
));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

pub type Res<T> = Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Env {
    pub version: String,
    pub default_dir: String,
    pub installed: bool,
    pub installed_dir: Option<String>,
    pub installed_version: Option<String>,
    pub uninstall_mode: bool,
    pub payload_mb: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub percent: u32,
    /// 0 — подготовка, 1 — файлы, 2 — ярлыки, 3 — реестр, 4 — готово.
    pub stage: u8,
    pub text: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOpts {
    pub dir: String,
    pub desktop: bool,
    pub start_menu: bool,
    pub launch: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallDone {
    pub dir: String,
    pub version: String,
    pub launched: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallOpts {
    pub dir: String,
    pub data: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallDone {
    pub data_removed: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marker {
    version: String,
    installed_at: u64,
}

struct Installed {
    dir: PathBuf,
    version: String,
}

/// Состояние для первого экрана: версия, путь, признак установки.
pub fn environment() -> Env {
    let uninstall_mode = std::env::args().any(|a| a == "--uninstall");
    let current = std::env::current_exe().ok();
    let fallback = current
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(default_install_dir);

    let mut installed = installed_info();
    // uninstall.exe мог остаться без записи в реестре — ориентируемся на маркер.
    if uninstall_mode && installed.is_none() && fallback.join(MARKER).is_file() {
        installed = Some(Installed {
            dir: fallback.clone(),
            version: marker_version(&fallback).unwrap_or_default(),
        });
    }

    let dir = if uninstall_mode {
        installed
            .as_ref()
            .map(|i| i.dir.clone())
            .unwrap_or(fallback)
    } else {
        installed
            .as_ref()
            .map(|i| i.dir.clone())
            .unwrap_or_else(default_install_dir)
    };

    let installer_size = current
        .as_ref()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0);
    let total = LAUNCHER.len() as u64 + installer_size;
    let payload_mb = ((total + 1024 * 1024 - 1) / (1024 * 1024)).max(1);

    Env {
        version: env!("CARGO_PKG_VERSION").into(),
        default_dir: dir.to_string_lossy().into_owned(),
        installed: installed.is_some(),
        installed_dir: installed
            .as_ref()
            .map(|i| i.dir.to_string_lossy().into_owned()),
        installed_version: installed.as_ref().map(|i| i.version.clone()),
        uninstall_mode,
        payload_mb,
    }
}

pub fn default_install_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs")
        .join("Quarry")
}

pub async fn run_install(app: AppHandle, opts: InstallOpts) -> Res<InstallDone> {
    let dir = normalize_dir(&opts.dir)?;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || install_blocking(&handle, &dir, &opts))
        .await
        .map_err(|e| Error::Msg(format!("Не удалось запустить установку: {e}")))?
}

pub async fn run_uninstall(app: AppHandle, opts: UninstallOpts) -> Res<UninstallDone> {
    let dir = normalize_dir(&opts.dir)?;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || uninstall_blocking(&handle, &dir, opts.data))
        .await
        .map_err(|e| Error::Msg(format!("Не удалось запустить удаление: {e}")))?
}

fn install_blocking(app: &AppHandle, dir: &Path, opts: &InstallOpts) -> Res<InstallDone> {
    emit(app, 2, 0, "Подготовка");
    let previous = installed_info().map(|i| i.dir);
    fs::create_dir_all(dir)?;

    // Проверяем запись сразу, чтобы не копировать десятки мегабайт впустую.
    let probe = dir.join(".quarry-write-test");
    fs::write(&probe, b"ok")?;
    fs::remove_file(&probe)?;

    let exe = dir.join("quarry.exe");
    if exe.is_file() && is_locked(&exe) {
        return Err(Error::Msg(
            "Quarry запущен. Закройте лаунчер и попробуйте снова.".into(),
        ));
    }

    emit(app, 6, 1, "Копирование quarry.exe");
    copy_with_progress(app, &exe, LAUNCHER, 6, 82)?;

    emit(app, 84, 1, "Подготовка удаления");
    let me = std::env::current_exe()?;
    let uninstaller = dir.join("uninstall.exe");
    if me != uninstaller {
        fs::copy(&me, &uninstaller)?;
    }

    let marker = Marker {
        version: env!("CARGO_PKG_VERSION").into(),
        installed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    fs::write(dir.join(MARKER), serde_json::to_vec_pretty(&marker)?)?;

    emit(app, 88, 2, "Создание ярлыков");
    // Старые ярлыки убираем всегда: если галочку сняли, ярлык не должен остаться.
    if let Some(desktop) = dirs::desktop_dir() {
        let _ = fs::remove_file(desktop.join("Quarry.lnk"));
    }
    if let Ok(menu) = start_menu_dir() {
        let _ = fs::remove_file(menu.join("Quarry.lnk"));
    }
    if opts.desktop {
        let desktop = dirs::desktop_dir()
            .ok_or_else(|| Error::Msg("Не удалось найти рабочий стол".into()))?;
        make_shortcut(&desktop.join("Quarry.lnk"), &exe, dir, &exe)?;
    }
    if opts.start_menu {
        let menu = start_menu_dir()?;
        fs::create_dir_all(&menu)?;
        make_shortcut(&menu.join("Quarry.lnk"), &exe, dir, &exe)?;
    }

    emit(app, 96, 3, "Запись в реестр");
    let size = fs::metadata(&exe)?.len() + fs::metadata(&uninstaller)?.len();
    write_registry(dir, &exe, size)?;

    // Переезд в другую папку: старую установку подчищаем, чужие файлы не трогаем.
    if let Some(old) = previous {
        if old != dir {
            let _ = remove_known_files(&old);
            let _ = fs::remove_dir(&old);
        }
    }

    let mut launched = false;
    if opts.launch {
        Command::new(&exe).current_dir(dir).spawn()?;
        launched = true;
    }

    emit(app, 100, 4, "Готово");
    Ok(InstallDone {
        dir: dir.to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION").into(),
        launched,
    })
}

fn uninstall_blocking(app: &AppHandle, dir: &Path, data: bool) -> Res<UninstallDone> {
    uninstall_impl(dir, data, &mut |p, s, t| emit(app, p, s, t))
}

/// Общий код удаления. Отчёты о прогрессе идут через `emit`, поэтому один и тот
/// же путь обслуживает и окно установщика, и тихое удаление без интерфейса.
fn uninstall_impl(
    dir: &Path,
    data: bool,
    emit: &mut dyn FnMut(u32, u8, &str),
) -> Res<UninstallDone> {
    if !dir.join(MARKER).is_file() {
        return Err(Error::Msg(
            "В этой папке нет установленного Quarry.".into(),
        ));
    }
    let exe = dir.join("quarry.exe");
    if exe.is_file() && is_locked(&exe) {
        return Err(Error::Msg(
            "Quarry запущен. Закройте лаунчер и попробуйте снова.".into(),
        ));
    }

    emit(6, 0, "Подготовка");
    emit(18, 1, "Удаление ярлыков");
    if let Some(desktop) = dirs::desktop_dir() {
        let _ = fs::remove_file(desktop.join("Quarry.lnk"));
    }
    if let Ok(menu) = start_menu_dir() {
        let _ = fs::remove_file(menu.join("Quarry.lnk"));
    }

    emit(40, 2, "Удаление файлов");
    remove_known_files(dir)?;

    emit(72, 3, "Очистка реестра");
    delete_registry();

    let mut data_removed = false;
    if data {
        emit(86, 3, "Удаление данных");
        data_removed = remove_data()?;
    }

    // Папка удалится, только если в ней не осталось чужих файлов.
    let _ = fs::remove_dir(dir);

    emit(100, 4, "Готово");
    Ok(UninstallDone { data_removed })
}

/// Тихий режим (`--uninstall --quiet`): удаление из списка программ Windows без
/// окна. Данные пользователя при этом сохраняются.
pub fn run_uninstall_quiet() -> Res<()> {
    let dir = installed_info()
        .map(|i| i.dir)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
                .filter(|d| d.join(MARKER).is_file())
        })
        .ok_or_else(|| Error::Msg("Quarry не установлен.".into()))?;

    let mut quiet = |_p: u32, _s: u8, _t: &str| {};
    uninstall_impl(&dir, false, &mut quiet)?;

    // uninstall.exe держит сам себя: папку дочистит хвостовой процесс.
    if let Ok(exe) = std::env::current_exe() {
        if exe.parent() == Some(dir.as_path()) {
            schedule_cleanup(&dir);
        }
    }
    Ok(())
}

fn emit(app: &AppHandle, percent: u32, stage: u8, text: &str) {
    let _ = app.emit(
        "progress",
        Progress {
            percent: percent.min(100),
            stage,
            text: text.into(),
        },
    );
}

/// Копирует байты кусками, чтобы полоса прогресса двигалась по-настоящему.
fn copy_bytes<F: FnMut(u32)>(
    dst: &Path,
    bytes: &[u8],
    from: u32,
    to: u32,
    mut on_progress: F,
) -> Res<()> {
    let mut file = File::create(dst)?;
    let total = bytes.len().max(1);
    let mut written = 0usize;
    let mut last = from;
    for chunk in bytes.chunks(512 * 1024) {
        file.write_all(chunk)?;
        written += chunk.len();
        let span = to.saturating_sub(from) as usize;
        let pct = from + (span * written / total) as u32;
        if pct > last {
            last = pct;
            on_progress(pct);
        }
    }
    file.sync_all()?;
    Ok(())
}

fn copy_with_progress(
    app: &AppHandle,
    dst: &Path,
    bytes: &[u8],
    from: u32,
    to: u32,
) -> Res<()> {
    copy_bytes(dst, bytes, from, to, |pct| {
        emit(app, pct, 1, "Копирование файлов");
    })
}

fn make_shortcut(lnk: &Path, target: &Path, work: &Path, icon: &Path) -> Res<()> {
    if let Some(parent) = lnk.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut link = mslnk::ShellLink::new(target)
        .map_err(|e| Error::Msg(format!("Не удалось создать ярлык: {e}")))?;
    link.set_name(Some("Quarry — лаунчер Minecraft".into()));
    link.set_working_dir(Some(work.to_string_lossy().into_owned()));
    link.set_icon_location(Some(icon.to_string_lossy().into_owned()));
    link.create_lnk(lnk)
        .map_err(|e| Error::Msg(format!("Не удалось создать ярлык: {e}")))?;
    Ok(())
}

fn start_menu_dir() -> Res<PathBuf> {
    dirs::data_dir()
        .map(|p| {
            p.join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
        })
        .ok_or_else(|| Error::Msg("Не удалось найти меню «Пуск»".into()))
}

fn installed_info() -> Option<Installed> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(UNINSTALL_KEY, KEY_READ)
        .ok()?;
    let location: String = key.get_value("InstallLocation").ok()?;
    let version: String = key.get_value("DisplayVersion").unwrap_or_default();
    let dir = PathBuf::from(location);
    if dir.join("quarry.exe").is_file() && dir.join(MARKER).is_file() {
        Some(Installed { dir, version })
    } else {
        None
    }
}

fn marker_version(dir: &Path) -> Option<String> {
    let raw = fs::read(dir.join(MARKER)).ok()?;
    let marker: Marker = serde_json::from_slice(&raw).ok()?;
    Some(marker.version)
}

fn write_registry(dir: &Path, exe: &Path, size: u64) -> Res<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(UNINSTALL_KEY)?;
    let dir_s = dir.to_string_lossy().into_owned();
    let exe_s = exe.to_string_lossy().into_owned();
    let uninstall_s = format!("\"{}\" --uninstall", dir.join("uninstall.exe").display());

    key.set_value("DisplayName", &"Quarry")?;
    key.set_value("DisplayVersion", &env!("CARGO_PKG_VERSION"))?;
    key.set_value("Publisher", &"Quarry")?;
    key.set_value("DisplayIcon", &exe_s)?;
    key.set_value("InstallLocation", &dir_s)?;
    key.set_value("UninstallString", &uninstall_s)?;
    key.set_value("QuietUninstallString", &format!("{uninstall_s} --quiet"))?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    key.set_value("EstimatedSize", &(((size + 1023) / 1024) as u32))?;
    Ok(())
}

fn delete_registry() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(UNINSTALL_KEY);
}

fn remove_known_files(dir: &Path) -> Res<()> {
    for name in ["quarry.exe", "uninstall.exe", MARKER] {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        if let Err(e) = fs::remove_file(&path) {
            // Запущенный uninstall.exe удалит хвост после выхода.
            if name == "uninstall.exe" {
                continue;
            }
            return Err(e.into());
        }
    }
    Ok(())
}

fn remove_data() -> Res<bool> {
    let Some(root) = dirs::data_dir().map(|p| p.join("Quarry")) else {
        return Ok(false);
    };
    if !root.is_dir() {
        return Ok(false);
    }
    fs::remove_dir_all(&root)?;
    Ok(true)
}

fn is_locked(path: &Path) -> bool {
    OpenOptions::new().write(true).open(path).is_err()
}

fn normalize_dir(input: &str) -> Res<PathBuf> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(Error::Msg("Укажите папку установки.".into()));
    }
    let path = std::path::absolute(raw)?;
    if path.parent().is_none() {
        return Err(Error::Msg("Нельзя установить в корень диска.".into()));
    }
    if let Some(home) = dirs::home_dir() {
        if path == home {
            return Err(Error::Msg("Выберите папку внутри профиля.".into()));
        }
    }
    if let Some(windir) = std::env::var_os("WINDIR") {
        if path.starts_with(PathBuf::from(windir)) {
            return Err(Error::Msg("Системная папка не подходит для установки.".into()));
        }
    }
    Ok(path)
}

/// Запускает лаунчер из установленной папки.
pub fn launch_installed() -> Res<()> {
    let installed =
        installed_info().ok_or_else(|| Error::Msg("Quarry не установлен.".into()))?;
    Command::new(installed.dir.join("quarry.exe"))
        .current_dir(&installed.dir)
        .spawn()?;
    Ok(())
}

pub fn open_folder(path: &str) -> Res<()> {
    let dir = PathBuf::from(path);
    if !dir.is_dir() {
        return Err(Error::Msg("Папка не найдена.".into()));
    }
    Command::new("explorer").arg(&dir).spawn()?;
    Ok(())
}

/// После выхода uninstall.exe дочищает свою папку (только если она пустая).
pub fn schedule_cleanup(dir: &Path) {
    let path = dir.to_string_lossy().replace('"', "");
    let cmd = format!(
        "timeout /t 2 /nobreak >nul & del /f /q \"{path}\\uninstall.exe\" >nul 2>&1 & rmdir /q \"{path}\" >nul 2>&1"
    );
    let mut command = Command::new("cmd");
    command.args(["/C", &cmd]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let _ = command.spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "quarry-inst-{}-{}-{}",
            std::process::id(),
            tag,
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn normalize_dir_rejects_drive_root() {
        assert!(normalize_dir("C:\\").is_err());
    }

    #[test]
    fn normalize_dir_makes_absolute() {
        let path = match normalize_dir("QuarryTest") {
            Ok(p) => p,
            Err(e) => panic!("path rejected: {e}"),
        };
        assert!(path.is_absolute());
        assert!(path.ends_with("QuarryTest"));
    }

    #[test]
    fn copy_bytes_writes_everything_and_reports_progress() {
        let dir = tmp("copy");
        let dst = dir.join("payload.bin");
        let data = vec![7u8; 3 * 1024 * 1024 + 17];
        let mut points = Vec::new();
        let res = copy_bytes(&dst, &data, 10, 90, |p| points.push(p));
        assert!(res.is_ok());
        let read = fs::read(&dst).unwrap_or_default();
        assert_eq!(read.len(), data.len());
        assert_eq!(read.first(), Some(&7));
        assert!(!points.is_empty());
        assert!(points.iter().all(|p| (10..=90).contains(p)));
        assert_eq!(points.last().copied(), Some(90));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shortcut_has_shell_link_header() {
        let dir = tmp("lnk");
        let target = dir.join("quarry.exe");
        assert!(fs::write(&target, b"MZ").is_ok());
        let lnk = dir.join("Quarry.lnk");
        let res = make_shortcut(&lnk, &target, &dir, &target);
        assert!(res.is_ok(), "shortcut failed");
        let bytes = fs::read(&lnk).unwrap_or_default();
        assert!(bytes.len() > 4);
        assert_eq!(&bytes[..4], &[0x4c, 0x00, 0x00, 0x00]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn uninstall_keeps_foreign_files() {
        let dir = tmp("uninst");
        for name in ["quarry.exe", "uninstall.exe", MARKER, "mine.txt"] {
            assert!(fs::write(dir.join(name), b"x").is_ok());
        }
        let res = remove_known_files(&dir);
        assert!(res.is_ok());
        assert!(!dir.join("quarry.exe").exists());
        assert!(!dir.join("uninstall.exe").exists());
        assert!(!dir.join(MARKER).exists());
        assert!(dir.join("mine.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_values_round_trip() {
        const TEST_KEY: &str =
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\QuarrySetupTest";
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = match hkcu.create_subkey(TEST_KEY) {
            Ok(v) => v,
            Err(e) => panic!("registry create failed: {e}"),
        };
        assert!(key.set_value("DisplayName", &"Quarry").is_ok());
        assert!(key.set_value("EstimatedSize", &123u32).is_ok());
        let name: String = key.get_value("DisplayName").unwrap_or_default();
        let size: u32 = key.get_value("EstimatedSize").unwrap_or(0);
        assert_eq!(name, "Quarry");
        assert_eq!(size, 123);
        drop(key);
        assert!(hkcu.delete_subkey_all(TEST_KEY).is_ok());
    }

    #[test]
    fn marker_round_trips() {
        let marker = Marker {
            version: "1.0.0".into(),
            installed_at: 42,
        };
        let raw = match serde_json::to_string(&marker) {
            Ok(v) => v,
            Err(e) => panic!("serialize failed: {e}"),
        };
        assert!(raw.contains("installedAt"));
        let back: Marker = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => panic!("deserialize failed: {e}"),
        };
        assert_eq!(back.version, "1.0.0");
        assert_eq!(back.installed_at, 42);
    }
}
