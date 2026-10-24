//! Синхронизация настроек между инстансами (Б1).
//!
//! Пользователь выбирает «главный» профиль и набор ключей options.txt
//! (+ опционально servers.dat). Перед запуском любого другого профиля
//! лаунчер подмешивает ТОЛЬКО выбранные ключи, причём только те, что целевая
//! версия игры знает (ключ уже присутствует в её options.txt) — в разных
//! версиях Minecraft наборы ключей отличаются. Файл целиком не копируется.

use crate::error::Res;
use crate::{paths, store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Настройки синхронизации (в config.json).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncSettings {
    /// id главного профиля.
    pub master: String,
    /// Выбранные ключи options.txt.
    pub keys: Vec<String>,
    /// Переносить servers.dat (список серверов).
    pub servers_dat: bool,
}

/// Пресеты наборов ключей: имя -> префиксы/точные ключи.
pub(crate) const PRESETS: &[(&str, &[&str])] = &[
    ("control", &["key_"]),
    ("sound", &["soundCategory_"]),
    (
        "graphics",
        &[
            "fov", "gamma", "renderDistance", "graphicsMode", "maxFps", "particles",
            "mipmapLevels", "biomeBlendRadius", "entityShadows", "smoothLighting",
            "entityDistanceScaling", "guiScale",
        ],
    ),
];

/// Разбирает options.txt в (ключ -> значение). Строки без ':' пропускаются.
pub(crate) fn parse_options(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// Собирает текст options.txt из карты (сортировка ключей устойчива —
/// как в BTreeMap).
pub(crate) fn render_options(map: &BTreeMap<String, String>) -> String {
    let mut s = String::new();
    for (k, v) in map {
        s.push_str(&format!("{k}:{v}\n"));
    }
    s
}

/// Подмешивает выбранные ключи из `master` в `target` (in-memory).
/// Подмешиваются только ключи, которые целевой файл уже знает: у другой
/// версии игры их может не быть, такие пропускаются. Возвращает фактически
/// применённые ключи.
pub(crate) fn merge_options(
    master: &BTreeMap<String, String>,
    target: &mut BTreeMap<String, String>,
    selected: &[String],
) -> Vec<String> {
    let mut applied = Vec::new();
    for key in selected {
        if let Some(v) = master.get(key) {
            if target.contains_key(key) && target.get(key) != Some(v) {
                target.insert(key.clone(), v.clone());
                applied.push(key.clone());
            } else if target.contains_key(key) {
                applied.push(key.clone());
            }
        }
    }
    applied
}

/// Ключи, попадающие в пресет.
pub(crate) fn preset_keys(preset: &str, keys: &[String]) -> Vec<String> {
    let Some((_, pats)) = PRESETS.iter().find(|(name, _)| *name == preset) else {
        return Vec::new();
    };
    keys.iter()
        .filter(|k| pats.iter().any(|p| k == p || k.starts_with(p)))
        .cloned()
        .collect()
}

fn options_path(id: &str) -> std::path::PathBuf {
    paths::instance_minecraft_dir(id).join("options.txt")
}

/// Ключи и значения из options.txt главного профиля (для списка в UI).
pub async fn master_keys(master: &str) -> Res<BTreeMap<String, String>> {
    crate::paths::validate_instance_id(master)?;
    match tokio::fs::read_to_string(options_path(master)).await {
        Ok(text) => Ok(parse_options(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

/// Применяет синхронизацию к профилю `id` перед запуском. Главный профиль
/// и профили без options.txt пропускаются тихо.
pub async fn apply_before_launch(id: &str, st: &SyncSettings) -> Res<()> {
    if st.master.is_empty() || st.keys.is_empty() && !st.servers_dat {
        return Ok(());
    }
    if id == st.master {
        return Ok(());
    }
    crate::paths::validate_instance_id(id)?;

    // options.txt: подмешиваем выбранные ключи.
    if !st.keys.is_empty() {
        let master = master_keys(&st.master).await.unwrap_or_default();
        if !master.is_empty() {
            let path = options_path(id);
            if let Ok(text) = tokio::fs::read_to_string(&path).await {
                let mut target = parse_options(&text);
                let applied = merge_options(&master, &mut target, &st.keys);
                if !applied.is_empty() {
                    let data = render_options(&target);
                    store::write_bytes_atomic(&path, data.as_bytes()).await?;
                }
            }
            // Нет options.txt у цели — игра ещё не запускалась: нечего
            // подмешивать, версии ключей неизвестны.
        }
    }

    // servers.dat: перенос списка серверов главного профиля (замена,
    // старый файл остаётся рядом как servers.dat.bak-sync).
    if st.servers_dat {
        let src = paths::instance_minecraft_dir(&st.master).join("servers.dat");
        let dst = paths::instance_minecraft_dir(id).join("servers.dat");
        if tokio::fs::metadata(&src).await.is_ok() {
            if tokio::fs::metadata(&dst).await.is_ok() {
                let bak = paths::instance_minecraft_dir(id).join("servers.dat.bak-sync");
                let _ = tokio::fs::copy(&dst, &bak).await;
            }
            let bytes = tokio::fs::read(&src).await?;
            store::write_bytes_atomic(&dst, &bytes).await?;
        }
    }
    Ok(())
}

/// Читает servers.dat и возвращает список серверов (имя + адрес).
/// Нужно для «быстрого входа» и каталога; здесь же один парсер.
pub(crate) fn read_servers_dat(path: &Path) -> Vec<(String, String)> {
    let Ok(raw) = std::fs::read(path) else {
        return Vec::new();
    };
    #[derive(Deserialize)]
    struct Root {
        #[serde(rename = "servers", default)]
        servers: Vec<Server>,
    }
    #[derive(Deserialize)]
    struct Server {
        #[serde(rename = "name", default)]
        name: String,
        #[serde(rename = "ip", default)]
        ip: String,
    }
    let root: Root = fastnbt::from_bytes(&raw).unwrap_or(Root { servers: Vec::new() });
    root.servers
        .into_iter()
        .map(|s| (s.name, s.ip))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn parse_options_skips_comments_and_garbage() {
        let text = "# комментарий\nfov:70.0\nkey_key.forward:key.keyboard.w\n\nбез двоеточия\n";
        let m = parse_options(text);
        assert_eq!(m.get("fov").map(String::as_str), Some("70.0"));
        assert_eq!(m.get("key_key.forward").map(String::as_str), Some("key.keyboard.w"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn render_round_trips() {
        let m = opts(&[("a", "1"), ("b", "2")]);
        let text = render_options(&m);
        assert_eq!(parse_options(&text), m);
    }

    #[test]
    fn merge_only_known_keys_and_changed_values() {
        let master = opts(&[
            ("fov", "90.0"),
            ("gamma", "1.0"),
            ("soundCategory_music", "0.2"),
        ]);
        // Целевая версия знает fov и gamma, не знает soundCategory_music.
        let mut target = opts(&[("fov", "70.0"), ("gamma", "1.0"), ("renderDistance", "12")]);
        let applied = merge_options(
            &master,
            &mut target,
            &["fov".into(), "gamma".into(), "soundCategory_music".into()],
        );
        assert_eq!(target.get("fov").map(String::as_str), Some("90.0"));
        assert_eq!(target.get("gamma").map(String::as_str), Some("1.0"));
        assert!(!target.contains_key("soundCategory_music"));
        assert_eq!(applied, vec!["fov".to_string(), "gamma".to_string()]);
        // renderDistance не тронут.
        assert_eq!(target.get("renderDistance").map(String::as_str), Some("12"));
    }

    #[test]
    fn presets_pick_matching_keys() {
        let keys: Vec<String> = vec![
            "key_key.forward".into(),
            "key_key.jump".into(),
            "soundCategory_music".into(),
            "fov".into(),
            "renderDistance".into(),
            "lang".into(),
        ]
        .into_iter()
        .map(|s: &str| s.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
        let control = preset_keys("control", &keys);
        assert_eq!(control.len(), 2);
        let sound = preset_keys("sound", &keys);
        assert_eq!(sound, vec!["soundCategory_music".to_string()]);
        let gfx = preset_keys("graphics", &keys);
        assert!(gfx.contains(&"fov".to_string()));
        assert!(gfx.contains(&"renderDistance".to_string()));
        assert!(!gfx.contains(&"lang".to_string()));
        assert!(preset_keys("nope", &keys).is_empty());
    }

    /// servers.dat (несжатый NBT): проверяем парсер на файле, собранном
    /// вручную байт за байтом.
    #[test]
    fn servers_dat_parser_reads_list() {
        fn tag_string(out: &mut Vec<u8>, name: &str, value: &str) {
            out.push(8); // TAG_String
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name.as_bytes());
            let v = value.as_bytes();
            out.extend_from_slice(&(v.len() as u16).to_be_bytes());
            out.extend_from_slice(v);
        }
        let mut nbt: Vec<u8> = Vec::new();
        nbt.extend_from_slice(&[10, 0, 0]); // TAG_Compound, имя ""
        nbt.push(9); // TAG_List
        nbt.extend_from_slice(&(7u16).to_be_bytes());
        nbt.extend_from_slice(b"servers");
        nbt.push(10); // тип элемента — TAG_Compound
        nbt.extend_from_slice(&2u32.to_be_bytes()); // длина списка
        for (name, ip) in [("Сервер 1", "play.example.net:25565"), ("Test", "mc2.example.org")] {
            // Элементы списка — только полезная нагрузка, без тега и имени.
            tag_string(&mut nbt, "name", name);
            tag_string(&mut nbt, "ip", ip);
            nbt.push(0); // TAG_End элемента
        }
        nbt.push(0); // TAG_End корня

        let dir = std::env::temp_dir().join(format!("arbuz-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("servers.dat");
        std::fs::write(&path, &nbt).unwrap();
        let servers = read_servers_dat(&path);
        assert_eq!(servers.len(), 2, "{servers:?}");
        assert_eq!(servers[0].0, "Сервер 1");
        assert_eq!(servers[1].1, "mc2.example.org");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
