// Оверлей с консолью запущенной игры (стили .ov, .mod.wide, .con-h)
import { useEffect, useRef } from 'react';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

/** Стабильная ссылка: селектор не должен возвращать новый массив на каждый вызов. */
const EMPTY_LINES: string[] = [];

export function ConsoleModal() {
  const id = useStore((s) => s.console);
  const lines = useStore((s) => (id ? s.logs[id] ?? EMPTY_LINES : EMPTY_LINES));
  const t = useT();
  const ref = useRef<HTMLPreElement>(null);

  // Автопрокрутка вниз при появлении новых строк.
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  if (id === null) return null;

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) useStore.getState().set({ console: null });
      }}
    >
      <div className="mod wide">
        <div className="r">
          <div className="con-h">
            <b style={{ flex: 1, fontWeight: 600 }}>{t('Консоль игры')}</b>
            <button className="btn sm" onClick={() => useStore.getState().set({ console: null })}>
              {t('Закрыть')}
            </button>
          </div>
          <pre ref={ref}>{lines.join('\n')}</pre>
        </div>
      </div>
    </div>
  );
}
