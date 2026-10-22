import { useT } from '../lib/i18n';
import { MaterialIcon } from '../lib/material-icons';
import { Icon } from '../lib/nav-icons';
import { monetLike } from '../lib/theme';
import { curInstance, useStore } from '../store';
import { LanShare } from './LanShare';

/** Вкладка «Co-op»: игра с друзьями по коду (P2P) или через роутер. */
export function CoopTab() {
  const t = useT();
  const s = useStore();
  const monet = monetLike(s.config.style);
  const inst = curInstance(s);

  return (
    <div style={{ maxWidth: 780, margin: '0 auto', padding: '26px 0' }}>
      <div className="row g8" style={{ alignItems: 'center', marginBottom: 6 }}>
        {monet ? <MaterialIcon name="person_add" size={24} /> : <Icon name="coop" size={22} />}
        <h2 style={{ margin: 0 }}>{t('Co-op')}</h2>
      </div>
      <p className="hint" style={{ marginBottom: 12 }}>
        {t('Комнату можно создать заранее: отправьте код друзьям, потом запустите игру и откройте мир для сети — остальное лаунчер сделает сам.')}
      </p>
      <LanShare id={inst ? inst.id : ''} live={false} always />
    </div>
  );
}
