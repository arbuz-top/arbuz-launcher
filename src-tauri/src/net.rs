//! Игра по сети без ручной настройки роутера.
//!
//! Своего сервера лаунчер не поднимает: он просит пробросить порт сам роутер
//! (SSDP-поиск шлюза, затем SOAP `AddPortMapping` по UPnP IGD). Работает, если
//! в роутере включён UPnP, а провайдер выдаёт «белый» IP; иначе покажем
//! понятную ошибку — порт придётся пробросить вручную.

use crate::error::{err, Res};
use crate::paths;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Mutex;
use std::time::Duration;
use tokio::net::UdpSocket;

const SSDP_ADDR: &str = "239.255.255.250:1900";
const SSDP_ST: &str = "urn:schemas-upnp-org:device:InternetGatewayDevice:1";
/// Подпись проброса в списке роутера.
const DESC: &str = "Arbuz Launcher";
const WAIT_SSDP: Duration = Duration::from_secs(3);
const WAIT_HTTP: Duration = Duration::from_secs(6);

/// Что показываем игроку: куда подключаться друзьям.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub port: u16,
    /// Внешний адрес (может быть пустым, если роутер его не сообщил).
    pub host: String,
    /// Адрес внутри сети: `192.168.0.5:25565`.
    pub local: String,
}

/// Найденная служба проброса портов на роутере.
struct Gateway {
    control: String,
    service: String,
}

struct Active {
    port: u16,
    control: String,
    service: String,
    host: String,
    local: String,
}

static ACTIVE: Lazy<Mutex<Option<Active>>> = Lazy::new(|| Mutex::new(None));

/// Отдельный клиент: шлюз — это локальная сеть, системный прокси тут не нужен.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(WAIT_HTTP)
        .read_timeout(WAIT_HTTP)
        .build()
        .unwrap_or_default()
}

/// Что-то из тегов описания службы.
fn tag(s: &str, name: &str) -> Option<String> {
    let start = s.find(&format!("<{name}"))?;
    let rest = s.get(start + name.len() + 1..)?;
    let content = rest.split_once('>')?.1;
    let end = content.find(&format!("</{name}>"))?;
    Some(content[..end].trim().to_string())
}

fn header(msg: &str, name: &str) -> Option<String> {
    msg.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if k.trim().eq_ignore_ascii_case(name) {
            Some(v.trim().to_string())
        } else {
            None
        }
    })
}

/// Отрезает путь у адреса описания: `http://192.168.0.1:1900/igd.xml` → хост.
fn base_url(location: &str) -> String {
    match location.split_once("//") {
        Some((scheme, rest)) => {
            let host = rest.split('/').next().unwrap_or("");
            format!("{scheme}//{host}")
        }
        None => location.to_string(),
    }
}

async fn service_info(client: &reqwest::Client, location: &str) -> Res<Gateway> {
    let xml = client
        .get(location)
        .timeout(WAIT_HTTP)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    for chunk in xml.split("<service").skip(1) {
        let block = chunk.split("</service>").next().unwrap_or_default();
        let Some(st) = tag(block, "serviceType") else {
            continue;
        };
        if !st.contains("WANIPConnection") && !st.contains("WANPPPConnection") {
            continue;
        }
        let Some(cu) = tag(block, "controlURL") else {
            continue;
        };
        let control = if cu.starts_with("http") {
            cu
        } else if cu.starts_with('/') {
            format!("{}{cu}", base_url(location))
        } else {
            format!("{}/{}", base_url(location), cu)
        };
        return Ok(Gateway { control, service: st });
    }
    err("Роутер не умеет пробрасывать порты: служба UPnP IGD не найдена")
}

/// Ищет шлюз в локальной сети и возвращает его службу и наш локальный адрес.
async fn discover(client: &reqwest::Client) -> Res<(Gateway, IpAddr)> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let msg = format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: {SSDP_ADDR}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {SSDP_ST}\r\n\r\n"
    );
    // Без connect: на Windows соединение с multicast-адресом запрещено.
    sock.send_to(msg.as_bytes(), SSDP_ADDR).await?;

    let mut buf = [0u8; 4096];
    let deadline = tokio::time::Instant::now() + WAIT_SSDP;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        let Ok(Ok((n, from))) = tokio::time::timeout(left, sock.recv_from(&mut buf)).await else {
            break;
        };
        let text = String::from_utf8_lossy(&buf[..n]).to_string();
        let Some(loc) = header(&text, "location") else {
            continue;
        };
        if let Ok(g) = service_info(client, &loc).await {
            // Шлюз ответил — маршрут до него известен, значит и свой адрес тоже.
            return Ok((g, local_ip_for(from)));
        }
    }
    err("Роутер не отвечает по UPnP. Включите UPnP в его настройках или пробросьте порт вручную")
}

/// Свой адрес в сети: запрос «в пустоту» — пакеты не уходят, но маршрут известен.
fn local_ip_for(target: std::net::SocketAddr) -> IpAddr {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect(target)?;
            s.local_addr()
        })
        .map(|a| a.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn soap_error(text: &str) -> String {
    tag(text, "errorDescription")
        .or_else(|| tag(text, "errorCode"))
        .unwrap_or_else(|| text.chars().take(120).collect::<String>().trim().to_string())
}

async fn soap(
    client: &reqwest::Client,
    g: &Gateway,
    action: &str,
    args: &str,
) -> Res<String> {
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} \
xmlns:u=\"{svc}\">{args}</u:{action}></s:Body></s:Envelope>",
        svc = g.service,
    );
    let resp = client
        .post(&g.control)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", format!("\"{}#{action}\"", g.service))
        .timeout(WAIT_HTTP)
        .body(body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let why = soap_error(&text);
        return err(if why.is_empty() {
            format!("Роутер отказался открыть порт (HTTP {status})")
        } else {
            format!("Роутер отказался открыть порт: {why}")
        });
    }
    Ok(text)
}

/// Пробрасывает порт на этом компьютере и возвращает адрес для друзей.
pub async fn share(port: u16) -> Res<Share> {
    let client = client();
    let _ = unshare().await;
    let (g, ip) = discover(&client).await?;
    if ip.is_unspecified() || ip.is_loopback() {
        return err("Не удалось определить адрес компьютера в локальной сети");
    }
    let args = format!(
        "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort>\
<NewProtocol>TCP</NewProtocol><NewInternalPort>{port}</NewInternalPort>\
<NewInternalClient>{ip}</NewInternalClient><NewEnabled>1</NewEnabled>\
<NewPortMappingDescription>{DESC}</NewPortMappingDescription><NewLeaseDuration>0</NewLeaseDuration>"
    );
    soap(&client, &g, "AddPortMapping", &args).await?;

    let host = match soap(&client, &g, "GetExternalIPAddress", "").await {
        Ok(text) => tag(&text, "NewExternalIPAddress").unwrap_or_default(),
        Err(_) => String::new(),
    };
    let host = if host == "0.0.0.0" { String::new() } else { host };
    let local = format!("{ip}:{port}");
    *ACTIVE.lock().unwrap() = Some(Active {
        port,
        control: g.control,
        service: g.service,
        host: host.clone(),
        local: local.clone(),
    });
    Ok(Share { port, host, local })
}

/// Закрывает проброс, если он был открыт.
pub async fn unshare() -> Res<()> {
    let Some(a) = ACTIVE.lock().unwrap().take() else {
        return Ok(());
    };
    let client = client();
    let g = Gateway {
        control: a.control,
        service: a.service,
    };
    let args = format!(
        "<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort>\
<NewProtocol>TCP</NewProtocol>",
        a.port
    );
    soap(&client, &g, "DeletePortMapping", &args).await.map(|_| ())
}

/// Открыт ли проброс прямо сейчас.
pub fn active() -> Option<Share> {
    ACTIVE.lock().unwrap().as_ref().map(|a| Share {
        port: a.port,
        host: a.host.clone(),
        local: a.local.clone(),
    })
}

/// Порт, который игра открыла для сети: Minecraft пишет его в лог.
pub async fn lan_port(id: &str) -> Res<Option<u16>> {
    if let Some(port) = port_from_log(&paths::instance_minecraft_dir(id).join("logs/latest.log")).await
    {
        return Ok(Some(port));
    }
    lan_any().await
}

/// Порт из самого свежего лога любого профиля: профиль мог быть выбран не тот
/// (игра запущена из другого), а у работающей игры лог пишется прямо сейчас.
pub async fn lan_any() -> Res<Option<u16>> {
    let Ok(mut rd) = tokio::fs::read_dir(paths::instances_dir()).await else {
        return Ok(None);
    };
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    while let Ok(Some(entry)) = rd.next_entry().await {
        let log = entry.path().join("minecraft/logs/latest.log");
        let Ok(meta) = tokio::fs::metadata(&log).await else {
            continue;
        };
        let Ok(time) = meta.modified() else {
            continue;
        };
        let fresh = time
            .elapsed()
            .map(|d| d.as_secs() < 30 * 60)
            .unwrap_or(false);
        if !fresh {
            continue;
        }
        if newest.as_ref().map(|(t, _)| time > *t).unwrap_or(true) {
            newest = Some((time, log));
        }
    }
    match newest {
        Some((_, log)) => Ok(port_from_log(&log).await),
        None => Ok(None),
    }
}

/// Читает хвост лога: файлы у модпаков бывают по несколько мегабайт,
/// а нужная строка почти всегда в самом конце.
async fn port_from_log(log: &std::path::Path) -> Option<u16> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    const TAIL: u64 = 128 * 1024;
    let mut f = tokio::fs::File::open(log).await.ok()?;
    let len = f.metadata().await.ok()?.len();
    f.seek(std::io::SeekFrom::Start(len.saturating_sub(TAIL)))
        .await
        .ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).await.ok()?;
    let text = String::from_utf8_lossy(&buf);
    text.lines().rev().take(800).find_map(lan_line)
}

/// Строка с портом LAN. Версии и моды пишут по-разному:
/// `Published LAN server on port 56770` (1.20+),
/// `Starting Minecraft server on *:54321` (выделенный сервер),
/// `Started serving on 54321` (одиночная игра в других версиях),
/// `Started on port 54321` (старые версии).
fn lan_line(line: &str) -> Option<u16> {
    for key in [
        "Published LAN server on port",
        "LAN server on port",
        "Starting Minecraft server on",
        "Started serving on",
        "Started on port",
        "hosted on port",
    ] {
        let Some(i) = line.find(key) else {
            continue;
        };
        let Some(tail) = line.get(i + key.len()..) else {
            continue;
        };
        let tail = tail.trim().trim_end_matches('.');
        let port = tail.rsplit_once(':').map(|(_, p)| p).unwrap_or(tail);
        let Ok(port) = port.trim().parse::<u16>() else {
            continue;
        };
        if port > 0 {
            return Some(port);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_lan_port_from_log_line() {
        let line = "[12:00:00] [Server thread/INFO]: Starting Minecraft server on *:54321";
        assert_eq!(lan_line(line), Some(54321));
        let ip = "[12:00:00] [Server thread/INFO]: Starting Minecraft server on 0.0.0.0:25565";
        assert_eq!(lan_line(ip), Some(25565));
        // Одиночная игра: «Открыть для сети» (реальная строка из лога 1.20.1).
        let published = "[10:16:12] [Render thread/INFO]: Published LAN server on port 56770";
        assert_eq!(lan_line(published), Some(56770));
        let serving = "[18:20:31] [Render thread/INFO]: Started serving on 54545";
        assert_eq!(lan_line(serving), Some(54545));
        // Старые версии.
        assert_eq!(lan_line("[12:00:00] [Server thread/INFO]: Started on port 25566"), Some(25566));
        // Чат e4mc: домен вместо порта — не наш случай.
        assert_eq!(lan_line("[CHAT] Local game hosted on domain [verdict-steep.de.e4mc.link]"), None);
        assert_eq!(lan_line("что-то другое"), None);
        assert_eq!(lan_line("Started serving on 0"), None);
    }

    #[test]
    fn tag_and_header_parsing() {
        let xml = "<service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl</controlURL></service>";
        assert_eq!(
            tag(xml, "controlURL").as_deref(),
            Some("/ctl")
        );
        let ssdp = "HTTP/1.1 200 OK\r\nLOCATION: http://192.168.0.1:1900/igd.xml\r\n";
        assert_eq!(
            header(ssdp, "location").as_deref(),
            Some("http://192.168.0.1:1900/igd.xml")
        );
        assert_eq!(base_url("http://192.168.0.1:1900/igd.xml"), "http://192.168.0.1:1900");
    }
}
