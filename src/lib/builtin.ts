// Встроенные ассеты.
//
// Иконки профилей и обои ищутся автоматически среди файлов
// `src/assets/profiles` и `src/assets/wallpapers`: достаточно перетащить туда
// PNG/SVG/JPG — код менять не нужно.
// Аватары лежат в `public/builtin/avatars` и адресуются по id.

export interface BuiltinAsset {
  id: string;
  url: string;
}

/** Превращает результат import.meta.glob в список id → url. */
function collect(modules: Record<string, string>): BuiltinAsset[] {
  return Object.entries(modules)
    .map(([path, url]) => {
      const file = path.split('/').pop() ?? '';
      return { id: file.replace(/\.[^.]+$/, ''), url };
    })
    .filter((x) => x.id.length > 0)
    .sort((a, b) => a.id.localeCompare(b.id));
}

const iconModules = import.meta.glob('/src/assets/profiles/*.{png,svg,jpg,jpeg,webp}', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>;

const wallModules = import.meta.glob('/src/assets/wallpapers/*.{png,svg,jpg,jpeg,webp}', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>;

/** Все иконки профилей из `src/assets/profiles`, по алфавиту. */
export const BUILTIN_ICONS: BuiltinAsset[] = collect(iconModules);

/** Все обои из `src/assets/wallpapers`, по алфавиту. */
export const BUILTIN_WALLPAPERS: BuiltinAsset[] = collect(wallModules);

/** URL иконки по id (пустая строка, если файла больше нет). */
export const builtinIcon = (id: string): string =>
  BUILTIN_ICONS.find((x) => x.id === id)?.url ?? '';

/** URL обоев по id (пустая строка, если файла больше нет). */
export const builtinWallpaper = (id: string): string =>
  BUILTIN_WALLPAPERS.find((x) => x.id === id)?.url ?? '';

export const BUILTIN_AVATARS = ['a1', 'a2', 'a3', 'a4', 'a5', 'a6'];

/** Аватар для аккаунта без скина. */
export const DEFAULT_AVATAR = 'a1';

const safe = (id: string): string => id.replace(/[^a-zA-Z0-9_-]/g, '');

export const builtinAvatar = (id: string): string => `/builtin/avatars/${safe(id)}.svg`;
