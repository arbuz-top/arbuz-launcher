//! Разбор отчётов о падении Minecraft.
//!
//! Ищем свежий crash-report (или hs_err / latest.log), вытаскиваем исключение,
//! список модов и — если получается — виновника. Результат показывается в окне
//! «Игра упала»: причина, виновный мод и что делать.

use crate::error::Res;
use crate::paths;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

static RE_DESC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*Description:\s*(.+?)\s*$").unwrap());
static RE_THROW: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z_$][\w.$]*(?:Exception|Error|Throwable)\b").unwrap());
static RE_SUSPECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*\**Suspected Mods:?\**\s*(.+?)\s*$").unwrap());
static RE_PID: Lazy<Regex> = Lazy::new(|| Regex::new(r"^hs_err_pid\d+\.log$").unwrap());
/// Настоящие ошибки создания окна/контекста, а не любое упоминание OpenGL.
static RE_GPU: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(glfw error|failed to (create|initialize) (a )?(glfw )?window|pixel format not accelerated|no opengl context|failed to create (an )?opengl context|could not create (an )?opengl context|cannot create opengl context|wglcreatecontext|wglmakecurrent|unsupported opengl|does not support opengl|glxbad|egl_bad|glcreatecontext.*(failed|error)|opengl.*(context|window).*(failed|error)|the driver does not appear to support opengl)",
    )
    .unwrap()
});
/// Нативный фрейм драйвера видеокарты.
static RE_GPU_FRAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(nvoglv|atio6axx|atidxx|amdvlk|nvwgf2um|ig\d+icd|opengl32|libgl|libegl|glfw|vulkan|d3d1[12])")
        .unwrap()
});

/// Разбор падения: причина, виновник и подсказки.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    /// Нашли ли отчёт или лог вообще.
    pub found: bool,
    /// Источник: `crash-report` | `hs-err` | `latest-log` | `debug-log`.
    pub source: String,
    /// Имя файла, из которого взяли данные.
    pub file: Option<String>,
    /// Категория: `mod` | `dependency` | `mixin` | `memory` | `java` | `gpu` | `native` | `unknown`.
    pub kind: String,
    /// Короткий заголовок.
    pub title: String,
    /// Что именно произошло.
    pub summary: String,
    /// Мод-виновник (отображаемое имя).
    pub culprit: Option<String>,
    /// Файл мода-виновника.
    pub culprit_file: Option<String>,
    /// Все моды, попавшие под подозрение.
    pub suspects: Vec<String>,
    /// Что делать.
    pub hints: Vec<String>,
    /// Начало отчёта — для кнопки «показать лог».
    pub excerpt: Vec<String>,
    /// Ключ заголовка для `t()` (русская строка с `{...}`).
    pub title_key: Option<String>,
    /// Ключ описания для `t()`.
    pub summary_key: Option<String>,
    /// Ключи подсказок для `t()`.
    pub hint_keys: Vec<String>,
    /// Подстановки для ключей: `mod`, `frame`, `error`, `mods`.
    pub vars: BTreeMap<String, String>,
}

/// Строка списка модов из отчёта.
struct ModEntry {
    id: String,
    name: String,
    file: Option<String>,
}

impl ModEntry {
    /// Подпись для интерфейса: имя и, если отличается, id в скобках.
    fn label(&self) -> String {
        let n = self.name.trim();
        if n.is_empty() || n.eq_ignore_ascii_case(&self.id) {
            self.id.clone()
        } else if n.to_lowercase().starts_with(&self.id.to_lowercase()) {
            n.to_string()
        } else {
            format!("{n} ({})", self.id)
        }
    }
}

/// Диагностирует последнее падение профиля.
pub async fn diagnose(id: &str) -> Res<Diagnosis> {
    paths::validate_instance_id(id)?;
    let game = paths::instance_minecraft_dir(id);

    let Some((path, source)) = find_report(&game) else {
        let mut d = Diagnosis {
            found: false,
            kind: "unknown".into(),
            ..Default::default()
        };
        set_text(
            &mut d,
            "Отчёт не найден",
            "Лаунчер не нашёл ни crash-отчёта, ни логов: игра упала до их создания.",
            &[
                "Откройте папку игры и проверьте каталог crash-reports вручную.",
                "Запустите игру ещё раз — при новом падении отчёт появится здесь.",
            ],
            BTreeMap::new(),
        );
        return Ok(d);
    };
    let text = read_capped(&path, 2_000_000);

    let from_log = source.ends_with("log");
    let mut d = analyze(&text, from_log);
    d.found = true;
    d.source = source.to_string();
    d.file = path.file_name().map(|s| s.to_string_lossy().to_string());
    Ok(d)
}

/// Ближайший к игре отчёт или лог. Возвращает путь и метку источника.
fn find_report(game: &Path) -> Option<(PathBuf, &'static str)> {
    if let Some(p) = newest(&game.join("crash-reports"), Some("txt")) {
        return Some((p, "crash-report"));
    }
    if let Some(p) = newest(&game, None).filter(|p| is_hs_err(p)) {
        return Some((p, "hs-err"));
    }
    let latest = game.join("logs").join("latest.log");
    if latest.is_file() {
        return Some((latest, "latest-log"));
    }
    let debug = game.join("logs").join("debug.log");
    if debug.is_file() {
        return Some((debug, "debug-log"));
    }
    None
}

fn is_hs_err(p: &Path) -> bool {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(|s| RE_PID.is_match(s))
        .unwrap_or(false)
}

/// Самый свежий файл каталога (при желании — только с заданным расширением).
fn newest(dir: &Path, ext: Option<&str>) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        if let Some(ext) = ext {
            let ok = p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(ext))
                .unwrap_or(false);
            if !ok {
                continue;
            }
        }
        let mt = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().map(|(b, _)| mt > *b).unwrap_or(true) {
            best = Some((mt, p));
        }
    }
    best.map(|(_, p)| p)
}

/// Читает файл, обрезая по размеру и корректной границе UTF-8.
fn read_capped(path: &Path, max: usize) -> String {
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    let slice = if bytes.len() > max { &bytes[..max] } else { &bytes[..] };
    let end = match std::str::from_utf8(slice) {
        Ok(_) => slice.len(),
        Err(e) => e.valid_up_to(),
    };
    String::from_utf8_lossy(&slice[..end]).to_string()
}

/// Подставляет `{name}` в шаблон.
fn fill(tpl: &str, vars: &BTreeMap<String, String>) -> String {
    let mut s = tpl.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Заполняет «русский» текст (его видит ИИ) и ключи для локализации в интерфейсе.
fn set_text(
    d: &mut Diagnosis,
    title: &str,
    summary: &str,
    hints: &[&str],
    vars: BTreeMap<String, String>,
) {
    d.title = fill(title, &vars);
    d.summary = fill(summary, &vars);
    d.hints = hints.iter().map(|h| fill(h, &vars)).collect();
    d.title_key = Some(title.to_string());
    d.summary_key = Some(summary.to_string());
    d.hint_keys = hints.iter().map(|h| (*h).to_string()).collect();
    d.vars = vars;
}

/// Похоже ли, что latest.log закончился нормальным выходом, а не падением.
fn log_looks_clean(text: &str) -> bool {
    let mut lines: Vec<&str> = text.lines().rev().take(80).collect();
    lines.reverse();
    let tail = lines.join("\n").to_lowercase();
    let normal = tail.contains("stopping!")
        || tail.contains("exiting event polling thread")
        || tail.contains("all dimensions are saved");
    if !normal {
        return false;
    }
    let low = text.to_lowercase();
    let markers = [
        "exception in thread \"main\"",
        "uncaught exception",
        "a fatal error has been detected",
        "---- minecraft crash report ----",
        "crash report",
        "failed to start the minecraft",
    ];
    !markers.iter().any(|m| low.contains(m))
}

/// Разбирает текст отчёта/лога.
fn analyze(text: &str, from_log: bool) -> Diagnosis {
    let low = text.to_lowercase();
    let mods = parse_mods(text);
    let suspects = parse_suspected(text);
    let description = RE_DESC
        .captures(text)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let throws = collect_throws(text, from_log);
    let primary = throws.first().cloned().unwrap_or_default();
    let stack = stack_region(text);

    let mut d = Diagnosis {
        kind: "unknown".into(),
        title: "Игра упала".into(),
        summary: if description.is_empty() {
            primary.clone()
        } else {
            description.clone()
        },
        excerpt: head_excerpt(text),
        title_key: Some("Игра упала".into()),
        ..Default::default()
    };

    if !primary.is_empty() {
        d.summary = if description.is_empty() {
            primary.clone()
        } else {
            format!("{description} — {primary}")
        };
    }

    // 1. Память.
    if low.contains("outofmemoryerror")
        || low.contains("java heap space")
        || low.contains("gc overhead limit exceeded")
        || low.contains("insufficient memory")
    {
        d.kind = "memory".into();
        set_text(
            &mut d,
            "Не хватило оперативной памяти",
            "Игре не хватило выделенной памяти — JVM была убита с OutOfMemoryError.",
            &[
                "Откройте профиль → «Java и память» и поднимите верхний предел (например, 6144 МБ).",
                "Не ставьте больше ~70% всей оперативной памяти — станет только хуже.",
                "Проверьте, нет ли рядом тяжёлых сборок с сотнями модов.",
            ],
            BTreeMap::new(),
        );
        return d;
    }

    // 2. Падение самой JVM (нативный код, драйвер, мод с JNI).
    if low.contains("a fatal error has been detected by the java runtime")
        || low.contains("exception_access_violation")
        || low.contains("sigtrap")
        || low.contains("problematic frame")
    {
        let frame = problematic_frame(text);
        let gpu = frame
            .as_deref()
            .map(|f| RE_GPU_FRAME.is_match(f))
            .unwrap_or(false);
        let mut vars = BTreeMap::new();
        if let Some(f) = &frame {
            vars.insert("frame".to_string(), f.clone());
        }
        if gpu {
            d.kind = "gpu".into();
            set_text(
                &mut d,
                "Проблема с графикой",
                if frame.is_some() {
                    "JVM упала в драйвере видеокарты: {frame}"
                } else {
                    "JVM упала в нативном коде драйвера видеокарты."
                },
                &[
                    "Обновите драйвер видеокарты (NVIDIA/AMD/Intel).",
                    "Проверьте, что видеокарта не отключена и игра не запущена в режиме удалённого рабочего стола.",
                    "Уберите моды рендера/шейдеров, если падение началось после их установки.",
                ],
                vars,
            );
        } else {
            d.kind = "native".into();
            set_text(
                &mut d,
                "Авария Java (нативный код)",
                if frame.is_some() {
                    "JVM упала на нативном вызове: {frame}"
                } else {
                    "Процесс Java завершился аварийно (не обычное исключение)."
                },
                &[
                    "Чаще всего это драйвер видеокарты или мод с нативным кодом (шейдеры, рендер).",
                    "Обновите драйвер видеокарты.",
                    "Уберите недавно добавленные моды и запустите снова.",
                ],
                vars,
            );
        }
        d.culprit = frame;
        return d;
    }

    // 3. Неподходящая версия Java.
    if low.contains("unsupportedclassversionerror")
        || low.contains("class file version")
        || low.contains("requires java")
        || low.contains("unsupported class file major version")
    {
        d.kind = "java".into();
        let mut vars = BTreeMap::new();
        if primary.is_empty() {
            set_text(
                &mut d,
                "Неподходящая версия Java",
                "Мод или загрузчик собраны под другую версию Java. Игра не смогла загрузить классы.",
                &[
                    "Откройте профиль → «Java и память» и выберите версию: 1.16 и ниже — Java 8, 1.17–1.20.4 — Java 17, 1.20.5+ — Java 21.",
                    "Если Java нужной версии нет — скачайте её в настройках лаунчера.",
                ],
                vars,
            );
        } else {
            vars.insert("error".into(), primary.clone());
            set_text(
                &mut d,
                "Неподходящая версия Java",
                "Мод или загрузчик собраны под другую версию Java. {error}",
                &[
                    "Откройте профиль → «Java и память» и выберите версию: 1.16 и ниже — Java 8, 1.17–1.20.4 — Java 17, 1.20.5+ — Java 21.",
                    "Если Java нужной версии нет — скачайте её в настройках лаунчера.",
                ],
                vars,
            );
        }
        return d;
    }

    // 4. latest.log без падения: не выдумываем GPU из-за слова OpenGL.
    if from_log && log_looks_clean(text) {
        d.kind = "unknown".into();
        set_text(
            &mut d,
            "В логе нет падения",
            "latest.log заканчивается нормальным выходом игры, стека ошибки нет. Скорее всего, это лог прошлого запуска, а текущее падение не успело в него записаться.",
            &[
                "Посмотрите консоль лаунчера — ошибка могла остаться только в stderr.",
                "Запустите игру снова: при новом падении появится crash-report.",
            ],
            BTreeMap::new(),
        );
        return d;
    }

    // 5. Графика: только настоящие ошибки окна/контекста.
    if RE_GPU.is_match(text) {
        d.kind = "gpu".into();
        set_text(
            &mut d,
            "Проблема с графикой",
            "Игра не смогла создать окно/контекст OpenGL.",
            &[
                "Обновите драйвер видеокарты (NVIDIA/AMD/Intel).",
                "Проверьте, что видеокарта не отключена и игра не запущена в режиме удалённого рабочего стола.",
                "Уберите моды рендера/шейдеров, если падение началось после их установки.",
            ],
            BTreeMap::new(),
        );
        return d;
    }

    // 6. Конфликт зависимостей.
    if low.contains("incompatible mod set")
        || low.contains("missing or unsupported mandatory dependencies")
        || (low.contains("requires") && (low.contains("which is missing") || low.contains("but only the following")))
        || low.contains("mod resolution encountered an incompatible mod set")
    {
        d.kind = "dependency".into();
        let involved = quoted_entities(text);
        let mut vars = BTreeMap::new();
        if involved.is_empty() {
            set_text(
                &mut d,
                "Модам не хватает зависимостей",
                "Набор модов несовместим: чего-то не хватает или версии не совпадают.",
                &[
                    "Прочитайте строки «requires … which is missing» в отчёте — там перечислено, чего не хватает.",
                    "Установите недостающие моды или уберите лишние.",
                    "Проверьте, что все моды одной версии Minecraft.",
                ],
                vars,
            );
        } else {
            vars.insert("mods".into(), involved.join(", "));
            set_text(
                &mut d,
                "Модам не хватает зависимостей",
                "Проблема с модами: {mods}.",
                &[
                    "Прочитайте строки «requires … which is missing» в отчёте — там перечислено, чего не хватает.",
                    "Установите недостающие моды или уберите лишние.",
                    "Проверьте, что все моды одной версии Minecraft.",
                ],
                vars,
            );
        }
        d.culprit = involved.first().cloned();
        return d;
    }

    // 7. Падение миксина/Mixin.
    if low.contains("mixin apply")
        || low.contains("mixinapplyerror")
        || low.contains("critical injection failure")
        || low.contains("mixing")
    {
        d.kind = "mixin".into();
        let (culprit, file) = find_culprit(&stack, &mods, &suspects);
        let mut vars = BTreeMap::new();
        if let Some(c) = &culprit {
            vars.insert("mod".into(), c.clone());
            set_text(
                &mut d,
                "Мод не смог внедриться (Mixin)",
                "Mixin-патч не применился, вероятный виновник — {mod}.",
                &[
                    "Обновите виновный мод до версии под ваш Minecraft.",
                    "Уберите конфликтующий мод или его версию для другой сборки.",
                ],
                vars,
            );
        } else {
            set_text(
                &mut d,
                "Мод не смог внедриться (Mixin)",
                "Mixin-патч не применился: мод несовместим с этой версией игры или другого мода.",
                &[
                    "Обновите виновный мод до версии под ваш Minecraft.",
                    "Уберите конфликтующий мод или его версию для другой сборки.",
                ],
                vars,
            );
        }
        d.culprit = culprit;
        d.culprit_file = file;
        return d;
    }

    // 8. «Подозреваемые» из отчёта Forge/NeoForge.
    if !suspects.is_empty() {
        d.kind = "mod".into();
        let (culprit, file) = find_culprit(&stack, &mods, &suspects);
        let mut vars = BTreeMap::new();
        if let Some(c) = &culprit {
            vars.insert("mod".into(), c.clone());
        }
        if !primary.is_empty() {
            vars.insert("error".into(), primary.clone());
        }
        set_text(
            &mut d,
            if culprit.is_some() {
                "Виноват мод: {mod}"
            } else {
                "Виноват мод"
            },
            if primary.is_empty() {
                "Отчёт прямо указывает на мод."
            } else {
                "Отчёт прямо указывает на мод. {error}"
            },
            &[
                "Обновите или удалите виновный мод.",
                "Проверьте, что его версия совпадает с версией Minecraft.",
            ],
            vars,
        );
        d.culprit = culprit;
        d.culprit_file = file;
        return d;
    }

    // 9. Пробуем найти мод по стеку.
    let (culprit, culprit_file) = find_culprit(&stack, &mods, &suspects);
    if let Some(culprit) = culprit {
        d.kind = "mod".into();
        let mut vars = BTreeMap::new();
        vars.insert("mod".into(), culprit.clone());
        if !primary.is_empty() {
            vars.insert("error".into(), primary.clone());
        }
        set_text(
            &mut d,
            "Виноват мод: {mod}",
            if primary.is_empty() {
                "Судя по стеку, ошибка пришла из {mod}."
            } else {
                "{error}\n\nСудя по стеку, ошибка пришла из {mod}."
            },
            &[
                "Обновите виновный мод или удалите его и проверьте запуск.",
                "Если не помогло — отключите моды по половине, чтобы найти конфликт.",
            ],
            vars,
        );
        d.culprit = Some(culprit);
        d.culprit_file = culprit_file;
        return d;
    }

    // 10. Не разобрались, но покажем исключение.
    let mut vars = BTreeMap::new();
    if primary.is_empty() {
        d.summary = "Точную причину определить не удалось — смотрите лог.".into();
        d.summary_key = Some("Точную причину определить не удалось — смотрите лог.".into());
    } else {
        vars.insert("error".into(), primary.clone());
        d.summary =
            format!("{primary}\n\nТочную причину определить не удалось — смотрите лог.");
        d.summary_key =
            Some("{error}\n\nТочную причину определить не удалось — смотрите лог.".into());
    }
    d.hints = vec![
        "Откройте отчёт и посмотрите блок «Caused by» — там обычно указан мод.".into(),
        "Отключите половину модов и запустите: так конфликт находится быстрее всего.".into(),
    ];
    d.hint_keys = vec![
        "Откройте отчёт и посмотрите блок «Caused by» — там обычно указан мод.".into(),
        "Отключите половину модов и запустите: так конфликт находится быстрее всего.".into(),
    ];
    d.vars = vars;
    d
}

/// Ищет строку с нативным фреймом в hs_err.
fn problematic_frame(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Problematic frame:") {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
            // В hs_err фрейм обычно на следующей строке.
            if let Some(next) = lines.get(i + 1).map(|s| s.trim()) {
                if !next.is_empty() {
                    return Some(next.to_string());
                }
            }
        }
    }
    None
}

/// Список модов из секций «Mod List:» (Forge/NeoForge) и «Fabric Mods:».
fn parse_mods(text: &str) -> Vec<ModEntry> {
    let mut out: Vec<ModEntry> = Vec::new();
    let mut mode = "";
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Mod List:") {
            mode = "forge";
            continue;
        }
        if t.starts_with("Fabric Mods:") {
            mode = "fabric";
            continue;
        }
        if mode.is_empty() {
            continue;
        }
        if t.is_empty() || t.starts_with("-- ") || t.starts_with("Details:") || t.starts_with("Uptime:") {
            mode = "";
            continue;
        }
        if mode == "forge" {
            let parts: Vec<&str> = t.split('|').collect();
            if parts.len() >= 3 {
                out.push(ModEntry {
                    id: parts[0].trim().to_string(),
                    name: parts[1].trim().to_string(),
                    file: parts
                        .get(3)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                });
            }
        } else if let Some((id, rest)) = t.split_once(':') {
            out.push(ModEntry {
                id: id.trim().to_string(),
                name: rest.trim().to_string(),
                file: None,
            });
        }
    }
    out
}

/// Строка «Suspected Mods:» (Forge/NeoForge).
fn parse_suspected(text: &str) -> Vec<String> {
    RE_SUSPECT
        .captures(text)
        .map(|c| {
            c[1]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"))
                .collect()
        })
        .unwrap_or_default()
}

/// Заголовки исключений. В отчёте главное — первое, в логе — последнее.
fn collect_throws(text: &str, from_log: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let body = t.strip_prefix("Caused by: ").unwrap_or(t);
        if RE_THROW.is_match(body) {
            out.push(t.to_string());
        }
    }
    if from_log && out.len() > 1 {
        // В логе интересен последний краш, а не первый за сессию.
        let n = out.len();
        return out[n - 1..].to_vec();
    }
    out.truncate(10);
    out
}

/// Часть отчёта до подробного разбора — там исключение и стек.
fn stack_region(text: &str) -> String {
    let end = text
        .find("A detailed walkthrough")
        .or_else(|| text.find("-- System Details --"))
        .unwrap_or(text.len());
    text[..end].to_string()
}

/// Первые строки отчёта — для кнопки «показать лог».
fn head_excerpt(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if out.len() >= 60 {
            break;
        }
        if line.contains("-- System Details --") {
            break;
        }
        out.push(line.trim_end().to_string());
    }
    while out.first().map(|s| s.is_empty()).unwrap_or(false) {
        out.remove(0);
    }
    out
}

/// Ищет виновника: сначала по «Suspected Mods», потом по стеку.
fn find_culprit(
    stack: &str,
    mods: &[ModEntry],
    suspects: &[String],
) -> (Option<String>, Option<String>) {
    if let Some(s) = suspects.first() {
        let id = s.split('(').next().unwrap_or(s).trim();
        if let Some(m) = mods.iter().find(|m| m.id.eq_ignore_ascii_case(id)) {
            return (Some(m.label()), m.file.clone());
        }
        return (Some(s.clone()), None);
    }

    // Мешает только скучный минимум загрузчика.
    const SKIP: [&str; 7] = [
        "minecraft",
        "java",
        "forge",
        "neoforge",
        "fabricloader",
        "fabric-api",
        "quilt_loader",
    ];
    let low = stack.to_lowercase();
    let mut best: Option<(&ModEntry, usize)> = None;
    for m in mods {
        let id = m.id.to_lowercase();
        if id.len() < 4 || SKIP.contains(&id.as_str()) {
            continue;
        }
        let hits = low.matches(&id).count();
        if hits > 0 && best.as_ref().map(|(_, s)| hits > *s).unwrap_or(true) {
            best = Some((m, hits));
        }
    }
    match best {
        Some((m, _)) => (Some(m.label()), m.file.clone()),
        None => (None, None),
    }
}

/// Имена в кавычках из строк про зависимости.
fn quoted_entities(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let l = line.to_lowercase();
        if !(l.contains("requires") || l.contains("missing") || l.contains("incompatible")) {
            continue;
        }
        let mut parts = line.split('\'');
        let _ = parts.next();
        while let Some(v) = parts.next() {
            let _ = parts.next();
            let v = v.trim();
            if !v.is_empty() && !out.iter().any(|x| x == v) {
                out.push(v.to_string());
            }
            if out.len() >= 6 {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "-- Minecraft Crash Report --\n\
Description: Rendering overlay\n\n\
java.lang.NoSuchMethodError: 'void com.example.broken.Foo.bar()'\n\
\tat com.example.broken.Foo.bar(Foo.java:10)\n\n\
A detailed walkthrough of the error\n\
-- System Details --\n\
Details:\n\
\tMod List: \n\
\t\tminecraft|Minecraft|1.20.1|client.jar\n\
\t\tbroken|Broken Mod|1.0|broken-1.0.jar\n\
\tSuspected Mods: broken (Broken Mod)\n";

    #[test]
    fn finds_suspected_mod() {
        let d = analyze(REPORT, false);
        assert_eq!(d.kind, "mod");
        assert_eq!(d.culprit.as_deref(), Some("Broken Mod"));
        assert_eq!(d.culprit_file.as_deref(), Some("broken-1.0.jar"));
    }

    #[test]
    fn detects_oom() {
        let t = "Description: Ticking entity\njava.lang.OutOfMemoryError: Java heap space\n";
        let d = analyze(t, false);
        assert_eq!(d.kind, "memory");
    }

    #[test]
    fn detects_dependency() {
        let t = "Incompatible mod set!\nMod 'Sodium' (sodium) 0.5 requires version 1.20.1 of 'sodiumextra', which is missing!\n";
        let d = analyze(t, false);
        assert_eq!(d.kind, "dependency");
    }

    #[test]
    fn open_gl_success_is_not_gpu_crash() {
        let log = "[12:00:00] [Render thread/INFO]: Backend library: LWJGL version 3.3.3\n\
                   [12:00:01] [Render thread/INFO]: OpenGL Version: 3.3.0 NVIDIA 577.00\n\
                   [12:00:02] [Render thread/INFO]: Created window using SDL video driver: windows\n\
                   [12:00:03] [Render thread/ERROR]: RealmsServiceException: Failed to parse into SignedJWT\n\
                   [12:05:00] [Render thread/INFO]: Stopping!\n\
                   [12:05:01] [Render thread/INFO]: Exiting event polling thread.\n";
        let d = analyze(log, true);
        assert_ne!(d.kind, "gpu", "OpenGL в успешном логе — не краш GPU");
        assert_eq!(d.title_key.as_deref(), Some("В логе нет падения"));
    }

    #[test]
    fn real_glfw_error_is_gpu() {
        let log = "[12:00:00] [main/ERROR]: GLFW error 65542: WGL: The driver does not appear to support OpenGL\n\
                   [12:00:01] [main/ERROR]: Failed to create window\n";
        let d = analyze(log, true);
        assert_eq!(d.kind, "gpu");
        assert_eq!(d.title_key.as_deref(), Some("Проблема с графикой"));
    }

    #[test]
    fn missing_dependency_is_not_gpu() {
        let log = "Mod resolution encountered an incompatible mod set!\n\
                   Mod 'create' (create) 0.5.1 requires 'flywheel' which is missing!\n";
        let d = analyze(log, false);
        assert_eq!(d.kind, "dependency");
    }

    #[tokio::test]
    #[ignore = "читает реальный профиль; нужен ARBUZ_TEST_INSTANCE"]
    async fn live_instance_diagnosis() {
        let id = std::env::var("ARBUZ_TEST_INSTANCE").expect("ARBUZ_TEST_INSTANCE");
        let d = diagnose(&id).await.expect("diagnosis");
        println!(
            "kind={} title={} source={} file={:?}",
            d.kind, d.title, d.source, d.file
        );
        assert_ne!(d.kind, "gpu", "успешный latest.log не должен быть GPU-крашем");
    }

    #[test]
    fn heuristic_text_is_localizable() {
        let report = "Description: Mixin apply failed\n\
                      mixin apply failed create.mixins.json:ExampleMixin -> net.minecraft.Foo\n\
                      Suspected Mods: create\n";
        let d = analyze(report, false);
        assert!(d.title_key.is_some());
        assert!(d.summary_key.is_some());
        assert!(!d.hint_keys.is_empty());
        assert_eq!(d.vars.get("mod").map(String::as_str), Some("create"));
        assert!(!d.summary.contains("{mod}"));
    }
}
