import { memo, useMemo, useState, type CSSProperties } from 'react';
import { getRoles, softOf, type CustomTheme, type Style } from '../lib/theme';
import { pal, hue } from '../lib/color';
import { art, artCols } from '../lib/art';
import { useStore } from '../store';
import { useT } from '../lib/i18n';
import { ThemeEditor } from './ThemeEditor';

type T = { style: Style; dark: boolean; accent: string; custom: CustomTheme };

// Мини-окно лаунчера. Цвета берутся напрямую из ролей темы, а не из CSS-переменных окна.
const Preview = memo(function Preview({ style, dark, accent, custom }: T) {
  const r = useMemo(() => getRoles(style, dark, accent, custom), [style, dark, accent, custom]);
  const svg = useMemo(
    () => art('plains', 7, 288, 120, 12, artCols('plains', 0, { style, dark, accent, custom })),
    [style, dark, accent, custom],
  );
  const m = softOf(style, custom);
  const nav = (sel: boolean) => (
    <i className="tp-nv" style={{
      background: sel ? (m ? r.sec : r.s2) : r.s3, opacity: sel ? 1 : 0.6,
      borderRadius: m ? 99 : 4, boxShadow: !m && sel ? `inset 2px 0 0 ${r.pri}` : 'none',
    }} />
  );
  return (
    <div className="tp" style={{ background: r.bg, borderRadius: m ? 22 : 10 }}>
      <div className="tp-sb" style={{ background: r.sbg }}>
        <i className="tp-av" style={{ background: m ? r.pric : r.s3 }} />
        {nav(true)}{nav(false)}{nav(false)}
      </div>
      <div className="tp-mn">
        <div className="tp-h">
          <i style={{ width: '46%', background: r.on }} />
          <i style={{ width: '24%', background: r.onv, opacity: 0.6 }} />
        </div>
        <div className="tp-hero" style={{ borderRadius: m ? 14 : 6 }} dangerouslySetInnerHTML={{ __html: svg }} />
        <div className="tp-play" style={{ background: m ? r.pric : r.pri, borderRadius: m ? 12 : 6 }} />
        <div className="tp-tiles">
          {[0, 1, 2].map(k => (
            <i key={k} style={{ background: m ? [r.s2, r.sec, r.terc][k] : r.s1, borderRadius: m ? 10 : 5 }} />
          ))}
        </div>
      </div>
    </div>
  );
});

type Opt = { style: Style; dark: boolean; name: string; sub: string };

const OPTIONS: Opt[] = [
  { style: 'monet', dark: true, name: 'Monet', sub: 'Тёмная' },
  { style: 'monet', dark: false, name: 'Monet', sub: 'Светлая' },
  { style: 'catppuccin', dark: true, name: 'Catppuccin', sub: 'Mocha' },
  { style: 'catppuccin', dark: false, name: 'Catppuccin', sub: 'Latte' },
  { style: 'flat', dark: true, name: 'Плоский', sub: 'Тёмная' },
  { style: 'flat', dark: false, name: 'Плоский', sub: 'Светлая' },
  { style: 'amoled', dark: true, name: 'AMOLED', sub: 'Чёрная' },
  { style: 'custom', dark: true, name: 'Своя', sub: 'Цвета и размеры' },
];

function Card({ style, dark, accent, custom, name, sub, selected, index, onPick }: T & { name: string; sub: string; selected: boolean; index: number; onPick: () => void }) {
  const r = getRoles(style, dark, accent, custom);
  const dots = style === 'monet' ? [r.pri, r.sec, r.ter, r.pric]
    : style === 'catppuccin' ? [r.pri, r.ter, r.s3, r.pric]
      : style === 'custom' ? [r.pri, r.ter, r.s3, r.sbg]
        : [r.pri, r.s3];
  return (
    <button type="button" className={'tcard' + (selected ? ' on' : '')} style={{ '--i': index } as CSSProperties} aria-pressed={selected} onClick={onPick}>
      <Preview style={style} dark={dark} accent={accent} custom={custom} />
      {selected && (
        <svg className="tcard-ck" viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
      )}
      <div className="tcard-row">
        <div>
          <b>{name}</b>
          <span className="hint">{sub}</span>
        </div>
        <div className="tcard-dots">{dots.map((c, i) => <i key={i} style={{ background: c }} />)}</div>
      </div>
    </button>
  );
}

const TONES = [10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99];

function Tonal({ accent }: { accent: string }) {
  const t = useT();
  const Q = useMemo(() => pal(hue(accent)), [accent]);
  const rows: [string, (t: number) => string][] = [['Основной', Q.p], ['Вторичный', Q.s], ['Третичный', Q.t]];
  return (
    <div className="tonal">
      {rows.map(([name, f]) => (
        <div className="tonal-row" key={name}>
          <span className="hint">{t(name)}</span>
          <div>{TONES.map(t => <i key={t} style={{ background: f(t) }} title={String(t)} />)}</div>
        </div>
      ))}
    </div>
  );
}

export function ThemePicker() {
  const { config, setConfig } = useStore();
  const t = useT();
  const current = config.style as Style;
  const custom = config.custom;
  const [open, setOpen] = useState(false);
  const [edit, setEdit] = useState(false);
  const active =
    OPTIONS.find((o) => o.style === current && (current === 'amoled' || current === 'custom' || o.dark === config.dark)) ?? OPTIONS[0];

  return (
    <>
      <h2>{t('Тема оформления')}</h2>
      <div className="theme-row">
        <button
          type="button"
          className={'theme-sum' + (open ? ' open' : '')}
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          <span className="theme-thumb">
            <Preview style={active.style} dark={active.dark} accent={config.accent} custom={custom} />
          </span>
          <span className="theme-now">
            <b>{t(active.name)}</b>
            <span className="hint">{t(active.sub)}</span>
          </span>
          <span className="theme-act">{open ? t('Свернуть') : t('Сменить')}</span>
          <svg className="theme-chev" viewBox="0 0 16 16">
            <path d="M4 6l4 4 4-4" />
          </svg>
        </button>
        {current === 'custom' && (
          <button type="button" className="btn sm" onClick={() => setEdit(true)}>
            {t('Настроить')}
          </button>
        )}
      </div>

      <div className={'theme-body' + (open ? ' open' : '')}>
        <div>
          <div className="tgrid">
            {OPTIONS.map((o, i) => (
              <Card key={o.style + String(o.dark)} style={o.style} dark={o.dark} accent={config.accent} custom={custom}
                name={t(o.name)} sub={t(o.sub)} index={i}
                selected={current === o.style && (o.style === 'amoled' || o.style === 'custom' || config.dark === o.dark)}
                onPick={() => {
                  setConfig({ ...config, style: o.style, dark: o.style === 'custom' ? config.dark : o.dark });
                  setOpen(false);
                  if (o.style === 'custom') setEdit(true);
                }} />
            ))}
          </div>
          <div className="theme-move">
            <button
              type="button"
              className="btn sm"
              onClick={() => {
                setOpen(false);
                useStore.getState().setMove(true);
              }}
            >
              {t('Расставить кнопки')}
            </button>
            <span className="hint">{t('Любую кнопку можно перетащить на своё место')}</span>
          </div>
          {current === 'monet' && (
            <>
              <h2 style={{ marginTop: 20 }}>{t('Палитра Monet')}</h2>
              <div className="hint" style={{ marginBottom: 8 }}>{t('Все цвета окна строятся из выбранного акцента')}</div>
              <Tonal accent={config.accent} />
            </>
          )}
        </div>
      </div>

      <ThemeEditor open={edit} onClose={() => setEdit(false)} />
    </>
  );
}
