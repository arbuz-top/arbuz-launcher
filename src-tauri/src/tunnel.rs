//! Игра с друзьями без белого IP и без настройки роутера.
//!
//! Своего сервера у лаунчера нет: узлы находят друг друга через публичный
//! сервис n0 (библиотека iroh), пробивают прямой UDP-канал, а если NAT
//! симметричный — трафик идёт через их же релей. Игроку остаётся передать
//! другу код: принимающий его показывает, подключающийся — вводит.
//!
//! Внутри QUIC-потоки превращаются в обычный TCP: на стороне подключающегося
//! поднимается `127.0.0.1:<порт>` для Minecraft, на стороне принимающего
//! потоки идут на порт уже открытого в игре мира.
//!
//! Комната: у хозяина есть список игроков, владелец комнаты (сначала хозяин,
//! потом — кому он передал права) может выгонять гостей, а код можно
//! пересоздать: старый перестаёт пускать, но уже подключённые остаются.

use crate::error::{err, AppError, Res};
use iroh::endpoint::{presets, Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

/// Идентификатор нашего протокола: чужой клиент с тем же ALPN не подключится.
/// Версия 2 — комната с кодом-токеном и списком игроков.
const ALPN: &[u8] = b"arbuz-launcher/net/2";
/// Столько ждём выхода узла в сеть, прежде чем отдать код.
const ONLINE_WAIT: Duration = Duration::from_secs(10);
/// Сколько гость ждёт первый ответ хозяина (свою комнату).
const JOIN_WAIT: Duration = Duration::from_secs(8);

/// Что показываем игроку.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct P2p {
    /// Код для друга.
    pub code: String,
    /// Адрес для Minecraft: `127.0.0.1:25565` (у подключающегося — рабочий,
    /// у принимающего это адрес игры в локальной сети).
    pub local: String,
    pub port: u16,
    /// Мы принимаем (`true`) или подключаемся (`false`).
    pub hosting: bool,
    /// Комната уже создана, но мир ещё не открыт для сети.
    pub waiting: bool,
}

/// Игрок в комнате. `slot` 0 — хозяин (тот, кто создал комнату).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomPlayer {
    pub slot: u64,
    pub name: String,
}

/// Состояние комнаты — одинаково у всех, только `me` своё.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Room {
    pub players: Vec<RoomPlayer>,
    /// Слот владельца: он может выгонять игроков и менять код.
    pub admin: u64,
    /// Мой слот: 0 — я хозяин.
    pub me: u64,
    /// Меня исключили из комнаты.
    #[serde(default)]
    pub kicked: bool,
    /// Имя хозяина комнаты.
    #[serde(default)]
    pub host: String,
}

/// Подключённый гость на стороне хозяина.
struct Peer {
    slot: u64,
    name: String,
    conn: Connection,
}

struct Session {
    endpoint: Endpoint,
    /// Соединение гостя с хозяином: по нему гость шлёт команды комнаты.
    conn: Option<Connection>,
    code: String,
    local: String,
    port: u16,
    hosting: bool,
    /// Как зовут хозяина.
    me: String,
    /// Номер комнаты: старый наблюдатель логов понимает, что его сменили.
    gen: u64,
}

/// Приветствие гостя: токен из кода и имя игрока.
#[derive(Serialize, Deserialize)]
struct Hello {
    token: String,
    name: String,
}

static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));
/// Порт, который игра открыла для сети. 0 — мир ещё не открыт.
static GAME_PORT: AtomicU16 = AtomicU16::new(0);
/// Поколение комнаты.
static ROOM_GEN: AtomicU64 = AtomicU64::new(0);
/// Кто подключён к нашей комнате (только у хозяина).
static PEERS: Lazy<Mutex<Vec<Peer>>> = Lazy::new(|| Mutex::new(Vec::new()));
/// Слот владельца комнаты: 0 — хозяин.
static ADMIN: AtomicU64 = AtomicU64::new(0);
/// Номер следующего соединения (он же порядок входа).
static SLOT: AtomicU64 = AtomicU64::new(1);
/// Токен комнаты: входит в код, меняется кнопкой «Новый код».
static TOKEN: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));
/// Состояние комнаты у гостя (приходит от хозяина).
static GUEST: Lazy<Mutex<Option<Room>>> = Lazy::new(|| Mutex::new(None));
/// Сообщение игроку: «вас исключили», «связь потерялась».
static NOTICE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Имя игрока для списка комнаты: без управляющих символов, не длиннее 32.
fn clean_name(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .collect()
}

fn gen() -> u64 {
    ROOM_GEN.fetch_add(1, Ordering::Relaxed) + 1
}

fn msg<E: std::fmt::Display>(what: &str, e: E) -> AppError {
    AppError::Msg(format!("{what}: {e}"))
}

/// Случайный токен комнаты (8 hex-символов).
fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string().chars().take(8).collect()
}

async fn bind() -> Res<Endpoint> {
    Endpoint::builder(presets::N0)
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| msg("P2P-узел не запустился", e))
}

/// Переливает данные между QUIC-потоком и TCP-сокетом.
async fn bridge(send: SendStream, recv: RecvStream, tcp: TcpStream) {
    let (mut tcp_r, mut tcp_w) = tcp.into_split();
    let mut send = send;
    let mut recv = recv;
    let up = tokio::io::copy(&mut tcp_r, &mut send);
    let down = tokio::io::copy(&mut recv, &mut tcp_w);
    let _ = tokio::join!(up, down);
    let _ = send.finish();
}

/// Отправляет JSON одним uni-потоком. `false` — соединение уже закрыто.
async fn send_json<T: Serialize>(conn: &Connection, value: &T) -> bool {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return false;
    };
    let Ok(mut s) = conn.open_uni().await else {
        return false;
    };
    let ok = s.write_all(&bytes).await.is_ok();
    let _ = s.finish();
    ok
}

/// Имя, свободное в комнате: `manul`, `manul (1)`, `manul (2)`…
fn unique_name(raw: &str) -> String {
    let base = {
        let n = clean_name(raw);
        if n.is_empty() {
            "Игрок".to_string()
        } else {
            n
        }
    };
    let taken: Vec<String> = {
        let host = SESSION
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.me.clone())
            .unwrap_or_default();
        let peers = PEERS.lock().unwrap();
        let mut v = vec![host.to_lowercase()];
        v.extend(peers.iter().map(|p| p.name.to_lowercase()));
        v
    };
    if !taken.iter().any(|n| *n == base.to_lowercase()) {
        return base;
    }
    for i in 1..1000 {
        let cand = format!("{base} ({i})");
        if !taken.iter().any(|n| *n == cand.to_lowercase()) {
            return cand;
        }
    }
    base
}

/// Состояние комнаты для отправки гостям.
fn players_snapshot(host: &str) -> Vec<RoomPlayer> {
    let host = if host.is_empty() { "Игрок" } else { host };
    let mut players = vec![RoomPlayer {
        slot: 0,
        name: host.to_string(),
    }];
    players.extend(PEERS.lock().unwrap().iter().map(|p| RoomPlayer {
        slot: p.slot,
        name: p.name.clone(),
    }));
    players
}

/// Рассылает гостям свежий список игроков и владельца.
async fn broadcast() {
    let (host, admin) = {
        let g = SESSION.lock().unwrap();
        match g.as_ref() {
            Some(s) if s.hosting => (s.me.clone(), ADMIN.load(Ordering::Relaxed)),
            _ => return,
        }
    };
    let targets: Vec<(u64, Connection)> = PEERS
        .lock()
        .unwrap()
        .iter()
        .map(|p| (p.slot, p.conn.clone()))
        .collect();
    if targets.is_empty() {
        return;
    }
    let players = players_snapshot(&host);
    for (slot, conn) in targets {
        let room = Room {
            players: players.clone(),
            admin,
            me: slot,
            kicked: false,
            host: host.clone(),
        };
        let _ = send_json(&conn, &room).await;
    }
}

/// Следующий владелец после ухода `slot`: кто раньше зашёл (меньший слот),
/// а если гостей не осталось — снова хозяин.
fn next_admin() -> u64 {
    PEERS
        .lock()
        .unwrap()
        .iter()
        .map(|p| p.slot)
        .min()
        .unwrap_or(0)
}

/// Выгоняет гостя: сначала говорит «тебя исключили», потом закрывает канал.
async fn kick_peer(slot: u64) {
    if slot == 0 {
        return;
    }
    let target = PEERS
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.slot == slot)
        .map(|p| p.conn.clone());
    let Some(conn) = target else {
        return;
    };
    let _ = send_json(&conn, &serde_json::json!({ "kicked": true })).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    conn.close(0u32.into(), b"kicked");
    PEERS.lock().unwrap().retain(|p| p.slot != slot);
    if ADMIN.load(Ordering::Relaxed) == slot {
        ADMIN.store(next_admin(), Ordering::Relaxed);
    }
    broadcast().await;
}

/// Принимающий: каждое входящее соединение обслуживаем в своём потоке,
/// а каждый входящий поток — это одно TCP-подключение к игре.
async fn serve(endpoint: Endpoint) {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(accepting) = incoming.accept() else {
            continue;
        };
        let Ok(conn) = accepting.await else {
            continue;
        };
        tokio::spawn(handle_conn(conn));
    }
}

/// Один гость: приветствие с токеном, список комнаты, команды и TCP-потоки.
async fn handle_conn(conn: Connection) {
    let slot = SLOT.fetch_add(1, Ordering::Relaxed);

    // Здороваемся только с тем, кто знает актуальный код.
    let hello = tokio::time::timeout(Duration::from_secs(15), conn.accept_uni()).await;
    let Ok(Ok(mut stream)) = hello else {
        conn.close(0u32.into(), b"no hello");
        return;
    };
    let Ok(buf) = stream.read_to_end(1024).await else {
        conn.close(0u32.into(), b"no hello");
        return;
    };
    let Ok(hello) = serde_json::from_slice::<Hello>(&buf) else {
        conn.close(0u32.into(), b"no hello");
        return;
    };
    let token = TOKEN.lock().unwrap().clone();
    if hello.token != token {
        // Старый код после «Новый код» больше не пускает.
        conn.close(0u32.into(), b"old code");
        return;
    }

    let name = unique_name(&hello.name);
    PEERS.lock().unwrap().push(Peer {
        slot,
        name,
        conn: conn.clone(),
    });
    broadcast().await;

    // Команды комнаты от гостя (выгонять может только владелец).
    let ctl = conn.clone();
    tokio::spawn(async move {
        while let Ok(mut recv) = ctl.accept_uni().await {
            let Ok(buf) = recv.read_to_end(1024).await else {
                continue;
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf) else {
                continue;
            };
            if v.get("cmd").and_then(|x| x.as_str()) == Some("kick")
                && ADMIN.load(Ordering::Relaxed) == slot
            {
                if let Some(target) = v.get("slot").and_then(|x| x.as_u64()) {
                    kick_peer(target).await;
                }
            }
        }
    });

    loop {
        let Ok((send, recv)) = conn.accept_bi().await else {
            break;
        };
        // Друг может зайти раньше, чем хозяин откроет мир: ждём порт.
        let Some(port) = wait_game_port().await else {
            continue;
        };
        let Ok(tcp) = TcpStream::connect(("127.0.0.1", port)).await else {
            continue;
        };
        tokio::spawn(bridge(send, recv, tcp));
    }

    // Соединение закрылось — игрока больше нет в комнате.
    PEERS.lock().unwrap().retain(|p| p.slot != slot);
    if ADMIN.load(Ordering::Relaxed) == slot {
        ADMIN.store(next_admin(), Ordering::Relaxed);
    }
    broadcast().await;
}

/// Ждёт, пока в игре откроют мир для сети (до 20 секунд).
async fn wait_game_port() -> Option<u16> {
    for _ in 0..40 {
        let port = GAME_PORT.load(Ordering::Relaxed);
        if port != 0 {
            return Some(port);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    None
}

/// Следит за логами игры: как только мир открыт для сети — запоминаем порт,
/// закрыт — забываем. Так комнату можно создать заранее, до запуска игры.
fn watch_game(room: u64) {
    tokio::spawn(async move {
        let mine = |s: &Session| s.hosting && s.gen == room;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let hosting = SESSION.lock().unwrap().as_ref().map(mine).unwrap_or(false);
            if !hosting {
                return;
            }
            let port = crate::net::lan_any().await.ok().flatten().unwrap_or(0);
            // Порт из лога может быть от уже закрытого мира — проверяем, что он слушается.
            let live = if port > 0 {
                TcpStream::connect(("127.0.0.1", port)).await.is_ok()
            } else {
                false
            };
            GAME_PORT.store(if live { port } else { 0 }, Ordering::Relaxed);
            if live {
                if let Some(s) = SESSION.lock().unwrap().as_mut() {
                    if s.gen == room {
                        s.port = port;
                        s.local = format!("127.0.0.1:{port}");
                    }
                }
            }
        }
    });
}

/// Принимающий: создаёт комнату и отдаёт код. `port` — если мир уже открыт;
/// `None` — комната ждёт, пока в игре нажмут «Открыть для сети».
pub async fn open(port: Option<u16>, me: String) -> Res<P2p> {
    stop().await;
    let me = clean_name(&me);
    let port = port.unwrap_or(0);
    GAME_PORT.store(port, Ordering::Relaxed);
    let endpoint = bind().await?;
    // Код работает, только когда узел вышел в сеть и опубликовал свой адрес.
    let _ = tokio::time::timeout(ONLINE_WAIT, endpoint.online()).await;
    let token = new_token();
    *TOKEN.lock().unwrap() = token.clone();
    let code = format!("{}:{}", endpoint.id(), token);
    let local = if port > 0 {
        format!("127.0.0.1:{port}")
    } else {
        String::new()
    };
    tokio::spawn(serve(endpoint.clone()));
    let room = gen();
    watch_game(room);
    *SESSION.lock().unwrap() = Some(Session {
        endpoint,
        conn: None,
        code: code.clone(),
        local: local.clone(),
        port,
        hosting: true,
        me,
        gen: room,
    });
    Ok(P2p {
        code,
        local,
        port,
        hosting: true,
        waiting: port == 0,
    })
}

/// Принимающий: мир уже открыт на `port`.
pub async fn host(port: u16, me: String) -> Res<P2p> {
    open(Some(port), me).await
}

/// Принимающий: комната заранее — код можно отдать друзьям до запуска игры.
pub async fn room(me: String) -> Res<P2p> {
    open(None, me).await
}

/// Подключающийся: находит друга по коду и поднимает локальный порт для Minecraft.
pub async fn join(code: &str, me: String) -> Res<P2p> {
    let code = code.trim();
    if code.is_empty() {
        return err("Введите код друга");
    }
    // Код = идентификатор узла + токен комнаты: `abc…:1f2e3d4c`.
    let (id, token) = match code.rsplit_once(':') {
        Some((id, token)) if !id.is_empty() && !token.is_empty() => (id, token),
        _ => return err("Код устарел — попросите у друга новый"),
    };
    let id: EndpointId = id.parse().map_err(|e| msg("Код не распознан", e))?;
    stop().await;
    let endpoint = bind().await?;
    let conn = endpoint
        .connect(EndpointAddr::new(id), ALPN)
        .await
        .map_err(|e| msg("Не удалось соединиться с другом", e))?;

    // Приветствие: токен комнаты и имя. Хозяин проверит код и пришлёт комнату.
    let me = clean_name(&me);
    let hello = Hello {
        token: token.to_string(),
        name: me.clone(),
    };
    if !send_json(&conn, &hello).await {
        let _ = endpoint.close().await;
        return err("Не удалось представиться хозяину комнаты");
    }

    // Ждём список игроков: по нему понимаем, что код приняли.
    let first = tokio::time::timeout(JOIN_WAIT, conn.accept_uni()).await;
    let room = match first {
        Ok(Ok(mut recv)) => recv
            .read_to_end(64 * 1024)
            .await
            .ok()
            .and_then(|buf| serde_json::from_slice::<Room>(&buf).ok()),
        _ => None,
    };
    let Some(room) = room else {
        let _ = endpoint.close().await;
        return err("Не удалось войти: код устарел или хозяин отклонил подключение");
    };
    *GUEST.lock().unwrap() = Some(room);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let room_gen = gen();
    let bridge_conn = conn.clone();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                break;
            };
            let c = bridge_conn.clone();
            tokio::spawn(async move {
                let Ok((send, recv)) = c.open_bi().await else {
                    return;
                };
                bridge(send, recv, tcp).await;
            });
        }
    });

    // Обновления комнаты от хозяина: список игроков, владелец, «тебя исключили».
    let state_conn = conn.clone();
    tokio::spawn(async move {
        loop {
            let Ok(mut recv) = state_conn.accept_uni().await else {
                break;
            };
            let Ok(buf) = recv.read_to_end(64 * 1024).await else {
                continue;
            };
            let Ok(room) = serde_json::from_slice::<Room>(&buf) else {
                continue;
            };
            if room.kicked {
                if alive(room_gen) {
                    *NOTICE.lock().unwrap() = Some("Вас исключили из комнаты".to_string());
                    stop().await;
                }
                return;
            }
            *GUEST.lock().unwrap() = Some(room);
        }
        // Связь оборвалась не по нашей воле — говорим об этом игроку.
        if alive(room_gen) {
            *NOTICE.lock().unwrap() = Some("Связь с комнатой потерялась".to_string());
            stop().await;
        }
    });

    let local = format!("127.0.0.1:{port}");
    *SESSION.lock().unwrap() = Some(Session {
        endpoint,
        conn: Some(conn),
        code: code.to_string(),
        local: local.clone(),
        port,
        hosting: false,
        me,
        gen: room_gen,
    });
    Ok(P2p {
        code: code.to_string(),
        local,
        port,
        hosting: false,
        waiting: false,
    })
}

/// Комната всё ещё та, что мы открыли? (старый наблюдатель не должен
/// объявлять «связь потерялась» после обычной остановки).
fn alive(room: u64) -> bool {
    SESSION
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.gen == room)
        .unwrap_or(false)
}

/// Закрывает канал, если он был открыт.
pub async fn stop() {
    // Наблюдатель логов прошлой комнаты должен выйти.
    gen();
    GAME_PORT.store(0, Ordering::Relaxed);
    PEERS.lock().unwrap().clear();
    ADMIN.store(0, Ordering::Relaxed);
    TOKEN.lock().unwrap().clear();
    *GUEST.lock().unwrap() = None;
    let old = SESSION.lock().unwrap().take();
    if let Some(s) = old {
        s.endpoint.close().await;
    }
}

/// Список игроков и владелец комнаты.
pub fn room_state() -> Option<Room> {
    let (hosting, me) = {
        let g = SESSION.lock().unwrap();
        let s = g.as_ref()?;
        (s.hosting, s.me.clone())
    };
    if hosting {
        let players = players_snapshot(&me);
        Some(Room {
            players,
            admin: ADMIN.load(Ordering::Relaxed),
            me: 0,
            kicked: false,
            host: me,
        })
    } else {
        GUEST.lock().unwrap().clone()
    }
}

/// Выгнать гостя: хозяин делает это сам, владелец-гость — просит хозяина.
pub async fn kick(slot: u64) -> Res<()> {
    let (hosting, conn) = {
        let g = SESSION.lock().unwrap();
        match g.as_ref() {
            Some(s) => (s.hosting, s.conn.clone()),
            None => return err("Комната не создана"),
        }
    };
    if hosting {
        kick_peer(slot).await;
        return Ok(());
    }
    let Some(conn) = conn else {
        return err("Нет соединения с хозяином комнаты");
    };
    if send_json(&conn, &serde_json::json!({ "cmd": "kick", "slot": slot })).await {
        Ok(())
    } else {
        err("Хозяин комнаты уже не на связи")
    }
}

/// Передать права владельца другому игроку (0 — вернуть хозяину).
pub async fn make_admin(slot: u64) -> Res<()> {
    let hosting = SESSION
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.hosting)
        .unwrap_or(false);
    if !hosting {
        return err("Права передаёт хозяин комнаты");
    }
    if slot != 0 && !PEERS.lock().unwrap().iter().any(|p| p.slot == slot) {
        return err("Игрок уже вышел");
    }
    ADMIN.store(slot, Ordering::Relaxed);
    broadcast().await;
    Ok(())
}

/// Новый код комнаты: старый перестаёт пускать, текущие игроки остаются.
pub async fn new_code() -> Res<P2p> {
    let token = new_token();
    {
        let mut g = SESSION.lock().unwrap();
        let Some(s) = g.as_mut() else {
            return err("Комната не создана");
        };
        if !s.hosting {
            return err("Код меняет хозяин комнаты");
        }
        *TOKEN.lock().unwrap() = token.clone();
        s.code = format!("{}:{}", s.endpoint.id(), token);
    }
    active().ok_or_else(|| AppError::Msg("Комната не создана".to_string()))
}

/// Сообщение игроку (одноразовое): исключили из комнаты, связь потерялась.
pub fn notice() -> Option<String> {
    NOTICE.lock().unwrap().take()
}

/// Открыт ли канал прямо сейчас.
pub fn active() -> Option<P2p> {
    let guard = SESSION.lock().unwrap();
    let s = guard.as_ref()?;
    let port = if s.hosting {
        GAME_PORT.load(Ordering::Relaxed)
    } else {
        s.port
    };
    Some(P2p {
        code: s.code.clone(),
        local: if s.hosting && port > 0 {
            format!("127.0.0.1:{port}")
        } else {
            s.local.clone()
        },
        port,
        hosting: s.hosting,
        waiting: s.hosting && port == 0,
    })
}
