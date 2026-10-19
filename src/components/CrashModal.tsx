// Окно падения игры: разбираем отчёт и показываем причину и виновника.
import { useEffect, useState } from 'react';
import { api, type AiDiagnosis, type CrashDiagnosis } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

/** Подпись категории падения. */
const KIND_LABEL: Record<string, string> = {
  mod: 'Виноват мод',
  dependency: 'Зависимости',
  mixin: 'Mixin',
  memory: 'Память',
  java: 'Java',
  gpu: 'Графика',
  config: 'Конфиг',
  world: 'Мир',
  native: 'Сбой JVM',
  unknown: 'Ошибка',
};

export function CrashModal() {
  const crash = useStore((s) => s.crash);
  const inst = useStore((s) => s.instances.find((i) => i.id === crash?.id));
  const t = useT();
  const [d, setD] = useState<CrashDiagnosis | null>(null);
  const [busy, setBusy] = useState(false);
  const [showLog, setShowLog] = useState(false);
  const [ai, setAi] = useState<AiDiagnosis | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiErr, setAiErr] = useState<string | null>(null);

  useEffect(() => {
    if (!crash) {
      setD(null);
      setShowLog(false);
      setAi(null);
      setAiErr(null);
      return;
    }
    let alive = true;
    setBusy(true);
    setD(null);
    setShowLog(false);
    setAi(null);
    setAiErr(null);
    void api
      .diagnoseCrash(crash.id)
      .then((r) => {
        if (alive) setD(r);
      })
      .catch((e) => {
        if (alive) useStore.getState().toastMsg(String(e));
      })
      .finally(() => {
        if (alive) setBusy(false);
      });
    return () => {
      alive = false;
    };
  }, [crash]);

  if (!crash) return null;

  const close = () => useStore.getState().set({ crash: null });
  const kind = d?.kind ?? 'unknown';
  const label = KIND_LABEL[kind] ?? KIND_LABEL.unknown;
  // Эвристика отдаёт ключи, чтобы её текст переводился как остальной интерфейс.
  const vars = d?.vars ?? {};
  const title = d?.titleKey ? t(d.titleKey, vars) : d?.title;
  const summary = d?.summaryKey ? t(d.summaryKey, vars) : d?.summary;
  const hints = d?.hintKeys?.length ? d.hintKeys.map((k) => t(k, vars)) : (d?.hints ?? []);

  const askAi = async () => {
    setAiBusy(true);
    setAiErr(null);
    try {
      const st = useStore.getState();
      // Консоль лаунчера — это stdout/stderr процесса; в latest.log попадает
      // не всё, поэтому отдаём ИИ оба источника.
      const consoleText = (st.logs[crash.id] ?? []).slice(-600).join('\n');
      setAi(await api.aiDiagnoseCrash(crash.id, st.config.lang, consoleText));
    } catch (e) {
      setAiErr(t('Не удалось получить ответ ИИ') + ': ' + String(e));
    } finally {
      setAiBusy(false);
    }
  };

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="mod crash">
        <div className="r">
          <div className="crash-head">
            <span className={'crash-badge k-' + kind}>{t(label)}</span>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="crash-title">{title || t('Игра закрылась с ошибкой')}</div>
              <div className="hint">
                {inst ? inst.name : ''}
                {inst && crash.code !== null ? ' · ' : ''}
                {crash.code !== null ? t('Код выхода: {code}', { code: crash.code }) : ''}
              </div>
            </div>
          </div>

          {busy && <div className="hint" style={{ marginTop: 12 }}>{t('Разбираю отчёт…')}</div>}

          {d && (
            <>
              <div className="crash-summary">{summary}</div>

              {d.culprit && (
                <div className="crash-culprit">
                  <b>{t('Виновник')}:</b> <span className="crash-mod">{d.culprit}</span>
                  {d.culpritFile && <span className="hint"> · {d.culpritFile}</span>}
                  {d.suspects.length > 1 && (
                    <div className="hint" style={{ marginTop: 4 }}>
                      {t('Ещё под подозрением')}: {d.suspects.slice(1, 5).join(', ')}
                    </div>
                  )}
                </div>
              )}

              {!d.found && (
                <div className="hint" style={{ marginTop: 8 }}>
                  {t('Отчёт не найден — игра упала до его создания.')}
                </div>
              )}

              {hints.length > 0 && (
                <>
                  <h2 style={{ marginTop: 16 }}>{t('Что делать')}</h2>
                  <ul className="crash-hints">
                    {hints.map((h, i) => (
                      <li key={i}>{h}</li>
                    ))}
                  </ul>
                </>
              )}

              {d.excerpt.length > 0 && (
                <>
                  <button
                    type="button"
                    className="btn sm"
                    style={{ marginTop: 12 }}
                    onClick={() => setShowLog((v) => !v)}
                  >
                    {showLog ? t('Скрыть отчёт') : t('Показать отчёт')}
                  </button>
                  {showLog && <pre className="crash-log">{d.excerpt.join('\n')}</pre>}
                </>
              )}

              {d.file && (
                <div className="hint" style={{ marginTop: 10 }}>
                  {t('Источник: {f}', { f: d.file })}
                </div>
              )}
            </>
          )}

          <div className="crash-ai">
            <div className="crash-ai-head">
              <div style={{ minWidth: 0 }}>
                <b>{t('ИИ-разбор')}</b>
                <div className="hint">
                  {t('Отправит отчёт и список модов в OpenCode Zen (бесплатная модель)')}
                </div>
              </div>
              <button type="button" className="btn pr" disabled={aiBusy} onClick={() => void askAi()}>
                {aiBusy ? t('Читаю логи…') : ai ? t('Переспросить') : t('Помощь от ИИ')}
              </button>
            </div>

            {aiBusy && (
              <div className="crash-ai-busy">
                {t('ИИ читает отчёт и список модов…')}
              </div>
            )}
            {aiErr && !aiBusy && <div className="crash-ai-err">{aiErr}</div>}

            {ai && !aiBusy && (
              <div className="crash-ai-body">
                <div className="crash-ai-meta">
                  <span className={'crash-badge k-' + ai.cause}>
                    {t(KIND_LABEL[ai.cause] ?? KIND_LABEL.unknown)}
                  </span>
                  <span className="hint">
                    {ai.model} · {Math.round(ai.confidence * 100)}% · {ai.elapsedMs} ms
                  </span>
                </div>

                {ai.modName && (
                  <div className="crash-culprit" style={{ marginTop: 10 }}>
                    <b>{t('ИИ считает виновным')}:</b>{' '}
                    <span className="crash-mod">{ai.modName}</span>
                  </div>
                )}

                <div className="crash-ai-reason">{ai.reason}</div>

                {ai.actions.length > 0 && (
                  <>
                    <div className="crash-ai-sub">{t('Что сделать')}</div>
                    <ol className="crash-hints">
                      {ai.actions.map((a, i) => (
                        <li key={i}>{a}</li>
                      ))}
                    </ol>
                  </>
                )}
              </div>
            )}
          </div>

          <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
            <button
              type="button"
              className="btn"
              onClick={() => void api.openCrashReports(crash.id).catch((e) => useStore.getState().toastMsg(String(e)))}
            >
              {t('Отчёты')}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => void api.openInstanceFolder(crash.id).catch((e) => useStore.getState().toastMsg(String(e)))}
            >
              {t('Папка игры')}
            </button>
            <button type="button" className="btn pr" onClick={close}>
              {t('Понятно')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
