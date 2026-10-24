// Оверлей с консолью запущенной игры (стили .ov, .mod.wide, .con-h).
//
// Умная консоль (Б2): фильтры INFO/WARN/ERROR, поиск, подсветка ошибок,
// копирование выбранного/всего. Клик по строке стектрейса показывает,
// какому модулю принадлежит класс (индекс классов из jar-ов mods/).
import { useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

/** Стабильная ссылка: селектор не должен возвращать новый массив на каждый вызов. */
const EMPTY_LINES: string[] = [];

type Level = 'all' | 'info' | 'warn' | 'error';

/** Уровень строки: [12:34:56] [Thread/LEVEL]: … или Exception/Caused by. */
function levelOf(line: string): 'info' | 'warn' | 'error' {
  if (/\[(INFO|TRACE|DEBUG)\]/.test(line)) return 'info';
  if (/\[(WARN|WARNING)\]/.test(line)) return 'warn';
  if (/\[(ERROR|FATAL)\]/.test(line)) return 'error';
  // Стектрейс и причины считаем ошибками.
  if (/^\s*(at |Caused by: |Exception|Error|Suppressed:)/.test(line)) return 'error';
  return 'info';
}

/** Класс из строки стектрейса `at com.example.Foo.bar(Foo.java:1)`. */
function classOf(line: string): string | null {
  const m = /\bat\s+([\w.$]+)\.[\w$<>]+\(/.exec(line);
  if (!m) return null;
  const full = m[1];
  const cut = full.lastIndexOf('.');
  return cut > 0 ? full.slice(0, cut) : full;
}

export function ConsoleModal() {
  const id = useStore((s) => s.console);
  const lines = useStore((s) => (id ? s.logs[id] ?? EMPTY_LINES : EMPTY_LINES));
  const t = useT();
  const ref = useRef<HTMLPreElement>(null);
  const [level, setLevel] = useState<Level>('all');
  const [search, setSearch] = useState('');
  const [owner, setOwner] = useState<{ cls: string; text: string } | null>(null);

  // Автопрокрутка вниз при появлении новых строк.
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);

  useEffect(() => {
    if (id === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') useStore.getState().set({ console: null });
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [id]);

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    return lines.filter((l) => {
      if (level !== 'all' && levelOf(l) !== level) return false;
      if (q && !l.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [lines, level, search]);

  const counts = useMemo(() => {
    let warn = 0;
    let error = 0;
    for (const l of lines) {
      const lv = levelOf(l);
      if (lv === 'warn') warn += 1;
      else if (lv === 'error') error += 1;
    }
    return { warn, error };
  }, [lines]);

  const copyAll = () => {
    void navigator.clipboard
      .writeText(shown.join('\n'))
      .then(() => useStore.getState().toastMsg(t('Скопировано')))
      .catch((e) => useStore.getState().toastMsg(String(e)));
  };

  const copySelection = () => {
    const sel = document.getSelection()?.toString() ?? '';
    if (!sel) {
      useStore.getState().toastMsg(t('Сначала выделите строки'));
      return;
    }
    void navigator.clipboard
      .writeText(sel)
      .then(() => useStore.getState().toastMsg(t('Скопировано')))
      .catch((e) => useStore.getState().toastMsg(String(e)));
  };

  const onLineClick = (line: string) => {
    if (!id) return;
    const cls = classOf(line);
    if (!cls) return;
    setOwner({ cls, text: t('Ищу, какому модулю принадлежит класс…') });
    void api
      .findClassOwner(id, cls)
      .then((o) => {
        setOwner({
          cls,
          text: o
            ? t('Класс из мода: {name} ({file})', { name: o.name, file: o.file })
            : t('Класс не найден в mods/ — это класс игры или общей библиотеки'),
        });
      })
      .catch((e) => {
        setOwner({ cls, text: String(e) });
      });
  };

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
            <span className="hint" style={{ marginRight: 10 }}>
              {counts.error > 0 && (
                <span style={{ color: 'var(--err, #d9503f)', marginRight: 8 }}>
                  {t('ошибок: {n}', { n: counts.error })}
                </span>
              )}
              {counts.warn > 0 && (
                <span style={{ color: 'var(--warn, #d9a63a)' }}>
                  {t('предупреждений: {n}', { n: counts.warn })}
                </span>
              )}
            </span>
            <div className="seg" style={{ marginRight: 8 }}>
              {(['all', 'info', 'warn', 'error'] as Level[]).map((lv) => (
                <button
                  key={lv}
                  type="button"
                  className={level === lv ? 'on' : ''}
                  onClick={() => setLevel(lv)}
                >
                  {t(lv === 'all' ? 'Все' : lv === 'info' ? 'Инфо' : lv === 'warn' ? 'Предупр.' : 'Ошибки')}
                </button>
              ))}
            </div>
            <input
              type="text"
              placeholder={t('Поиск…')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{ width: 140, marginRight: 8 }}
            />
            <button className="btn sm" style={{ marginRight: 6 }} onClick={copySelection}>
              {t('Копировать выбранное')}
            </button>
            <button className="btn sm" style={{ marginRight: 6 }} onClick={copyAll}>
              {t('Копировать всё')}
            </button>
            <button className="btn sm" onClick={() => useStore.getState().set({ console: null })}>
              {t('Закрыть')}
            </button>
          </div>
          {owner && (
            <div className="row" style={{ padding: '6px 14px', background: 'var(--s1, #212225)' }}>
              <span className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>{owner.cls}</span>
              <span className="hint" style={{ marginLeft: 10 }}>
                {owner.text}
              </span>
              <span style={{ flex: 1 }} />
              <button className="btn sm" onClick={() => setOwner(null)}>
                ×
              </button>
            </div>
          )}
          <pre ref={ref}>
            {shown.map((l, i) => {
              const lv = levelOf(l);
              const clickable = /^\s*at\s/.test(l);
              return (
                <div
                  key={i}
                  className={'con-line' + (clickable ? ' con-frame' : '')}
                  style={{
                    color:
                      lv === 'error'
                        ? 'var(--err, #d9503f)'
                        : lv === 'warn'
                          ? 'var(--warn, #d9a63a)'
                          : undefined,
                    cursor: clickable ? 'pointer' : undefined,
                    minHeight: '1em',
                  }}
                  onClick={() => onLineClick(l)}
                >
                  {l || ' '}
                </div>
              );
            })}
          </pre>
        </div>
      </div>
    </div>
  );
}
