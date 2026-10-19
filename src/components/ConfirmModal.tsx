// Внутрилаунчерное подтверждение вместо системных окон Windows.
import { useEffect } from 'react';
import { useT } from '../lib/i18n';
import { answerConfirm, useStore } from '../store';

const DANGER_ICON = 'M12 3 2.6 20.5h18.8L12 3Zm0 6.2v5.2M12 17.4v.1';
const ASK_ICON = 'M9.6 9.2a2.6 2.6 0 1 1 3.8 2.3c-.9.5-1.4 1.1-1.4 2.1v.4M12 17.3v.1';

export function ConfirmModal() {
  const req = useStore((s) => s.confirm);
  const t = useT();

  useEffect(() => {
    if (!req) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        answerConfirm(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [req]);

  if (!req) return null;

  return (
    <div
      className="ov cf-ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) answerConfirm(false);
      }}
    >
      <div
        className={'cf' + (req.danger ? ' dg' : '')}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="cf-title"
      >
        <div className="cf-ic">
          <svg viewBox="0 0 24 24">
            <path d={req.danger ? DANGER_ICON : ASK_ICON} />
            {!req.danger && <circle cx="12" cy="12" r="9.2" />}
          </svg>
        </div>
        <h2 id="cf-title">{req.title}</h2>
        {req.text && <p className="hint cf-tx">{req.text}</p>}
        <div className="cf-actions">
          <button className="btn" autoFocus onClick={() => answerConfirm(false)}>
            {req.no ?? t('Отмена')}
          </button>
          <button className={'btn ' + (req.danger ? 'dg' : 'pr')} onClick={() => answerConfirm(true)}>
            {req.yes ?? t('Подтвердить')}
          </button>
        </div>
      </div>
    </div>
  );
}
