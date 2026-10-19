// Визуальный пикер цвета: поле «насыщенность/яркость», ползунок тона,
// HEX-поле, готовые цвета и шкала оттенков. Работает поверх любой темы.
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { hexToHsv, hsvToHex, ramp } from '../lib/color';

/** Готовые акценты: сочные современные цвета. */
export const PRESET_COLORS: string[] = [
  '#5fb339',
  '#8bc34a',
  '#00c853',
  '#00bfa5',
  '#00bcd4',
  '#29b6f6',
  '#3f8fe0',
  '#536dfe',
  '#7c4dff',
  '#9a63d6',
  '#c026d3',
  '#e91e63',
  '#f44336',
  '#ff5722',
  '#ff9800',
  '#ffc107',
  '#ffd600',
  '#cddc39',
  '#795548',
  '#607d8b',
];

const HEX = /^#[0-9a-f]{6}$/i;
const clamp01 = (v: number): number => (v < 0 ? 0 : v > 1 ? 1 : v);

export function ColorPicker({
  value,
  onChange,
  title,
  size = 32,
}: {
  value: string;
  onChange: (hex: string) => void;
  title?: string;
  size?: number;
}) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  const [hsv, setHsv] = useState(() => hexToHsv(value));
  const [text, setText] = useState(value.toLowerCase());
  const btn = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  const sv = useRef<HTMLDivElement>(null);
  const grab = useRef(false);

  // Внешнее значение (сброс настроек, смена темы) подхватываем, когда не тащим.
  useEffect(() => {
    if (grab.current) return;
    setHsv(hexToHsv(value));
    setText(value.toLowerCase());
  }, [value]);

  const place = useCallback(() => {
    const el = btn.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const W = 272;
    const H = 396;
    let left = r.left;
    let top = r.bottom + 8;
    if (left + W > window.innerWidth - 10) left = window.innerWidth - W - 10;
    if (top + H > window.innerHeight - 10) top = Math.max(10, r.top - H - 8);
    setPos({ left: Math.max(10, left), top });
  }, []);

  useLayoutEffect(() => {
    if (open) place();
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => {
      const t = e.target as Node;
      if (pop.current?.contains(t) || btn.current?.contains(t)) return;
      setOpen(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      setOpen(false);
    };
    window.addEventListener('mousedown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('resize', place);
    return () => {
      window.removeEventListener('mousedown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('resize', place);
    };
  }, [open, place]);

  const emit = (h: number, s: number, v: number) => {
    setHsv({ h, s, v });
    const hex = hsvToHex(h, s, v);
    setText(hex);
    onChange(hex);
  };

  const paint = (e: ReactPointerEvent<HTMLDivElement>) => {
    const el = sv.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const s = clamp01((e.clientX - r.left) / Math.max(1, r.width));
    const v = 1 - clamp01((e.clientY - r.top) / Math.max(1, r.height));
    emit(hsv.h, s, v);
  };

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    grab.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    paint(e);
  };

  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (grab.current) paint(e);
  };

  const up = (e: ReactPointerEvent<HTMLDivElement>) => {
    grab.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const pick = (hex: string) => {
    const c = hexToHsv(hex);
    emit(c.h, c.s, c.v);
  };

  const current = hsvToHex(hsv.h, hsv.s, hsv.v);

  return (
    <>
      <button
        ref={btn}
        type="button"
        className={'cp-btn' + (open ? ' on' : '')}
        style={{ width: size, height: size }}
        title={title}
        aria-label={title}
        onClick={() => setOpen((v) => !v)}
      >
        <i style={{ background: value }} />
      </button>

      {open &&
        createPortal(
          <div className="cp-pop" ref={pop} style={{ left: pos.left, top: pos.top }} role="dialog">
            <div
              className="cp-sv"
              ref={sv}
              style={{
                background: `linear-gradient(to top,#000,rgba(0,0,0,0)),linear-gradient(to right,#fff,rgba(255,255,255,0)),${hsvToHex(hsv.h, 1, 1)}`,
              }}
              onPointerDown={down}
              onPointerMove={move}
              onPointerUp={up}
              onPointerCancel={up}
            >
              <i
                className="cp-dot"
                style={{
                  left: `${hsv.s * 100}%`,
                  top: `${(1 - hsv.v) * 100}%`,
                  background: current,
                }}
              />
            </div>

            <div className="cp-hue-row">
              <i className="cp-preview" style={{ background: current }} />
              <input
                className="cp-hue"
                type="range"
                min={0}
                max={359}
                value={Math.round(hsv.h)}
                onChange={(e) => emit(+e.target.value, hsv.s, hsv.v)}
              />
            </div>

            <div className="cp-hex-row">
              <input
                className="cp-hex"
                type="text"
                spellCheck={false}
                value={text}
                onChange={(e) => {
                  const v = e.target.value;
                  setText(v);
                  if (HEX.test(v)) {
                    setHsv(hexToHsv(v));
                    onChange(v.toLowerCase());
                  }
                }}
                onBlur={() => setText(current)}
              />
              <span>HEX</span>
            </div>

            <div className="cp-presets">
              {PRESET_COLORS.map((col) => (
                <button
                  key={col}
                  type="button"
                  className={'cp-swatch' + (current.toLowerCase() === col ? ' on' : '')}
                  style={{ background: col }}
                  title={col}
                  onClick={() => pick(col)}
                />
              ))}
            </div>

            <div className="cp-ramp">
              {ramp(current).map((col) => (
                <i key={col} style={{ background: col }} title={col} onClick={() => pick(col)} />
              ))}
            </div>
          </div>,
          document.getElementById('win') ?? document.body,
        )}
    </>
  );
}
