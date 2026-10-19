// Главный экран: заставка и популярные сборки с выбором источника.
// Смена обоев живёт только в настройках — на Главной их не трогаем.
import { useEffect, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { api, type CatalogItem } from '../lib/api';
import { useT } from '../lib/i18n';
import { BUILTIN_WALLPAPERS, builtinWallpaper } from '../lib/builtin';
import { MaterialIcon } from '../lib/material-icons';
import { monetLike } from '../lib/theme';
import { curInstance, useStore } from '../store';

function fmtCount(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'K';
  return String(n);
}

/** Метка на карточке сборки: загрузчик (кроме Vanilla) или первая категория. */
function badge(p: CatalogItem): string {
  const ld = p.loaders.find((l) => l.toLowerCase() !== 'vanilla');
  const raw = ld || p.categories[0] || '';
  return raw ? raw.charAt(0).toUpperCase() + raw.slice(1) : '';
}

type Source = 'modrinth' | 'curseforge';

export function Home() {
  const s = useStore();
  const t = useT();
  const inst = curInstance(s);
  const monet = monetLike(s.config.style);
  const [source, setSource] = useState<Source>('modrinth');
  const [packs, setPacks] = useState<CatalogItem[]>([]);

  const dataRoot = s.dataRoot;
  const sep = dataRoot.includes('\\') ? '\\' : '/';
  const custom = s.config.homeWallpaper;
  const customUrl = custom ? convertFileSrc(`${dataRoot}${sep}wallpapers${sep}${custom}`) : '';
  const hero =
    customUrl ||
    (inst?.wallpaper ? builtinWallpaper(inst.wallpaper) : (BUILTIN_WALLPAPERS[0]?.url ?? ''));

  useEffect(() => {
    let alive = true;
    setPacks([]);
    api
      .searchCatalog({
        source,
        kind: 'pk',
        query: '',
        loaders: [],
        mcVersion: null,
        categories: [],
        offset: 0,
        sort: 'downloads',
        instanceId: null,
      })
      .then((r) => {
        if (alive) setPacks(r.hits.slice(0, 8));
      })
      .catch(() => {
        // без сети просто не показываем подборку
      });
    return () => {
      alive = false;
    };
  }, [source]);

  return (
    <div>
      <div className="hero2">
        {hero ? <img src={hero} alt="" /> : null}
        <div className="hero-fade" />
      </div>

      <div className="home-body">
        <div className="home-head">
          <div className="home-tag">
            {monet ? (
              <MaterialIcon name="local_fire_department" size={18} />
            ) : (
              <svg width="15" height="15" viewBox="0 -2.5 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round" style={{ display: 'block', flex: 'none' }} aria-hidden="true">
                <path d="M12 3c1 3 4 4.5 4 9a4 4 0 0 1-8 0c0-2 1-3 2-4 0 2 1 3 2 3 0-3-1-5 0-8z" />
              </svg>
            )}
            {t('Популярные сборки').toUpperCase()}
          </div>

          <div className="src-toggle">
            <button
              type="button"
              className={'src-btn' + (source === 'modrinth' ? ' on' : '')}
              onClick={() => setSource('modrinth')}
            >
              Modrinth
            </button>
            <button
              type="button"
              className={'src-btn' + (source === 'curseforge' ? ' on' : '')}
              onClick={() => setSource('curseforge')}
            >
              CurseForge
            </button>
          </div>

          <a
            className="home-more"
            onClick={() => useStore.getState().set({ view: 'browse', bt: 'pk', src: source === 'modrinth' ? 'Modrinth' : 'CurseForge', menu: false })}
          >
            {t('Открыть больше')}
          </a>
        </div>

        {packs.length > 0 && (
          <div className="packs-grid">
            {packs.map((p) => (
              <div
                className="pack-card"
                key={p.id}
                onClick={() => useStore.getState().set({ view: 'browse', bt: 'pk', bq: p.title, src: source === 'modrinth' ? 'Modrinth' : 'CurseForge', menu: false })}
              >
                <div className="pack-cover">
                  {badge(p) && <div className="pack-badge">{badge(p)}</div>}
                </div>
                <div className="pack-body">
                  <div className="pack-ico">
                    {p.iconUrl ? <img src={p.iconUrl} alt="" loading="lazy" /> : p.title.charAt(0)}
                  </div>
                  <div className="pack-name">{p.title}</div>
                  <div className="pack-desc">{p.description}</div>
                  <div className="pack-meta">
                    <span>
                      <svg viewBox="0 0 24 24" aria-hidden="true">
                        <path d="M12 4v11M7 10l5 5 5-5M5 20h14" />
                      </svg>
                      {fmtCount(p.downloads)}
                    </span>
                    <span>{p.source === 'curseforge' ? 'CurseForge' : 'Modrinth'}</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
