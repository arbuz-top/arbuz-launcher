// Редактор своей темы: цвета, размеры текста и кнопок, ширина сайдбара.
// Всё применяется сразу — за окном редактора видно настоящий лаунчер.
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import {
  DEFAULT_CUSTOM,
  applyTheme,
  getRoles,
  normCustom,
  type CustomColor,
  type CustomTheme,
} from '../lib/theme';
import { setConfigLive, useStore } from '../store';
import { useT } from '../lib/i18n';
import { ColorPicker } from './ColorPicker';
import { Toggle } from './Controls';

const COLORS: [CustomColor, string][] = [
  ['bg', 'Фон окна'],
  ['sbg', 'Сайдбар'],
  ['s1', 'Панели'],
  ['s2', 'Поля'],
  ['s3', 'Кнопки'],
  ['on', 'Текст'],
  ['onv', 'Тусклый текст'],
  ['ln', 'Границы'],
  ['pri', 'Акцент'],
  ['sec', 'Второй акцент'],
  ['ter', 'Третий акцент'],
];

const pct = (v: number, lo: number, hi: number): string =>
  `${((v - lo) / (hi - lo)) * 100}%`;

/** Макет окна лаунчера, который повторяет все настройки темы. */
function Mock({ c }: { c: CustomTheme }) {
  const t = useT();
  const r = getRoles('custom', true, c.pri, c);
  const fs = c.fontScale;
  const ctl = c.control;
  const den = c.density;
  const rad = Math.max(2, c.radius);
  const sb = Math.round(c.sidebar * 0.34);
  return (
    <div className="te-mock" style={{ background: r.bg, color: r.on, borderRadius: rad + 4 }}>
      <div className="te-mock-tb" style={{ background: r.sbg, fontSize: 9 * fs, color: r.onv }}>
        <b style={{ color: r.on }}>Arbuz</b>
        <span>— ▢ ×</span>
      </div>
      <div className="te-mock-body" style={{ gridTemplateColumns: `${sb}px 1fr` }}>
        <div className="te-mock-sb" style={{ background: r.sbg, padding: 6 * den, gap: 4 * den }}>
          <div className="te-mock-av" style={{ background: r.pric, width: 16, height: 16 }} />
          {[1, 2, 3].map((n) => (
            <div
              key={n}
              style={{
                background: n === 1 ? r.s2 : r.s3,
                height: Math.max(10, 14 * den),
                borderRadius: Math.max(2, rad * 0.6),
                opacity: n === 1 ? 1 : 0.65,
              }}
            />
          ))}
        </div>
        <div className="te-mock-mn" style={{ padding: 10 * den }}>
          <div style={{ fontSize: 13 * fs, fontWeight: 600 }}>{t('Новый профиль')}</div>
          <div style={{ fontSize: 8 * fs, color: r.onv, marginTop: 2 }}>1.21.1 · Fabric</div>
          <div
            className="te-mock-play"
            style={{
              background: r.pri,
              color: r.onpri,
              height: Math.max(18, 26 * ctl),
              width: `${Math.min(88, 62 * ctl)}%`,
              fontSize: 10 * fs,
              borderRadius: Math.max(2, rad * 0.8),
              marginTop: 10 * den,
            }}
          >
            {t('Играть')}
          </div>
          <div className="te-mock-tiles" style={{ gap: 5 * den, marginTop: 10 * den }}>
            {[r.s1, r.s2, r.s3].map((bg, n) => (
              <div
                key={n}
                style={{ background: bg, borderRadius: Math.max(2, rad * 0.7), height: Math.max(18, 28 * den) }}
              />
            ))}
          </div>
          <div style={{ display: 'flex', gap: 5 * den, marginTop: 8 * den }}>
            <span style={{ background: r.ter, height: 4, width: '30%', borderRadius: 2 }} />
            <span style={{ background: r.ln, height: 4, flex: 1, borderRadius: 2 }} />
          </div>
        </div>
      </div>
    </div>
  );
}

export function ThemeEditor({ open, onClose }: { open: boolean; onClose: () => void }) {
  const cfg = useStore((s) => s.config);
  const t = useT();
  const [draft, setDraft] = useState<CustomTheme>(() => normCustom(cfg.custom));
  const draftRef = useRef(draft);
  const timer = useRef<number | undefined>(undefined);
  const c = draft;

  // При открытии подхватываем сохранённую тему (её могли изменить в другом месте).
  useEffect(() => {
    if (!open) return;
    const cur = normCustom(useStore.getState().config.custom);
    draftRef.current = cur;
    setDraft(cur);
  }, [open]);

  const commit = (next: CustomTheme) => {
    const st = useStore.getState();
    setConfigLive({ ...st.config, custom: next });
  };

  const schedule = (next: CustomTheme) => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      timer.current = undefined;
      commit(next);
    }, 260);
  };

  const close = () => {
    if (timer.current) {
      window.clearTimeout(timer.current);
      timer.current = undefined;
      commit(draftRef.current);
    }
    onClose();
  };

  // Страховка: редактор закрыли навигацией — не теряем последнее изменение.
  useEffect(
    () => () => {
      if (timer.current) {
        window.clearTimeout(timer.current);
        timer.current = undefined;
        commit(draftRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        close();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;

  // Палитра шлёт события десятки раз в секунду: красим окно напрямую и
  // откладываем запись в стор, иначе весь лаунчер перерисовывается на каждый пиксель.
  const save = (patch: Partial<CustomTheme>) => {
    const next = normCustom({ ...draftRef.current, ...patch });
    draftRef.current = next;
    setDraft(next);
    const el = document.getElementById('win');
    if (el) {
      const st = useStore.getState();
      applyTheme(el, 'custom', st.config.dark, st.config.accent, next);
    }
    if ('soft' in patch) {
      // Переключатель формы коммитим сразу — но снимаем отложенный снимок,
      // иначе он вернёт старое значение через доли секунды.
      if (timer.current) {
        window.clearTimeout(timer.current);
        timer.current = undefined;
      }
      commit(next);
    } else {
      schedule(next);
    }
  };

  return createPortal(
    <div
      className="ov te-ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="te-mod" role="dialog" aria-modal="true" aria-label={t('Своя тема')}>
        <div className="te-ctl">
          <div className="te-head">
            <b>{t('Своя тема')}</b>
            <span className="hint">{t('Изменения применяются сразу ко всему лаунчеру')}</span>
          </div>

          <div className="te-scroll">
            <h2>{t('Цвета')}</h2>
            <div className="te-colors">
              {COLORS.map(([k, label]) => (
                <div className="te-color" key={k}>
                  <span>{t(label)}</span>
                  <ColorPicker
                    value={c[k]}
                    size={28}
                    title={t(label)}
                    onChange={(hex) => save({ [k]: hex } as Partial<CustomTheme>)}
                  />
                  <code>{c[k]}</code>
                </div>
              ))}
            </div>

            <h2>{t('Форма')}</h2>
            <div className="te-row">
              <span className="hint">{t('Скругление')}</span>
              <input
                type="range"
                min={0}
                max={26}
                step={1}
                value={c.radius}
                style={{ '--p': pct(c.radius, 0, 26) } as CSSProperties}
                onChange={(e) => save({ radius: +e.target.value })}
              />
              <b>{Math.round(c.radius)} px</b>
            </div>
            <div className="sw">
              <span>{t('Мягкие формы')}</span>
              <Toggle on={c.soft} onClick={() => save({ soft: !c.soft })} />
            </div>

            <h2>{t('Шрифт')}</h2>
            <input
              type="text"
              list="te-fonts"
              value={c.font}
              placeholder={t('Стандартный шрифт темы')}
              spellCheck={false}
              onChange={(e) => save({ font: e.target.value })}
            />
            <datalist id="te-fonts">
              {['Inter', 'Roboto', 'Segoe UI', 'Arial', 'Georgia', 'Verdana', 'Comic Sans MS', 'Consolas', 'Times New Roman'].map(
                (f) => <option key={f} value={f} />,
              )}
            </datalist>
            <div className="hint" style={{ marginTop: 5 }}>
              {t('Любой шрифт, установленный в Windows. Пусто — стандартный для темы.')}
            </div>

            <h2>{t('Размеры')}</h2>
            <div className="te-row">
              <span className="hint">{t('Текст')}</span>
              <input
                type="range"
                min={0.85}
                max={1.3}
                step={0.01}
                value={c.fontScale}
                style={{ '--p': pct(c.fontScale, 0.85, 1.3) } as CSSProperties}
                onChange={(e) => save({ fontScale: +e.target.value })}
              />
              <b>{Math.round(c.fontScale * 100)}%</b>
            </div>
            <div className="te-row">
              <span className="hint">{t('Кнопки')}</span>
              <input
                type="range"
                min={0.8}
                max={1.3}
                step={0.01}
                value={c.control}
                style={{ '--p': pct(c.control, 0.8, 1.3) } as CSSProperties}
                onChange={(e) => save({ control: +e.target.value })}
              />
              <b>{Math.round(c.control * 100)}%</b>
            </div>
            <div className="te-row">
              <span className="hint">{t('Отступы')}</span>
              <input
                type="range"
                min={0.85}
                max={1.3}
                step={0.01}
                value={c.density}
                style={{ '--p': pct(c.density, 0.85, 1.3) } as CSSProperties}
                onChange={(e) => save({ density: +e.target.value })}
              />
              <b>{Math.round(c.density * 100)}%</b>
            </div>
            <div className="te-row">
              <span className="hint">{t('Сайдбар')}</span>
              <input
                type="range"
                min={190}
                max={320}
                step={2}
                value={c.sidebar}
                style={{ '--p': pct(c.sidebar, 190, 320) } as CSSProperties}
                onChange={(e) => save({ sidebar: +e.target.value })}
              />
              <b>{Math.round(c.sidebar)} px</b>
            </div>
          </div>
        </div>

        <aside className="te-side">
          <Mock c={c} />
          <div className="hint te-note">
            {t('Макет повторяет цвета, размер текста, кнопки и ширину сайдбара. Настоящее окно меняется синхронно.')}
          </div>
          <button
            type="button"
            className="btn te-move-btn"
            onClick={() => {
              close();
              useStore.getState().setMove(true);
            }}
          >
            {t('Расставить кнопки')}
          </button>
          <div className="te-actions">
            <button type="button" className="btn" onClick={() => save(DEFAULT_CUSTOM)}>
              {t('Сбросить')}
            </button>
            <button type="button" className="btn pr" onClick={close}>
              {t('Готово')}
            </button>
          </div>
        </aside>
      </div>
    </div>,
    document.getElementById('win') ?? document.body,
  );
}
