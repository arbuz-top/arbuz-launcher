// Настройки профиля → «Консоль»: живой лог Minecraft этого профиля.
import { useEffect, useMemo, useRef, useState } from 'react';
import type { Instance } from '../lib/api';
import { useT } from '../lib/i18n';
import { isRunning, playInstance } from '../lib/launch';
import { useStore } from '../store';

/** Стабильная ссылка: селектор не должен возвращать новый массив на каждый вызов. */
const EMPTY_LINES: string[] = [];
/** Больше в один экран не рендерим: лог всё равно прокручивается к концу. */
const MAX_SHOWN = 1200;

/** Подсветка: ошибки — красным, предупреждения — жёлтым. */
function lineClass(line: string): string {
  const s = line.toLowerCase();
  if (
    s.includes('error') ||
    s.includes('severe') ||
    s.includes('fatal') ||
    s.includes('exception') ||
    s.includes('caused by:')
  ) {
    return 'con-line err';
  }
  if (s.includes('warn')) return 'con-line warn';
  return 'con-line';
}

export function ConsoleTab({ inst }: { inst: Instance }) {
  const t = useT();
  const s = useStore();
  const lines = useStore((st) => st.logs[inst.id] ?? EMPTY_LINES);
  const live = s.live[inst.id] === true;
  const running = isRunning(s, inst.id);

  const [q, setQ] = useState('');
  const [follow, setFollow] = useState(true);
  const ref = useRef<HTMLDivElement>(null);

  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const list = needle ? lines.filter((l) => l.toLowerCase().includes(needle)) : lines;
    return list.slice(-MAX_SHOWN);
  }, [lines, q]);

  // Автопрокрутка: пока пользователь не ушёл вверх по логу.
  useEffect(() => {
    if (!follow) return;
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [shown, follow]);

  const onScroll = () => {
    const el = ref.current;
    if (!el) return;
    const bottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32;
    if (bottom !== follow) setFollow(bottom);
  };

  const copy = () => {
    const text = lines.join('\n');
    void navigator.clipboard
      .writeText(text)
      .then(() => useStore.getState().toastMsg(t('Лог скопирован')))
      .catch(() => useStore.getState().toastMsg(t('Не удалось скопировать лог')));
  };

  const clear = () => {
    const st = useStore.getState();
    st.set({ logs: { ...st.logs, [inst.id]: [] } });
    st.toastMsg(t('Консоль очищена'));
  };

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>{t('Консоль')}</h2>
      <p className="hint" style={{ marginBottom: 14 }}>
        {t('Живой лог игры этого профиля: ошибки и предупреждения подсвечены. Запустите игру — строки появятся сами.')}
      </p>

      <div className="con-panel">
        <div className="con-bar">
          <span className={'con-dot' + (live ? ' on' : '')} />
          <span className="con-state">{live ? t('Игра запущена') : t('Игра не запущена')}</span>
          <input
            className="con-q"
            type="text"
            value={q}
            placeholder={t('Поиск по логу')}
            onChange={(e) => setQ(e.target.value)}
          />
          <span className="con-count">
            {shown.length}
            {q.trim() ? ` / ${lines.length}` : ''} {t('строк')}
          </span>
          <button className="btn sm" onClick={copy} disabled={lines.length === 0}>
            {t('Копировать')}
          </button>
          <button className="btn sm" onClick={clear} disabled={lines.length === 0}>
            {t('Очистить')}
          </button>
          {!follow && (
            <button
              className="btn sm"
              onClick={() => {
                const el = ref.current;
                if (el) el.scrollTop = el.scrollHeight;
                setFollow(true);
              }}
            >
              {t('Вниз')}
            </button>
          )}
          <button className="btn sm pr" onClick={() => void playInstance(inst.id)}>
            {running ? t('Остановить') : t('Играть')}
          </button>
        </div>

        <div className="con-body" ref={ref} onScroll={onScroll}>
          {shown.length === 0 ? (
            <div className="con-empty">
              {q.trim()
                ? t('По запросу ничего не найдено.')
                : t('Логи появятся здесь после запуска игры.')}
            </div>
          ) : (
            shown.map((line, i) => (
              <div className={lineClass(line)} key={i}>
                {line || ' '}
              </div>
            ))
          )}
        </div>

        <div className="con-foot">
          <span>{t('Показаны последние {n} строк', { n: MAX_SHOWN })}</span>
          <span>{follow ? t('Автопрокрутка включена') : t('Автопрокрутка выключена')}</span>
        </div>
      </div>
    </div>
  );
}
