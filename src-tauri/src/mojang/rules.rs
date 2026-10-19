// mojang/rules.rs
use serde::Deserialize; use std::collections::HashMap;
#[derive(Deserialize, Clone)] pub struct Rule { pub action: String, pub os: Option<RuleOs>, pub features: Option<HashMap<String, bool>> }
#[derive(Deserialize, Clone)] pub struct RuleOs { pub name: Option<String>, pub arch: Option<String> }
#[derive(Deserialize, Clone)] #[serde(untagged)] pub enum StrOrVec { One(String), Many(Vec<String>) }
#[derive(Deserialize, Clone)] #[serde(untagged)] pub enum Arg { Plain(String), Ruled { rules: Vec<Rule>, value: StrOrVec } }

#[cfg(target_os = "windows")] pub const OS_NAME: &str = "windows";
#[cfg(target_os = "macos")]   pub const OS_NAME: &str = "osx";
#[cfg(target_os = "linux")]   pub const OS_NAME: &str = "linux";

pub fn allowed(rules: &[Rule], feats: &HashMap<&str, bool>) -> bool {
    if rules.is_empty() { return true; }
    let mut ok = false;
    for r in rules {
        let os_ok = r.os.as_ref().map_or(true, |o|
            o.name.as_deref().map_or(true, |n| n == OS_NAME)
            && o.arch.as_deref().map_or(true, arch_matches));
        let f_ok = r.features.as_ref().map_or(true, |f|
            f.iter().all(|(k, v)| feats.get(k.as_str()).copied().unwrap_or(false) == *v));
        if os_ok && f_ok { ok = r.action == "allow"; }
    }
    ok
}

/// `os.arch` в правилах Mojang: `x86`, `x86_64`, `arm64`. Раньше проверялось
/// только `x86`, из-за чего `arm64`-правила считались подходящими для amd64.
fn arch_matches(a: &str) -> bool {
    match a {
        "x86" => cfg!(target_arch = "x86"),
        "x86_64" | "amd64" => cfg!(target_arch = "x86_64"),
        "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
        _ => false,
    }
}

/// В новых профилях Mojang все нативные classifier'ы (`natives-windows`,
/// `natives-windows-arm64`, ...) идут обычными библиотеками с одинаковым
/// `os.name`. Архитектуру надо проверять по имени classifier'а, иначе на
/// amd64 в classpath попадает arm64-натив и LWJGL падает с
/// `Platform/architecture mismatch`.
pub fn native_classifier_ok(name: &str) -> bool {
    let p: Vec<&str> = name.split(':').collect();
    let Some(cls) = p.get(3) else { return true; };
    if !cls.starts_with("natives-") {
        return true;
    }
    let c = cls.to_ascii_lowercase();
    if c.contains("arm64") || c.contains("aarch64") {
        cfg!(target_arch = "aarch64")
    } else if c.ends_with("-x86") || c.contains("-x86-") {
        cfg!(target_arch = "x86")
    } else {
        // classifier без суффикса архитектуры — это x86_64
        cfg!(target_arch = "x86_64")
    }
}
pub fn flat(args: &[Arg], feats: &HashMap<&str, bool>) -> Vec<String> {
    let mut o = vec![];
    for a in args { match a {
        Arg::Plain(s) => o.push(s.clone()),
        Arg::Ruled { rules, value } => if allowed(rules, feats) { match value {
            StrOrVec::One(s) => o.push(s.clone()),
            StrOrVec::Many(v) => o.extend(v.iter().cloned()) } } } }
    o
}
pub fn subst(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut r = s.to_string();
    for (k, v) in vars { r = r.replace(&format!("${{{k}}}"), v); }
    r
}
/// Путь артефакта в maven-репозитории. `None`, если имя не разбирается
/// (`group:artifact:version`) — раньше здесь была паника индексинга.
pub fn maven_path(name: &str) -> Option<String> {
    let p: Vec<&str> = name.split(':').collect(); // group:artifact:version[:classifier][@ext]
    if p.len() < 3 {
        return None;
    }
    let (v, ext) = match p[2].split_once('@') { Some((v, e)) => (v, e), None => (p[2], "jar") };
    let cls = p.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!("{}/{}/{}/{}-{}{}.{}", p[0].replace('.', "/"), p[1], v, p[1], v, cls, ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_path_builds_coordinates() {
        assert_eq!(
            maven_path("org.ow2.asm:asm:9.6").as_deref(),
            Some("org/ow2/asm/asm/9.6/asm-9.6.jar")
        );
        assert_eq!(
            maven_path("net.fabricmc:sponge-mixin:0.15.4+mixin.0.8.7").as_deref(),
            Some("net/fabricmc/sponge-mixin/0.15.4+mixin.0.8.7/sponge-mixin-0.15.4+mixin.0.8.7.jar")
        );
    }

    #[test]
    fn maven_path_rejects_malformed_name() {
        // Раньше здесь была паника индексинга p[2].
        assert!(maven_path("just-a-name").is_none());
        assert!(maven_path("g:a").is_none());
        assert!(maven_path("").is_none());
    }

    #[test]
    fn native_classifier_respects_arch() {
        if cfg!(target_arch = "x86_64") {
            assert!(native_classifier_ok("org.lwjgl:lwjgl:3.4.3:natives-windows"));
            assert!(!native_classifier_ok(
                "org.lwjgl:lwjgl:3.4.3:natives-windows-arm64"
            ));
        }
        if cfg!(target_arch = "aarch64") {
            assert!(!native_classifier_ok("org.lwjgl:lwjgl:3.4.3:natives-windows"));
            assert!(native_classifier_ok(
                "org.lwjgl:lwjgl:3.4.3:natives-windows-arm64"
            ));
        }
        // Обычная библиотека без classifier'а не фильтруется.
        assert!(native_classifier_ok("org.lwjgl:lwjgl:3.4.3"));
    }

    #[test]
    fn arch_rule_does_not_match_other_arch() {
        let arm = Rule {
            action: "allow".into(),
            os: Some(RuleOs {
                name: Some(OS_NAME.into()),
                arch: Some("arm64".into()),
            }),
            features: None,
        };
        let x64 = Rule {
            action: "allow".into(),
            os: Some(RuleOs {
                name: Some(OS_NAME.into()),
                arch: Some("x86_64".into()),
            }),
            features: None,
        };
        assert_eq!(allowed(&[arm], &HashMap::new()), cfg!(target_arch = "aarch64"));
        assert_eq!(allowed(&[x64], &HashMap::new()), cfg!(target_arch = "x86_64"));
    }
}
