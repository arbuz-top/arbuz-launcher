// Добавление и удаление пользовательских версий Minecraft.
import { useState, type MouseEvent } from 'react';
import { createPortal } from 'react-dom';
import { open as openFile } from '@tauri-apps/plugin-dialog';
import { api, type McVersion } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

export function CustomVersionModal({
  open,
  versions,
  onClose,
}: {
  open: boolean;
  versions: McVersion[];
  onClose: () => void;
}) {
  const t = useT();
  const [id, setId] = useState('');
  const [base, setBase] = useState('');
  const [jsonPath, setJsonPath] = useState('');
  const [jarPath, setJarPath] = useState('');
  const [busy, setBusy] = useState(false);

  if (!open) return null;

  const toast = (m: string) => useStore.getState().toastMsg(m);

  const pick = async (kind: 'json' | 'jar') => {
    try {
      const p = await openFile({
        multiple: false,
        directory: false,
        filters:
          kind === 'json'
            ? [{ name: 'version.json', extensions: ['json'] }]
            : [{ name: 'client.jar', extensions: ['jar'] }],
      });
      if (typeof p !== 'string') return;
      if (kind === 'json') setJsonPath(p);
      else setJarPath(p);
    } catch (e) {
      toast(String(e));
    }
  };

  const refresh = async () => {
    useStore.getState().set({ versions: await api.listMcVersions() });
  };

  const add = async () => {
    const vid = id.trim();
    if (!vid) {
      toast(t('Введите id версии'));
      return;
    }
    if (!jsonPath && !base) {
      toast(t('Выберите базовую версию'));
      return;
    }
    setBusy(true);
    try {
      await api.addCustomVersion(vid, base, jsonPath || null, jarPath || null);
      await refresh();
      toast(t('Версия добавлена: {id}', { id: vid }));
      setId('');
      onClose();
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (vid: string) => {
    setBusy(true);
    try {
      await api.removeCustomVersion(vid);
      await refresh();
      toast(t('Версия удалена: {id}', { id: vid }));
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
  };

  const backdrop = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) onClose();
  };

  const customs = versions.filter((v) => v.kind === 'custom');
  const releases = versions.filter((v) => v.kind === 'release');

  return createPortal(
    <div className="ov" onClick={backdrop}>
      <div className="mod" style={{ width: 560 }}>
        <div className="r">
          <h2>{t('Своя версия')}</h2>
          <p className="hint" style={{ marginBottom: 14 }}>
            {t('Наследуется от базовой версии или использует свой version.json.')}
          </p>

          <label className="l">{t('id версии')}</label>
          <input
            type="text"
            value={id}
            onChange={(e) => setId(e.target.value)}
            placeholder="my-custom-1.20.1"
            spellCheck={false}
          />

          <label className="l">{t('Базовая версия')}</label>
          <select value={base} onChange={(e) => setBase(e.target.value)}>
            <option value="">—</option>
            {releases.map((v) => (
              <option key={v.id} value={v.id}>
                {v.id}
              </option>
            ))}
          </select>

          <label className="l">version.json</label>
          <div className="row g8">
            <div style={{ flex: 1 }}>
              <input type="text" readOnly value={jsonPath} placeholder={t('Не выбран')} />
            </div>
            <button className="btn sm" onClick={() => void pick('json')}>
              {t('Выбрать')}
            </button>
          </div>

          <label className="l">client.jar</label>
          <div className="row g8">
            <div style={{ flex: 1 }}>
              <input type="text" readOnly value={jarPath} placeholder={t('Не выбран')} />
            </div>
            <button className="btn sm" onClick={() => void pick('jar')}>
              {t('Выбрать')}
            </button>
          </div>

          <div className="row g8" style={{ marginTop: 18 }}>
            <button className="btn pr" disabled={busy} onClick={() => void add()}>
              {t('Добавить')}
            </button>
            <button className="btn" onClick={onClose}>
              {t('Отмена')}
            </button>
          </div>

          {customs.length > 0 && (
            <>
              <label className="l" style={{ marginTop: 20 }}>
                {t('Мои версии')}
              </label>
              <div className="panel" style={{ padding: '2px 14px' }}>
                {customs.map((v) => (
                  <div className="sw" key={v.id}>
                    <span style={{ fontFamily: 'ui-monospace,Consolas,monospace' }}>{v.id}</span>
                    <button className="btn sm" disabled={busy} onClick={() => void remove(v.id)}>
                      {t('Удалить')}
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>,
    document.getElementById('win') ?? document.body,
  );
}
