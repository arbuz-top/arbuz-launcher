// Настройки и применение анимаций интерфейса.
import { useEffect, useState } from 'react';
import type { AnimConfig } from './api';

/** Точка своей кривой: x — прогресс времени, y — прогресс движения. */
export type CurvePoint = [number, number];

export const DEFAULT_ANIM: AnimConfig = {
  enabled: true,
  preset: 'system',
  speed: 1,
  amplitude: 100,
  stagger: 24,
  easing: 'ease-out',
  curve: 'cubic-bezier(.22,.9,.28,1)',
  points: [],
  dir: 'up',
  screens: true,
  lists: true,
  modals: true,
  press: true,
  launch: true,
};

export interface AnimPreset {
  id: string;
  name: string;
  hint: string;
  enabled: boolean;
  speed: number;
  amplitude: number;
  stagger: number;
  easing: string;
  curve: string;
  dir: string;
}

/** Пресеты меняют только характер движения, категории остаются как выбрал пользователь. */
export const ANIM_PRESETS: AnimPreset[] = [
  { id: 'system', name: 'Системная', hint: 'Как в Windows', enabled: true, speed: 1, amplitude: 100, stagger: 24, easing: 'ease-out', curve: DEFAULT_ANIM.curve, dir: 'up' },
  { id: 'off', name: 'Выключено', hint: 'Без движения', enabled: false, speed: 1, amplitude: 100, stagger: 24, easing: 'ease-out', curve: DEFAULT_ANIM.curve, dir: 'up' },
  { id: 'soft', name: 'Мягко', hint: 'Короткие сдвиги', enabled: true, speed: 0.75, amplitude: 55, stagger: 16, easing: 'ease-out', curve: DEFAULT_ANIM.curve, dir: 'up' },
  { id: 'smooth', name: 'Плавно', hint: 'Базовый ритм', enabled: true, speed: 1, amplitude: 100, stagger: 24, easing: 'ease-out', curve: DEFAULT_ANIM.curve, dir: 'up' },
  { id: 'fast', name: 'Быстро', hint: 'Минимум ожидания', enabled: true, speed: 1.8, amplitude: 70, stagger: 8, easing: 'ease-out', curve: DEFAULT_ANIM.curve, dir: 'up' },
  { id: 'spring', name: 'Пружина', hint: 'С отскоком', enabled: true, speed: 1.05, amplitude: 110, stagger: 28, easing: 'spring', curve: 'cubic-bezier(.34,1.56,.64,1)', dir: 'scale' },
];

export const EASINGS: [string, string][] = [
  ['ease-out', 'Плавное затухание'],
  ['ease-in-out', 'Симметричное'],
  ['linear', 'Линейное'],
  ['spring', 'Пружина'],
  ['custom', 'Своя кривая'],
];

export const DIRS: [string, string][] = [
  ['up', 'Снизу вверх'],
  ['left', 'Слева'],
  ['right', 'Справа'],
  ['scale', 'Масштаб'],
];

const clamp = (v: number, lo: number, hi: number): number => (v < lo ? lo : v > hi ? hi : v);

const PRESET_IDS = new Set<string>([...ANIM_PRESETS.map((p) => p.id), 'custom']);
const EASE_IDS = new Set<string>(EASINGS.map(([k]) => k));
const DIR_IDS = new Set<string>(DIRS.map(([k]) => k));

/**
 * Дополняет конфиг из старого config.json значениями по умолчанию и
 * нормализует поля: в файле могли оказаться строка вместо числа или
 * неизвестный пресет после ручной правки.
 */
export function mergeAnim(a?: Partial<AnimConfig> | null): AnimConfig {
  const raw: Record<string, unknown> = { ...DEFAULT_ANIM, ...(a ?? {}) };
  const num = (v: unknown, d: number, lo: number, hi: number): number =>
    clamp(typeof v === 'number' && Number.isFinite(v) ? v : d, lo, hi);
  const bool = (v: unknown, d: boolean): boolean => (typeof v === 'boolean' ? v : d);
  const pick = (v: unknown, ids: Set<string>, d: string): string =>
    typeof v === 'string' && ids.has(v) ? v : d;

  const preset = pick(raw.preset, PRESET_IDS, DEFAULT_ANIM.preset);
  const curve =
    typeof raw.curve === 'string' && raw.curve.trim().length > 0 && raw.curve.length <= 128
      ? raw.curve
      : DEFAULT_ANIM.curve;

  return {
    enabled: preset === 'off' ? false : bool(raw.enabled, DEFAULT_ANIM.enabled),
    preset,
    speed: num(raw.speed, DEFAULT_ANIM.speed, 0.25, 3),
    amplitude: num(raw.amplitude, DEFAULT_ANIM.amplitude, 0, 140),
    stagger: num(raw.stagger, DEFAULT_ANIM.stagger, 0, 60),
    easing: pick(raw.easing, EASE_IDS, DEFAULT_ANIM.easing),
    curve,
    points: normPoints(raw.points),
    dir: pick(raw.dir, DIR_IDS, DEFAULT_ANIM.dir),
    screens: bool(raw.screens, DEFAULT_ANIM.screens),
    lists: bool(raw.lists, DEFAULT_ANIM.lists),
    modals: bool(raw.modals, DEFAULT_ANIM.modals),
    press: bool(raw.press, DEFAULT_ANIM.press),
    launch: bool(raw.launch, DEFAULT_ANIM.launch),
  };
}

const ANIM_CACHE = 'arbuz.anim';
let animCacheTimer: number | undefined;

/**
 * Настройки анимаций из прошлого запуска. Нужны до ответа backend, чтобы
 * первый экран не анимировался у тех, кто анимации выключил.
 */
export function cachedAnim(): AnimConfig {
  if (typeof window === 'undefined') return { ...DEFAULT_ANIM, enabled: false };
  try {
    const raw = window.localStorage.getItem(ANIM_CACHE);
    if (raw) return mergeAnim(JSON.parse(raw) as Partial<AnimConfig>);
  } catch {
    // повреждённый кэш не должен мешать запуску
  }
  return { ...DEFAULT_ANIM, enabled: false };
}

/** Приводит точки кривой из конфига к безопасному виду. */
export function normPoints(v: unknown): CurvePoint[] {
  if (!Array.isArray(v)) return [];
  const out: CurvePoint[] = [];
  for (const item of v) {
    if (!Array.isArray(item) || item.length < 2) continue;
    const x = item[0];
    const y = item[1];
    if (typeof x !== 'number' || typeof y !== 'number') continue;
    if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
    out.push([clamp(x, 0, 1), clamp(y, 0, 1.35)]);
  }
  out.sort((a, b) => a[0] - b[0]);
  const ded: CurvePoint[] = [];
  for (const p of out) {
    if (!ded.length || p[0] - ded[ded.length - 1][0] >= 0.02) ded.push(p);
  }
  return ded.slice(0, 16);
}

/** Разбирает cubic-bezier(...), возвращает четыре числа или null. */
function parseCurve(s: string): [number, number, number, number] | null {
  const m = /^cubic-bezier\(\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*,\s*(-?[\d.]+)\s*\)$/.exec(
    s.trim(),
  );
  if (!m) return null;
  const n = m.slice(1).map(Number);
  if (!n.every(Number.isFinite)) return null;
  if (n[0] < 0 || n[0] > 1 || n[2] < 0 || n[2] > 1) return null;
  return [n[0], n[1], n[2], n[3]];
}

/** Проверяет пользовательскую кривую cubic-bezier. */
export function validCurve(s: string): boolean {
  return parseCurve(s) !== null;
}

const bez = (t: number, a: number, b: number): number => {
  const u = 1 - t;
  return 3 * u * u * t * a + 3 * u * t * t * b + t * t * t;
};

/** y кривой cubic-bezier при заданном x (x решается бисекцией). */
function bezierYAt(x: number, c: [number, number, number, number]): number {
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (bez(mid, c[0], c[2]) < x) lo = mid;
    else hi = mid;
  }
  return bez((lo + hi) / 2, c[1], c[3]);
}

/** Пять опорных точек, повторяющих готовую кривую — старт для редактора. */
export function sampleCubic(curve: string, n = 5): CurvePoint[] {
  const c = parseCurve(curve);
  if (!c || n < 2) return [[0, 0], [1, 1]];
  const out: CurvePoint[] = [];
  for (let i = 0; i < n; i++) {
    const x = i / (n - 1);
    if (i === 0) out.push([0, 0]);
    else if (i === n - 1) out.push([1, 1]);
    else out.push([x, bezierYAt(x, c)]);
  }
  return out;
}

/** Стартовая кривая для кнопки «Сбросить». */
export const DEFAULT_POINTS: CurvePoint[] = sampleCubic(DEFAULT_ANIM.curve, 5);

/** Касательные Catmull-Rom с ограничителем Фрича—Карлсона: без лишних петель. */
function tangents(pts: CurvePoint[]): number[] {
  const n = pts.length;
  const d: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    d.push((pts[i + 1][1] - pts[i][1]) / Math.max(0.0001, pts[i + 1][0] - pts[i][0]));
  }
  const m: number[] = new Array(n);
  m[0] = d[0];
  m[n - 1] = d[n - 2];
  for (let i = 1; i < n - 1; i++) m[i] = (d[i - 1] + d[i]) / 2;
  for (let i = 0; i < n - 1; i++) {
    if (d[i] === 0) {
      m[i] = 0;
      m[i + 1] = 0;
      continue;
    }
    let a = m[i] / d[i];
    let b = m[i + 1] / d[i];
    // Касательная против хода кривой даёт петлю — гасим её.
    if (a < 0) {
      m[i] = 0;
      a = 0;
    }
    if (b < 0) {
      m[i + 1] = 0;
      b = 0;
    }
    const s = a * a + b * b;
    if (s > 9) {
      const t = 3 / Math.sqrt(s);
      m[i] = t * a * d[i];
      m[i + 1] = t * b * d[i];
    }
  }
  return m;
}

/** Значения своей кривой в n равномерных точках по x. */
export function sampleY(points: CurvePoint[], n: number): number[] {
  const pts = normPoints(points);
  if (pts.length < 2) return Array.from({ length: n }, (_, i) => i / (n - 1));
  // Кривая обязана начинаться в 0 и заканчиваться в 1.
  if (pts[0][0] > 0.001) pts.unshift([0, 0]);
  else pts[0] = [0, 0];
  if (pts[pts.length - 1][0] < 0.999) pts.push([1, 1]);
  else pts[pts.length - 1] = [1, 1];

  const m = tangents(pts);
  const out: number[] = [];
  let seg = 0;
  for (let i = 0; i < n; i++) {
    const x = i / (n - 1);
    while (seg < pts.length - 2 && x > pts[seg + 1][0]) seg++;
    const [x0, y0] = pts[seg];
    const [x1, y1] = pts[seg + 1];
    const dx = Math.max(0.0001, x1 - x0);
    const t = clamp((x - x0) / dx, 0, 1);
    const t2 = t * t;
    const t3 = t2 * t;
    const h00 = 2 * t3 - 3 * t2 + 1;
    const h10 = t3 - 2 * t2 + t;
    const h01 = -2 * t3 + 3 * t2;
    const h11 = t3 - t2;
    out.push(clamp(h00 * y0 + h10 * dx * m[seg] + h01 * y1 + h11 * dx * m[seg + 1], 0, 1.35));
  }
  return out;
}

const LINEAR_OK =
  typeof CSS !== 'undefined' &&
  typeof CSS.supports === 'function' &&
  CSS.supports('animation-timing-function', 'linear(0, 0.5, 1)');

/** Превращает точки в CSS-кривую linear(...) — без ограничения на две ручки. */
export function linearOf(points: CurvePoint[], steps = 32): string {
  const ys = sampleY(points, steps + 1);
  const stops = ys.map((y, i) => {
    if (i === 0) return '0';
    if (i === steps) return '1';
    const v = Math.round(y * 1000) / 1000;
    const p = Math.round((i / steps) * 1000) / 10;
    return `${v} ${p}%`;
  });
  return `linear(${stops.join(',')})`;
}

/** Итоговая кривая с учётом пресета, своих точек и валидации. */
export function curveOf(a: AnimConfig): string {
  if (a.easing === 'custom') {
    if (a.points.length >= 2) {
      if (LINEAR_OK) return linearOf(a.points);
      // Старый WebView2 без linear(): приближаем кривую одной безье.
      const p = normPoints(a.points);
      if (p.length === 2) return 'linear';
      if (p.length >= 3) {
        const c1 = p[1];
        const c2 = p[p.length - 2];
        return `cubic-bezier(${c1[0].toFixed(3)},${c1[1].toFixed(3)},${c2[0].toFixed(3)},${c2[1].toFixed(3)})`;
      }
    }
    if (validCurve(a.curve)) return a.curve.trim();
    return DEFAULT_ANIM.curve;
  }
  if (a.easing === 'spring') return 'cubic-bezier(.34,1.56,.64,1)';
  if (a.easing === 'ease-in-out' || a.easing === 'linear' || a.easing === 'ease-out') return a.easing;
  return 'ease-out';
}

const EASE_CURVES: Record<string, string> = {
  'ease-out': 'cubic-bezier(0,0,.58,1)',
  'ease-in-out': 'cubic-bezier(.42,0,.58,1)',
  linear: 'cubic-bezier(0,0,1,1)',
  spring: 'cubic-bezier(.34,1.56,.64,1)',
};

/** Точки для редактора: свои, а если их нет — форма текущего пресета. */
export function pointsFromEasing(a: AnimConfig): CurvePoint[] {
  if (a.easing === 'custom' && a.points.length >= 2) return normPoints(a.points);
  const curve =
    a.easing === 'custom'
      ? validCurve(a.curve)
        ? a.curve
        : DEFAULT_ANIM.curve
      : EASE_CURVES[a.easing] ?? DEFAULT_ANIM.curve;
  return sampleCubic(curve, 5);
}

/** Патч конфига при выборе пресета. */
export function presetPatch(id: string): Partial<AnimConfig> {
  if (id === 'custom') return { preset: 'custom' };
  const p = ANIM_PRESETS.find((x) => x.id === id);
  if (!p) return { preset: 'custom' };
  return {
    preset: p.id,
    enabled: p.enabled,
    speed: p.speed,
    amplitude: p.amplitude,
    stagger: p.stagger,
    easing: p.easing,
    curve: p.curve,
    dir: p.dir,
  };
}

/** Включены ли анимации с учётом системной настройки «уменьшить движение». */
export function animActive(a: AnimConfig, reduce: boolean): boolean {
  return a.enabled && !(a.preset === 'system' && reduce);
}

/** Пересчитывает CSS-переменные на #win. Вызывается при каждом изменении настроек. */
export function applyAnim(el: HTMLElement, a: AnimConfig, reduce: boolean) {
  const active = animActive(a, reduce);
  const speed = clamp(Number.isFinite(a.speed) ? a.speed : 1, 0.25, 3);
  const k = 1 / speed;
  const amp = active ? clamp(Number.isFinite(a.amplitude) ? a.amplitude : 100, 0, 140) / 100 : 0;
  const stagger = active ? clamp(Number.isFinite(a.stagger) ? a.stagger : 24, 0, 60) : 0;

  const set = (name: string, value: string) => el.style.setProperty(name, value);
  el.dataset.anim = active ? 'on' : 'off';

  const dir = a.dir;
  const dist = 12 * amp;
  const x = dir === 'left' ? -dist * 1.5 : dir === 'right' ? dist * 1.5 : 0;
  const y = dir === 'up' ? dist : 0;
  const from = dir === 'scale' ? 1 - 0.04 * amp : 1;

  set('--anim-ease', curveOf(a));
  set('--anim-x', `${x.toFixed(2)}px`);
  set('--anim-y', `${y.toFixed(2)}px`);
  set('--anim-from', String(from));
  set('--stagger-list', active && a.lists ? `${stagger}ms` : '0ms');
  set('--stagger-launch', active && a.launch ? `${stagger}ms` : '0ms');

  // Базовые длительности держим в пределах 150 мс из ТЗ; скорость двигает их вокруг.
  set('--d-screen', active && a.screens ? `${Math.round(150 * k)}ms` : '0ms');
  set('--d-list', active && a.lists ? `${Math.round(150 * k)}ms` : '0ms');
  set('--d-modal', active && a.modals ? `${Math.round(140 * k)}ms` : '0ms');
  set('--d-press', active && a.press ? `${Math.round(90 * k)}ms` : '0ms');
  set('--d-launch', active && a.launch ? `${Math.round(150 * k)}ms` : '0ms');
  // Предпросмотр показывает характер движения независимо от категорий.
  set('--demo-d', active ? `${Math.round(150 * k)}ms` : '0ms');
  set('--demo-s', active ? `${stagger}ms` : '0ms');

  set('--press-scale', active && a.press ? String(1 - 0.02 * amp) : '1');
  set('--press-lift', active && a.press ? `${(2 * amp).toFixed(2)}px` : '0px');
  set('--hover-scale', active && a.press ? String(1 + 0.08 * amp) : '1');

  // Кэш для первого кадра пишем не на каждый пиксель перетаскивания.
  if (animCacheTimer) window.clearTimeout(animCacheTimer);
  animCacheTimer = window.setTimeout(() => {
    animCacheTimer = undefined;
    try {
      window.localStorage.setItem(ANIM_CACHE, JSON.stringify(a));
    } catch {
      // приватный режим или переполненное хранилище — не критично
    }
  }, 250);
}

/** Системная настройка «Уменьшить движение» (Windows). */
export function useReducedMotion(): boolean {
  const [reduce, setReduce] = useState(
    () => typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );
  useEffect(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const on = () => setReduce(mq.matches);
    on();
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return reduce;
}
