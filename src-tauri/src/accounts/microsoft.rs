//! Microsoft — вход по коду устройства (device code) + цепочка Xbox Live →
//! XSTS → Minecraft (ТЗ §8.3).
//!
//! Схема взята из открытой библиотеки MinecraftAuth (RaphiMC): используется
//! приложение Microsoft первого лица с title-id `00000000402b5328` в окружении
//! LIVE (`login.live.com/oauth20_*`) и область
//! `service::user.auth.xboxlive.com::MBI_SSL`. Такой client id уже одобрен для
//! Minecraft API, поэтому вход работает без регистрации собственного
//! приложения в Azure. Тикет Xbox Live для такого токена помечается `t=`
//! (для своего Azure-приложения с областью `XboxLive.signin` — `d=`).
//! Свой Azure client id можно задать переменной сборки `ARBUZ_MS_CLIENT_ID`.

use crate::error::{err, AppError, Res};
use crate::models::{Account, AccountKind};
use crate::state::AppState;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// title-id приложения Minecraft (как в MinecraftAuth). Это не UUID, а
/// «title id» старого образца — отсюда окружение LIVE и тикет `t=`.
const TITLE_CLIENT_ID: &str = "00000000402b5328";
/// Необязательное переопределение своим Azure client id при сборке.
const ENV_CLIENT_ID: Option<&str> = option_env!("ARBUZ_MS_CLIENT_ID");

const SCOPE_TITLE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const SCOPE_AZURE: &str = "XboxLive.signin offline_access";

/// Страница входа Microsoft, если ответ не содержит `verification_uri`.
const DEFAULT_LINK: &str = "https://www.microsoft.com/link";
/// Столько ждём подтверждения в браузере (устройство живёт 15 минут).
const MAX_POLL_SECS: u64 = 900;

/// Окружение авторизации Microsoft (аналог `MsaEnvironment` из MinecraftAuth).
struct Env {
    device_code_url: &'static str,
    token_url: &'static str,
    scope: &'static str,
    /// true — title id: тикет `t=`, `grant_type=device_code`.
    title: bool,
}

const ENV_LIVE: Env = Env {
    device_code_url: "https://login.live.com/oauth20_connect.srf",
    token_url: "https://login.live.com/oauth20_token.srf",
    scope: SCOPE_TITLE,
    title: true,
};

const ENV_ONLINE: Env = Env {
    device_code_url: "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode",
    token_url: "https://login.microsoftonline.com/consumers/oauth2/v2.0/token",
    scope: SCOPE_AZURE,
    title: false,
};

/// Клиентский идентификатор: свой из сборки, иначе встроенный title-id.
fn client_id() -> &'static str {
    ENV_CLIENT_ID
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(TITLE_CLIENT_ID)
}

/// Выбор окружения по виду client id.
fn env_for(id: &str) -> &'static Env {
    if is_dashed_uuid(id) {
        &ENV_ONLINE
    } else {
        &ENV_LIVE
    }
}

fn is_dashed_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    b.iter().enumerate().all(|(i, c)| match i {
        8 | 13 | 18 | 23 => *c == b'-',
        _ => c.is_ascii_hexdigit(),
    })
}

fn default_interval() -> u64 {
    5
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    #[serde(default)]
    pub verification_uri: String,
    /// Готовая ссылка для браузера: страница подставит код сама.
    #[serde(default)]
    pub verification_url: String,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

/// Результат входа в Minecraft-сервисы.
struct McLogin {
    mc_token: String,
    expires_at: i64,
    uuid: String,
    name: String,
}

/// Текущий этап входа — фронтенд показывает его вместо «ничего не происходит».
#[derive(Clone, serde::Serialize)]
pub struct MsStep {
    pub text: String,
}

fn step(app: &AppHandle, text: &str) {
    let _ = app.emit("ms-step", MsStep { text: text.to_string() });
}

/// Ссылка на страницу входа с уже подставленным кодом (`otc`).
/// Если `otc` уже есть в адресе (его возвращает Azure v2), не дублируем.
fn with_otc(base: &str, code: &str) -> String {
    if base.contains("otc=") {
        return base.to_string();
    }
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}otc={}", urlencoding::encode(code))
}

/// Шаг 1: запрашиваем код устройства.
pub async fn begin(state: &AppState) -> Res<DeviceCode> {
    // Новая попытка входа — сбрасываем возможную прошлую отмену.
    state.ms_cancel.store(false, Ordering::Relaxed);
    let id = client_id();
    let env = env_for(id);

    let mut form: Vec<(&str, &str)> = vec![("client_id", id), ("scope", env.scope)];
    if env.title {
        form.push(("response_type", "device_code"));
    }

    let r = state
        .http
        .post(env.device_code_url)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await?;

    let status = r.status();
    let text = r.text().await?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return err(format!(
            "Microsoft не выдал код устройства ({}): {}",
            status.as_u16(),
            describe(&body)
        ));
    }

    let mut dc: DeviceCode = serde_json::from_value(body.clone())
        .map_err(|_| AppError::Msg("Microsoft вернул неожиданный ответ кода устройства".into()))?;
    if dc.device_code.is_empty() || dc.user_code.is_empty() {
        return err("Microsoft не вернул код устройства");
    }
    if dc.interval == 0 {
        dc.interval = default_interval();
    }

    // Ссылка, которую открываем в браузере: код подставится сам.
    let complete = str_of(&body, "verification_uri_complete");
    let base = if !complete.is_empty() {
        complete
    } else if !dc.verification_uri.is_empty() {
        dc.verification_uri.clone()
    } else {
        DEFAULT_LINK.to_string()
    };
    dc.verification_uri = base.clone();
    dc.verification_url = with_otc(&base, &dc.user_code);

    Ok(dc)
}

/// Отмена ожидания кода устройства (закрытие диалога).
pub fn cancel(state: &AppState) {
    state.ms_cancel.store(true, Ordering::Relaxed);
}

/// Шаг 2: ждём подтверждения и проходим цепочку Xbox → Minecraft.
pub async fn poll(app: &AppHandle, state: &AppState, device_code: &str, interval: u64) -> Res<Account> {
    let id = client_id();
    let env = env_for(id);
    let mut interval = interval.clamp(1, 15);
    let deadline = Instant::now() + Duration::from_secs(MAX_POLL_SECS);
    let mut net_errors = 0u32;

    step(app, "Ожидание подтверждения в браузере");
    let (ms_access, ms_refresh) = loop {
        if Instant::now() >= deadline {
            return err("Код устройства истёк. Войдите заново.");
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if state.ms_cancel.swap(false, Ordering::Relaxed) {
            return err("Вход отменён");
        }

        let r = match state
            .http
            .post(env.token_url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", id),
                ("grant_type", grant_type(env)),
                ("device_code", device_code),
            ])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // Сеть отвалилась — это не отказ входа, продолжаем ждать.
                net_errors += 1;
                if net_errors > 10 {
                    return err(format!("Нет связи с Microsoft: {e}"));
                }
                continue;
            }
        };

        let status = r.status();
        let text = r.text().await?;
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);

        if status.is_success() {
            let access = str_of(&body, "access_token");
            let refresh = str_of(&body, "refresh_token");
            break (access, refresh);
        }

        match str_of(&body, "error").as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval += 5;
                continue;
            }
            other => return Err(AppError::Msg(oauth_message(other, &body))),
        }
    };

    if ms_access.is_empty() {
        return err("Microsoft не вернул токен доступа");
    }

    let login = mc_login(app, &state.http, env, &ms_access).await?;
    step(app, "Сохранение аккаунта");
    super::secret_set(
        &login.uuid,
        &json!({
            "msRefresh": ms_refresh,
            "mcToken": login.mc_token,
            "expiresAt": login.expires_at
        }),
    )?;

    let acc = Account {
        id: login.uuid,
        kind: AccountKind::Microsoft,
        name: login.name,
        note: "Microsoft".to_string(),
    };
    if super::list().await?.iter().all(|a| a.id != acc.id) {
        super::add(acc.clone()).await?;
    }
    super::set_active(&acc.id).await?;
    Ok(acc)
}

/// Актуальный Minecraft-токен: при истечении обновляем MS-токен и проходим
/// цепочку заново (ТЗ §7.5, шаг 6).
pub async fn token(http: &reqwest::Client, acc: &Account) -> Res<String> {
    let secret = super::secret_get(&acc.id)
        .ok_or_else(|| AppError::Msg("Нет данных входа Microsoft — войдите заново".into()))?;

    let mc = secret
        .get("mcToken")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let expires = secret
        .get("expiresAt")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    if !mc.is_empty() && expires > chrono::Utc::now().timestamp() + 60 {
        return Ok(mc);
    }

    let refresh = secret
        .get("msRefresh")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if refresh.is_empty() {
        return err("Нет данных входа Microsoft — войдите заново");
    }

    let id = client_id();
    let env = env_for(id);
    let r = http
        .post(env.token_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", id),
            ("scope", env.scope),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
        ])
        .send()
        .await?;

    let status = r.status();
    let text = r.text().await?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return err(format!(
            "Не удалось обновить токен Microsoft — войдите заново ({})",
            oauth_message(&str_of(&body, "error"), &body)
        ));
    }

    let ms_access = str_of(&body, "access_token");
    if ms_access.is_empty() {
        return err("Не удалось обновить токен Microsoft — войдите заново");
    }
    let new_refresh = {
        let s = str_of(&body, "refresh_token");
        if s.is_empty() {
            refresh
        } else {
            s
        }
    };

    let login = mc_login_quiet(http, env, &ms_access).await?;
    super::secret_set(
        &acc.id,
        &json!({
            "msRefresh": new_refresh,
            "mcToken": login.mc_token,
            "expiresAt": login.expires_at
        }),
    )?;
    Ok(login.mc_token)
}

/// Xbox Live → XSTS → Minecraft → профиль, с отчётом об этапах.
async fn mc_login(app: &AppHandle, http: &reqwest::Client, env: &Env, ms_access: &str) -> Res<McLogin> {
    step(app, "Авторизация в Xbox Live");
    let login = mc_login_quiet(http, env, ms_access).await?;
    step(app, "Готово");
    Ok(login)
}

/// Xbox Live → XSTS → Minecraft → профиль.
async fn mc_login_quiet(http: &reqwest::Client, env: &Env, ms_access: &str) -> Res<McLogin> {
    // 1. Xbox Live. Для title id тикет помечается `t=`, для Azure — `d=`.
    let ticket = if env.title {
        format!("t={ms_access}")
    } else {
        format!("d={ms_access}")
    };
    let r = http
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .header("x-xbl-contract-version", "1")
        .header("Accept", "application/json")
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": ticket
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    let status = r.status();
    let text = r.text().await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return err(xbox_message(&v, status.as_u16()));
    }

    let xbl = str_of(&v, "Token");
    let uhs = v
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if xbl.is_empty() || uhs.is_empty() {
        return err("Xbox Live не вернул токен");
    }

    // 2. XSTS.
    let r = http
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .header("x-xbl-contract-version", "1")
        .header("Accept", "application/json")
        .json(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    let status = r.status();
    let text = r.text().await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return err(xbox_message(&v, status.as_u16()));
    }
    let xsts = str_of(&v, "Token");
    if xsts.is_empty() {
        return err("XSTS не вернул токен");
    }

    // 3. Minecraft.
    let r = http
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .header("Accept", "application/json")
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts}") }))
        .send()
        .await?;
    let status = r.status();
    let text = r.text().await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        let reason = str_of(&v, "errorMessage");
        if reason.is_empty() {
            let c = str_of(&v, "error");
            return err(format!(
                "Minecraft отклонил вход ({}{})",
                status.as_u16(),
                if c.is_empty() {
                    String::new()
                } else {
                    format!(": {c}")
                }
            ));
        }
        return err(format!("Minecraft отклонил вход: {reason}"));
    }

    let mc_token = str_of(&v, "access_token");
    if mc_token.is_empty() {
        return err("Minecraft не вернул токен доступа");
    }
    let expires_in = v.get("expires_in").and_then(|x| x.as_i64()).unwrap_or(86_400);
    let expires_at = chrono::Utc::now().timestamp() + expires_in;

    // 4. Профиль.
    let r = http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Accept", "application/json")
        .bearer_auth(&mc_token)
        .send()
        .await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return err("На аккаунте нет лицензии Minecraft");
    }
    let status = r.status();
    let text = r.text().await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !status.is_success() {
        return err(format!("Minecraft не вернул профиль игрока ({})", status.as_u16()));
    }

    let uuid = str_of(&v, "id");
    let name = str_of(&v, "name");
    if uuid.is_empty() || name.is_empty() {
        return err("Minecraft не вернул профиль игрока");
    }

    Ok(McLogin {
        mc_token,
        expires_at,
        uuid,
        name,
    })
}

/// `grant_type` для опроса токена: у LIVE — короткое значение.
fn grant_type(env: &Env) -> &'static str {
    if env.title {
        "device_code"
    } else {
        "urn:ietf:params:oauth:grant-type:device_code"
    }
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

/// Русский текст для ошибок Xbox Live / XSTS.
fn xbox_message(v: &Value, status: u16) -> String {
    match v.get("XErr").and_then(|x| x.as_u64()) {
        Some(2148916233) => return "У аккаунта нет Xbox-профиля".to_string(),
        Some(2148916238) => return "Детский аккаунт: нужен семейный доступ".to_string(),
        _ => {}
    }
    match status {
        401 => "Xbox Live отклонил вход: проверьте учётную запись".to_string(),
        403 => "Xbox Live отклонил вход (403): нет доступа к Minecraft".to_string(),
        _ => format!("Xbox Live отклонил вход ({status})"),
    }
}

/// Русский текст для ошибок OAuth device code.
fn oauth_message(code: &str, body: &Value) -> String {
    match code {
        "" => describe(body),
        "authorization_declined" => "Вход отклонён в браузере".to_string(),
        "expired_token" | "code_expired" => "Код устройства истёк. Войдите заново.".to_string(),
        "bad_verification_code" => "Неверный код устройства. Войдите заново.".to_string(),
        "invalid_client" => {
            "Приложение не зарегистрировано в Microsoft. Проверьте ARBUZ_MS_CLIENT_ID".to_string()
        }
        "unauthorized_client" => {
            "Приложение не разрешено для входа. Проверьте ARBUZ_MS_CLIENT_ID".to_string()
        }
        _ => describe(body),
    }
}

/// Человекочитаемый текст ошибки OAuth из тела ответа.
fn describe(body: &Value) -> String {
    if let Some(d) = body.get("error_description").and_then(|x| x.as_str()) {
        if !d.is_empty() {
            return d.to_string();
        }
    }
    match body.get("error").and_then(|x| x.as_str()) {
        Some(e) if !e.is_empty() => {
            if let Some(d) = body.get("error_description").and_then(|x| x.as_str()) {
                format!("{e}: {d}")
            } else {
                format!("Microsoft: {e}")
            }
        }
        _ => body
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("неизвестная ошибка Microsoft")
            .to_string(),
    }
}