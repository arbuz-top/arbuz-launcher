use crate::error::{err, AppError, Res};
use crate::models::{default_java, Account, Config, Instance, Loader};
use crate::state::AppState;
use crate::{paths, store};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

const BIOMES: [&str; 5] = ["plains", "taiga", "desert", "nether", "snow"];

/// Маска, которой ключ CurseForge подменяется при выдаче в webview.
const CF_KEY_MASK: &str = "************";

/// Читает config.json и приводит поля к безопасным значениям.
async fn read_config() -> Res<Config> {
    // Нет config.json — первый запуск: показываем визард знакомства.
    // (У существующего файла поля `onboarded` может не быть, тогда serde
    // возьмёт значение из `Config::default()` — true, и визард не всплывёт.)
    let fresh = paths::first_run();
    let mut cfg: Config = store::read_json(&paths::config_file()).await?;
    if fresh {
        cfg.onboarded = false;
    }
    cfg.sanitize();
    Ok(cfg)
}

/// Проверяет путь к Java, пришедший из IPC: абсолютный, без `..`, обычный файл
/// с именем `java`/`java.exe`/`javaw.exe`. Не даёт подсунуть произвольный exe.
async fn validate_java_path(raw: &str) -> Res<()> {
    let p = raw.trim();
    if p.is_empty() {
        return Ok(());
    }
    if p.contains("..") {
        return err("Некорректный путь к Java");
    }
    let path = std::path::Path::new(p);
    if !path.is_absolute() {
        return err("Путь к Java должен быть абсолютным");
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(name.as_str(), "java" | "java.exe" | "javaw.exe") {
        return err("Укажите путь к java или java.exe");
    }
    match tokio::fs::metadata(path).await {
        Ok(m) if m.is_file() => Ok(()),
        _ => err("Файл Java не найден"),
    }
}

/// Читает все `instances/*/instance.json`, сортировка по `created`.
async fn load_instances() -> Res<Vec<Instance>> {
    let mut out: Vec<Instance> = Vec::new();
    let mut rd = match tokio::fs::read_dir(paths::instances_dir()).await {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    while let Some(entry) = rd.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let path = entry.path().join("instance.json");
        if tokio::fs::metadata(&path).await.is_err() {
            continue;
        }
        if let Ok(raw) = tokio::fs::read_to_string(&path).await {
            if let Ok(inst) = serde_json::from_str::<Instance>(&raw) {
                out.push(inst);
            }
        }
    }
    out.sort_by_key(|i| i.created);
    Ok(out)
}

/// Последний release из манифеста Mojang.
async fn latest_release(http: &reqwest::Client) -> Res<String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        versions: Vec<ManifestVersion>,
    }
    #[derive(serde::Deserialize)]
    struct ManifestVersion {
        id: String,
        #[serde(rename = "type")]
        kind: String,
    }

    let manifest: Manifest = http
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    match manifest.versions.into_iter().find(|v| v.kind == "release") {
        Some(v) => Ok(v.id),
        None => err("Не удалось определить последнюю версию Minecraft"),
    }
}

// ---------------------------------------------------------------------------
// ЭТАП 1
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Res<Config> {
    // Читаем под тем же мьютексом, под которым пишет set_config, иначе
    // параллельный вызов может вернуть в память устаревший снимок.
    let mut guard = state.config.lock().await;
    let cfg = read_config().await?;
    *guard = cfg.clone();
    // Ключ CurseForge в webview не отдаём — только признак «задан».
    let mut shown = cfg;
    if !shown.curseforge_api_key.is_empty() {
        shown.curseforge_api_key = CF_KEY_MASK.to_string();
    }
    Ok(shown)
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, mut cfg: Config) -> Res<()> {
    // Значения из webview приводим к безопасным диапазонам до записи на диск.
    cfg.sanitize();
    let mut guard = state.config.lock().await;
    // Пришла маска — значит ключ не меняли, оставляем сохранённый.
    if cfg.curseforge_api_key == CF_KEY_MASK {
        cfg.curseforge_api_key = if guard.curseforge_api_key.is_empty() {
            read_config().await?.curseforge_api_key
        } else {
            guard.curseforge_api_key.clone()
        };
    }
    store::write_json_atomic(&paths::config_file(), &cfg).await?;
    *guard = cfg;
    Ok(())
}

/// Экспорт настроек в JSON. Ключ CurseForge в файл не попадает.
#[tauri::command]
pub async fn export_settings(state: State<'_, AppState>, path: String) -> Res<()> {
    let mut cfg = state.config.lock().await.clone();
    cfg.curseforge_api_key = CF_KEY_MASK.to_string();
    let p = settings_path(&path)?;
    let data = serde_json::to_vec_pretty(&cfg)?;
    tokio::fs::write(&p, data).await?;
    Ok(())
}

/// Импорт настроек из JSON. Возвращает применённый конфиг.
#[tauri::command]
pub async fn import_settings(state: State<'_, AppState>, path: String) -> Res<Config> {
    let p = settings_path(&path)?;
    let meta = tokio::fs::metadata(&p).await?;
    if meta.len() > 1024 * 1024 {
        return err("Файл настроек слишком большой");
    }
    let text = tokio::fs::read_to_string(&p).await?;
    let mut cfg: Config = serde_json::from_str(&text)
        .map_err(|e| AppError::Msg(format!("Это не файл настроек Arbuz Launcher: {e}")))?;
    cfg.sanitize();
    // Маска вместо ключа — оставляем текущий ключ.
    if cfg.curseforge_api_key == CF_KEY_MASK {
        cfg.curseforge_api_key = state.config.lock().await.curseforge_api_key.clone();
    }
    // Визард знакомства после импорта не нужен.
    cfg.onboarded = true;
    store::write_json_atomic(&paths::config_file(), &cfg).await?;
    *state.config.lock().await = cfg.clone();
    Ok(cfg)
}

/// Путь к файлу настроек: абсолютный, без `..`, с расширением `.json`.
fn settings_path(raw: &str) -> Res<std::path::PathBuf> {
    let p = raw.trim();
    if p.is_empty() {
        return err("Не указан файл настроек");
    }
    if p.contains("..") {
        return err("Некорректный путь к файлу настроек");
    }
    let path = std::path::PathBuf::from(p);
    if !path.is_absolute() {
        return err("Укажите полный путь к файлу настроек");
    }
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        return err("Файл настроек должен быть .json");
    }
    Ok(path)
}

#[tauri::command]
pub async fn list_instances() -> Res<Vec<Instance>> {
    load_instances().await
}#[tauri::command]
pub async fn create_instance(
    state: State<'_, AppState>,
    name: String,
    mc_version: Option<String>,
    loader: Loader,
) -> Res<Instance> {
    let mc = match mc_version {
        Some(v) if !v.trim().is_empty() => v,
        _ => latest_release(&state.http).await?,
    };

    let count = load_instances().await?.len();
    let biome = BIOMES[count % BIOMES.len()].to_string();
    let id = Uuid::new_v4().simple().to_string();
    let created = chrono::Utc::now().timestamp();

    let instance = Instance {
        id: id.clone(),
        name,
        mc_version: mc.clone(),
        loader,
        loader_version: None,
        biome,
        icon: None,
        icon_preset: None,
        wallpaper: None,
        ram_min_mb: 1024,
        ram_max_mb: 4096,
        java_major: default_java(&mc),
        java_path: None,
        jvm_args: String::new(),
        width: 1280,
        height: 720,
        fullscreen: false,
        playtime_sec: 0,
        playtime_days: std::collections::BTreeMap::new(),
        last_played: None,
        scanner_mode: None,
        created,
        pack: None,
        game_dir: None,
    };

    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    let game_dir = paths::instance_minecraft_dir(&id);
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(game_dir.join(sub)).await?;
    }

    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

/// Имя файла иконки профиля: одна безопасная компонента пути.
fn valid_icon_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\'])
        && crate::content::safe_rel(name).is_some()
}

#[tauri::command]
pub async fn update_instance(instance: Instance) -> Res<()> {
    paths::validate_instance_id(&instance.id)?;
    validate_java_path(instance.java_path.as_deref().unwrap_or("")).await?;
    if instance.jvm_args.contains('\0') {
        return err("Недопустимые JVM-аргументы");
    }
    if let Some(icon) = &instance.icon {
        if !valid_icon_name(icon) {
            return err("Некорректное имя иконки профиля");
        }
    }
    store::write_json_atomic(&paths::instance_json(&instance.id), &instance).await
}

/// Привязывает папку игры к профилю: junction на Windows (без прав админа,
/// работает между дисками), symlink на macOS/Linux.
async fn link_game_dir(link: &std::path::Path, target: &std::path::Path) -> Res<()> {
    if let Some(parent) = link.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(["/C", "mklink", "/J"]).arg(link).arg(target);
        crate::winproc::hide_console(&mut cmd);
        let out = cmd.output().await?;
        if !out.status.success() {
            let e = String::from_utf8_lossy(&out.stderr);
            return err(format!("Не удалось привязать папку игры: {}", e.trim()));
        }
    }
    #[cfg(not(windows))]
    {
        tokio::fs::symlink(target, link).await?;
    }
    Ok(())
}

/// Снимает ссылку, не трогая саму папку с игрой.
async fn unlink_game_dir(link: &std::path::Path) -> Res<()> {
    let meta = match tokio::fs::symlink_metadata(link).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    if meta.file_type().is_symlink() {
        tokio::fs::remove_file(link).await?;
    } else {
        #[cfg(windows)]
        tokio::fs::remove_dir(link).await?;
        #[cfg(not(windows))]
        tokio::fs::remove_dir_all(link).await?;
    }
    Ok(())
}

/// Переносит содержимое старой папки игры в новую (только если та пустая).
fn move_children(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if dst.exists() {
            continue;
        }
        let src = e.path();
        if std::fs::rename(&src, &dst).is_err() {
            copy_recursive(&src, &dst)?;
            if src.is_dir() {
                std::fs::remove_dir_all(&src)?;
            } else {
                std::fs::remove_file(&src)?;
            }
        }
    }
    Ok(())
}

fn copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

/// Пустая папка игры профиля (на случай сброса или ручного удаления ссылки).
async fn ensure_profile_game_dir(id: &str) -> Res<()> {
    let dir = paths::instance_minecraft_dir(id);
    tokio::fs::create_dir_all(&dir).await?;
    for sub in ["mods", "config", "saves", "resourcepacks", "shaderpacks", "logs"] {
        tokio::fs::create_dir_all(dir.join(sub)).await?;
    }
    Ok(())
}

/// Своя папка игры: `path = None` — вернуть папку профиля.
#[tauri::command]
pub async fn set_instance_game_dir(id: String, path: Option<String>) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    let mut inst = store::read_instance(&id).await?;
    let link = paths::instance_minecraft_dir(&id);
    let custom = path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());

    let reset = match custom.as_deref() {
        None => true,
        Some(raw) => {
            if raw.contains("..") {
                return err("Некорректный путь к папке игры");
            }
            let p = std::path::PathBuf::from(raw);
            if !p.is_absolute() {
                return err("Укажите полный путь к папке игры");
            }
            let same = std::fs::canonicalize(&p).ok() == std::fs::canonicalize(&link).ok();
            same
        }
    };

    if reset {
        if inst.game_dir.is_some() {
            unlink_game_dir(&link).await?;
            ensure_profile_game_dir(&id).await?;
            inst.game_dir = None;
        } else {
            ensure_profile_game_dir(&id).await?;
        }
    } else {
        let raw = custom.unwrap_or_default();
        let target = std::path::PathBuf::from(&raw);
        tokio::fs::create_dir_all(&target).await?;
        // Внутрь профиля ссылку не кладём: получится рекурсия.
        let inst_dir = paths::instance_dir(&id);
        let canon_target = std::fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
        let canon_inst = std::fs::canonicalize(&inst_dir).unwrap_or_else(|_| inst_dir.clone());
        if canon_target.starts_with(&canon_inst) {
            let _ = ensure_profile_game_dir(&id).await;
            return err("Выберите папку вне профиля лаунчера");
        }

        if inst.game_dir.is_some() {
            unlink_game_dir(&link).await?;
        } else if tokio::fs::metadata(&link).await.map(|m| m.is_dir()).unwrap_or(false) {
            // Первый раз: если новая папка пустая, переносим туда моды и миры.
            let empty = std::fs::read_dir(&target)?.next().is_none();
            if empty {
                let (from, to) = (link.clone(), target.clone());
                tokio::task::spawn_blocking(move || move_children(&from, &to)).await??;
            }
            if tokio::fs::remove_dir(&link).await.is_err() {
                // В папке профиля что-то осталось — не удаляем, а отодвигаем в сторону.
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                let backup = paths::instance_dir(&id).join(format!("minecraft.old-{stamp}"));
                tokio::fs::rename(&link, &backup).await?;
            }
        }

        if let Err(e) = link_game_dir(&link, &target).await {
            // Профиль не должен остаться без папки игры.
            ensure_profile_game_dir(&id).await?;
            inst.game_dir = None;
            let _ = store::write_instance(&inst).await;
            return Err(e);
        }
        inst.game_dir = Some(target.to_string_lossy().to_string());
    }

    store::write_instance(&inst).await?;
    Ok(inst)
}

#[tauri::command]
pub async fn delete_instance(id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_dir(&id);
    // Если папка игры вынесена — снимаем ссылку, иначе remove_dir_all удалит
    // данные игрока вместе с профилем.
    if let Ok(inst) = store::read_instance(&id).await {
        if inst.game_dir.is_some() {
            let _ = unlink_game_dir(&paths::instance_minecraft_dir(&id)).await;
        }
    }
    if tokio::fs::metadata(&dir).await.is_ok() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_instance_icon(id: String, source_path: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    let src = std::path::PathBuf::from(&source_path);
    if source_path.contains("..") || !src.is_absolute() {
        return err("Некорректный путь к иконке");
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "ico") {
        return err("Поддерживаются только PNG, JPG и ICO");
    }
    match tokio::fs::metadata(&src).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл иконки не найден"),
    }

    let mut instance = store::read_instance(&id).await?;

    if let Some(old) = instance.icon.clone() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            if tokio::fs::metadata(&old_path).await.is_ok() {
                let _ = tokio::fs::remove_file(&old_path).await;
            }
        }
    }

    let file = format!("icon.{ext}");
    let dst = paths::instance_icon(&id, &file);
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    tokio::fs::copy(&src, &dst).await?;

    instance.icon = Some(file);
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

#[tauri::command]
pub async fn set_instance_icon_url(state: State<'_, AppState>, id: String, url: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return err("Некорректный адрес иконки");
    }
    let resp = state.http.get(&url).send().await?;
    let bytes = resp.error_for_status()?.bytes().await?;
    if bytes.is_empty() || bytes.len() > 4 * 1024 * 1024 {
        return err("Иконка слишком большая");
    }
    let mut instance = store::read_instance(&id).await?;
    if let Some(old) = instance.icon.clone() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            let _ = tokio::fs::remove_file(&old_path).await;
        }
    }
    let file = "icon.png".to_string();
    let dst = paths::instance_icon(&id, &file);
    tokio::fs::create_dir_all(paths::instance_dir(&id)).await?;
    tokio::fs::write(&dst, &bytes).await?;
    instance.icon = Some(file);
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

#[tauri::command]
pub async fn clear_instance_icon(id: String) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    let mut instance = store::read_instance(&id).await?;
    if let Some(old) = instance.icon.take() {
        if valid_icon_name(&old) {
            let old_path = paths::instance_icon(&id, &old);
            if tokio::fs::metadata(&old_path).await.is_ok() {
                tokio::fs::remove_file(&old_path).await?;
            }
        }
    }
    store::write_json_atomic(&paths::instance_json(&id), &instance).await?;
    Ok(instance)
}

#[tauri::command]
pub async fn open_instance_folder(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn get_system_ram_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

// ---------------------------------------------------------------------------
// ЭТАП 2
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_mc_versions(
    state: State<'_, AppState>,
) -> Res<Vec<crate::mojang::versions::McVersion>> {
    crate::mojang::versions::list(&state.http).await
}

/// Добавляет пользовательскую версию (свой version.json и/или jar).
#[tauri::command]
pub async fn add_custom_version(
    id: String,
    base: String,
    json_path: Option<String>,
    jar_path: Option<String>,
) -> Res<String> {
    crate::mojang::versions::add_custom(&id, &base, json_path.as_deref(), jar_path.as_deref()).await
}

#[tauri::command]
pub async fn remove_custom_version(id: String) -> Res<()> {
    crate::mojang::versions::remove_custom(&id).await
}

#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: Loader,
    mc_version: String,
) -> Res<Vec<String>> {
    let list = crate::loaders::versions(&state.http, loader, &mc_version).await?;
    Ok(list.into_iter().map(|v| v.version).collect())
}

/// Подробности проекта каталога для страницы мода.
#[tauri::command]
pub async fn catalog_project(
    state: State<'_, AppState>,
    source: String,
    id: String,
) -> Res<crate::catalog::ProjectDetails> {
    crate::catalog::project(&state, &source, &id).await
}

/// Состояние сессии аккаунта: "ok", "expired" или "offline".
/// Для Microsoft/Ely.by пробуем получить токен — если обновление не прошло,
/// сессия действительно истекла и нужен повторный вход.
#[tauri::command]
pub async fn account_status(state: State<'_, AppState>, id: String) -> Res<String> {
    let acc = crate::accounts::list()
        .await?
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::Msg("Аккаунт не найден".into()))?;
    let ok = match acc.kind {
        crate::models::AccountKind::Offline => return Ok("offline".into()),
        crate::models::AccountKind::Microsoft => {
            crate::accounts::microsoft::token(&state.http, &acc).await.is_ok()
        }
        crate::models::AccountKind::Elyby => {
            crate::accounts::elyby::token(&state.http, &acc).await.is_ok()
        }
        crate::models::AccountKind::Littleskin => {
            crate::accounts::littleskin::token(&state.http, &acc).await.is_ok()
        }
    };
    Ok(if ok { "ok".into() } else { "expired".into() })
}

#[tauri::command]
pub async fn detect_javas() -> Res<Vec<crate::java::JavaInstall>> {
    crate::java::detect_javas().await
}

/// Скачивает JRE нужной версии с Adoptium прямо из лаунчера.
#[tauri::command]
pub async fn install_java(
    app: AppHandle,
    state: State<'_, AppState>,
    major: u8,
) -> Res<String> {
    let path = crate::java::ensure_java(&app, &state.http, "java", major, 0.0, 100.0).await?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_accounts() -> Res<Vec<Account>> {
    crate::accounts::list().await
}

#[tauri::command]
pub async fn offline_add(state: State<'_, AppState>, name: String) -> Res<Account> {
    let acc = crate::accounts::offline::add(&name).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(acc)
}

#[tauri::command]
pub async fn set_active_account(state: State<'_, AppState>, id: String) -> Res<()> {
    crate::accounts::set_active(&id).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(())
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Res<()> {
    crate::accounts::remove(&id).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(())
}

#[tauri::command]
pub async fn little_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Res<Account> {
    let acc = crate::accounts::littleskin::login(&state.http, login.trim(), &password).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(acc)
}

#[tauri::command]
pub async fn ely_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Res<Account> {
    let acc = crate::accounts::elyby::login(&state.http, login.trim(), &password).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(acc)
}

#[tauri::command]
pub async fn ms_begin(state: State<'_, AppState>) -> Res<crate::accounts::microsoft::DeviceCode> {
    crate::accounts::microsoft::begin(&state).await
}

#[tauri::command]
pub async fn ms_poll(
    app: AppHandle,
    state: State<'_, AppState>,
    device_code: String,
    interval: u64,
) -> Res<Account> {
    let acc = crate::accounts::microsoft::poll(&app, &state, &device_code, interval).await?;
    let cfg = read_config().await?;
    *state.config.lock().await = cfg;
    Ok(acc)
}

#[tauri::command]
pub async fn ms_cancel(state: State<'_, AppState>) -> Res<()> {
    crate::accounts::microsoft::cancel(&state);
    Ok(())
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    world: Option<String>,
    server: Option<String>,
    // Пользователь подтвердил «запустить всё равно» — пропустить гейт сканера.
    scan_override: Option<bool>,
) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::mojang::launch::launch(app, state.inner(), id, world, server, scan_override.unwrap_or(false)).await
}

#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let mut running = state.running.lock().await;
    if let Some(tx) = running.remove(&id) {
        let _ = tx.send(());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ЭТАП 6: контент и миры
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_content(
    state: State<'_, AppState>,
    id: String,
) -> Res<Vec<crate::content::ContentItem>> {
    paths::validate_instance_id(&id)?;
    let mut items = crate::content::list(&id).await?;
    crate::catalog::fill_icons(&state.http, &mut items).await;
    Ok(items)
}

#[tauri::command]
pub async fn toggle_content(id: String, file: String, enabled: bool) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::content::toggle(&id, &file, enabled).await
}

#[tauri::command]
pub async fn remove_content(id: String, file: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::content::remove(&id, &file).await
}

/// Помечает файл контента как изолируемый (или снимает пометку).
#[tauri::command]
pub async fn toggle_isolation(id: String, file: String, isolated: bool) -> Res<()> {
    paths::validate_instance_id(&id)?;
    crate::content::set_isolated(&id, &file, isolated).await
}

#[tauri::command]
pub async fn list_worlds(id: String) -> Res<Vec<crate::content::World>> {
    paths::validate_instance_id(&id)?;
    crate::content::worlds(&id).await
}

#[tauri::command]
pub async fn list_screenshots(id: String) -> Res<Vec<crate::content::Shot>> {
    paths::validate_instance_id(&id)?;
    crate::content::screenshots(&id).await
}

/// Открывает папку `screenshots` профиля (создаёт её при необходимости).
#[tauri::command]
pub async fn open_screenshots_folder(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("screenshots");
    tokio::fs::create_dir_all(&dir).await?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

/// Открывает один скриншот системным просмотрщиком.
#[tauri::command]
pub async fn open_screenshot(app: AppHandle, id: String, file: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    if file.is_empty() || file.contains(['/', '\\', ':']) || file.contains("..") {
        return err("Некорректное имя файла");
    }
    let path = paths::instance_minecraft_dir(&id).join("screenshots").join(&file);
    if tokio::fs::metadata(&path).await.is_err() {
        return err("Скриншот не найден");
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ЭТАП 7: каталог
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn search_catalog(
    state: State<'_, AppState>,
    args: crate::catalog::SearchArgs,
) -> Res<crate::catalog::SearchResult> {
    crate::catalog::search(&state, args).await
}

#[tauri::command]
pub async fn install_catalog_item(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source: String,
    project_id: String,
    kind: String,
    world: Option<String>,
) -> Res<()> {
    paths::validate_instance_id(&instance_id)?;
    crate::catalog::install(&app, &state, &instance_id, &source, &project_id, &kind, world).await
}

#[tauri::command]
pub async fn check_updates(state: State<'_, AppState>, id: String) -> Res<Vec<crate::catalog::UpdateInfo>> {
    paths::validate_instance_id(&id)?;
    crate::catalog::check_updates(&state, &id).await
}

#[tauri::command]
pub async fn apply_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    files: Vec<String>,
) -> Res<u32> {
    paths::validate_instance_id(&id)?;
    crate::catalog::apply_updates(&app, &state, &id, files).await
}

// ---------------------------------------------------------------------------
// ЭТАП 8: сборки
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    version_id: Option<String>,
) -> Res<Instance> {
    crate::packs::install_modpack(&app, &state, &source, &project_id, version_id).await
}

#[tauri::command]
pub async fn list_modpack_versions(
    state: State<'_, AppState>,
    source: String,
    project_id: String,
) -> Res<Vec<crate::packs::PackVersion>> {
    crate::packs::list_versions(&state, &source, &project_id).await
}

/// Есть ли для профиля более новая версия сборки.
#[tauri::command]
pub async fn pack_update(
    state: State<'_, AppState>,
    id: String,
) -> Res<Option<crate::packs::PackUpdate>> {
    paths::validate_instance_id(&id)?;
    crate::packs::check_pack(&state, &id).await
}

/// Ставит новую версию сборки поверх профиля.
#[tauri::command]
pub async fn apply_pack_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version_id: String,
) -> Res<Instance> {
    paths::validate_instance_id(&id)?;
    if version_id.trim().is_empty() {
        return err("Не выбрана версия сборки");
    }
    crate::packs::apply_update(&app, &state, &id, &version_id).await
}

#[tauri::command]
pub async fn import_pack(app: AppHandle, state: State<'_, AppState>, path: String) -> Res<Instance> {
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mrpack" | "zip") {
        return err("Поддерживаются файлы .mrpack и .zip");
    }
    match tokio::fs::metadata(p).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл сборки не найден"),
    }
    crate::packs::import_pack(&app, &state, &path).await
}

/// Импорт инстанса MultiMC / Prism Launcher.
#[tauri::command]
pub async fn import_prism_instance(path: String) -> Res<Instance> {
    crate::prism::import(&path).await
}

/// Список профилей Modrinth App, доступных для импорта.
#[tauri::command]
pub async fn list_modrinth_profiles() -> Res<Vec<crate::modrinth_app::MrProfile>> {
    crate::modrinth_app::list()
}

/// Импорт профиля из Modrinth App.
#[tauri::command]
pub async fn import_modrinth_profile(path: String) -> Res<Instance> {
    crate::modrinth_app::import(&path).await
}

/// Разбор последнего падения профиля: причина и виновный мод.
#[tauri::command]
pub async fn diagnose_crash(id: String) -> Res<crate::crash::Diagnosis> {
    crate::crash::diagnose(&id).await
}

/// ИИ-разбор падения через free-модели OpenCode Zen.
///
/// `lang` — язык интерфейса, на нём модель пишет ответ; `console` — хвост
/// консоли лаунчера (stdout/stderr), который может отличаться от `latest.log`.
#[tauri::command]
pub async fn ai_diagnose_crash(
    id: String,
    lang: String,
    console: String,
) -> Res<crate::ai::AiDiagnosis> {
    crate::ai::diagnose(&id, &lang, &console).await
}

/// ИИ-ассистент сборок: по промпту составляет план и подбирает проекты каталога.
///
/// Скачивание и установка выполняются обычными командами каталога —
/// здесь только подбор состава под версию, загрузчик и язык интерфейса.
/// `limit` — предел числа модов (0 или None — без ограничения),
/// `source` — приоритетный каталог (пусто — сразу Modrinth и CurseForge),
/// `model` — выбранная модель (пусто — первая доступная бесплатная).
#[tauri::command]
pub async fn ai_pack_plan(
    state: State<'_, AppState>,
    prompt: String,
    lang: String,
    mc_version: Option<String>,
    loader: Option<String>,
    limit: Option<u32>,
    source: Option<String>,
    model: Option<String>,
) -> Res<crate::builder::PackPlan> {
    if !state.config.lock().await.ai_builder {
        return err("ИИ-ассистент сборок выключен в настройках");
    }
    crate::builder::plan(
        &state, &prompt, &lang, mc_version, loader, limit, source, model,
    )
    .await
}

/// Бесплатные модели, которыми может собирать ИИ-ассистент.
#[tauri::command]
pub async fn ai_models() -> Res<Vec<String>> {
    crate::ai::models().await
}

/// Открывает папку `crash-reports` профиля (создаёт её при необходимости).
#[tauri::command]
pub async fn open_crash_reports(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("crash-reports");
    tokio::fs::create_dir_all(&dir).await?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Редактор сборки: манифест, конфиги, проверка, экспорт
// ---------------------------------------------------------------------------

/// Манифест и статистика сборки.
#[tauri::command]
pub async fn pack_info(id: String) -> Res<crate::studio::PackInfo> {
    crate::studio::info(&id).await
}

/// Сохраняет имя, версию, автора и описание сборки.
#[tauri::command]
pub async fn save_pack_info(
    id: String,
    manifest: crate::studio::PackManifest,
) -> Res<crate::studio::PackInfo> {
    crate::studio::save(&id, manifest).await
}

/// Все файлы конфигурации профиля, с привязкой к модам.
#[tauri::command]
pub async fn pack_configs(id: String) -> Res<Vec<crate::studio::ConfigEntry>> {
    crate::studio::configs(&id).await
}

/// Читает конфиг для редактора.
#[tauri::command]
pub async fn read_pack_config(id: String, path: String) -> Res<crate::studio::ConfigDoc> {
    crate::studio::read_config(&id, &path).await
}

/// Сохраняет конфиг (старая версия уходит в `.bak`).
#[tauri::command]
pub async fn save_pack_config(
    id: String,
    path: String,
    content: String,
) -> Res<crate::studio::ConfigDoc> {
    crate::studio::save_config(&id, &path, &content).await
}

/// Откатывает конфиг из `.bak`.
#[tauri::command]
pub async fn restore_pack_config(id: String, path: String) -> Res<crate::studio::ConfigDoc> {
    crate::studio::restore_config(&id, &path).await
}

/// Проверка сборки: дубликаты, зависимости, Java.
#[tauri::command]
pub async fn validate_pack(id: String) -> Res<Vec<crate::studio::PackIssue>> {
    crate::studio::validate(&id).await
}

/// Экспорт сборки в `.mrpack` или `.zip`.
#[tauri::command]
pub async fn export_pack(
    id: String,
    dest: String,
    format: String,
) -> Res<crate::studio::ExportResult> {
    crate::studio::export(&id, &dest, &format).await
}

/// Открывает папку `config` профиля.
#[tauri::command]
pub async fn open_pack_configs(app: AppHandle, id: String) -> Res<()> {
    paths::validate_instance_id(&id)?;
    let dir = paths::instance_minecraft_dir(&id).join("config");
    tokio::fs::create_dir_all(&dir).await?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn account_skin(state: State<'_, AppState>, id: String) -> Res<Option<crate::skins::Skin>> {
    crate::skins::account_skin(&state, &id).await
}

// ---------------------------------------------------------------------------
// Сканер модов
// ---------------------------------------------------------------------------

/// Сканирует контент профиля и возвращает отчёты по каждому файлу.
#[tauri::command]
pub async fn scan_mods(state: State<'_, AppState>, id: String) -> Res<Vec<crate::scanner::ModReport>> {
    crate::scanner::scan_instance(&state, &id).await
}

/// Ручное исключение файла из блокировки сканера (или его снятие).
#[tauri::command]
pub async fn scanner_allow_mod(id: String, file: String, allow: bool) -> Res<()> {
    crate::scanner::allow_mod(&id, &file, allow).await
}

/// Файлы в карантине профиля (параноидальный режим).
#[tauri::command]
pub async fn scanner_quarantine(id: String) -> Res<Vec<String>> {
    crate::scanner::quarantine_list(&id).await
}

/// Возвращает файл из карантина в mods/.
#[tauri::command]
pub async fn scanner_quarantine_restore(id: String, name: String) -> Res<()> {
    crate::scanner::quarantine_restore(&id, &name).await
}

/// ИИ-заметка по одному подозрительному моду (через free-модели Zen).
/// Только пояснение для пользователя: вердикт сканера не меняет.
#[tauri::command]
pub async fn ai_scan_note(
    state: State<'_, AppState>,
    id: String,
    file: String,
    lang: String,
) -> Res<crate::scanner::ai_note::AiNote> {
    crate::scanner::ai_note_for(&state, &id, &file, &lang).await
}

// ---------------------------------------------------------------------------
// Синхронизация настроек между профилями (Б1)
// ---------------------------------------------------------------------------

/// Ключи и значения options.txt главного профиля (для списка в UI).
/// Настройки синхронизации кладутся в config.json через set_config.
#[tauri::command]
pub async fn sync_master_options(state: State<'_, AppState>) -> Res<std::collections::BTreeMap<String, String>> {
    let master = state
        .config
        .lock()
        .await
        .sync
        .clone()
        .map(|s| s.master)
        .unwrap_or_default();
    if master.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }
    crate::sync::master_keys(&master).await
}

// ---------------------------------------------------------------------------
// Умная консоль (Б2)
// ---------------------------------------------------------------------------

/// Какому модулю принадлежит класс из стектрейса.
#[tauri::command]
pub async fn find_class_owner(
    id: String,
    class: String,
) -> Res<Option<crate::scanner::classindex::ClassOwner>> {
    crate::scanner::classindex::find_owner(&id, &class).await
}

/// Окончательно удаляет файл из карантина.
#[tauri::command]
pub async fn scanner_quarantine_delete(id: String, name: String) -> Res<()> {
    crate::scanner::quarantine_delete(&id, &name).await
}

// ---------------------------------------------------------------------------
// ЭТАП 9: новости
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_news(state: State<'_, AppState>) -> Res<Vec<crate::news::NewsItem>> {
    crate::news::get(&state.http).await
}

#[tauri::command]
pub fn data_dir() -> String {
    paths::root().to_string_lossy().to_string()
}

#[tauri::command]
pub async fn open_data_folder(app: AppHandle) -> Res<()> {
    let dir = paths::root();
    tokio::fs::create_dir_all(&dir).await?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(())
}

/// Копирует выбранную картинку в папку обоев и возвращает имя файла.
#[tauri::command]
pub async fn set_home_wallpaper(source_path: String) -> Res<String> {
    let src = std::path::Path::new(&source_path);
    if source_path.contains("..") || !src.is_absolute() {
        return err("Некорректный путь к обоям");
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return err("Поддерживаются PNG, JPG и WEBP");
    }
    match tokio::fs::metadata(src).await {
        Ok(m) if m.is_file() => {}
        _ => return err("Файл обоев не найден"),
    }

    let dir = paths::wallpapers_dir();
    tokio::fs::create_dir_all(&dir).await?;
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
    }
    let name = {
        // Уникальное имя: webview кэширует картинку по URL, поэтому при смене
        // обоев путь обязан меняться, иначе показывается предыдущая картинка.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("home-{stamp}.{ext}")
    };
    tokio::fs::copy(src, dir.join(&name)).await?;
    Ok(name)
}

#[tauri::command]
pub async fn clear_home_wallpaper() -> Res<()> {
    let dir = paths::wallpapers_dir();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                let _ = tokio::fs::remove_file(e.path()).await;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ЭТАП 10: игра по сети
// ---------------------------------------------------------------------------

/// Порт, который Minecraft открыл для сети (ищем в логе игры).
#[tauri::command]
pub async fn lan_port(id: String) -> Res<Option<u16>> {
    paths::validate_instance_id(&id)?;
    crate::net::lan_port(&id).await
}

/// Просит роутер пробросить порт — друзья смогут подключиться по внешнему IP.
#[tauri::command]
pub async fn net_share(port: u16) -> Res<crate::net::Share> {
    if port == 0 {
        return err("Неверный порт");
    }
    crate::net::share(port).await
}

/// Закрывает проброс порта.
#[tauri::command]
pub async fn net_unshare() -> Res<()> {
    crate::net::unshare().await
}

/// Открыт ли проброс прямо сейчас (например, после перезапуска окна).
#[tauri::command]
pub fn net_active() -> Option<crate::net::Share> {
    crate::net::active()
}

/// P2P: принимающий показывает код, друзьям проброс порта и белый IP не нужны.
#[tauri::command]
pub async fn p2p_host(port: u16, name: String) -> Res<crate::tunnel::P2p> {
    if port == 0 {
        return err("Неверный порт");
    }
    crate::tunnel::host(port, name).await
}

/// P2P: комната заранее — код можно отдать друзьям до запуска игры.
/// Порт лаунчер найдёт сам, когда в игре нажмут «Открыть для сети».
#[tauri::command]
pub async fn p2p_room(name: String) -> Res<crate::tunnel::P2p> {
    crate::tunnel::room(name).await
}

/// P2P: подключаемся к другу по коду и открываем локальный порт для Minecraft.
#[tauri::command]
pub async fn p2p_join(code: String, name: String) -> Res<crate::tunnel::P2p> {
    crate::tunnel::join(&code, name).await
}

/// P2P: закрыть канал.
#[tauri::command]
pub async fn p2p_stop() -> Res<()> {
    crate::tunnel::stop().await;
    Ok(())
}

/// P2P: открыт ли канал прямо сейчас.
#[tauri::command]
pub fn p2p_active() -> Option<crate::tunnel::P2p> {
    crate::tunnel::active()
}

/// P2P: кто сейчас в комнате и кто её владелец.
#[tauri::command]
pub fn p2p_room_state() -> Option<crate::tunnel::Room> {
    crate::tunnel::room_state()
}

/// P2P: выгнать игрока (хозяин — сразу, владелец-гость — через хозяина).
#[tauri::command]
pub async fn p2p_kick(slot: u64) -> Res<()> {
    crate::tunnel::kick(slot).await
}

/// P2P: передать права владельца другому игроку.
#[tauri::command]
pub async fn p2p_admin(slot: u64) -> Res<()> {
    crate::tunnel::make_admin(slot).await
}

/// P2P: новый код комнаты (старый перестаёт пускать).
#[tauri::command]
pub async fn p2p_new_code() -> Res<crate::tunnel::P2p> {
    crate::tunnel::new_code().await
}

/// P2P: одноразовое сообщение игроку («вас исключили», «связь потерялась»).
#[tauri::command]
pub fn p2p_notice() -> Option<String> {
    crate::tunnel::notice()
}

// ---------------------------------------------------------------------------
// Обновление лаунчера
// ---------------------------------------------------------------------------

/// Есть ли на GitHub более новый релиз лаунчера.
#[tauri::command]
pub async fn check_launcher_update(
    state: State<'_, AppState>,
) -> Res<Option<crate::updates::LauncherUpdate>> {
    crate::updates::check(&state).await
}
