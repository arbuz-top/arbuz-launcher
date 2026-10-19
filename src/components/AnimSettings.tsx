// Блок настроек анимаций: пресеты, тонкая настройка и живой предпросмотр.
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import type { AnimConfig } from '../lib/api';
import {
  ANIM_PRESETS,
  DEFAULT_ANIM,
  DEFAULT_POINTS,
  DIRS,
  EASINGS,
  pointsFromEasing,
  presetPatch,
  useReducedMotion,
} from '../lib/anim';
import { setConfigLive, useStore } from '../store';
import { langOf, useT } from '../lib/i18n';
import { Toggle } from './Controls';
import { CurveEditor } from './CurveEditor';

const CATS: [keyof AnimConfig, string][] = [
  ['screens', 'Экраны и вкладки'],
  ['lists', 'Списки и карточки'],
  ['modals', 'Модальные окна'],
  ['press', 'Наведение и нажатия'],
  ['launch', 'Запуск и прогресс'],
];

const pct = (v: number, min: number, max: number): string =>
  Math.round(((v - min) / (max - min)) * 100) + '%';

export function AnimSettings() {
  const c = useStore((s) => s.config);
  const t = useT();
  const a = c.anim;
  const reduce = useReducedMotion();
  const [replay, setReplay] = useState(0);
  const firstRun = useRef(true);

  // После изменения любого ползунка проигрываем предпросмотр ещё раз.
  // На первом рендере не перезапускаем — демо уже анимируется при монтировании.
  useEffect(() => {
    if (firstRun.current) {
      firstRun.current = false;
      return;
    }
    const t = window.setTimeout(() => setReplay((r) => r + 1), 140);
    return () => window.clearTimeout(t);
  }, [
    a.enabled,
    a.preset,
    a.speed,
    a.amplitude,
    a.stagger,
    a.easing,
    a.curve,
    a.points,
    a.dir,
    a.screens,
    a.lists,
    a.modals,
    a.press,
    a.launch,
  ]);

  // Патч всегда строится от актуального состояния store: при быстром
  // перетаскивании точки несколько событий могут прийти до ре-рендера.
  const save = (patch: Partial<AnimConfig>) => {
    const st = useStore.getState();
    setConfigLive({ ...st.config, anim: { ...st.config.anim, ...patch } });
  };
  // Ручная правка любого ползунка делает пресет «Своим» и включает анимации.
  const custom = (patch: Partial<AnimConfig>) => save({ ...patch, preset: 'custom', enabled: true });

  const pick = (id: string) => {
    if (id === 'custom') {
      // Своя кривая: точки уже есть — используем их, иначе редактор
      // начнёт с формы текущего пресета.
      save({ preset: 'custom', enabled: true, easing: 'custom' });
    } else {
      save(presetPatch(id));
    }
  };

  // «Системная» + просьба Windows уменьшить движение: показываем это явно и
  // не даём ползункам незаметно включить анимации обратно.
  const sysOff = reduce && a.preset === 'system';
  const animOn = a.enabled && !sysOff;
  const tuning = animOn;
  const lang = langOf();
  const speedLabel = (Math.round(a.speed * 100) / 100)
    .toString()
    .replace('.', lang === 'en' || lang === 'eo' ? '.' : ',');

  return (
    <div className="panel" style={{ padding: 20 }}>
      <div className="anim-grid">
        <div>
          <div className="sw" style={{ paddingTop: 0 }}>
            <div>
              <b style={{ fontWeight: 500 }}>{t('Анимации интерфейса')}</b>
              <div className="hint">{t('Переходы экранов, появление списков и модальных окон')}</div>
            </div>
            <Toggle
              on={animOn}
              onClick={() =>
                save(
                  animOn
                    ? { enabled: false }
                    : {
                        enabled: true,
                        preset: sysOff || a.preset === 'off' ? 'custom' : a.preset,
                      },
                )
              }
            />
          </div>

          <div className="anim-presets">
            {ANIM_PRESETS.map((p) => (
              <button
                key={p.id}
                type="button"
                title={t(p.hint)}
                className={'btn sm' + (a.preset === p.id ? ' pr' : '')}
                onClick={() => pick(p.id)}
              >
                {t(p.name)}
              </button>
            ))}
            <button
              type="button"
              className={'btn sm' + (a.preset === 'custom' ? ' pr' : '')}
              onClick={() => pick('custom')}
            >
              {t('Своя')}
            </button>
          </div>

          {sysOff && (
            <div className="hint" style={{ marginBottom: 10 }}>
              {t('Windows просит уменьшить движение — анимации выключены. Включите переключатель выше, если хотите их видеть.')}
            </div>
          )}
          {!a.enabled && !sysOff && (
            <div className="hint" style={{ marginBottom: 10 }}>
              {t('Анимации выключены — включите переключатель, чтобы настроить кривую.')}
            </div>
          )}

          <div style={{ opacity: tuning ? 1 : 0.45, pointerEvents: tuning ? 'auto' : 'none' }}>
            <div className="anim-row">
              <span className="hint">{t('Скорость')}</span>
              <input
                type="range"
                min={0.25}
                max={3}
                step={0.05}
                value={a.speed}
                disabled={!tuning}
                style={{ '--p': pct(a.speed, 0.25, 3) } as CSSProperties}
                onChange={(e) => custom({ speed: +e.target.value })}
              />
              <b>{speedLabel}×</b>
            </div>

            <div className="anim-row">
              <span className="hint">{t('Амплитуда')}</span>
              <input
                type="range"
                min={0}
                max={140}
                step={5}
                value={a.amplitude}
                disabled={!tuning}
                style={{ '--p': pct(a.amplitude, 0, 140) } as CSSProperties}
                onChange={(e) => custom({ amplitude: +e.target.value })}
              />
              <b>{Math.round(a.amplitude)}%</b>
            </div>

            <div className="anim-row">
              <span className="hint">{t('Каскад')}</span>
              <input
                type="range"
                min={0}
                max={60}
                step={4}
                value={a.stagger}
                disabled={!tuning}
                style={{ '--p': pct(a.stagger, 0, 60) } as CSSProperties}
                onChange={(e) => custom({ stagger: +e.target.value })}
              />
              <b>{Math.round(a.stagger)} {t('мс')}</b>
            </div>

            <div className="two" style={{ marginTop: 10 }}>
              <div>
                <label className="l">{t('Кривая')}</label>
                <select value={a.easing} disabled={!tuning} onChange={(e) => custom({ easing: e.target.value })}>
                  {EASINGS.map(([k, l]) => (
                    <option key={k} value={k}>
                      {t(l)}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="l">{t('Направление')}</label>
                <select value={a.dir} disabled={!tuning} onChange={(e) => custom({ dir: e.target.value })}>
                  {DIRS.map(([k, l]) => (
                    <option key={k} value={k}>
                      {t(l)}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <label className="l" style={{ marginTop: 12 }}>{t('Кривая движения')}</label>
            <CurveEditor
              points={pointsFromEasing(a)}
              onChange={(pts) => custom({ points: pts, easing: 'custom' })}
            />
            <div className="hint" style={{ marginTop: 6 }}>
              {t('Тяните точки — это сразу своя кривая. Клик по полю добавляет точку, двойной клик убирает.')}
            </div>
            <button
              type="button"
              className="btn sm"
              style={{ marginTop: 8 }}
              onClick={() =>
                custom({ points: DEFAULT_POINTS.map((p) => [...p] as [number, number]), easing: 'custom' })
              }
            >
              {t('Сбросить кривую')}
            </button>

            <label className="l">{t('Что анимировать')}</label>
            <div className="panel" style={{ padding: '2px 14px', background: 'var(--s2)' }}>
              {CATS.map(([key, label]) => (
                <div className="sw" key={key}>
                  <span>{t(label)}</span>
                  <Toggle on={a[key] as boolean} onClick={() => custom({ [key]: !a[key] } as Partial<AnimConfig>)} />
                </div>
              ))}
            </div>
          </div>
        </div>

        <div>
          <div className="hint" style={{ marginBottom: 8 }}>
            {t('Предпросмотр')}
          </div>
          <div className="anim-demo" key={replay}>
            {[0, 1, 2, 3].map((i) => (
              <i key={i} style={{ '--i': i } as CSSProperties} />
            ))}
          </div>
          <button type="button" className="btn sm" onClick={() => setReplay((r) => r + 1)}>
            {t('Проиграть')}
          </button>
          <button
            type="button"
            className="btn sm"
            style={{ marginLeft: 8 }}
            onClick={() => {
              save({ ...DEFAULT_ANIM });
              setReplay((r) => r + 1);
            }}
          >
            {t('Сбросить')}
          </button>
          <div className="hint" style={{ marginTop: 12 }}>
            {t('Пресеты задают характер движения, а переключатели ниже — где именно оно работает.')}
          </div>
        </div>
      </div>
    </div>
  );
}
