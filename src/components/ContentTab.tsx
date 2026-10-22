// Вкладка «Контент» профиля (vMods + cl из макета)
import { convertFileSrc } from '@tauri-apps/api/core';
import { memo, useCallback, useState, type CSSProperties } from 'react';
import { pi } from '../lib/art';
import { api, type ContentItem, type Instance } from '../lib/api';
import { fmtBytes } from '../lib/format';
import { useT } from '../lib/i18n';
import { LOADER_LABEL } from '../lib/labels';
import { curAccount, iconUrl, loadProfileContent, useArtTheme, useStore } from '../store';
import { Avatar, Toggle } from './Controls';

const KIND_LABEL: Record<string, string> = {
  mod: 'Мод',
  rp: 'Ресурспак',
  sh: 'Шейдер',
  dp: 'Датапак',
};

const FILTERS: [string, string][] = [
  ['mod', 'Моды'],
  ['rp', 'Ресурспаки'],
  ['sh', 'Шейдеры'],
  ['dp', 'Датапаки'],
];

/** Иконка контента: из архива, иначе с CDN Modrinth, иначе первая буква. */
export const ModIcon = memo(function ModIcon({ item, size }: { item: ContentItem; size: number }) {
  const src = item.icon ? convertFileSrc(item.icon) : item.iconUrl;
  if (!src) {
    const ch = (item.name || '?').trim().charAt(0).toUpperCase() || '?';
    return (
      <div className="mi" style={{ width: size, height: size, fontSize: Math.round(size * 0.42) }}>
        {ch}
      </div>
    );
  }
  return (
    <img
      className="mi"
      src={src}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      style={{ width: size, height: size, objectFit: 'cover' }}
    />
  );
});

export function ContentTab({
  inst,
  onOpenConfigs,
}: {
  inst: Instance;
  /** Переход на вкладку «Сборка» с фильтром конфигов этого мода. */
  onOpenConfigs?: (modName: string) => void;
}) {
  const cq = useStore((s) => s.cq);
  const flt = useStore((s) => s.flt);
  const items = useStore((s) => s.content);
  const instances = useStore((s) => s.instances);
  const dataRoot = useStore((s) => s.dataRoot);
  const skins = useStore((s) => s.skins);
  const bust = useStore((s) => s.iconBust[inst.id]);
  const theme = useArtTheme();
  const acc = curAccount(useStore.getState());
  const t = useT();
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      await loadProfileContent(inst.id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  }, [inst.id]);

  const run = useCallback(
    async (fn: () => Promise<void>) => {
      setBusy(true);
      try {
        await fn();
        await reload();
      } catch (e) {
        useStore.getState().toastMsg(String(e));
      } finally {
        setBusy(false);
      }
    },
    [reload],
  );

  const toBrowse = () =>
    useStore.getState().set({ view: 'browse', tgt: inst.id, bt: flt === 'all' ? 'mod' : flt });

  const q = cq.toLowerCase();
  const shown = items.filter((m) => (flt === 'all' || m.kind === flt) && m.name.toLowerCase().includes(q));

  return (
    <>
      <div className="ct-head">
        <span className="ct-ico">
          {inst.icon || inst.iconPreset ? (
            pi(
              {
                biome: inst.biome,
                icon: iconUrl(dataRoot, inst.id, inst.icon, bust),
                preset: inst.iconPreset,
              },
              40,
              0,
              theme,
            )
          ) : (
            <Avatar name={acc?.name ?? '?'} skin={acc ? skins[acc.id] : null} size={40} />
          )}
        </span>
        <span className="ct-info">
          <b>{inst.name}</b>
          <small>
            {LOADER_LABEL[inst.loader]} {inst.mcVersion}
          </small>
        </span>
        <select
          className="ct-sel"
          value={inst.id}
          onChange={(e) => useStore.getState().set({ sel: e.target.value })}
        >
          {instances.map((x) => (
            <option key={x.id} value={x.id}>
              {x.name}
            </option>
          ))}
        </select>
      </div>

      <div className="row g8" style={{ marginBottom: 14 }}>
        <div style={{ flex: 1 }}>
          <input
            type="text"
            placeholder={t('Поиск по установленному…')}
            value={cq}
            onChange={(e) => useStore.getState().set({ cq: e.target.value })}
          />
        </div>
        <div className="seg">
          {FILTERS.map(([k, l]) => (
            <button
              key={k}
              type="button"
              className={flt === k ? 'on' : ''}
              onClick={() => useStore.getState().set({ flt: k })}
            >
              {t(l)}
            </button>
          ))}
        </div>
        <button className="btn pr" onClick={toBrowse}>
          {t('Добавить')}
        </button>
      </div>
      <div className="panel">
        {shown.length > 0 ? (
          <table>
            <thead>
              <tr>
                <th style={{ width: 64 }} />
                <th>{t('Название')}</th>
                <th>{t('Тип')}</th>
                <th>{t('Версия')}</th>
                <th>{t('Размер')}</th>
                <th style={{ width: 220 }} />
              </tr>
            </thead>
            <tbody>
              {shown.map((m, n) => (
                <tr key={m.file} style={{ '--i': n } as CSSProperties}>
                  <td>
                    <Toggle on={m.enabled} onClick={() => void run(() => api.toggleContent(inst.id, m.file, !m.enabled))} />
                  </td>
                  <td>
                    <div className="row g8">
                      <ModIcon item={m} size={32} />
                      <b style={{ fontWeight: 500, opacity: m.enabled ? 1 : 0.45 }}>{m.name}</b>
                    </div>
                  </td>
                  <td>
                    <span className="lab">{t(KIND_LABEL[m.kind] ?? m.kind)}</span>
                  </td>
                  <td className="hint">{m.version}</td>
                  <td className="hint">{fmtBytes(m.sizeBytes)}</td>
                  <td>
                    <div className="row g8" style={{ justifyContent: 'flex-end' }}>
                      {m.kind === 'mod' && onOpenConfigs && (
                        <button
                          className="btn sm"
                          disabled={busy}
                          title={t('Открыть конфиги этого мода')}
                          onClick={() => onOpenConfigs(m.name)}
                        >
                          {t('Конфиги')}
                        </button>
                      )}
                      {m.kind === 'mod' && (
                        <button
                          className={'btn sm' + (m.isolated ? ' pr' : '')}
                          disabled={busy}
                          title={t('Изолировать этот мод')}
                          onClick={() => void run(() => api.toggleIsolation(inst.id, m.file, !m.isolated))}
                        >
                          {m.isolated ? t('Изолирован') : t('Изолировать')}
                        </button>
                      )}
                      <button className="btn sm" disabled={busy} onClick={() => void run(() => api.removeContent(inst.id, m.file))}>
                        {t('Удалить')}
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <div className="hint" style={{ padding: 40, textAlign: 'center' }}>
            {t('Здесь пока пусто.')}{' '}
            <a style={{ color: 'var(--pri)', cursor: 'pointer' }} onClick={toBrowse}>
              {t('Найти в каталоге')}
            </a>
          </div>
        )}
      </div>
    </>
  );
}
