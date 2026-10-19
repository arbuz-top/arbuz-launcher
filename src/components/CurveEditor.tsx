// Визуальный редактор кривой: точки двигаются мышью, без цифр и формул.
import { useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { normPoints, sampleY, type CurvePoint } from '../lib/anim';
import { useT } from '../lib/i18n';

const SIZE = 240;
const PAD = 20;
// Ниже нуля кривая не нужна: элемент не должен ехать назад. Выше единицы —
// место для отскока.
const Y_MIN = 0;
const Y_MAX = 1.35;
const STEPS = 72;
const GAP = 0.025;

const cl = (v: number, lo: number, hi: number): number => (v < lo ? lo : v > hi ? hi : v);
const gx = (x: number): number => PAD + x * (SIZE - PAD * 2);
const gy = (y: number): number => PAD + ((Y_MAX - y) / (Y_MAX - Y_MIN)) * (SIZE - PAD * 2);

function pathOf(pts: CurvePoint[]): string {
  const ys = sampleY(pts, STEPS);
  let d = '';
  for (let i = 0; i < STEPS; i++) {
    d += `${i ? 'L' : 'M'}${gx(i / (STEPS - 1)).toFixed(1)} ${gy(ys[i]).toFixed(1)}`;
  }
  return d;
}

export function CurveEditor({
  points,
  onChange,
}: {
  points: CurvePoint[];
  onChange: (p: CurvePoint[]) => void;
}) {
  const svg = useRef<SVGSVGElement>(null);
  const t = useT();
  const [drag, setDrag] = useState<number | null>(null);

  const pts = normPoints(points);
  const safe: CurvePoint[] = pts.length >= 2 ? pts : [[0, 0], [1, 1]];

  /** Координаты указателя в системе графика. */
  const at = (e: ReactPointerEvent): CurvePoint => {
    const el = svg.current;
    if (!el) return [0, 0];
    const ctm = el.getScreenCTM();
    if (!ctm) return [0, 0];
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    return [
      cl((p.x - PAD) / (SIZE - PAD * 2), 0, 1),
      cl(Y_MAX - ((p.y - PAD) / (SIZE - PAD * 2)) * (Y_MAX - Y_MIN), Y_MIN, Y_MAX),
    ];
  };

  const add = (e: ReactPointerEvent) => {
    if (safe.length >= 16) return;
    const [x, y] = at(e);
    if (x < 0.03 || x > 0.97) return;
    // Слишком близко к существующей точке — не плодим дубли.
    if (safe.some((p) => Math.abs(p[0] - x) < GAP * 1.5)) return;
    const added: CurvePoint = [x, y];
    const next = [...safe, added].sort((a, b) => a[0] - b[0]);
    const idx = next.indexOf(added);
    onChange(next);
    setDrag(idx);
    svg.current?.setPointerCapture(e.pointerId);
  };

  const down = (e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    if ((e.target as Element).tagName === 'circle') return;
    add(e);
  };

  const start = (i: number, e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    if (i <= 0 || i >= safe.length - 1) return;
    e.stopPropagation();
    setDrag(i);
    svg.current?.setPointerCapture(e.pointerId);
  };

  const move = (e: ReactPointerEvent) => {
    if (drag === null || drag <= 0 || drag >= safe.length - 1) return;
    // Потеряли pointerup (окно потеряло фокус) — прекращаем перетаскивание.
    if ((e.buttons & 1) === 0) {
      setDrag(null);
      return;
    }
    const [x, y] = at(e);
    const next = safe.map((p) => [...p] as CurvePoint);
    const lo = next[drag - 1][0] + GAP;
    const hi = next[drag + 1][0] - GAP;
    next[drag] = [hi > lo ? cl(x, lo, hi) : next[drag][0], y];
    onChange(next);
  };

  const up = (e: ReactPointerEvent) => {
    if (drag === null) return;
    setDrag(null);
    svg.current?.releasePointerCapture(e.pointerId);
  };

  const remove = (i: number, e: ReactMouseEvent) => {
    e.stopPropagation();
    if (i <= 0 || i >= safe.length - 1) return;
    setDrag(null);
    onChange(safe.filter((_, n) => n !== i));
  };

  return (
    <svg
      ref={svg}
      className="curve-ed"
      viewBox={`0 0 ${SIZE} ${SIZE}`}
      role="img"
      aria-label={t('Редактор кривой движения')}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
    >
      <rect
        x={PAD}
        y={PAD}
        width={SIZE - PAD * 2}
        height={SIZE - PAD * 2}
        fill="transparent"
      />
      {[0, 0.25, 0.5, 0.75, 1].map((v) => (
        <g key={v}>
          <line className="ce-grid" x1={gx(v)} y1={gy(1)} x2={gx(v)} y2={gy(0)} />
          <line className="ce-grid" x1={gx(0)} y1={gy(v)} x2={gx(1)} y2={gy(v)} />
        </g>
      ))}
      <line className="ce-diag" x1={gx(0)} y1={gy(0)} x2={gx(1)} y2={gy(1)} />
      <path className="ce-line" d={pathOf(safe)} />
      {safe.map((p, i) => (
        <circle
          key={i}
          className={'ce-pt' + (i === 0 || i === safe.length - 1 ? ' ce-end' : '')}
          cx={gx(p[0])}
          cy={gy(p[1])}
          r={i === 0 || i === safe.length - 1 ? 5 : 7}
          onPointerDown={(e) => start(i, e)}
          onDoubleClick={(e) => remove(i, e)}
        />
      ))}
    </svg>
  );
}
