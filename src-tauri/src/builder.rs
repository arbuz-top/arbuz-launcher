//! ИИ-ассистент сборок: по промпту составляет план и подбирает моды,
//! ресурс-паки и шейдеры в каталоге Modrinth и CurseForge.
//!
//! Модель отвечает только за состав сборки; конкретные проекты подбираются
//! поиском по каталогу, поэтому в план не могут попасть несуществующие моды.

use crate::ai;
use crate::catalog::{self, CatalogItem, SearchArgs};
use crate::error::{err, AppError, Res};
use crate::models::Loader;
use crate::state::AppState;
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Ограничение промпта: длинные тексты не улучшают план, но жгут токены.
const MAX_PROMPT: usize = 2000;
/// Страховка от бесконечного списка от модели (лимит задаёт пользователь).
const MAX_ITEMS: usize = 200;
/// Сколько ресурс-паков и шейдеров максимум.
const MAX_EXTRA: usize = 12;
/// Параллельных поисков по каталогу.
const WORKERS: usize = 4;

const SYSTEM: &str = r#"Ты — опытный составитель сборок Minecraft (Fabric, NeoForge, Forge, Quilt).
Тебе дают описание желаемой сборки. Верни ТОЛЬКО валидный JSON без markdown и пояснений:
{"name":"короткое название","summary":"одно предложение","description":"3-6 предложений: что за сборка, для кого, что внутри","mcVersion":"1.20.1 или null","loader":"fabric|neoforge|forge|quilt или null","mods":[{"query":"название мода для поиска","why":"зачем он"}],"resourcepacks":[{"query":"...","why":"..."}],"shaders":[{"query":"...","why":"..."}]}

Правила:
- Бери только реально существующие проекты Modrinth или CurseForge.
- query — английское название проекта так, как его ищут в каталоге: "JEI", "Sodium", "Iris Shaders", "Fabric API".
- Модов бери столько, сколько нужно сборке: обычно 15-60, без «воды» ради количества.
- Обязательно добавляй нужные библиотеки: Fabric API, Cloth Config API, Architectury API, GeckoLib, YUNG's API и подобные.
- Моды должны быть совместимы между собой и с выбранным загрузчиком.
- shaders добавляй только если просят графику или шейдеры; resourcepacks — если просят текстурпаки.
- Учитывай ограничения: «слабый ПК» — упор на оптимизацию (Sodium, Lithium, FerriteCore, ImmediatelyFast) и без тяжёлых модов.
- Не выдумывай проекты, которых нет. Сомневаешься — не добавляй.
- mcVersion и loader возвращай только если пользователь их не задал и они однозначно следуют из запроса; иначе null.
- name, summary, description и why — на языке пользователя.
- Не вызывай инструменты: ответ должен быть только JSON."#;

/// Пункт плана от модели: что искать и зачем.
#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct RawItem {
    query: String,
    why: String,
}

/// План от модели до поиска по каталогу.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPlan {
    name: String,
    summary: String,
    description: String,
    mc_version: Option<String>,
    loader: Option<String>,
    mods: Vec<RawItem>,
    resourcepacks: Vec<RawItem>,
    shaders: Vec<RawItem>,
}

/// Найденный в каталоге проект сборки.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    /// mod | rp | sh
    pub kind: String,
    pub source: String,
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    /// Зачем проект в сборке (от модели).
    pub why: String,
}

/// Готовый план сборки для интерфейса.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPlan {
    pub model: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub mc_version: String,
    pub loader: String,
    pub mods: Vec<PlanItem>,
    pub resourcepacks: Vec<PlanItem>,
    pub shaders: Vec<PlanItem>,
    /// Пункты плана, которые не нашлись в каталоге (для честного отчёта).
    pub skipped: Vec<String>,
}

/// Разбирает строку загрузчика; Vanilla для сборки с модами не годится.
fn parse_loader(s: &str) -> Option<(Loader, &'static str)> {
    match s.trim().to_ascii_lowercase().as_str() {
        "fabric" => Some((Loader::Fabric, "fabric")),
        "forge" => Some((Loader::Forge, "forge")),
        "neoforge" | "neo-forge" => Some((Loader::Neoforge, "neoforge")),
        "quilt" => Some((Loader::Quilt, "quilt")),
        _ => None,
    }
}

/// Приводит название к сравнимому виду: строчные буквы и цифры.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Оценка совпадения названия проекта с запросом: 2 — точное, 1 — частичное.
fn score(title: &str, q: &str) -> u8 {
    let t = norm(title);
    if t.is_empty() || q.is_empty() {
        return 0;
    }
    if t == q {
        2
    } else if t.contains(q) || q.contains(&t) {
        1
    } else {
        0
    }
}

/// Лучший результат среди источников: сначала точное название, затем частичное,
/// иначе первый результат первого непустого источника (порядок = приоритет).
fn choose(groups: Vec<Vec<CatalogItem>>, query: &str) -> Option<CatalogItem> {
    let q = norm(query);
    if q.is_empty() {
        return groups.into_iter().flatten().next();
    }
    for g in &groups {
        if let Some(h) = g.iter().find(|h| score(&h.title, &q) == 2) {
            return Some(h.clone());
        }
    }
    for g in &groups {
        if let Some(h) = g.iter().find(|h| score(&h.title, &q) == 1) {
            return Some(h.clone());
        }
    }
    groups.into_iter().flatten().next()
}

fn args_for(kind: &str, loader: &str, mc: &str, query: &str, source: &str) -> SearchArgs {
    let loaders = if kind == "mod" && !loader.is_empty() {
        vec![loader.to_string()]
    } else {
        Vec::new()
    };
    SearchArgs {
        source: source.to_string(),
        kind: kind.to_string(),
        query: query.chars().take(120).collect(),
        loaders,
        mc_version: Some(mc.to_string()),
        categories: Vec::new(),
        offset: 0,
        sort: "relevance".into(),
        instance_id: None,
    }
}

/// Ищет проект с учётом приоритета: без источника — сразу в двух каталогах,
/// с источником — сначала в нём, а если там пусто, то в другом.
async fn search_one(
    state: &AppState,
    kind: &str,
    loader: &str,
    mc: &str,
    query: &str,
    source: Option<&str>,
) -> Option<CatalogItem> {
    let mr_args = args_for(kind, loader, mc, query, "modrinth");
    let cf_args = args_for(kind, loader, mc, query, "curseforge");
    let mr = catalog::modrinth::search(&state.http, &mr_args);
    let cf = catalog::curseforge::search(state, &cf_args);

    match source {
        None => {
            let (a, b) = tokio::join!(mr, cf);
            choose(
                vec![
                    a.map(|r| r.hits).unwrap_or_default(),
                    b.map(|r| r.hits).unwrap_or_default(),
                ],
                query,
            )
        }
        Some("curseforge") => {
            let first = cf.await.map(|r| r.hits).unwrap_or_default();
            if !first.is_empty() {
                return choose(vec![first], query);
            }
            let second = mr.await.map(|r| r.hits).unwrap_or_default();
            choose(vec![second], query)
        }
        _ => {
            let first = mr.await.map(|r| r.hits).unwrap_or_default();
            if !first.is_empty() {
                return choose(vec![first], query);
            }
            let second = cf.await.map(|r| r.hits).unwrap_or_default();
            choose(vec![second], query)
        }
    }
}

/// Накопитель подбора: что уже взято и что не нашлось.
#[derive(Default)]
struct Sink {
    skipped: Vec<String>,
    seen: std::collections::HashSet<String>,
}

/// Ищет пункты плана в каталоге; ненайденные копятся в `sink`.
async fn resolve_many(
    state: &AppState,
    kind: &str,
    loader: &str,
    mc: &str,
    source: Option<&str>,
    raws: Vec<RawItem>,
    sink: &mut Sink,
) -> Vec<PlanItem> {
    let mut out = Vec::new();
    let raws: Vec<RawItem> = raws
        .into_iter()
        .filter(|r| !r.query.trim().is_empty())
        .collect();
    if raws.is_empty() {
        return out;
    }

    let found: Vec<(RawItem, Option<CatalogItem>)> = stream::iter(raws)
        .map(move |r| async move {
            let query = r.query.trim().to_string();
            let hit = search_one(state, kind, loader, mc, &query, source).await;
            // Проверяем, что у проекта правда есть файл под версию игры и
            // загрузчик профиля: иначе ассистент притащит мод не для этой сборки.
            let hit = match hit {
                Some(h) => {
                    let ok = catalog::has_file(state, &h.source, &h.id, kind, loader, mc)
                        .await
                        .unwrap_or(true);
                    if ok {
                        Some(h)
                    } else {
                        None
                    }
                }
                None => None,
            };
            (r, hit)
        })
        .buffered(WORKERS)
        .collect()
        .await;

    for (raw, hit) in found {
        let Some(hit) = hit else {
            sink.skipped.push(raw.query.trim().to_string());
            continue;
        };
        if !sink.seen.insert(format!("{}:{}", hit.source, hit.id)) {
            continue;
        }
        out.push(PlanItem {
            kind: kind.to_string(),
            source: hit.source,
            id: hit.id,
            title: hit.title,
            author: hit.author,
            description: hit.description,
            icon_url: hit.icon_url,
            downloads: hit.downloads,
            why: raw.why.trim().to_string(),
        });
    }
    out
}

/// Версия Minecraft: явная от пользователя, затем предложенная моделью
/// (если она есть в манифесте Mojang), затем последний релиз.
async fn resolve_mc(state: &AppState, explicit: Option<String>, ai: Option<String>) -> Res<String> {
    if let Some(v) = explicit.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        return Ok(v);
    }
    let entries = crate::mojang::versions::entries(&state.http)
        .await
        .unwrap_or_default();
    if let Some(v) = ai.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        if entries.iter().any(|e| e.v.id == v && e.v.kind == "release") {
            return Ok(v);
        }
    }
    match entries.iter().find(|e| e.v.kind == "release") {
        Some(e) => Ok(e.v.id.clone()),
        None => err("Не удалось определить версию Minecraft"),
    }
}

fn build_user(
    prompt: &str,
    lang: &str,
    mc: Option<&str>,
    loader: Option<&str>,
    limit: Option<usize>,
) -> String {
    let mut s = String::with_capacity(prompt.len() + 600);
    s.push_str("Запрос игрока: ");
    s.push_str(prompt);
    s.push('\n');
    s.push_str(&format!("Язык текстов сборки: {}.\n", lang));
    match mc {
        Some(v) => s.push_str(&format!("Версия Minecraft задана: {v}. Поле mcVersion не заполняй.\n")),
        None => s.push_str("Версия Minecraft не задана — предложи подходящую в mcVersion.\n"),
    }
    match loader {
        Some(l) => s.push_str(&format!(
            "Загрузчик задан: {l}. Поле loader не заполняй.\n"
        )),
        None => s.push_str("Загрузчик не задан — предложи подходящий в loader.\n"),
    }
    match limit {
        Some(n) => s.push_str(&format!(
            "Игрок просит примерно {n} модов: подбери близко к этому числу, но не больше.\n"
        )),
        None => s.push_str("Число модов не ограничено — бери столько, сколько нужно сборке.\n"),
    }
    s.push_str(
        "\nСоставь сборку под этот запрос и верни JSON по схеме. Только JSON, без пояснений.",
    );
    s
}

/// Модели любят вернуть `null` вместо пустого списка и мусор в элементах:
/// приводим ответ к схеме, чтобы разбор не падал на мелочах.
fn normalize(v: &mut Value) {
    let Some(o) = v.as_object_mut() else {
        *v = Value::Object(Default::default());
        return;
    };
    for key in ["mods", "resourcepacks", "shaders"] {
        let arr = match o.get_mut(key) {
            Some(x) if x.is_array() => x.as_array_mut().expect("массив"),
            _ => {
                o.insert(key.to_string(), Value::Array(Vec::new()));
                continue;
            }
        };
        arr.retain_mut(|item| {
            let Some(obj) = item.as_object_mut() else {
                return false;
            };
            for f in ["query", "why"] {
                if !obj.get(f).map(|x| x.is_string()).unwrap_or(false) {
                    obj.insert(f.to_string(), Value::String(String::new()));
                }
            }

            let blank = |o: &serde_json::Map<String, Value>, f: &str| {
                o.get(f)
                    .and_then(|x| x.as_str())
                    .map(|s| s.trim().is_empty())
                    .unwrap_or(true)
            };

            if blank(obj, "query") {
                let found = ["name", "title", "mod", "project"]
                    .iter()
                    .find_map(|alt| {
                        obj.get(*alt)
                            .and_then(|x| x.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    });
                if let Some(v) = found {
                    obj.insert("query".into(), Value::String(v));
                }
            }

            if blank(obj, "why") {
                let found = ["reason", "description", "note"]
                    .iter()
                    .find_map(|alt| {
                        obj.get(*alt)
                            .and_then(|x| x.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                    });
                if let Some(v) = found {
                    obj.insert("why".into(), Value::String(v));
                }
            }

            !blank(obj, "query")
        });
    }
    for key in ["name", "summary", "description"] {
        if !o.get(key).map(|x| x.is_string()).unwrap_or(false) {
            o.insert(key.to_string(), Value::String(String::new()));
        }
    }
    for key in ["mcVersion", "loader"] {
        if o.get(key).map(|x| !x.is_string() && !x.is_null()).unwrap_or(false) {
            o.insert(key.to_string(), Value::Null);
        }
    }
}

/// Составляет план сборки по промпту и сам подбирает проекты в каталоге.
///
/// `limit` — сколько модов максимум хочет игрок (`None` — без ограничения).
/// `source` — приоритетный каталог; `None` — искать сразу в Modrinth и CurseForge.
/// `model` — выбранная модель Zen; `None` или пусто — первая доступная.
pub async fn plan(
    state: &AppState,
    prompt: &str,
    lang: &str,
    mc_version: Option<String>,
    loader: Option<String>,
    limit: Option<u32>,
    source: Option<String>,
    model: Option<String>,
) -> Res<PackPlan> {
    let prompt = prompt.trim();
    if prompt.chars().count() < 4 {
        return err("Опишите сборку хотя бы одним предложением");
    }
    let prompt: String = prompt.chars().take(MAX_PROMPT).collect();

    let limit = limit
        .map(|n| (n.max(1) as usize).min(MAX_ITEMS))
        .filter(|n| *n > 0);
    let source = source
        .as_deref()
        .filter(|s| matches!(*s, "modrinth" | "curseforge"));

    let asked_loader = loader.as_deref().and_then(parse_loader);
    let user = build_user(
        &prompt,
        lang,
        mc_version.as_deref().filter(|v| !v.trim().is_empty()),
        asked_loader.map(|(_, slug)| slug),
        limit,
    );

    let (model, text) = ai::ask_text(SYSTEM, &user, 3200, 0.4, model.as_deref()).await?;
    let clean = ai::strip_fences(&text);
    let mut value = ai::extract_json(&clean)
        .ok_or_else(|| AppError::Msg("ИИ вернул ответ не по схеме, попробуйте ещё раз".into()))?;
    normalize(&mut value);
    let raw: RawPlan = serde_json::from_value(value).unwrap_or_default();

    let loader_slug = asked_loader
        .or_else(|| raw.loader.as_deref().and_then(parse_loader))
        .map(|(_, slug)| slug)
        .unwrap_or("fabric");
    let mc = resolve_mc(state, mc_version, raw.mc_version.clone()).await?;

    let mut sink = Sink::default();

    let mut mods_raw = raw.mods;
    mods_raw.truncate(limit.unwrap_or(MAX_ITEMS));
    let mut mods = resolve_many(state, "mod", loader_slug, &mc, source, mods_raw, &mut sink).await;
    if let Some(n) = limit {
        mods.truncate(n);
    }

    let mut rp_raw = raw.resourcepacks;
    rp_raw.truncate(MAX_EXTRA);
    let resourcepacks =
        resolve_many(state, "rp", loader_slug, &mc, source, rp_raw, &mut sink).await;

    let mut sh_raw = raw.shaders;
    sh_raw.truncate(MAX_EXTRA);
    let shaders = resolve_many(state, "sh", loader_slug, &mc, source, sh_raw, &mut sink).await;
    let skipped = sink.skipped;

    if mods.is_empty() {
        return err("ИИ не смог подобрать моды, попробуйте описать сборку иначе");
    }

    let name = raw.name.trim();
    let name = if name.is_empty() {
        prompt.chars().take(48).collect::<String>()
    } else {
        name.chars().take(60).collect()
    };
    let summary = raw.summary.trim().chars().take(200).collect::<String>();
    let description = raw.description.trim().chars().take(4000).collect::<String>();

    Ok(PackPlan {
        model,
        name,
        summary,
        description: if description.is_empty() {
            prompt.clone()
        } else {
            description
        },
        mc_version: mc,
        loader: loader_slug.to_string(),
        mods,
        resourcepacks,
        shaders,
        skipped,
    })
}

/// Разбор ответа модели: используется тестами и как страховка для плана.
#[cfg(test)]
fn parse_plan(text: &str) -> Option<RawPlan> {
    let clean = ai::strip_fences(text);
    let value = ai::extract_json(&clean)?;
    serde_json::from_value::<RawPlan>(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: &str, title: &str) -> CatalogItem {
        CatalogItem {
            id: id.into(),
            title: title.into(),
            author: "a".into(),
            description: "d".into(),
            downloads: 1,
            loaders: vec![],
            categories: vec![],
            icon_url: None,
            source: "modrinth".into(),
            kind: "mod".into(),
            installed: false,
        }
    }

    #[test]
    fn choose_prefers_exact_title() {
        let hits = vec![hit("1", "Sodium Extra"), hit("2", "Sodium")];
        assert_eq!(choose(vec![hits], "Sodium").unwrap().id, "2");
    }

    #[test]
    fn choose_falls_back_to_first_source() {
        let mr = vec![hit("1", "Something Else")];
        let cf = vec![hit("2", "Another Thing")];
        assert_eq!(choose(vec![mr, cf], "нечто").unwrap().id, "1");
    }

    #[test]
    fn choose_finds_exact_title_in_second_source() {
        let mr = vec![hit("1", "Something Else")];
        let cf = vec![hit("2", "Sodium")];
        assert_eq!(choose(vec![mr, cf], "Sodium").unwrap().id, "2");
    }

    #[test]
    fn loader_parsing_covers_slugs() {
        assert!(parse_loader("Fabric").is_some());
        assert!(parse_loader("neoforge").is_some());
        assert!(parse_loader("vanilla").is_none());
    }

    #[test]
    fn plan_parses_json_with_fences() {
        let text = "```json\n{\"name\":\"Тест\",\"mods\":[{\"query\":\"JEI\",\"why\":\"рецепты\"}]}\n```";
        let p = parse_plan(text).unwrap();
        assert_eq!(p.name, "Тест");
        assert_eq!(p.mods.len(), 1);
        assert_eq!(p.mods[0].query, "JEI");
    }
}
