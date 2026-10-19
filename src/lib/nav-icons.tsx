// Пиксельные иконки интерфейса (исходники — папка `icons/` в корне проекта).
// Нарисованы блоками и заливаются `currentColor`, поэтому подходят любой теме.
const PATHS: Record<string, string[]> = {
  home: [
    'M10 20h4v-6h2v6h4v2H4v-2h4v-6h2v6Zm-6 0H2V10h2v10Zm18 0h-2V10h2v10Zm-8-6h-4v-2h4v2Zm-8-4H4V8h2v2Zm14 0h-2V8h2v2ZM8 8H6V6h2v2Zm10 0h-2V6h2v2Zm-8-2H8V4h2v2Zm6 0h-2V4h2v2Zm-2-2h-4V2h4v2Z',
  ],
  folder: ['M20 20H4v-2h16v2ZM4 18H2V6h2v12Zm18 0h-2V8h2v10ZM20 8H10V6H4V4h8v2h8v2Z'],
  modpacks: [
    'M5 23H3v-2h2v2Zm8 0h-2v-4h2v4ZM3 21H1v-2h2v2Zm4 0H5v-2h2v2Zm-2-2H3v-2h2v2Zm6 0H9v-4h2v4Zm4 0h-2v-4h2v4Zm-6-4H5v-2h4v2Zm10 0h-4v-2h4v2ZM5 13H1v-2h4v2Zm18 0h-4v-2h4v2ZM9 11H5V9h4v2Zm10 0h-4V9h4v2Zm-8-2H9V5h2v4Zm4 0h-2V5h2v4Zm6-6h2v2h-2v2h-2V5h-2V3h2V1h2v2Zm-8 2h-2V1h2v4Z',
  ],
  accounts: [
    'M8 20h8v-2h4v2h-2v2H6v-2H4v-2h4v2Zm-4-2H2V6h2v12Zm12 0H8v-2h8v2Zm6 0h-2V6h2v12Zm-8-4h-4v-2h4v2Zm-4-2H8V8h2v4Zm6 0h-2V8h2v4Zm-2-4h-4V6h4v2ZM6 6H4V4h2v2Zm14 0h-2V4h2v2Zm-2-2H6V2h12v2Z',
  ],
  settings: [
    'M4 20h3v-2h4v4h2v-4h4v2h-2v4H9v-4H7v2H2v-5h2v3Zm18 2h-5v-2h3v-3h2v5ZM6 11H2v2h4v4H4v-2H0V9h4V7h2v4Zm14-2h4v6h-4v2h-2v-4h4v-2h-4V7h2v2Zm-6 7h-4v-2h4v2Zm-4-2H8v-4h2v4Zm6 0h-2v-4h2v4Zm-2-4h-4V8h4v2ZM7 4H4v3H2V2h5v2Zm8 0h2V2h5v5h-2V4h-3v2h-4V2h-2v4H7V4h2V0h6v4Z',
  ],
  'new-profile': [
    'M14 4h4v2h-4zm-4-2h4v2h-4zM6 8h4v2H6zm0 10h4v2H6zm4-8h4v2h-4zm0 10h4v2h-4zm4-12h4v2h-4zm0 10h4v2h-4zM6 4h4v2H6zM2 6h4v2H2zm0 10h4v2H2zM18 6h4v2h-4zm0 10h4v2h-4z',
    'M2 6h2v12H2zm18 0h2v12h-2zm-8 6h2v8h-2z',
  ],
  'popular-modpacks': [
    'M13 16h2v4h2v2H7v-2h2v-4h2v-2h2v2Zm-6 4H5v-2h2v2Zm12 0h-2v-2h2v2ZM5 18H3v-8h2v8Zm16 0h-2v-6h2v6Zm-6-6h-2v-2h2v2Zm4 0h-2v-2h2v2ZM7 10H5V8h2v2Zm6 0h-2V6h2v4Zm4 0h-2V8h2v2ZM9 8H7V6h2v2Zm2-2H9V2h2v4Z',
  ],
};

export type IconName = keyof typeof PATHS;

/** Кадр иконки. У большинства арт нарисован с полями (1–3 px по краям), а
 *  шестерёнка занимает квадрат целиком — от этого она казалась крупнее и
 *  смещённой. Ей даём кадр с полями, как у остальных, чтобы центр совпадал. */
const VIEWBOX: Record<string, string> = {
  settings: '-2 -2 28 28',
};

/** Иконка 24×24 из блоков: резкие края и цвет текущего текста. */
export function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  return (
    <svg
      className="nvi"
      viewBox={VIEWBOX[name] ?? '0 0 24 24'}
      fill="currentColor"
      width={size}
      height={size}
      shapeRendering="crispEdges"
      aria-hidden="true"
      style={{ display: 'block' }}
    >
      {PATHS[name].map((d, i) => (
        <path key={i} d={d} />
      ))}
    </svg>
  );
}
