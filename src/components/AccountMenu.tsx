// Выпадающее меню аккаунтов (.pop из макета, строка ovl())
import { type CSSProperties } from 'react';
import { ACCOUNT_LABEL } from '../lib/labels';
import { api } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore, askConfirm, flushConfig } from '../store';
import { Avatar } from './Controls';

export function AccountMenu() {
  const open = useStore((s) => s.menu);
  const accounts = useStore((s) => s.accounts);
  const skins = useStore((s) => s.skins);
  const activeId = useStore((s) => s.config.activeAccount);
  const t = useT();

  if (!open) return null;

  const pick = async (id: string) => {
    try {
      // Отложенные настройки должны лечь на диск до смены аккаунта, иначе
      // старый снимок config.json может вернуть прежний activeAccount.
      await flushConfig();
      await api.setActiveAccount(id);
      const st = useStore.getState();
      st.set({ menu: false, config: { ...st.config, activeAccount: id } });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  // «Выход»: удаляем активный аккаунт вместе с его токенами в keyring.
  const logout = async () => {
    const acc = accounts.find((a) => a.id === activeId) ?? accounts[0];
    if (!acc) return;
    const ok = await askConfirm({
      title: t('Выйти из аккаунта?'),
      text: t('Аккаунт {name} и сохранённые токены будут удалены с этого компьютера.', { name: acc.name }),
      yes: t('Выйти'),
      danger: true,
    });
    if (!ok) return;
    try {
      await flushConfig();
      await api.removeAccount(acc.id);
      const list = await api.listAccounts();
      const st = useStore.getState();
      st.set({
        accounts: list,
        menu: false,
        config: { ...st.config, activeAccount: list[0]?.id ?? null },
      });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  return (
    <>
      <div
        style={{ position: 'absolute', inset: 0, zIndex: 5 }}
        onClick={() => useStore.getState().set({ menu: false })}
      />
      <div className="pop">
        {accounts.map((a, i) => (
          <div className="it" key={a.id} style={{ '--i': i } as CSSProperties} onClick={() => void pick(a.id)}>
            <Avatar name={a.name} skin={skins[a.id]} size={32} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <b style={{ fontWeight: 500 }}>{a.name}</b>
              <div className="hint">{t(ACCOUNT_LABEL[a.kind])}</div>
            </div>
            {a.id === activeId && (
              <span className="hint" style={{ color: 'var(--pri)', fontWeight: 600 }}>
                {t('Активный')}
              </span>
            )}
          </div>
        ))}
        {accounts.length === 0 && (
          <div className="hint" style={{ padding: '10px 12px' }}>
            {t('Аккаунтов пока нет')}
          </div>
        )}
        <div
          className="it"
          style={{ color: 'var(--pri)', fontWeight: 500 }}
          onClick={() => useStore.getState().set({ menu: false, modal: { p: 'pick' } })}
        >
          {'+ '}{t('Добавить аккаунт')}
        </div>
        {accounts.length > 0 && (
          <div className="it" style={{ color: 'var(--onv)', fontWeight: 500 }} onClick={() => void logout()}>
            {t('Выйти')}
          </div>
        )}
      </div>
    </>
  );
}
