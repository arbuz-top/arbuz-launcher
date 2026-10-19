// Общие элементы управления, повторяющие разметку макета
import { memo, type CSSProperties } from 'react';
import type { Skin } from '../lib/api';
import { builtinAvatar, DEFAULT_AVATAR } from '../lib/builtin';

export const IHome = () => (
  <svg viewBox="0 0 24 24">
    <path d="M3 11 12 3l9 8M5 9v12h14V9" />
  </svg>
);

export const ISearch = () => (
  <svg viewBox="0 0 24 24">
    <circle cx="11" cy="11" r="7" />
    <path d="m21 21-4.3-4.3" />
  </svg>
);

export const ISettings = () => (
  <svg viewBox="0 0 24 24">
    <path d="M4 6h16M4 12h16M4 18h16" />
  </svg>
);

export const IPlus = () => (
  <svg viewBox="0 0 24 24">
    <path d="M12 5v14M5 12h14" />
  </svg>
);

export const IUp = () => (
  <svg viewBox="0 0 24 24">
    <path d="M12 16V4m0 0-4 4m4-4 4 4M4 20h16" />
  </svg>
);

export const PlayIcon = () => (
  <svg viewBox="0 0 24 24">
    <path d="M7 4v16l13-8z" />
  </svg>
);

export const Toggle = memo(function Toggle({ on, onClick }: { on: boolean; onClick: () => void }) {
  return <div className={'tg' + (on ? ' on' : '')} onClick={onClick} />;
});

export function Seg<T extends string>({
  items,
  value,
  onPick,
}: {
  items: [T, string][];
  value: T;
  onPick: (v: T) => void;
}) {
  return (
    <div className="seg">
      {items.map(([k, l]) => (
        <button key={k} type="button" className={value === k ? 'on' : ''} onClick={() => onPick(k)}>
          {l}
        </button>
      ))}
    </div>
  );
}

export const Check = memo(function Check({
  on,
  label,
  onClick,
}: {
  on: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <div className={'ck' + (on ? ' on' : '')} onClick={onClick}>
      <i />
      {label}
    </div>
  );
});

/**
 * Аватар аккаунта: голова, собранная из текстуры скина. Лицо — это пиксели
 * (8,8)-(16,16) текстуры, поверх кладётся слой шляпы (40,8). Если скина нет,
 * показывается стандартный аватар из `public/builtin/avatars`.
 */
export const Avatar = memo(function Avatar({
  name,
  skin,
  size,
}: {
  name: string;
  skin?: Skin | null;
  size: number;
}) {
  const box: CSSProperties = { width: size, height: size, position: 'relative', overflow: 'hidden' };

  if (!skin) {
    return (
      <div className="av" style={box} title={name}>
        <img
          src={builtinAvatar(DEFAULT_AVATAR)}
          alt=""
          style={{ width: '100%', height: '100%', display: 'block' }}
        />
      </div>
    );
  }

  // 1 пиксель текстуры = size/8 экранных пикселей.
  const k = size / 8;
  const layer = (px: number, py: number): CSSProperties => ({
    position: 'absolute',
    inset: 0,
    backgroundImage: `url("${skin.url}")`,
    backgroundSize: `${skin.width * k}px ${skin.height * k}px`,
    backgroundPosition: `${-px * k}px ${-py * k}px`,
    imageRendering: 'pixelated',
  });

  return (
    <div className="av" style={box} title={name}>
      <div style={layer(8, 8)} />
      <div style={layer(40, 8)} />
    </div>
  );
});

export function RamSlider({
  value,
  min,
  max,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  const pct = Math.round(((value - min) / Math.max(1, max - min)) * 100) + '%';
  return (
    <input
      type="range"
      min={min}
      max={max}
      step={512}
      value={value}
      style={{ '--p': pct } as CSSProperties}
      onChange={(e) => onChange(+e.target.value)}
    />
  );
}
