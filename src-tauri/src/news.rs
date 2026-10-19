//! Новости Minecraft (ТЗ §12-b).
//!
//! Источник — `javaPatchNotes.json` от Mojang. Берём 3 первые записи и кэшируем
//! их в памяти на 30 минут, чтобы не дёргать сеть при каждом открытии «Главной».

use crate::error::Res;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const URL: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const BASE: &str = "https://launchercontent.mojang.com";
const TTL: Duration = Duration::from_secs(30 * 60);
const COUNT: usize = 3;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    pub version: String,
    /// Дата в ISO-8601, как её отдаёт Mojang.
    pub date: String,
    /// Абсолютная ссылка на картинку новости.
    pub image_url: Option<String>,
}

static CACHE: Lazy<Mutex<Option<(Instant, Vec<NewsItem>)>>> = Lazy::new(|| Mutex::new(None));

fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// Три последние новости. Кэш 30 минут.
pub async fn get(http: &reqwest::Client) -> Res<Vec<NewsItem>> {
    if let Ok(g) = CACHE.lock() {
        if let Some((at, list)) = g.as_ref() {
            if at.elapsed() < TTL {
                return Ok(list.clone());
            }
        }
    }

    let v: Value = http.get(URL).send().await?.error_for_status()?.json().await?;
    let mut out: Vec<NewsItem> = Vec::new();
    if let Some(arr) = v.get("entries").and_then(|x| x.as_array()) {
        for e in arr.iter().take(COUNT) {
            out.push(NewsItem {
                title: str_of(e, "title"),
                version: str_of(e, "version"),
                date: str_of(e, "date"),
                image_url: e
                    .pointer("/image/url")
                    .and_then(|x| x.as_str())
                    .filter(|u| !u.is_empty())
                    .map(|u| format!("{BASE}{u}")),
            });
        }
    }

    if let Ok(mut g) = CACHE.lock() {
        *g = Some((Instant::now(), out.clone()));
    }
    Ok(out)
}
