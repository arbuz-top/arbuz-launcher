use crate::error::{err, AppError, Res};
use crate::models::{Account, AccountKind};
use serde_json::json;

const AUTH: &str = "https://littleskin.cn/api/yggdrasil/authserver";
pub const INJECTOR_URL: &str = "https://littleskin.cn/api/yggdrasil";

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

fn failure(status: reqwest::StatusCode, body: &serde_json::Value) -> AppError {
    let msg = body
        .get("errorMessage")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return AppError::Msg("Неверный логин или пароль".into());
    }
    if msg.is_empty() {
        AppError::Msg(format!("LittleSkin: ошибка {status}"))
    } else {
        AppError::Msg(msg.to_string())
    }
}

pub async fn login(http: &reqwest::Client, login: &str, password: &str) -> Res<Account> {
    let client_token = uuid::Uuid::new_v4().simple().to_string();
    let r = http
        .post(format!("{AUTH}/authenticate"))
        .json(&json!({
            "agent": { "name": "Minecraft", "version": 1 },
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
        .ok_or_else(|| AppError::Msg("LittleSkin не вернул игровой профиль".into()))?;

    super::secret_set(
        &profile.id,
        &json!({
            "accessToken": resp.access_token,
            "clientToken": resp.client_token.unwrap_or(client_token)
        }),
    )?;

    let acc = Account {
        id: profile.id,
        kind: AccountKind::Littleskin,
        name: profile.name,
        note: "littleskin.cn".to_string(),
    };
    if super::list().await?.iter().all(|a| a.id != acc.id) {
        super::add(acc.clone()).await?;
    }
    super::set_active(&acc.id).await?;
    Ok(acc)
}

pub async fn token(http: &reqwest::Client, acc: &Account) -> Res<String> {
    let secret = super::secret_get(&acc.id)
        .ok_or_else(|| AppError::Msg("Нет данных входа LittleSkin — войдите заново".into()))?;

    let access = secret
        .get("accessToken")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if access.is_empty() {
        return err("Нет данных входа LittleSkin — войдите заново");
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
        return err("Сессия LittleSkin истекла — войдите заново");
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
