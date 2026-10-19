// Monet-палитра установщика: те же формулы, что в лаунчере.

const lin = (v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);

export function hex2lch(h) {
  const [r, g, b] = [1, 3, 5].map((i) => lin(parseInt(h.slice(i, i + 2), 16) / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [0, Math.hypot(a, bb), ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360];
}

export function lch2hex(L, C, h) {
  const t = (h * Math.PI) / 180;
  let r = 0;
  let g = 0;
  let b = 0;
  for (let k = 0; k < 30; k++) {
    const a = C * Math.cos(t);
    const c = C * Math.sin(t);
    const l = (L + 0.3963377774 * a + 0.2158037573 * c) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * c) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * c) ** 3;
    r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    b = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
    if (Math.min(r, g, b) >= -0.002 && Math.max(r, g, b) <= 1.002) break;
    C *= 0.92;
  }
  return (
    '#' +
    [r, g, b]
      .map((v) => {
        let x = Math.min(1, Math.max(0, v));
        x = x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
        return Math.round(x * 255)
          .toString(16)
          .padStart(2, '0');
      })
      .join('')
  );
}

const tL = (T) => Math.cbrt(T > 8 ? ((T + 16) / 116) ** 3 : T / 903.3);

const pal = (h) => ({
  p: (t) => lch2hex(tL(t), 0.14, h),
  s: (t) => lch2hex(tL(t), 0.045, h),
  t: (t) => lch2hex(tL(t), 0.085, h + 60),
  n: (t) => lch2hex(tL(t), 0.008, h),
  v: (t) => lch2hex(tL(t), 0.016, h),
});

const hue = (accent) => hex2lch(accent)[2];

export function monet(dark, accent) {
  const Q = pal(hue(accent));
  return dark
    ? {
        bg: Q.n(6),
        sbg: Q.n(10),
        s1: Q.n(12),
        s2: Q.n(17),
        s3: Q.n(22),
        on: Q.n(90),
        onv: Q.v(80),
        ln: Q.v(30),
        outl: Q.v(60),
        pri: Q.p(80),
        onpri: Q.p(20),
        sec: Q.s(30),
        onsec: Q.s(90),
        pric: Q.p(30),
        onpric: Q.p(90),
        ter: Q.t(80),
        terc: Q.t(30),
        onterc: Q.t(90),
      }
    : {
        bg: Q.n(98),
        sbg: Q.n(96),
        s1: Q.n(94),
        s2: Q.n(92),
        s3: Q.n(90),
        on: Q.n(10),
        onv: Q.v(30),
        ln: Q.v(80),
        outl: Q.v(50),
        pri: Q.p(40),
        onpri: Q.p(100),
        sec: Q.s(90),
        onsec: Q.s(10),
        pric: Q.p(90),
        onpric: Q.p(10),
        ter: Q.t(40),
        terc: Q.t(90),
        onterc: Q.t(10),
      };
}

/** Смешивает два цвета: t = 0 — первый, t = 1 — второй. */
export function mix(a, b, t) {
  const na = parseInt(a.slice(1), 16);
  const nb = parseInt(b.slice(1), 16);
  const k = Math.min(1, Math.max(0, t));
  return (
    '#' +
    [16, 8, 0]
      .map((sh) => {
        const va = (na >> sh) & 255;
        const vb = (nb >> sh) & 255;
        return Math.round(va + (vb - va) * k)
          .toString(16)
          .padStart(2, '0');
      })
      .join('')
  );
}
