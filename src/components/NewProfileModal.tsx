// Маленькое окно создания профиля: имя и выбор версии (релиз/снапшот/любая).
import { useState } from 'react';
import { createPortal } from 'react-dom';
import { api } from '../lib/api';
import { BUILTIN_ICONS } from '../lib/builtin';
import { useT } from '../lib/i18n';
import { VERSION_GROUPS } from '../lib/versions';
import { saveInstance, useStore } from '../store';

export function NewProfileModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const s = useStore();
  const [name, setName] = useState('');
  const [version, setVersion] = useState('');
  const [busy, setBusy] = useState(false);

  if (!open) return null;

  const latestRelease = s.versions.find((v) => v.kind === 'release')?.id ?? '';
  const latestSnapshot = s.versions.find((v) => v.kind === 'snapshot')?.id ?? '';
  const picked = version || latestRelease;

  const create = async () => {
    setBusy(true);
    try {
      const nm = name.trim() || t('Новый профиль {n}', { n: s.instances.length + 1 });
      const inst = await api.createInstance(nm, picked || null, 'vanilla');
      const preset = BUILTIN_ICONS.length
        ? BUILTIN_ICONS[Math.floor(Math.random() * BUILTIN_ICONS.length)].id
        : null;
      const withIcon = { ...inst, iconPreset: preset };
      const st = useStore.getState();
      st.set({ instances: [...st.instances, withIcon], sel: withIcon.id });
      saveInstance(withIcon);
      setName('');
      setVersion('');
      onClose();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  return createPortal(
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="mod np-mod">
        <div className="r">
          <h2>{t('Новый профиль')}</h2>

          <label className="l">{t('Название')}</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t('Новый профиль {n}', { n: s.instances.length + 1 })}
          />

          <label className="l">{t('Версия игры')}</label>
          <div className="np-quick">
            <button
              type="button"
              className={'np-card' + (picked === latestRelease ? ' on' : '')}
              onClick={() => setVersion(latestRelease)}
            >
              <b>{t('Последняя версия')}</b>
              <span className="hint">{latestRelease}</span>
            </button>
            <button
              type="button"
              className={'np-card' + (picked === latestSnapshot ? ' on' : '')}
              onClick={() => setVersion(latestSnapshot)}
            >
              <b>{t('Последний снапшот')}</b>
              <span className="hint">{latestSnapshot}</span>
            </button>
          </div>

          <select value={picked} onChange={(e) => setVersion(e.target.value)}>
            {!s.versions.some((v) => v.id === picked) && <option value={picked}>{picked}</option>}
            {VERSION_GROUPS.map(([kind, label]) => {
              const list = s.versions.filter((v) => v.kind === kind);
              if (list.length === 0) return null;
              return (
                <optgroup key={kind} label={t(label)}>
                  {list.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.id}
                    </option>
                  ))}
                </optgroup>
              );
            })}
          </select>

          <div className="row g8" style={{ marginTop: 18 }}>
            <button className="btn pr" disabled={busy || !picked} onClick={() => void create()}>
              {t('Создать профиль')}
            </button>
            <button className="btn" onClick={onClose}>
              {t('Отмена')}
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.getElementById('win') ?? document.body,
  );
}
