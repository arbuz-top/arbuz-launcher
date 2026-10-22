import { useEffect, useState } from 'react';
import { api, type LanShare as Share, type P2pShare, type Room } from '../lib/api';
import { useT } from '../lib/i18n';
import { MaterialIcon } from '../lib/material-icons';
import { Icon } from '../lib/nav-icons';
import { monetLike } from '../lib/theme';
import { useStore } from '../store';
import { curAccount } from '../store';

/** Игра по сети: P2P по коду (проброс порта не нужен) либо доступ через
 *  роутер по UPnP, если у него есть внешний IP. */
export function LanShare({
  id,
  live,
  always,
}: {
  id: string;
  live: boolean;
  always?: boolean;
}) {
  const t = useT();
  const monet = monetLike(useStore((st) => st.config.style));
  const [share, setShare] = useState<Share | null>(null);
  const [p2p, setP2p] = useState<P2pShare | null>(null);
  const [code, setCode] = useState('');
  const [join, setJoin] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  /** Кто в комнате и кто владелец (одинаково у всех, кроме `me`). */
  const [room, setRoom] = useState<Room | null>(null);

  /** Как зовут игрока: имя аккаунта (или пусто — тогда «Вы»). */
  const myName = () => curAccount(useStore.getState())?.name ?? '';

  useEffect(() => {
    void api
      .netActive()
      .then(setShare)
      .catch(() => {});
    void api
      .p2pActive()
      .then(setP2p)
      .catch(() => {});
  }, []);

  // Комната живёт без игры: следим за её состоянием и кодом.
  useEffect(() => {
    if (!p2p) {
      setRoom(null);
      return;
    }
    let alive = true;
    const load = () => {
      void api
        .p2pActive()
        .then((s) => {
          if (alive) setP2p(s);
        })
        .catch(() => {});
      void api
        .p2pRoomState()
        .then((r) => {
          if (alive) setRoom(r);
        })
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 2000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [p2p?.code, p2p?.hosting]);

  // Одноразовые сообщения комнаты: «исключили», «связь потерялась».
  // Слушаем всегда: после исключения комнаты уже нет, а сказать надо.
  useEffect(() => {
    let alive = true;
    const check = () => {
      void api
        .p2pNotice()
        .then((n) => {
          if (!n || !alive) return;
          const text =
            n === 'Вас исключили из комнаты'
              ? t('Вас исключили из комнаты')
              : n === 'Связь с комнатой потерялась'
                ? t('Связь с комнатой потерялась')
                : n;
          setNote(text);
          useStore.getState().toastMsg(text);
        })
        .catch(() => {});
    };
    check();
    const timer = window.setInterval(check, 3000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [t]);

  const addr = share ? (share.host ? `${share.host}:${share.port}` : share.local) : '';

  const copy = (text: string, msg: string) => {
    void navigator.clipboard
      .writeText(text)
      .then(() => useStore.getState().toastMsg(msg))
      .catch(() => setNote(t('Скопируйте вручную.')));
  };

  // Комнату можно создать заранее: порт лаунчер найдёт сам, когда мир откроют.
  const host = async () => {
    setBusy(true);
    setNote(null);
    try {
      setP2p(await api.p2pRoom(myName()));
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  const connect = async () => {
    setBusy(true);
    setNote(null);
    try {
      setP2p(await api.p2pJoin(code.trim(), myName()));
      setJoin(false);
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  const drop = async () => {
    setBusy(true);
    setNote(null);
    try {
      await api.p2pStop();
      setP2p(null);
      setRoom(null);
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** Выгнать игрока: хозяин делает это сам, владелец-гость — просит хозяина. */
  const kick = async (slot: number) => {
    setBusy(true);
    setNote(null);
    try {
      await api.p2pKick(slot);
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** Передать права владельца другому игроку. */
  const makeAdmin = async (slot: number) => {
    setBusy(true);
    setNote(null);
    try {
      await api.p2pAdmin(slot);
      setRoom(await api.p2pRoomState());
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  /** Новый код: старый перестаёт пускать, подключённые остаются. */
  const regen = async () => {
    setBusy(true);
    setNote(null);
    try {
      setP2p(await api.p2pNewCode());
      useStore.getState().toastMsg(t('Код обновлён'));
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  const open = async () => {
    setBusy(true);
    setNote(null);
    try {
      const port = await api.lanPort(id);
      if (!port) {
        setNote(t('Сначала откройте мир для сети: в игре Esc → «Открыть для сети…».'));
        return;
      }
      setShare(await api.netShare(port));
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  const close = async () => {
    setBusy(true);
    setNote(null);
    try {
      await api.netUnshare();
      setShare(null);
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!always && !live && !share && !p2p) return null;

  return (
    <div className="panel lan-card">
      <div className="row g8" style={{ justifyContent: 'space-between' }}>
        <div>
          <b style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            {monet ? <MaterialIcon name="person_add" size={18} /> : <Icon name="coop" size={16} />}
            {t('Играть с друзьями')}
          </b>
          <div className="hint">
            {t('Другу не нужен белый IP и настройка роутера — просто передайте ему код.')}
          </div>
        </div>
        {p2p && (
          <button className="btn sm" disabled={busy} onClick={() => void drop()}>
            {t('Остановить канал')}
          </button>
        )}
      </div>

      {p2p ? (
        <div style={{ marginTop: 10 }}>
          <div className="hint" style={{ marginBottom: 6 }}>
            {p2p.hosting ? t('Код комнаты') : t('Адрес для Minecraft')}
          </div>
          <div className="row g8" style={{ flexWrap: 'wrap' }}>
            {p2p.hosting ? (
              <code className="lan-code">{p2p.code}</code>
            ) : (
              <code className="lan-addr">{p2p.local}</code>
            )}
            <button className="btn sm" onClick={() => copy(p2p.hosting ? p2p.code : p2p.local, t('Код скопирован'))}>
              {t('Скопировать')}
            </button>
            {p2p.hosting && (
              <button className="btn sm" disabled={busy} onClick={() => void regen()}>
                {t('Новый код')}
              </button>
            )}
          </div>
          {p2p.hosting ? (
            <div className="hint" style={{ marginTop: 8 }}>
              {p2p.waiting
                ? t('Комната создана — код уже можно отправить друзьям. Откройте мир для сети: в игре Esc → «Открыть для сети…».')
                : t('Мир открыт (порт {p}) — друзья могут заходить.', { p: p2p.port })}
            </div>
          ) : (
            <div className="hint" style={{ marginTop: 8 }}>
              {t('В Minecraft: Сетевая игра → Прямое подключение → добавьте {a}', { a: p2p.local })}
              <br />
              {t('Если хозяин ещё не открыл мир, подключение получится, как только он это сделает.')}
            </div>
          )}

          <div className="room-list">
            <div className="room-head-lb">
              {t('В комнате')}
              {room ? ` · ${room.players.length}` : ''}
            </div>
            {(room?.players ?? []).map((p) => {
              const mine = p.slot === room?.me;
              const owner = p.slot === room?.admin;
              const canMod = room ? p2p.hosting || room.me === room.admin : false;
              return (
                <div className={'room-row' + (mine ? ' me' : '')} key={p.slot}>
                  <span className="room-ava">{(p.name || '?').trim().charAt(0).toUpperCase() || '?'}</span>
                  <span className="room-nm">
                    {p.name || t('Игрок')}
                    {mine && <span className="room-you">{t('вы')}</span>}
                  </span>
                  {owner && <span className="room-owner">{t('владелец')}</span>}
                  <span style={{ flex: 1 }} />
                  {!mine && canMod && (
                    <>
                      {p2p.hosting && !owner && (
                        <button className="btn sm" disabled={busy} onClick={() => void makeAdmin(p.slot)}>
                          {t('Права')}
                        </button>
                      )}
                      <button className="btn sm" disabled={busy} onClick={() => void kick(p.slot)}>
                        {t('Выгнать')}
                      </button>
                    </>
                  )}
                </div>
              );
            })}
            {!room && <div className="hint" style={{ padding: '8px 2px' }}>{t('Ждём игроков…')}</div>}
          </div>
        </div>
      ) : (
        <div className="row g8" style={{ marginTop: 10 }}>
          <button className="btn sm pr with-ico" disabled={busy} onClick={() => void host()}>
            {monet ? <MaterialIcon name="person_add" size={16} /> : <Icon name="coop" size={14} />}
            {busy ? t('Соединяю…') : t('Создать комнату')}
          </button>
          <button className="btn sm with-ico" disabled={busy} onClick={() => setJoin((v) => !v)}>
            {monet ? <MaterialIcon name="person_add" size={16} /> : <Icon name="coop" size={14} />}
            {t('Подключиться по коду')}
          </button>
        </div>
      )}

      {join && !p2p && (
        <div className="row g8" style={{ marginTop: 10 }}>
          <input
            type="text"
            className="lan-in"
            value={code}
            placeholder={t('Введите код друга')}
            onChange={(e) => setCode(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && code.trim() && !busy) void connect();
            }}
          />
          <button className="btn sm pr with-ico" disabled={busy || !code.trim()} onClick={() => void connect()}>
            {monet ? <MaterialIcon name="person_add" size={16} /> : <Icon name="coop" size={16} />}
            {busy ? t('Соединяю…') : t('Подключиться')}
          </button>
        </div>
      )}

      {!p2p && (
        <>
          <div className="lan-sep">{t('Через роутер (UPnP)')}</div>
          <div className="row g8" style={{ justifyContent: 'space-between' }}>
            <div className="hint">
              {share
                ? t('Скажите друзьям этот адрес — порт открыт на роутере.')
                : t('Порт откроет сам роутер по UPnP: свой сервер и сторонние программы не нужны.')}
            </div>
            {share ? (
              <button className="btn sm" disabled={busy} onClick={() => void close()}>
                {t('Закрыть доступ')}
              </button>
            ) : (
              <button className="btn sm with-ico" disabled={busy} onClick={() => void open()}>
                {monet ? <MaterialIcon name="person_add" size={16} /> : <Icon name="coop" size={16} />}
                {busy ? t('Открываю…') : t('Открыть доступ')}
              </button>
            )}
          </div>
          {share && (
            <div className="row g8" style={{ marginTop: 10 }}>
              <code className="lan-addr">{addr}</code>
              <button className="btn sm" onClick={() => copy(addr, t('Адрес скопирован'))}>
                {t('Скопировать')}
              </button>
              {share.host && <span className="hint">{t('по сети: {a}', { a: share.local })}</span>}
            </div>
          )}
        </>
      )}

      {note && (
        <div className="hint" style={{ marginTop: 8, color: '#e0a13f' }}>
          {note}
        </div>
      )}
    </div>
  );
}
