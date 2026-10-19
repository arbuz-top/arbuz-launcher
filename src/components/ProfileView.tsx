// Страница установки: только настройки (без отдельной вкладки «Играть»).
import { useEffect } from 'react';
import { useT } from '../lib/i18n';
import { LOADER_LABEL } from '../lib/labels';
import { curInstance, loadProfileContent, useStore } from '../store';
import { InstanceSettings } from './InstanceSettings';

export function ProfileView() {
  const s = useStore();
  const t = useT();
  const inst = curInstance(s);
  const instId = inst?.id ?? null;

  // Контент и миры нужны вкладкам внутри настроек.
  useEffect(() => {
    if (!instId) return;
    let alive = true;
    void loadProfileContent(instId).catch((e) => {
      if (alive) useStore.getState().toastMsg(String(e));
    });
    return () => {
      alive = false;
    };
  }, [instId]);

  if (!inst) return null;

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <div className="screen-title">{inst.name}</div>
          <div className="screen-sub">
            {LOADER_LABEL[inst.loader]} {inst.mcVersion}
          </div>
        </div>
        <button className="btn" onClick={() => s.set({ view: 'installs' })}>
          {t('Назад')}
        </button>
      </div>
      <InstanceSettings inst={inst} />
    </div>
  );
}
