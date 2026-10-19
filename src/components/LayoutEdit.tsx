// Режим расстановки: любую кнопку можно перетащить мышью.
// Сдвиги хранятся в config.moves и применяются на всех экранах.
import { useEffect, useLayoutEffect, useRef } from 'react';
import { useT } from '../lib/i18n';
import { setConfigLive, useStore } from '../store';

type MoveMap = Record<string, [number, number]>;

// Кнопки, вкладки, плитки, переключатели — всё, что выглядит кликабельным.
const SELECTOR = 'button, .nv, .si, .tile, .tab, .acct, .tg, .dot, .tcard';

const screenKey = (): string => {
  const st = useStore.getState();
  // Настройки и главная не зависят от вкладки профиля.
  if (st.view === 'p') return `p/${st.tab}/${st.sel ?? '-'}`;
  return st.view;
};

/** Ключ элемента: экран + путь в DOM. */
function keyOf(el: HTMLElement, screen: string): string {
  const explicit = el.dataset.k;
  if (explicit) {
    // Сайдбар и шапка общие: их сдвиги не зависят от открытой вкладки.
    const base = el.closest('#sb') ? 'sb' : el.closest('.hd') ? 'hdr' : screen;
    return `${base}/${explicit}`;
  }
  // Сайдбар общий для всех экранов — его сдвиги не должны зависеть от вкладки.
  const scope = el.closest('#sb') ? 'sb' : screen;
  const parts: string[] = [];
  let n: HTMLElement | null = el;
  while (n && n.id !== 'win') {
    const p: HTMLElement | null = n.parentElement;
    if (!p) break;
    const idx = Array.prototype.indexOf.call(p.children, n);
    const cls =
      typeof n.className === 'string' && n.className.trim()
        ? '.' + n.className.trim().split(/\s+/)[0]
        : '';
    parts.unshift(`${n.tagName.toLowerCase()}${cls}#${idx}`);
    n = p;
  }
  return `${scope}/${parts.join('>')}`;
}

/** Помечает кликабельные элементы и применяет сохранённые сдвиги. */
function applyAll(moves: MoveMap) {
  const root = document.getElementById('win');
  if (!root) return;
  const screen = screenKey();
  root.querySelectorAll<HTMLElement>(SELECTOR).forEach((el) => {
    if (el.closest('.te-ov') || el.closest('.mv-bar')) return;
    el.setAttribute('data-move-el', '1');
    const m = moves[keyOf(el, screen)];
    // Отдельное свойство translate не затирает transform элемента
    // (у кнопки «Играть» там центрирование -50%).
    if (m) el.style.translate = `${m[0]}px ${m[1]}px`;
    else el.style.removeProperty('translate');
  });
}

export function LayoutEdit() {
  const move = useStore((s) => s.move);
  const moves = useStore((s) => s.config.moves);
  const t = useT();
  const movesRef = useRef(moves);
  movesRef.current = moves;

  useLayoutEffect(() => {
    applyAll(moves);
  }, [moves, move]);

  // Новые элементы (сменили экран, загрузился список) — сразу со сдвигами.
  useEffect(() => {
    const root = document.getElementById('win');
    if (!root) return;
    let raf = 0;
    const run = () => {
      raf = 0;
      applyAll(movesRef.current);
    };
    const schedule = () => {
      if (!raf) raf = requestAnimationFrame(run);
    };
    const obs = new MutationObserver(schedule);
    obs.observe(root, { childList: true, subtree: true });
    return () => {
      obs.disconnect();
      if (raf) cancelAnimationFrame(raf);
    };
  }, []);

  useEffect(() => {
    if (!move) return;
    const root = document.getElementById('win');
    if (!root) return;

    let drag: {
      el: HTMLElement;
      key: string;
      sx: number;
      sy: number;
      ox: number;
      oy: number;
      nx: number;
      ny: number;
      base: DOMRect;
    } | null = null;

    const down = (e: PointerEvent) => {
      if (e.button !== 0) return;
      const t = (e.target as Element | null)?.closest?.('[data-move-el]') as HTMLElement | null;
      if (!t) return;
      e.preventDefault();
      e.stopPropagation();
      const screen = screenKey();
      const m = useStore.getState().config.moves[keyOf(t, screen)] ?? [0, 0];
      drag = {
        el: t,
        key: keyOf(t, screen),
        sx: e.clientX,
        sy: e.clientY,
        ox: m[0],
        oy: m[1],
        nx: m[0],
        ny: m[1],
        base: t.getBoundingClientRect(),
      };
      t.classList.add('mv-drag');
      t.setPointerCapture(e.pointerId);
    };

    const moveEv = (e: PointerEvent) => {
      if (!drag) return;
      const dx = e.clientX - drag.sx;
      const dy = e.clientY - drag.sy;
      const left0 = drag.base.left - drag.ox;
      const top0 = drag.base.top - drag.oy;
      const nx = Math.max(
        8 - left0,
        Math.min(drag.ox + dx, window.innerWidth - 8 - (left0 + drag.base.width)),
      );
      const ny = Math.max(
        8 - top0,
        Math.min(drag.oy + dy, window.innerHeight - 8 - (top0 + drag.base.height)),
      );
      drag.nx = Math.round(nx);
      drag.ny = Math.round(ny);
      drag.el.style.translate = `${drag.nx}px ${drag.ny}px`;
    };

    const up = () => {
      if (!drag) return;
      const d = drag;
      drag = null;
      d.el.classList.remove('mv-drag');
      const st = useStore.getState();
      const next: MoveMap = { ...st.config.moves };
      if (Math.abs(d.nx) < 1 && Math.abs(d.ny) < 1) delete next[d.key];
      else next[d.key] = [d.nx, d.ny];
      setConfigLive({ ...st.config, moves: next });
    };

    const cancel = () => {
      if (!drag) return;
      drag.el.classList.remove('mv-drag');
      drag = null;
    };

    // Клики в режиме расстановки не должны срабатывать.
    const click = (e: MouseEvent) => {
      if ((e.target as Element | null)?.closest?.('[data-move-el]')) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    root.addEventListener('pointerdown', down, true);
    root.addEventListener('click', click, true);
    window.addEventListener('pointermove', moveEv);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    return () => {
      root.removeEventListener('pointerdown', down, true);
      root.removeEventListener('click', click, true);
      window.removeEventListener('pointermove', moveEv);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
    };
  }, [move]);

  useEffect(() => {
    if (!move) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') useStore.getState().setMove(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [move]);

  if (!move) return null;

  return (
    <div className="mv-bar">
      <b>{t('Расстановка')}</b>
      <span className="hint">{t('Тащите кнопки мышью. Клики временно отключены.')}</span>
      <button
        type="button"
        className="btn sm"
        onClick={() => {
          const st = useStore.getState();
          setConfigLive({ ...st.config, moves: {} });
          applyAll({});
        }}
      >
        {t('Сбросить всё')}
      </button>
      <button type="button" className="btn sm pr" onClick={() => useStore.getState().setMove(false)}>
        {t('Готово')}
      </button>
    </div>
  );
}
