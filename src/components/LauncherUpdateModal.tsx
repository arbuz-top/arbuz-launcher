// Окно «вышло обновление лаунчера»: версия, что нового и кнопка скачивания.
import { openUrl } from '@tauri-apps/plugin-opener';
import { useT } from '../lib/i18n';
import { Markdown } from '../lib/markdown';
import { useStore } from '../store';

export function LauncherUpdateModal() {
  const upd = useStore((s) => s.appUpdate);
  const t = useT();
  if (!upd) return null;

  const close = () => useStore.getState().set({ appUpdate: null });
  const open = (url: string) => {
    if (!url) return;
    void openUrl(url).catch((e) => useStore.getState().toastMsg(String(e)));
  };

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="mod">
        <div className="r">
          <div className="con-h">
            <b style={{ flex: 1, fontWeight: 600 }}>{t('Обновление лаунчера')}</b>
            <button className="btn sm" onClick={close}>
              {t('Позже')}
            </button>
          </div>
          <p className="hint" style={{ marginBottom: 10 }}>
            {t('Вышла версия {v}, у вас {c}.', { v: upd.version, c: upd.current })}
          </p>
          {upd.notes && (
            <div className="pk-log">
              <Markdown text={upd.notes} />
            </div>
          )}
          <div className="row g8" style={{ marginTop: 14, justifyContent: 'flex-end' }}>
            {upd.page && (
              <button className="btn sm" onClick={() => open(upd.page)}>
                {t('Что нового')}
              </button>
            )}
            <button className="btn pr" onClick={() => open(upd.download)}>
              {t('Скачать обновление')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
