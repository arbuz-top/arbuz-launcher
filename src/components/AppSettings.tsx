import { useEffect, useState, type CSSProperties } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { api, type Config } from '../lib/api';
import { onProgress } from '../lib/events';
import { LANGS, normLang, useT } from '../lib/i18n';
import { setConfigLive, useStore } from '../store';
import { AnimSettings } from './AnimSettings';
import { ColorPicker, PRESET_COLORS } from './ColorPicker';
import { Toggle } from './Controls';
import { ThemePicker } from './ThemePicker';

type ToggleKey = 'closeOnLaunch' | 'showConsole' | 'checkUpdates' | 'hideEmpty';

const TOGGLES: [ToggleKey, string][] = [
  ['closeOnLaunch', 'Закрывать лаунчер при запуске игры'],
  ['showConsole', 'Показывать консоль игры'],
  ['checkUpdates', 'Проверять обновления модов'],
  ['hideEmpty', 'Скрывать пустые профили'],
];

export function AppSettings() {
  const s = useStore();
  const t = useT();
  const c = s.config;
  const [apiKey, setApiKey] = useState(c.curseforgeApiKey);
  const set = (patch: Partial<Config>) => s.setConfig({ ...c, ...patch });
  // Акцент меняется перетаскиванием: пишем на диск с задержкой, а не на каждый пиксель.
  const setAccent = (accent: string) => setConfigLive({ ...useStore.getState().config, accent });
  const [javaBusy, setJavaBusy] = useState<number | null>(null);
  const [javaPct, setJavaPct] = useState(0);

  // Прогресс скачивания JRE приходит событием с id "java".
  useEffect(() => {
    let un: (() => void) | null = null;
    let dead = false;
    void onProgress((p) => {
      if (!dead && p.id === 'java') setJavaPct(p.pct);
    }).then((f) => {
      if (dead) f();
      else un = f;
    });
    return () => {
      dead = true;
      un?.();
    };
  }, []);

  const installJava = async (major: number) => {
    setJavaBusy(major);
    setJavaPct(0);
    try {
      await api.installJava(major);
      const list = await api.detectJavas();
      useStore.getState().set({ javas: list });
      useStore.getState().toastMsg(t('Java {v} установлена', { v: major }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setJavaBusy(null);
    }
  };

  const findJavas = async () => {
    try {
      const list = await api.detectJavas();
      const st = useStore.getState();
      st.set({ javas: list });
      st.toastMsg(list.length ? t('Найдено установок Java: {n}', { n: list.length }) : t('Java не найдена'));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const pickHomeWallpaper = async () => {
    try {
      const p = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Wallpaper', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
      });
      if (typeof p !== 'string') return;
      const name = await api.setHomeWallpaper(p);
      const st = useStore.getState();
      st.setConfig({ ...st.config, homeWallpaper: name });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const clearHomeWallpaper = async () => {
    try {
      await api.clearHomeWallpaper();
      const st = useStore.getState();
      st.setConfig({ ...st.config, homeWallpaper: null });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const root = s.dataRoot;
  const sep = root.includes('\\') ? '\\' : '/';
  const homeWp = c.homeWallpaper ? convertFileSrc(`${root}${sep}wallpapers${sep}${c.homeWallpaper}`) : '';

  const openData = async () => {
    try {
      await api.openDataFolder();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  return (
    <>
      <div className="hd">
        <div className="lb">{t('Настройки')}</div>
        <div className="ttl">{t('Внешний вид и поведение')}</div>
        <div className="tabs" />
      </div>
      <div className="pg">
        <div className="row" style={{ alignItems: 'flex-start', gap: 20 }}>
          <div style={{ flex: 1 }}>
            <h2>{t('Акцентный цвет')}</h2>
            <div className="panel" style={{ padding: 20 }}>
              <div className="accent-row">
                <ColorPicker value={c.accent} onChange={setAccent} title={t('Акцентный цвет')} size={46} />
                <div className="accent-meta">
                  <b>{c.accent.toUpperCase()}</b>
                  <span className="hint">{t('Нажмите, чтобы выбрать цвет')}</span>
                </div>
                <span style={{ flex: 1 }} />
                <button className="btn sm" onClick={() => setAccent('#5fb339')}>
                  {t('Сбросить')}
                </button>
              </div>
              <div className="accent-presets">
                {PRESET_COLORS.map((col) => (
                  <button
                    key={col}
                    type="button"
                    className={'cp-swatch' + (c.accent.toLowerCase() === col ? ' on' : '')}
                    style={{ background: col }}
                    title={col}
                    onClick={() => setAccent(col)}
                  />
                ))}
              </div>
              <div className="hint" style={{ margin: '10px 0 16px' }}>
                {c.style === 'monet'
                  ? t('Из выбранного цвета строится вся палитра Monet: фоны, карточки, кнопки и заставки.')
                  : c.style === 'catppuccin'
                    ? t('Catppuccin использует официальную палитру Mocha или Latte — выбранный цвет на неё не влияет.')
                    : c.style === 'amoled'
                      ? t('AMOLED: чистый чёрный фон. Цвет применяется к кнопке «Играть», выбранным пунктам и переключателям.')
                      : t('Цвет применяется к кнопке «Играть», выбранным пунктам и переключателям.')}
              </div>
              <div className="row g8">
                <button className="btn pr">{t('Кнопка')}</button>
                <button className="btn">{t('Вторичная')}</button>
                <div className="tg on" />
                <div className="ck on" style={{ width: 'auto' }}>
                  <i />
                </div>
              </div>
            </div>

            <h2>{t('Обои главной')}</h2>
            <div className="panel" style={{ padding: 16 }}>
              <div className="wp-row">
                <div className="wp-thumb">
                  {homeWp ? <img src={homeWp} alt="" /> : <span className="hint">{t('По умолчанию')}</span>}
                </div>
                <div className="wp-actions">
                  <button className="btn sm" onClick={() => void pickHomeWallpaper()}>
                    {t('Выбрать обои')}
                  </button>
                  {c.homeWallpaper && (
                    <button className="btn sm" onClick={() => void clearHomeWallpaper()}>
                      {t('Сбросить')}
                    </button>
                  )}
                </div>
              </div>
              <div className="hint" style={{ marginTop: 10 }}>
                {t('PNG, JPG или WEBP. Обои показываются на главной.')}
              </div>
            </div>

            {c.experimentalThemes && <ThemePicker />}

            <h2>{t('Java по умолчанию')}</h2>
            <div className="panel">
              {s.javas.map((j, n) => (
                <div
                  className="lr"
                  key={j.path}
                  style={{ cursor: 'pointer', alignItems: 'center', '--i': n } as CSSProperties}
                  onClick={() => set({ defaultJava: j.major })}
                >
                  <div className={'ck' + (c.defaultJava === j.major ? ' on' : '')} style={{ padding: 0 }}>
                    <i style={{ borderRadius: '50%' }} />
                  </div>
                  <div className="d">
                    <b style={{ fontWeight: 500 }}>Java {j.major}</b> <span className="hint">{j.version}</span>
                    <div className="hint">{j.path}</div>
                  </div>
                </div>
              ))}
              {s.javas.length === 0 && (
                <div className="hint" style={{ padding: 16 }}>
                  {t('Java не найдена. Нажмите «Найти автоматически».')}
                </div>
              )}
            </div>
            <div style={{ marginTop: 10 }}>
              <button className="btn sm" onClick={() => void findJavas()}>
                {t('Найти автоматически')}
              </button>
            </div>

            <div className="hint" style={{ marginTop: 18 }}>{t('Скачать Java')}</div>
            <div className="row g8" style={{ marginTop: 6, flexWrap: 'wrap' }}>
              {[8, 17, 21, 25].map((m) => (
                <button
                  key={m}
                  className="btn sm"
                  disabled={javaBusy !== null}
                  onClick={() => void installJava(m)}
                >
                  {javaBusy === m
                    ? `${t('Установка Java {v}…', { v: m })} ${Math.round(javaPct)}%`
                    : `Java ${m}`}
                </button>
              ))}
            </div>
            <div className="hint" style={{ marginTop: 6 }}>
              {t('Скачивается JRE Adoptium и распаковывается в папку данных')}
            </div>
          </div>

          <div style={{ width: 320, flex: 'none' }}>
            <h2>{t('Язык')}</h2>
            <label className="l" style={{ marginTop: 0 }}>
              {t('Язык интерфейса')}
            </label>
            <select value={normLang(c.lang)} onChange={(e) => set({ lang: e.target.value })}>
              {LANGS.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>

            <h2>{t('Лаунчер')}</h2>
            <div className="panel" style={{ padding: '6px 18px' }}>
              {TOGGLES.map(([key, label]) => (
                <div className="sw" key={key}>
                  <span>{t(label)}</span>
                  <Toggle on={c[key]} onClick={() => set({ [key]: !c[key] } as Partial<Config>)} />
                </div>
              ))}
            </div>
            <label className="l" style={{ marginTop: 18 }}>
              {t('API-ключ CurseForge')}
            </label>
            <input
              type="text"
              value={apiKey}
              placeholder={t('необязательно')}
              onChange={(e) => setApiKey(e.target.value)}
              onBlur={() => set({ curseforgeApiKey: apiKey })}
            />
            <div className="hint" style={{ marginTop: 6 }}>
              {t('Без ключа каталог CurseForge работает через публичный прокси')}
            </div>
            <label className="l" style={{ marginTop: 18 }}>
              {t('Папка данных')}
            </label>
            <div className="row g8">
              <div style={{ flex: 1 }}>
                <input type="text" value={s.dataRoot} readOnly />
              </div>
              <button className="btn sm" onClick={() => void openData()}>
                {t('Открыть')}
              </button>
            </div>
          </div>
        </div>

        <h2 style={{ marginTop: 28 }}>{t('Экспериментальные')}</h2>
        <div className="panel" style={{ padding: '6px 18px', maxWidth: 660 }}>
          <div className="sw">
            <span>{t('Включить темы')}</span>
            <Toggle on={c.experimentalThemes} onClick={() => set({ experimentalThemes: !c.experimentalThemes, ...(c.experimentalThemes ? { style: 'flat' } : {}) })} />
          </div>
          <div className="sw">
            <span>{t('Включить анимации')}</span>
            <Toggle on={c.anim.enabled} onClick={() => set({ anim: { ...c.anim, enabled: !c.anim.enabled } })} />
          </div>
          <div className="sw">
            <span>{t('ИИ-ассистент сборок')}</span>
            <Toggle on={c.aiBuilder} onClick={() => set({ aiBuilder: !c.aiBuilder })} />
          </div>
          <div className="hint" style={{ paddingBottom: 12 }}>
            {t('Кнопка «ИИ-сборка» в «Установках»: ассистент подбирает моды, ресурс-паки и шейдеры по описанию и сам скачивает их в новый профиль.')}
          </div>
          <div className="sw">
            <span>{t('Включить изоляцию')}</span>
            <Toggle on={c.modIsolationEnabled} onClick={() => set({ modIsolationEnabled: !c.modIsolationEnabled })} />
          </div>
          <div className="hint" style={{ paddingBottom: 12 }}>
            {t('Игра запускается в песочнице: мод не может писать за пределами папки профиля. На стандартном и жёстком уровнях он ещё и не может запускать другие программы. Буфер обмена не ограничивается.')}
          </div>
          <div className="seg" style={{ marginBottom: 12, opacity: c.modIsolationEnabled ? 1 : 0.45 }}>
            {(['min', 'std', 'strict'] as const).map((lv) => (
              <button
                key={lv}
                type="button"
                disabled={!c.modIsolationEnabled}
                className={c.modIsolationLevel === lv ? 'on' : ''}
                onClick={() => set({ modIsolationLevel: lv })}
              >
                {t(lv === 'min' ? 'Минимальная' : lv === 'std' ? 'Стандартная' : 'Жёсткая')}
              </button>
            ))}
          </div>
          <div className="hint" style={{ paddingBottom: 4 }}>
            {t('Минимальная: игра в песочнице — не может писать за пределами папки профиля. Стандартная: ещё и не может запускать другие программы.')}
          </div>
          <div className="hint" style={{ paddingBottom: 12 }}>
            {t('Жёсткая: то же плюс отдельные HOME/APPDATA/TEMP внутри профиля.')}
          </div>
        </div>
        <div className="hint" style={{ marginTop: 6 }}>
          {t('По умолчанию доступна только плоская тема.')}
        </div>

        {c.anim.enabled && (
          <>
            <h2 style={{ marginTop: 24 }}>{t('Анимации')}</h2>
            <AnimSettings />
          </>
        )}
      </div>
    </>
  );
}
