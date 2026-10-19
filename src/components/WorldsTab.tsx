// Вкладка «Миры» профиля (vWorlds из макета)
import { memo, useMemo, type CSSProperties } from 'react';
import { art, artCols } from '../lib/art';
import type { Instance, World } from '../lib/api';
import { fmtRelative } from '../lib/format';
import { useT } from '../lib/i18n';
import { playInstance } from '../lib/launch';
import { useArtTheme, useStore } from '../store';

/** Картинка мира: у level.dat нет биома, поэтому берём биом профиля. */
const WorldRow = memo(function WorldRow({
  inst,
  world,
  index,
}: {
  inst: Instance;
  world: World;
  index: number;
}) {
  const theme = useArtTheme();
  const t = useT();
  const svg = useMemo(
    () => art(inst.biome, index * 9 + (inst.created % 100000), 288, 180, 12, artCols(inst.biome, index, theme)),
    [inst.biome, inst.created, index, theme],
  );

  return (
    <div className="lr" style={{ alignItems: 'center', '--i': index } as CSSProperties}>
      <div
        style={{ width: 120, height: 75, borderRadius: 'var(--r)', overflow: 'hidden', flex: 'none' }}
        dangerouslySetInnerHTML={{ __html: svg }}
      />
      <div className="d">
        <b style={{ fontSize: 15, fontWeight: 600 }}>{world.name}</b>
        <div className="hint">
          {world.mode} · {fmtRelative(Math.floor(world.lastPlayedMs / 1000)).toLowerCase()} · saves/{world.dir}
        </div>
      </div>
      <button className="btn sm" onClick={() => void playInstance(inst.id, world.dir)}>
        {t('Играть')}
      </button>
    </div>
  );
});

export function WorldsTab({ inst }: { inst: Instance }) {
  const worlds = useStore((s) => s.worlds);
  const t = useT();

  if (worlds.length === 0) {
    return (
      <div className="panel">
        <div className="hint" style={{ padding: 40, textAlign: 'center' }}>
          {t('Миров пока нет. Создайте мир в игре — он появится здесь.')}
        </div>
      </div>
    );
  }

  return (
    <div className="panel">
      {worlds.map((w, k) => (
        <WorldRow key={w.dir} inst={inst} world={w} index={k} />
      ))}
    </div>
  );
}
