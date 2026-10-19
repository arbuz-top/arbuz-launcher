import { useEffect, useState } from 'react';
import { api, type Account } from '../lib/api';
import { useT } from '../lib/i18n';
import { ACCOUNT_LABEL } from '../lib/labels';
import { askConfirm, flushConfig, useStore } from '../store';
import { Avatar } from './Controls';

export function Accounts() {
  const s = useStore();
  const t = useT();
  const activeId = s.config.activeAccount;
  const [status, setStatus] = useState<Record<string, string>>({});

  const count = s.accounts.length;
  useEffect(() => {
    let alive = true;
    const list = useStore.getState().accounts.filter((a) => a.kind !== 'offline');
    if (list.length === 0) {
      setStatus({});
      return;
    }
    void Promise.all(
      list.map(async (a): Promise<[string, string]> => {
        try {
          return [a.id, await api.accountStatus(a.id)];
        } catch {
          return [a.id, 'expired'];
        }
      }),
    ).then((pairs) => {
      if (alive) setStatus(Object.fromEntries(pairs));
    });
    return () => {
      alive = false;
    };
  }, [count]);

  const pick = async (id: string) => {
    try {
      await flushConfig();
      await api.setActiveAccount(id);
      const st = useStore.getState();
      st.set({ config: { ...st.config, activeAccount: id } });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const remove = async (a: Account) => {
    const ok = await askConfirm({
      title: t('Удалить аккаунт?'),
      text: t('Аккаунт {name} будет удалён с этого компьютера.', { name: a.name }),
      yes: t('Удалить'),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.removeAccount(a.id);
      const list = await api.listAccounts();
      const st = useStore.getState();
      st.set({ accounts: list });
      if (st.config.activeAccount === a.id) st.setConfig({ ...st.config, activeAccount: null });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const add = (p: 'ms' | 'ely' | 'off' | 'lsk') => s.set({ modal: { p }, menu: false });

  return (
    <div className="screen">
      <div className="screen-title">{t('Аккаунты')}</div>
      <div className="screen-sub">{t('Войдите через Microsoft или Ely.by, или играйте без аккаунта.')}</div>

      <div className="acc-list" style={{ marginTop: 18 }}>
        {s.accounts.map((a) => {
          const on = a.id === activeId || (!activeId && s.accounts[0]?.id === a.id);
          const st = status[a.id];
          const expired = st === 'expired';
          return (
            <div className={'acc-card' + (on ? ' on' : '')} key={a.id}>
              <Avatar name={a.name} skin={s.skins[a.id]} size={46} />
              <div className="acc-info">
                <b>{a.name}</b>
                <small>
                  <span className="acc-type">{t(ACCOUNT_LABEL[a.kind])}</span>
                  {a.kind === 'microsoft' && st === 'ok' && <span className="acc-ok">✓ {t('Лицензия')}</span>}
                  {expired && <span className="acc-bad">! {t('Сессия истекла')}</span>}
                </small>
              </div>
              <div className="row g8">
                {expired ? (
                  <button className="btn sm pr" onClick={() => add(a.kind === 'microsoft' ? 'ms' : 'ely')}>
                    {t('Войти заново')}
                  </button>
                ) : (
                  <button className={'btn sm' + (on ? ' acc-on' : ' pr')} disabled={on} onClick={() => void pick(a.id)}>
                    {on ? t('Активный') : t('Использовать')}
                  </button>
                )}
                <button className="btn sm danger" onClick={() => void remove(a)}>
                  {t('Удалить')}
                </button>
              </div>
            </div>
          );
        })}
        {s.accounts.length === 0 && (
          <div className="panel">
            <div className="hint" style={{ padding: 24, textAlign: 'center' }}>
              {t('Аккаунтов пока нет')}
            </div>
          </div>
        )}
      </div>

      <div className="acc-add-title">{t('Добавить аккаунт').toUpperCase()}</div>
      <div className="acc-add">
        <button className="btn" onClick={() => add('ms')}>
          + Microsoft
        </button>
        <button className="btn" onClick={() => add('ely')}>
          + Ely.by
        </button>
        <button className="btn" onClick={() => add('lsk')}>
          + LittleSkin
        </button>
        <button className="btn" onClick={() => add('off')}>
          + {t('Offline-аккаунт')}
        </button>
      </div>
    </div>
  );
}
