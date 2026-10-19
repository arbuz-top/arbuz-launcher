use crate::error::{err, Res};
use crate::models::{Account, AccountKind};

/// UUID офлайн-игрока: md5 от `OfflinePlayer:{name}` с правкой битов версии/варианта.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut b: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    hex::encode(b)
}

/// Ник: 3–16 символов, только A-Za-z0-9_.
fn validate(name: &str) -> Res<()> {
    let n = name.chars().count();
    if !(3..=16).contains(&n) {
        return err("Ник: 3–16 символов, латиница, цифры и _");
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return err("Ник: 3–16 символов, латиница, цифры и _");
        }
    }
    Ok(())
}

/// Добавляет офлайн-аккаунт и делает его активным.
pub async fn add(name: &str) -> Res<Account> {
    let name = name.trim();
    validate(name)?;
    let uuid = offline_uuid(name);

    if let Some(existing) = super::list().await?.into_iter().find(|a| a.id == uuid) {
        super::set_active(&existing.id).await?;
        return Ok(existing);
    }

    let acc = Account {
        id: uuid,
        kind: AccountKind::Offline,
        name: name.to_string(),
        note: "без интернета".to_string(),
    };
    super::add(acc.clone()).await?;
    super::set_active(&acc.id).await?;
    Ok(acc)
}
