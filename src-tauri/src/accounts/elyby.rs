//! Ely.by — ТЗ §8.2.

use crate::error::{err, AppError, Res};
use crate::models::{Account, AccountKind};
use crate::paths;
use serde_json::json;
use std::path::PathBuf;

const AUTH: &str = "https://authserver.ely.by/auth";

#[derive(serde::Deserialize)]
struct Profile {
    id: String,
    name: String,
}

#[derive(serde::Deserialize)]
struct AuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "clientToken")]
    client_token: Option<String>,
    #[serde(rename = "selectedProfile")]
    selected_profile: Option<Profile>,
}

/// Текст ошибки Ely.by, если ответ не успешный.
fn failure(status: reqwest::StatusCode, body: &serde_json::Value) -> AppError {
    let msg = body
        .get("errorMessage")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    let low = msg.to_lowercase();
    if low.contains("two factor") || low.contains("two-factor") || low.contains("2fa") {
        return AppError::Msg(
            "Включена двухфакторная защита. Введите пароль в формате «пароль:токен»".into(),
        );
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return AppError::Msg("Неверный логин или пароль".into());
    }
    if msg.is_empty() {
        AppError::Msg(format!("Ely.by: ошибка {status}"))
    } else {
        AppError::Msg(msg.to_string())
    }
}

/// Вход по логину и паролю.
pub async fn login(http: &reqwest::Client, login: &str, password: &str) -> Res<Account> {
    let client_token = uuid::Uuid::new_v4().simple().to_string();

    let r = http
        .post(format!("{AUTH}/authenticate"))
        .json(&json!({
            "username": login,
            "password": password,
            "clientToken": client_token,
            "requestUser": true
        }))
        .send()
        .await?;

    let status = r.status();
    let text = r.text().await?;
    let body: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if !status.is_success() {
        return Err(failure(status, &body));
    }

    let resp: AuthResponse = serde_json::from_value(body)?;
    let profile = resp
        .selected_profile
        .ok_or_else(|| AppError::Msg("Ely.by не вернул игровой профиль".into()))?;

    super::secret_set(
        &profile.id,
        &json!({
            "accessToken": resp.access_token,
            "clientToken": resp.client_token.unwrap_or(client_token)
        }),
    )?;

    let acc = Account {
        id: profile.id,
        kind: AccountKind::Elyby,
        name: profile.name,
        note: "ely.by".to_string(),
    };
    if super::list().await?.iter().all(|a| a.id != acc.id) {
        super::add(acc.clone()).await?;
    }
    super::set_active(&acc.id).await?;
    Ok(acc)
}

/// Актуальный access-токен: проверяем через `/validate`, при отказе обновляем.
pub async fn token(http: &reqwest::Client, acc: &Account) -> Res<String> {
    let secret = super::secret_get(&acc.id)
        .ok_or_else(|| AppError::Msg("Нет данных входа Ely.by — войдите заново".into()))?;

    let access = secret
        .get("accessToken")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if access.is_empty() {
        return err("Нет данных входа Ely.by — войдите заново");
    }

    let ok = http
        .post(format!("{AUTH}/validate"))
        .json(&json!({ "accessToken": access }))
        .send()
        .await?
        .status()
        == reqwest::StatusCode::NO_CONTENT;
    if ok {
        return Ok(access);
    }

    let client = secret
        .get("clientToken")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let r = http
        .post(format!("{AUTH}/refresh"))
        .json(&json!({
            "accessToken": access,
            "clientToken": client,
            "requestUser": true
        }))
        .send()
        .await?;

    if !r.status().is_success() {
        return err("Сессия Ely.by истекла — войдите заново");
    }
    let resp: AuthResponse = r.json().await?;
    let token = resp.access_token;
    super::secret_set(
        &acc.id,
        &json!({
            "accessToken": token,
            "clientToken": resp.client_token.unwrap_or(client)
        }),
    )?;
    Ok(token)
}

/// `authlib-injector` подключается к JVM как `-javaagent`, то есть исполняется.
/// Поэтому скачиваем его только со сверкой sha256, опубликованной источником.
pub async fn injector(http: &reqwest::Client) -> Res<PathBuf> {
    let path = paths::authlib_jar();
    if tokio::fs::metadata(&path).await.is_ok() {
        return Ok(path);
    }

    #[derive(serde::Deserialize)]
    struct Latest {
        download_url: String,
        #[serde(default)]
        checksums: Checksums,
    }
    #[derive(serde::Deserialize, Default)]
    struct Checksums {
        #[serde(default)]
        sha256: String,
    }
    #[derive(serde::Deserialize)]
    struct Release {
        assets: Vec<Asset>,
    }
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
        #[serde(default)]
        digest: Option<String>,
    }

    /// Разбирает `sha256:<hex>` (или просто `<hex>`) в шестнадцатеричный хеш.
    fn sha256_of(s: &str) -> Option<String> {
        let hex = s.strip_prefix("sha256:").unwrap_or(s).trim();
        if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            Some(hex.to_ascii_lowercase())
        } else {
            None
        }
    }

    let mut url: Option<String> = None;
    let mut sha256: Option<String> = None;

    if let Ok(r) = http
        .get("https://authlib-injector.yushi.moe/artifact/latest.json")
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        if let Ok(l) = r.json::<Latest>().await {
            if let Some(s) = sha256_of(&l.checksums.sha256) {
                url = Some(l.download_url);
                sha256 = Some(s);
            }
        }
    }

    if url.is_none() {
        // Запасной источник — релизы на GitHub (там же лежит digest).
        let rel: Release = http
            .get("https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(a) = rel
            .assets
            .into_iter()
            .find(|a| a.name.starts_with("authlib-injector") && a.name.ends_with(".jar"))
        {
            sha256 = a.digest.as_deref().and_then(sha256_of);
            url = Some(a.browser_download_url);
        }
    }

    let Some(url) = url else {
        return err("Не удалось скачать authlib-injector");
    };
    let Some(sha256) = sha256 else {
        return err("Не удалось проверить целостность authlib-injector");
    };

    crate::http::download_file(
        http,
        &crate::http::Dl {
            url,
            path: path.clone(),
            sha1: None,
            sha256: Some(sha256),
            size: 0,
        },
    )
    .await?;
    Ok(path)
}
