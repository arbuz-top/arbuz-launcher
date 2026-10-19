// Вкладка «Скриншоты» профиля: сетка снимков и открытие папки.
import { convertFileSrc } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { api, type Instance, type Shot } from '../lib/api';
import { fmtBytes } from '../lib/format';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

export function ScreenshotsTab({ inst }: { inst: Instance }) {
  const t = useT();
  const root = useStore((s) => s.dataRoot);
  const [shots, setShots] = useState<Shot[]>([]);
  const [busy, setBusy] = useState(true);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      setShots(await api.listScreenshots(inst.id));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [inst.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const openFolder = async () => {
    try {
      await api.openScreenshotsFolder(inst.id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const open = async (file: string) => {
    try {
      await api.openScreenshot(inst.id, file);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const sep = root.includes('\\') ? '\\' : '/';
  const base = `${root}${sep}instances${sep}${inst.id}${sep}minecraft${sep}screenshots${sep}`;

  return (
    <>
      <div className="row g8" style={{ marginBottom: 14 }}>
        <div className="hint" style={{ flex: 1 }}>
          {shots.length > 0 ? t('Скриншотов: {n}', { n: shots.length }) : ''}
        </div>
        <button className="btn sm" onClick={() => void load()}>
          {t('Обновить')}
        </button>
        <button className="btn sm pr" onClick={() => void openFolder()}>
          {t('Открыть папку')}
        </button>
      </div>

      {!busy && shots.length === 0 ? (
        <div className="panel">
          <div className="hint" style={{ padding: 40, textAlign: 'center' }}>
            {t('Скриншотов пока нет. Нажмите F2 в игре.')}
          </div>
        </div>
      ) : (
        <div className="shots">
          {shots.map((sh) => (
            <button
              key={sh.file}
              type="button"
              className="shot"
              onClick={() => void open(sh.file)}
              title={sh.file}
            >
              <img src={convertFileSrc(base + sh.file)} alt={sh.file} loading="lazy" />
              <span className="hint">{fmtBytes(sh.size)}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}
