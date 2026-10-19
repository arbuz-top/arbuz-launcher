use crate::error::{err, Res};
use std::path::PathBuf;

/// Корень данных: `dirs::data_dir()/Arbuz Launcher` (Windows: `%APPDATA%\Arbuz Launcher`).
pub fn root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Arbuz Launcher")
}

pub fn config_file() -> PathBuf {
    root().join("config.json")
}

/// Первый запуск: файла config.json ещё нет — значит показываем визард знакомства.
pub fn first_run() -> bool {
    !config_file().exists()
}

pub fn accounts_file() -> PathBuf {
    root().join("accounts.json")
}

// runtimes/
pub fn runtimes_dir() -> PathBuf {
    root().join("runtimes")
}

pub fn java_dir(major: u8) -> PathBuf {
    runtimes_dir().join(format!("java{major}"))
}

pub fn authlib_jar() -> PathBuf {
    runtimes_dir().join("authlib-injector.jar")
}

/// Пользовательские обои главной.
pub fn wallpapers_dir() -> PathBuf {
    root().join("wallpapers")
}

pub fn installers_dir() -> PathBuf {
    runtimes_dir().join("installers")
}

pub fn tmp_dir() -> PathBuf {
    runtimes_dir().join("tmp")
}

// shared/
pub fn shared_dir() -> PathBuf {
    root().join("shared")
}

pub fn libraries_dir() -> PathBuf {
    shared_dir().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    shared_dir().join("assets")
}

pub fn assets_indexes_dir() -> PathBuf {
    assets_dir().join("indexes")
}

pub fn assets_objects_dir() -> PathBuf {
    assets_dir().join("objects")
}

pub fn versions_dir() -> PathBuf {
    shared_dir().join("versions")
}

pub fn version_dir(id: &str) -> PathBuf {
    versions_dir().join(id)
}

pub fn version_json(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.json"))
}

pub fn version_jar(id: &str) -> PathBuf {
    version_dir(id).join(format!("{id}.jar"))
}

pub fn launcher_profiles() -> PathBuf {
    shared_dir().join("launcher_profiles.json")
}

// instances/
pub fn instances_dir() -> PathBuf {
    root().join("instances")
}

/// Кэш распакованных из архивов иконок контента.
pub fn icons_cache_dir() -> PathBuf {
    root().join("cache").join("icons")
}

pub fn instance_dir(id: &str) -> PathBuf {
    instances_dir().join(id)
}

/// Проверяет, что `id` — идентификатор профиля, созданный лаунчером
/// (`Uuid::new_v4().simple()`: 32 строчных hex-символа). Не даёт `..` и
/// разделителям вывести путь за пределы `instances/`.
pub fn validate_instance_id(id: &str) -> Res<()> {
    let ok = id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if ok {
        Ok(())
    } else {
        err("Некорректный идентификатор профиля")
    }
}

pub fn instance_json(id: &str) -> PathBuf {
    instance_dir(id).join("instance.json")
}

pub fn instance_content_json(id: &str) -> PathBuf {
    instance_dir(id).join("content.json")
}

/// gameDir профиля.
pub fn instance_minecraft_dir(id: &str) -> PathBuf {
    instance_dir(id).join("minecraft")
}

pub fn instance_natives_dir(id: &str) -> PathBuf {
    instance_dir(id).join("natives")
}

/// Путь к файлу иконки профиля (имя файла из поля `icon`).
pub fn instance_icon(id: &str, file: &str) -> PathBuf {
    instance_dir(id).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_accepts_uuid_simple() {
        assert!(validate_instance_id("0123456789abcdef0123456789abcdef").is_ok());
    }

    #[test]
    fn instance_id_rejects_traversal_and_odd_input() {
        assert!(validate_instance_id("..").is_err());
        assert!(validate_instance_id("../..").is_err());
        assert!(validate_instance_id("0123456789abcdef0123456789abcdef/..").is_err());
        assert!(validate_instance_id("0123456789ABCDEF0123456789abcdef").is_err());
        assert!(validate_instance_id("0123456789abcdef").is_err());
        assert!(validate_instance_id("0123456789abcdef0123456789abcdeg").is_err());
        assert!(validate_instance_id("").is_err());
    }
}
