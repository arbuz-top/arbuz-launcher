// Процедурный арт профилей и иконки (порт из launcher.html)
import { createElement, type ReactElement } from 'react';
import { pal, hue, shade, mix } from './color';
import { builtinIcon } from './builtin';
import { catTokens, type CustomTheme, type Style } from './theme';

export type ArtTheme = { style: Style; dark: boolean; accent: string; custom?: CustomTheme };

export interface BioColors {
  s1: string;
  s2: string;
  sun: string;
  cl: string;
  far: string;
  mid: string;
  near: string;
  leaf: string;
  trunk: string;
}

export const A_BIO: Record<string, BioColors> = {
  plains: { s1: '#5b7fa6', s2: '#6b8fb4', sun: '#e3cf8a', cl: '#e8edf2', far: '#4b6f55', mid: '#4a8044', near: '#5a9a45', leaf: '#3b6e33', trunk: '#6b4a2e' },
  taiga: { s1: '#6d8091', s2: '#7d90a0', sun: '#d9d8c4', cl: '#c9d2d9', far: '#3f5d52', mid: '#3b6b48', near: '#4a7d4c', leaf: '#2c5a3a', trunk: '#4a3322' },
  desert: { s1: '#86a3bd', s2: '#a9bccb', sun: '#f0e0a4', cl: '#efe9db', far: '#c2a770', mid: '#d0b576', near: '#dcc78a', leaf: '#5a8f46', trunk: '#5a8f46' },
  nether: { s1: '#3a1a1d', s2: '#451f22', sun: '#d9a24a', cl: '#4a2326', far: '#5a2528', mid: '#74302f', near: '#8a3c3a', leaf: '#a24a3f', trunk: '#5a2528' },
  snow: { s1: '#8ea3b6', s2: '#a8b9c8', sun: '#f1f1e6', cl: '#f3f6f9', far: '#a3b3c2', mid: '#c3d0da', near: '#e6edf2', leaf: '#2c5a3a', trunk: '#4a3322' },
};

export const A_IC: Record<string, string> = {
  plains: '#6aa84f',
  taiga: '#4f8a6a',
  desert: '#d1a95a',
  nether: '#b0504a',
  snow: '#9db7c9',
};

export const BK: Record<string, string> = { plains: 'tree', taiga: 'spruce', snow: 'spruce', desert: 'cactus', nether: 'nether' };

export const R = (sd: number): (() => number) => {
  let s = (Math.imul(sd, 2654435761) >>> 0) || 1;
  return () => ((s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 4294967296);
};

export function icCols(bm: string, k: number, t: ArtTheme): [string, string, string] {
  if (t.style === 'custom' && t.custom) {
    const c = t.custom;
    const base = [c.pri, c.ter, c.sec][k % 3];
    return [base, shade(base, 0.78), shade(base, 0.6)];
  }
  if (t.style === 'flat' || t.style === 'amoled') {
    const c = A_IC[bm] ?? A_IC.plains;
    return [c, shade(c, 0.78), shade(c, 0.6)];
  }
  if (t.style === 'catppuccin') {
    const c = catTokens(t.dark);
    const pick = [c.mauve, c.blue, c.pink, c.teal, c.green][k % 5];
    return [pick, shade(pick, 0.78), shade(pick, 0.6)];
  }
  const Q = pal(hue(t.accent) + (k % 5) * 28);
  return t.dark ? [Q.p(80), Q.p(60), Q.p(45)] : [Q.p(70), Q.p(55), Q.p(40)];
}

export function artCols(bm: string, k: number, t: ArtTheme): BioColors {
  if (t.style === 'custom' && t.custom) {
    const c = t.custom;
    return {
      s1: c.sbg,
      s2: mix(c.sbg, c.s1, 0.55),
      sun: mix(c.ter, c.on, 0.25),
      cl: mix(c.s1, c.on, 0.35),
      far: mix(c.s3, c.pri, 0.15),
      mid: mix(c.s3, c.pri, 0.35),
      near: mix(c.s3, c.pri, 0.55),
      leaf: mix(c.pri, c.s3, 0.25),
      trunk: mix(c.onv, c.s3, 0.55),
    };
  }
  if (t.style === 'flat' || t.style === 'amoled') return A_BIO[bm] ?? A_BIO.plains;
  if (t.style === 'catppuccin') {
    const c = catTokens(t.dark);
    return t.dark
      ? { s1: c.base, s2: c.surface0, sun: c.yellow, cl: c.surface2, far: c.surface0, mid: c.surface1, near: c.surface2, leaf: c.green, trunk: c.overlay1 }
      : { s1: c.crust, s2: c.surface0, sun: c.yellow, cl: c.base, far: c.surface0, mid: c.surface1, near: c.surface2, leaf: c.green, trunk: c.overlay1 };
  }
  const Q = pal(hue(t.accent) + (k % 5) * 28);
  return t.dark
    ? { s1: Q.s(25), s2: Q.s(32), sun: Q.t(80), cl: Q.s(40), far: Q.s(40), mid: Q.p(45), near: Q.p(55), leaf: Q.p(30), trunk: Q.n(22) }
    : { s1: Q.p(90), s2: Q.p(95), sun: Q.t(70), cl: Q.n(100), far: Q.s(80), mid: Q.p(75), near: Q.p(65), leaf: Q.p(45), trunk: Q.n(35) };
}

function buildArt(bm: string, sd: number, vw: number, vh: number, u: number, c: BioColors): string {
  const r = R(sd);
  const k = BK[bm] ?? 'tree';
  const nth = k === 'nether';
  const cols = Math.ceil(vw / u);
  let o = `<rect width="${vw}" height="${vh}" fill="${c.s1}"/><rect y="${vh * 0.42}" width="${vw}" height="${vh * 0.22}" fill="${c.s2}"/>`;
  if (!nth) {
    o += `<rect x="${Math.round((vw * 0.8) / u) * u}" y="${u}" width="${u * 1.5}" height="${u * 1.5}" fill="${c.sun}"/>`;
    for (let q = 0; q < 3; q++) {
      const x = ((r() * (cols - 8)) | 0) * u;
      const y = u * (1 + ((r() * 3) | 0));
      const n = 3 + ((r() * 3) | 0);
      o += `<rect x="${x}" y="${y}" width="${n * u}" height="${u}" fill="${c.cl}"/><rect x="${x + u}" y="${y - u}" width="${(n - 2) * u}" height="${u}" fill="${c.cl}"/>`;
    }
  }
  const lay = (b: number, a: number, ph: number, col: string): string => {
    let s = '';
    for (let x = 0; x < cols; x++) {
      const y = Math.round((b + Math.sin(x * 0.21 + ph) * a + Math.sin(x * 0.07 + ph * 2) * a * 0.7) / u) * u;
      s += `<rect x="${x * u}" y="${y}" width="${u + 0.5}" height="${vh - y}" fill="${col}"/>`;
    }
    return s;
  };
  o += lay(vh * 0.56, u * 1.2, sd, c.far) + lay(vh * 0.67, u * 1.1, sd * 2, c.mid);
  const hh: number[] = [];
  for (let x = 0; x < cols; x++) {
    hh[x] = Math.round((vh * 0.8 + Math.sin(x * 0.17 + sd * 3) * u * 0.9) / u) * u;
    o += `<rect x="${x * u}" y="${hh[x]}" width="${u + 0.5}" height="${vh - hh[x]}" fill="${c.near}"/>`;
  }
  for (let q = 0; q < Math.round(cols / 5); q++) {
    const x = 1 + ((r() * (cols - 2)) | 0);
    const y = hh[x];
    const X = x * u;
    if (k === 'tree')
      o += `<rect x="${X}" y="${y - 3 * u}" width="${u}" height="${3 * u}" fill="${c.trunk}"/><rect x="${X - u}" y="${y - 5 * u}" width="${3 * u}" height="${2 * u}" fill="${c.leaf}"/><rect x="${X}" y="${y - 6 * u}" width="${u}" height="${u}" fill="${c.leaf}"/>`;
    if (k === 'spruce')
      o += `<rect x="${X}" y="${y - u}" width="${u}" height="${u}" fill="${c.trunk}"/><rect x="${X - u}" y="${y - 2 * u}" width="${3 * u}" height="${u}" fill="${c.leaf}"/><rect x="${X - u}" y="${y - 3 * u}" width="${3 * u}" height="${u}" fill="${c.leaf}"/><rect x="${X}" y="${y - 4 * u}" width="${u}" height="${u}" fill="${c.leaf}"/><rect x="${X}" y="${y - 5 * u}" width="${u}" height="${u}" fill="${c.leaf}"/>`;
    if (k === 'cactus')
      o += `<rect x="${X}" y="${y - 3 * u}" width="${u}" height="${3 * u}" fill="${c.leaf}"/><rect x="${X + u}" y="${y - 2 * u}" width="${u}" height="${u}" fill="${c.leaf}"/><rect x="${X + u}" y="${y - 3 * u}" width="${u}" height="${u}" fill="${c.leaf}"/>`;
  }
  if (nth)
    for (let x = 0; x < cols; x++) {
      const hgt = (1 + ((r() * 3) | 0)) * u;
      o += `<rect x="${x * u}" width="${u + 0.5}" height="${hgt}" fill="${c.far}"/>`;
      if (r() < 0.08) o += `<rect x="${x * u}" y="${hgt}" width="${u}" height="${u}" fill="${c.sun}"/>`;
    }
  return `<svg viewBox="0 0 ${vw} ${vh}" preserveAspectRatio="xMidYMid slice" shape-rendering="crispEdges">${o}</svg>`;
}

/** Кэш готовых SVG: арт зависит только от аргументов (ТЗ §12-c). */
const ART_CACHE = new Map<string, string>();
const ART_CACHE_MAX = 64;

/** Процедурная заставка биома. Результат кэшируется по аргументам. */
export function art(bm: string, sd: number, vw: number, vh: number, u: number, c: BioColors): string {
  const key = `${bm}|${sd}|${vw}|${vh}|${u}|${c.s1},${c.s2},${c.sun},${c.cl},${c.far},${c.mid},${c.near},${c.leaf},${c.trunk}`;
  const hit = ART_CACHE.get(key);
  if (hit !== undefined) return hit;
  const out = buildArt(bm, sd, vw, vh, u, c);
  if (ART_CACHE.size >= ART_CACHE_MAX) ART_CACHE.clear();
  ART_CACHE.set(key, out);
  return out;
}

export interface PiItem {
  biome: string;
  icon: string | null;
  /** id встроенной иконки (src/assets/profiles). */
  preset?: string | null;
}

/** Иконка профиля: встроенная, своя картинка или процедурный блок. */
export function pi(i: PiItem, sz: number, k: number, t: ArtTheme): ReactElement {
  if (i.preset) {
    const src = builtinIcon(i.preset);
    if (src) {
      return createElement('img', {
        className: 'ic',
        src,
        width: sz,
        height: sz,
        style: { borderRadius: Math.round(sz * 0.22) },
      });
    }
  }
  if (i.icon) {
    return createElement('img', {
      className: 'ic',
      src: i.icon,
      width: sz,
      height: sz,
      style: { borderRadius: Math.round(sz * 0.22) },
    });
  }
  const c = icCols(i.biome, k, t);
  return createElement(
    'svg',
    { className: 'ic', width: sz, height: sz, viewBox: '0 0 32 32' },
    createElement('path', { d: 'M16 3 29 10 16 17 3 10Z', fill: c[0] }),
    createElement('path', { d: 'M3 10 16 17V30L3 23Z', fill: c[1] }),
    createElement('path', { d: 'M29 10 16 17V30L29 23Z', fill: c[2] }),
  );
}
