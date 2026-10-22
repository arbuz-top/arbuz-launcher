//! Modrinth (ТЗ §10.1).

use super::{loader_slug, push_link, str_arr, str_of, CatalogItem, NewFile, ProjectDetails, SearchArgs, SearchResult};
use crate::content::{self, ContentMeta, Entry};
use crate::error::{err, Res};
use crate::http::{download_all, Dl};
use crate::models::Instance;
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;

const API: &str = "https://api.modrinth.com/v2";

/// Загрузчики, которые Modrinth возвращает в общем списке `categories`.
const KNOWN_LOADERS: [&str; 4] = ["fabric", "forge", "neoforge", "quilt"];

fn project_type(kind: &str) -> &'static str {
    match kind {
        "mod" => "mod",
        "dp" => "datapack",
        "rp" => "resourcepack",
        "pk" => "modpack",
        "sh" => "shader",
        _ => "mod",
    }
}

/// Русская метка категории из макета -> slug Modrinth.
fn slug(cat: &str) -> String {
    match cat {
        "Оптимизация" => "optimization".into(),
        "Технологии" => "technology".into(),
        "Утилиты" => "utility".into(),
        "Графика" => "decoration".into(),
        "Приключения" => "adventure".into(),
        other => other.to_lowercase(),
    }
}

pub async fn search(c: &reqwest::Client, a: &SearchArgs) -> Res<SearchResult> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", project_type(&a.kind))]];
    // Загрузчик имеет смысл только для модов и сборок: у текстур-паков,
    // шейдеров и датапаков его нет — иначе поиск вернёт пустоту.
    if !a.loaders.is_empty() && matches!(a.kind.as_str(), "mod" | "pk") {
        facets.push(
            a.loaders
                .iter()
                .map(|l| format!("categories:{}", l.to_lowercase()))
                .collect(),
        );
    }
    if let Some(v) = &a.mc_version {
        if !v.is_empty() && !super::any_version(&a.kind) {
            facets.push(vec![format!("versions:{v}")]);
        }
    }
    for cat in &a.categories {
        facets.push(vec![format!("categories:{}", slug(cat))]);
    }

    let index = match a.sort.as_str() {
        "downloads" | "newest" | "updated" => a.sort.as_str(),
        _ => "relevance",
    };
    let url = format!(
        "{API}/search?query={}&facets={}&limit=20&offset={}&index={}",
        urlencoding::encode(&a.query),
        urlencoding::encode(&serde_json::to_string(&facets)?),
        a.offset,
        index
    );

    let v: Value = c.get(&url).send().await?.error_for_status()?.json().await?;
    let total = v.get("total_hits").and_then(|x| x.as_u64()).unwrap_or(0);

    let mut hits = Vec::new();
    if let Some(arr) = v.get("hits").and_then(|x| x.as_array()) {
        for h in arr {
            let cats = str_arr(h, "categories");
            hits.push(CatalogItem {
                id: str_of(h, "project_id"),
                title: str_of(h, "title"),
                author: str_of(h, "author"),
                description: str_of(h, "description"),
                downloads: h.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
                loaders: cats
                    .iter()
                    .filter(|c| KNOWN_LOADERS.contains(&c.as_str()))
                    .cloned()
                    .collect(),
                categories: cats,
                icon_url: h.get("icon_url").and_then(|x| x.as_str()).map(str::to_string),
                source: "modrinth".into(),
                kind: a.kind.clone(),
                installed: false,
            });
        }
    }
    Ok(SearchResult { hits, total })
}

/// Версии проекта, отфильтрованные по загрузчику и версии игры профиля.
async fn versions(c: &reqwest::Client, project_id: &str, kind: &str, inst: &Instance) -> Res<Vec<Value>> {
    let mut q: Vec<String> = Vec::new();
    // Для rp/dp/sh параметр `loaders` не передаём (ТЗ §10.1, п. 1).
    if kind == "mod" || kind == "pk" {
        if let Some(l) = loader_slug(inst.loader) {
            q.push(format!(
                "loaders={}",
                urlencoding::encode(&serde_json::to_string(&[l])?)
            ));
        }
    }
    // Ресурс-паки и шейдеры от версии игры не зависят — фильтр не ставим.
    if !super::any_version(kind) {
        q.push(format!(
            "game_versions={}",
            urlencoding::encode(&serde_json::to_string(&[inst.mc_version.clone()])?)
        ));
    }
    let url = format!("{API}/project/{project_id}/version?{}", q.join("&"));

    let v: Value = c.get(&url).send().await?.error_for_status()?.json().await?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

/// Совместима ли версия с профилем: та же версия игры и тот же загрузчик.
fn compatible(v: &Value, kind: &str, inst: &Instance) -> bool {
    // Текстуры и шейдеры подходят к любой версии игры.
    if super::any_version(kind) {
        return true;
    }
    let games = str_arr(v, "game_versions");
    if !games.iter().any(|g| g == &inst.mc_version) {
        return false;
    }
    if kind == "mod" || kind == "pk" {
        let Some(l) = loader_slug(inst.loader) else {
            return false;
        };
        let loaders = str_arr(v, "loaders");
        if !loaders.iter().any(|x| x == l) {
            return false;
        }
    }
    true
}

/// Первая release-версия, иначе первая доступная — среди совместимых.
fn pick_version<'a>(list: &'a [Value], kind: &str, inst: &Instance) -> Option<&'a Value> {
    let ok: Vec<&Value> = list.iter().filter(|v| compatible(v, kind, inst)).collect();
    ok.iter()
        .find(|v| str_of(v, "version_type") == "release")
        .or_else(|| ok.first())
        .copied()
}

/// Есть ли у проекта файл под версию игры и загрузчик профиля.
pub async fn has_file(
    c: &reqwest::Client,
    project_id: &str,
    kind: &str,
    loader: &str,
    mc: &str,
) -> Res<bool> {
    let mut q: Vec<String> = Vec::new();
    if (kind == "mod" || kind == "pk") && !loader.is_empty() {
        q.push(format!(
            "loaders={}",
            urlencoding::encode(&serde_json::to_string(&[loader])?)
        ));
    }
    if !super::any_version(kind) && !mc.is_empty() {
        q.push(format!(
            "game_versions={}",
            urlencoding::encode(&serde_json::to_string(&[mc])?)
        ));
    }
    let url = format!("{API}/project/{project_id}/version?{}", q.join("&"));
    let v: Value = c.get(&url).send().await?.error_for_status()?.json().await?;
    let list = v.as_array().cloned().unwrap_or_default();
    Ok(list.iter().any(|v| {
        v.get("files")
            .and_then(|f| f.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    }))
}

/// Основной файл версии: с `primary == true`, иначе первый.
fn primary_file(v: &Value) -> Option<&Value> {
    let files = v.get("files")?.as_array()?;
    files
        .iter()
        .find(|f| f.get("primary").and_then(|x| x.as_bool()).unwrap_or(false))
        .or_else(|| files.first())
}

fn new_file(v: &Value) -> Option<NewFile> {
    let f = primary_file(v)?;
    let url = str_of(f, "url");
    let filename = str_of(f, "filename");
    if url.is_empty() || filename.is_empty() {
        return None;
    }
    Some(NewFile {
        version_id: str_of(v, "id"),
        version_number: str_of(v, "version_number"),
        url,
        filename,
        sha1: f
            .pointer("/hashes/sha1")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        size: f.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
    })
}

/// Установка проекта вместе с обязательными зависимостями.
pub async fn install(
    app: &AppHandle,
    state: &AppState,
    inst: &Instance,
    project_id: &str,
    kind: &str,
    dir: &str,
    meta: &mut ContentMeta,
) -> Res<()> {
    let game = crate::paths::instance_minecraft_dir(&inst.id);
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: Vec<String> = vec![project_id.to_string()];

    while let Some(pid) = queue.pop() {
        if !visited.insert(pid.clone()) {
            continue;
        }
        let list = versions(&state.http, &pid, kind, inst).await?;
        let Some(ver) = pick_version(&list, kind, inst) else {
            return err("У проекта нет версии, совместимой с этим профилем");
        };
        let Some(file) = new_file(ver) else {
            return err("У версии нет файла для скачивания");
        };

        let rel = format!("{dir}/{}", file.filename);
        let path = content::safe_join(&game, &rel)?;
        let dl = Dl {
            url: file.url.clone(),
            path,
            sha1: if file.sha1.is_empty() {
                None
            } else {
                Some(file.sha1.clone())
            },
            sha256: None,
            size: file.size,
        };
        download_all(
            app,
            &state.http,
            &super::content_progress_id(&inst.id),
            vec![dl],
            0.0,
            100.0,
        )
        .await?;
        meta.files.insert(
            rel,
            Entry {
                source: "modrinth".into(),
                project_id: pid.clone(),
                version_id: file.version_id,
                sha1: file.sha1,
            },
        );

        // Обязательные зависимости ставим рекурсивно (ТЗ §10.1, п. 5).
        if let Some(deps) = ver.get("dependencies").and_then(|x| x.as_array()) {
            for d in deps {
                if str_of(d, "dependency_type") != "required" {
                    continue;
                }
                let dep = str_of(d, "project_id");
                if !dep.is_empty() && !visited.contains(&dep) {
                    queue.push(dep);
                }
            }
        }
    }
    Ok(())
}

/// Иконки проектов с CDN Modrinth: `project_id -> url`.
///
/// Иконка — украшение, поэтому ошибки сети не пробрасываем: вернём пустую карту.
pub async fn project_icons(c: &reqwest::Client, ids: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if ids.is_empty() {
        return out;
    }
    let Ok(ids_json) = serde_json::to_string(ids) else {
        return out;
    };
    let url = format!("{API}/projects?ids={}", urlencoding::encode(&ids_json));
    let Ok(resp) = c.get(&url).send().await else {
        return out;
    };
    let Ok(list) = resp.json::<Vec<Value>>().await else {
        return out;
    };
    for p in list {
        let (Some(id), Some(icon)) = (
            p.get("id").and_then(|x| x.as_str()),
            p.get("icon_url").and_then(|x| x.as_str()),
        ) else {
            continue;
        };
        if !id.is_empty() && !icon.is_empty() {
            out.insert(id.to_string(), icon.to_string());
        }
    }
    out
}

/// Файлы, для которых вышла новая версия (только записи Modrinth).
pub async fn resolve_updates(
    state: &AppState,
    inst: &Instance,
    meta: &ContentMeta,
) -> Res<Vec<(String, NewFile)>> {
    let hashes: Vec<String> = meta
        .files
        .values()
        .filter(|e| e.source == "modrinth" && !e.sha1.is_empty())
        .map(|e| e.sha1.clone())
        .collect();
    if hashes.is_empty() {
        return Ok(Vec::new());
    }

    let mut body = json!({ "hashes": hashes, "algorithm": "sha1" });
    if let Some(l) = loader_slug(inst.loader) {
        body["loaders"] = json!([l]);
    }
    body["game_versions"] = json!([inst.mc_version]);

    let v: Value = state
        .http
        .post(format!("{API}/version_files/update"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut out = Vec::new();
    for (file, entry) in &meta.files {
        if entry.source != "modrinth" || entry.sha1.is_empty() {
            continue;
        }
        let Some(ver) = v.get(&entry.sha1) else {
            continue;
        };
        let Some(new) = new_file(ver) else {
            continue;
        };
        if new.sha1.is_empty() || new.sha1.eq_ignore_ascii_case(&entry.sha1) {
            continue;
        }
        out.push((file.clone(), new));
    }
    Ok(out)
}

/// Подробности проекта для страницы мода.
pub async fn project(c: &reqwest::Client, id: &str) -> Res<ProjectDetails> {
    let url = format!("{API}/project/{}", urlencoding::encode(id));
    let v: Value = c.get(&url).send().await?.error_for_status()?.json().await?;
    let mut links = Vec::new();
    push_link(&mut links, "Исходный код", v.get("source_url").and_then(|x| x.as_str()));
    push_link(&mut links, "Багрепорты", v.get("issues_url").and_then(|x| x.as_str()));
    push_link(&mut links, "Вики", v.get("wiki_url").and_then(|x| x.as_str()));
    push_link(&mut links, "Discord", v.get("discord_url").and_then(|x| x.as_str()));
    if let Some(ds) = v.get("donation_urls").and_then(|x| x.as_array()) {
        for d in ds {
            let platform = str_of(d, "platform");
            let label = if platform.is_empty() {
                "Пожертвования".to_string()
            } else {
                format!("Пожертвования: {platform}")
            };
            push_link(&mut links, &label, d.get("url").and_then(|x| x.as_str()));
        }
    }
    Ok(ProjectDetails {
        id: id.to_string(),
        source: "modrinth".to_string(),
        title: str_of(&v, "title"),
        description: str_of(&v, "description"),
        body: str_of(&v, "body"),
        downloads: v.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
        icon_url: v.get("icon_url").and_then(|x| x.as_str()).map(str::to_string),
        links,
    })
}