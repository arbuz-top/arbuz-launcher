// Подписи и справочники для интерфейса
import type { AccountKind, Loader } from './api';

export const LOADER_LABEL: Record<Loader, string> = {
  vanilla: 'Vanilla',
  fabric: 'Fabric',
  forge: 'Forge',
  neoforge: 'NeoForge',
  quilt: 'Quilt',
};

export const LOADERS: Loader[] = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt'];

export const ACCOUNT_LABEL: Record<AccountKind, string> = {
  microsoft: 'Microsoft',
  elyby: 'Ely.by',
  littleskin: 'LittleSkin',
  offline: 'Без аккаунта',
};

export const BIOME_LABEL: Record<string, string> = {
  plains: 'Равнины',
  taiga: 'Тайга',
  desert: 'Пустыня',
  nether: 'Незер',
  snow: 'Снега',
};

export const BIOMES = ['plains', 'taiga', 'desert', 'nether', 'snow'];

export const JAVA_MAJORS = [8, 11, 17, 21, 25];

export const WINDOW_PRESETS: [number, number][] = [
  [1280, 720],
  [1600, 900],
  [1920, 1080],
  [2560, 1440],
];

export const JVM_PRESETS: [string, string][] = [
  ['G1GC', '-XX:+UseG1GC -XX:+ParallelRefProcEnabled'],
  ['ZGC', '-XX:+UseZGC'],
  ['Aikar', '-XX:+UseG1GC -XX:MaxGCPauseMillis=200'],
];
