// Визард первого запуска: язык → Java → внешний вид → аккаунт.
//
// Показывается, пока в конфиге `onboarded = false` (см. state::load_config:
// флаг сбрасывается, когда файла config.json ещё нет). Первый экран — всегда
// на английском: язык пользователь выбирает прямо здесь.
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { api, type Config } from '../lib/api';
import { onProgress } from '../lib/events';
import { LANGS, normLang, useT } from '../lib/i18n';
import { setConfigLive, useStore } from '../store';
import { ColorPicker, PRESET_COLORS } from './ColorPicker';

/** Шаги визарда по порядку. */
const STEPS = ['lang', 'java', 'look', 'account'] as const;

/** Английские названия языков — подсказка под самоназванием. */
const LANG_EN: Record<string, string> = {
  ru: 'Russian',
  en: 'English',
  pl: 'Polish',
  uk: 'Ukrainian',
  eo: 'Esperanto',
};

/** Способы входа — те же, что в модалке добавления аккаунта. */
const KINDS: { p: 'ms' | 'ely' | 'off' | 'lsk'; label: string; hint: string }[] = [
  { p: 'ms', label: 'Microsoft', hint: 'Вход по коду в браузере. Нужен аккаунт Microsoft с Minecraft.' },
  { p: 'ely', label: 'Ely.by', hint: 'Аккаунт Ely.by — логин и пароль.' },
  { p: 'lsk', label: 'LittleSkin', hint: 'Аккаунт LittleSkin — логин и пароль.' },
  {
    p: 'off',
    label: 'Offline-аккаунт',
    hint: 'Без аккаунта: введите ник и играйте. Серверы доступны, но скина не будет.',
  },
];

const KIND_ICON: Record<'ms' | 'ely' | 'off' | 'lsk', ReactNode> = {
  ms: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="3" y="3" width="8" height="8" />
      <rect x="13" y="3" width="8" height="8" />
      <rect x="3" y="13" width="8" height="8" />
      <rect x="13" y="13" width="8" height="8" />
    </svg>
  ),
  ely: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M12 3 4 6v6c0 4.5 3.4 7.6 8 9 4.6-1.4 8-4.5 8-9V6l-8-3Z" />
      <path d="m9 12 2 2 4-4" />
    </svg>
  ),
  lsk: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4 7h7v7H4zM13 7h7v7h-7zM4 16h7v4H4zM13 16h7v4h-7z" />
    </svg>
  ),
  off: (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="8" r="4" />
      <path d="M4 21c0-4.4 3.6-7 8-7s8 2.6 8 7" />
    </svg>
  ),
};

export function Onboarding() {
  const t = useT();
  const s = useStore();
  const c = s.config;
  const [i, setI] = useState(0);
  const [javaBusy, setJavaBusy] = useState<number | null>(null);
  const [javaPct, setJavaPct] = useState(0);

  // Язык ещё не выбран — знакомство начинаем на английском.
  useEffect(() => {
    const st = useStore.getState();
    if (normLang(st.config.lang) !== 'en') st.setConfig({ ...st.config, lang: 'en' });
  }, []);

  useEffect(() => {
    const st = useStore.getState();
    if (!st.config.dark) st.setConfig({ ...st.config, dark: true });
  }, []);

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

  const step = STEPS[i];
  const set = (patch: Partial<Config>) =>
    useStore.getState().setConfig({ ...useStore.getState().config, ...patch });
  const setAccent = (accent: string) =>
    setConfigLive({ ...useStore.getState().config, accent });
  const next = () => setI((v) => Math.min(STEPS.length - 1, v + 1));
  const back = () => setI((v) => Math.max(0, v - 1));

  /** Завершает знакомство: флаг уезжает в config.json. */
  const finish = () => {
    const st = useStore.getState();
    st.setConfig({ ...st.config, onboarded: true });
  };

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

  const pickWallpaper = async () => {
    try {
      const p = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Wallpaper', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
      });
      if (typeof p !== 'string') return;
      const name = await api.setHomeWallpaper(p);
      set({ homeWallpaper: name });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const clearWallpaper = async () => {
    try {
      await api.clearHomeWallpaper();
      set({ homeWallpaper: null });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const root = s.dataRoot;
  const sep = root.includes('\\') ? '\\' : '/';
  const homeWp =
    c.homeWallpaper && root ? convertFileSrc(`${root}${sep}wallpapers${sep}${c.homeWallpaper}`) : '';

  return (
    <div className="onb">
      <div className="onb-card">
        <div className="onb-head">
          <img src="/launcher.png" alt="" />
          <b>Arbuz Launcher</b>
          <div className="onb-dots" title={`${i + 1} / ${STEPS.length}`}>
            {STEPS.map((k, n) => (
              <i key={k} className={n === i ? 'on' : n < i ? 'done' : ''} />
            ))}
          </div>
        </div>

        <div className="onb-body">
          {step === 'lang' && (
            <>
              <h2>{t('Выберите язык интерфейса')}</h2>
              <p className="hint">{t('Это можно изменить позже в настройках.')}</p>
              <div className="onb-langs">
                {LANGS.map((l) => (
                  <button
                    key={l.id}
                    type="button"
                    className={'onb-lang' + (normLang(c.lang) === l.id ? ' on' : '')}
                    onClick={() => {
                      set({ lang: l.id });
                      next();
                    }}
                  >
                    <b>{l.name}</b>
                    <span className="hint">{LANG_EN[l.id]}</span>
                  </button>
                ))}
              </div>
            </>
          )}

          {step === 'java' && (
            <>
              <h2>{t('Java для запуска Minecraft')}</h2>
              <p className="hint">
                {t('Лаунчер скачает Java сам, а уже установленную найдёт автоматически.')}
              </p>
              <div className="panel" style={{ marginTop: 14 }}>
                {s.javas.map((j, n) => (
                  <div className="lr" key={j.path} style={{ '--i': n } as CSSProperties}>
                    <div className="ck on" style={{ padding: 0 }}>
                      <i style={{ borderRadius: '50%' }} />
                    </div>
                    <div className="d">
                      <b style={{ fontWeight: 500 }}>Java {j.major}</b>{' '}
                      <span className="hint">{j.version}</span>
                      <div className="hint">{j.path}</div>
                    </div>
                  </div>
                ))}
                {s.javas.length === 0 && (
                  <div className="hint" style={{ padding: 16 }}>
                    {t('Java пока не найдена — скачайте её ниже.')}
                  </div>
                )}
              </div>
              <div className="hint" style={{ marginTop: 14 }}>
                {t('Скачать Java')}
              </div>
              <div className="row g8" style={{ marginTop: 6, flexWrap: 'wrap' }}>
                {[8, 17, 21, 25].map((m) => (
                  <button
                    key={m}
                    className={'btn sm' + (m === 21 ? ' pr' : '')}
                    disabled={javaBusy !== null}
                    onClick={() => void installJava(m)}
                  >
                    {javaBusy === m
                      ? `${t('Установка Java {v}…', { v: m })} ${Math.round(javaPct)}%`
                      : `Java ${m}`}
                  </button>
                ))}
                <button className="btn sm" disabled={javaBusy !== null} onClick={() => void findJavas()}>
                  {t('Найти автоматически')}
                </button>
              </div>
              <div className="hint" style={{ marginTop: 8 }}>
                {t('Скачивается JRE Adoptium и распаковывается в папку данных')}
              </div>
            </>
          )}

          {step === 'look' && (
            <>
              <h2>{t('Настройте внешний вид')}</h2>
              <p className="hint">{t('Тема и акцентный цвет применяются сразу.')}</p>

              <label className="l">{t('Акцентный цвет')}</label>
              <div className="accent-row">
                <ColorPicker value={c.accent} onChange={setAccent} title={t('Акцентный цвет')} size={46} />
                <div className="accent-presets" style={{ margin: 0 }}>
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
              </div>

              <label className="l">{t('Обои главной')}</label>
              <div className="wp-row">
                <div className="wp-thumb">
                  {homeWp ? <img src={homeWp} alt="" /> : <span className="hint">{t('По умолчанию')}</span>}
                </div>
                <div className="wp-actions">
                  <button className="btn sm" onClick={() => void pickWallpaper()}>
                    {t('Выбрать обои')}
                  </button>
                  {c.homeWallpaper && (
                    <button className="btn sm" onClick={() => void clearWallpaper()}>
                      {t('Сбросить')}
                    </button>
                  )}
                </div>
              </div>
            </>
          )}

          {step === 'account' && (
            <>
              <h2>{t('Добавьте аккаунт')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {t('Войдите, чтобы играть под своим ником и со скином.')}
              </p>
              <div className="acc-pick">
                {KINDS.map((k) => (
                  <button
                    key={k.p}
                    type="button"
                    className="acc-card"
                    onClick={() => useStore.getState().set({ modal: { p: k.p } })}
                  >
                    <span className={'acc-ic ' + k.p}>{KIND_ICON[k.p]}</span>
                    <span className="acc-tx">
                      <b>{t(k.label)}</b>
                      <span className="hint">{t(k.hint)}</span>
                    </span>
                    <span className="acc-go">→</span>
                  </button>
                ))}
              </div>
              {s.accounts.length > 0 && (
                <p className="hint" style={{ marginTop: 12 }}>
                  {t('Аккаунт добавлен')}
                </p>
              )}
            </>
          )}
        </div>

        <div className="onb-actions">
          {i > 0 && (
            <button className="btn" onClick={back}>
              {t('Назад')}
            </button>
          )}
          <span style={{ flex: 1 }} />
          {step === 'account' ? (
            <>
              <button className="btn" onClick={finish}>
                {t('Пропустить')}
              </button>
              <button className="btn pr" onClick={finish}>
                {t('Готово')}
              </button>
            </>
          ) : step === 'lang' ? null : (
            <>
              <button className="btn" onClick={next}>
                {t('Пропустить')}
              </button>
              <button className="btn pr" onClick={next}>
                {t('Далее')}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
