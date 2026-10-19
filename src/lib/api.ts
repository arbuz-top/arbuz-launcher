// Типы данных и обёртки над Tauri-командами
import { invoke } from '@tauri-apps/api/core';
import { dataDir, join } from '@tauri-apps/api/path';
import type { CustomTheme } from './theme';

/** Корень данных лаунчера: <data_dir>/Arbuz Launcher (Windows: %APPDATA%\Arbuz Launcher). */
export const dataRoot = async (): Promise<string> => join(await dataDir(), 'Arbuz Launcher');

export type Loader = 'vanilla' | 'fabric' | 'forge' | 'neoforge' | 'quilt';
export type AccountKind = 'microsoft' | 'elyby' | 'littleskin' | 'offline';

export interface Instance {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  loaderVersion: string | null;
  biome: string;
  icon: string | null;
  /** id встроенной иконки профиля (src/assets/profiles). */
  iconPreset: string | null;
  /** id встроенных обоев профиля (public/builtin/wallpapers). */
  wallpaper: string | null;
  ramMinMb: number;
  ramMaxMb: number;
  javaMajor: number;
  javaPath: string | null;
  jvmArgs: string;
  width: number;
  height: number;
  fullscreen: boolean;
  playtimeSec: number;
  playtimeDays: Record<string, number>;
  lastPlayed: number | null;
  created: number;
}

export interface Account {
  id: string;
  kind: AccountKind;
  name: string;
  note: string;
}

export interface AnimConfig {
  enabled: boolean;
  preset: string;
  speed: number;
  amplitude: number;
  stagger: number;
  easing: string;
  curve: string;
  /** Точки своей кривой: [[x, y], ...], x 0..1, y может выходить за 0..1. */
  points: [number, number][];
  dir: string;
  screens: boolean;
  lists: boolean;
  modals: boolean;
  press: boolean;
  launch: boolean;
}

export interface Config {
  accent: string;
  dark: boolean;
  style: string;
  /** Язык интерфейса: ru | en | pl | uk | eo. */
  lang: string;
  /** Показывать выбор тем (экспериментально). */
  experimentalThemes: boolean;
  /** Свои обои главной: имя файла в папке wallpapers. */
  homeWallpaper: string | null;
  /** Визард первого запуска пройден. */
  onboarded: boolean;
  activeAccount: string | null;
  defaultJava: number;
  closeOnLaunch: boolean;
  showConsole: boolean;
  checkUpdates: boolean;
  hideEmpty: boolean;
  modIsolationEnabled: boolean;
  modIsolationLevel: string;
  curseforgeApiKey: string;
  /** Экспериментально: ИИ-ассистент сборок по промпту. */
  aiBuilder: boolean;
  anim: AnimConfig;
  custom: CustomTheme;
  moves: Record<string, [number, number]>;
}

export interface JavaInstall {
  major: number;
  version: string;
  path: string;
}

export interface McVersion {
  id: string;
  kind: string;
  releaseTime: string;
}

/** Установленный контент профиля (ТЗ §9). */
export interface ContentItem {
  /** Относительный путь от `minecraft/` без суффикса `.disabled`. */
  file: string;
  name: string;
  version: string;
  /** mod | rp | sh | dp */
  kind: string;
  sizeBytes: number;
  enabled: boolean;
  sha1: string;
  projectId: string | null;
  /** Источник, из которого файл поставлен через каталог. */
  source: string | null;
  /** Абсолютный путь к иконке в кэше — нужен `convertFileSrc`. */
  icon: string | null;
  /** Иконка проекта на CDN, если своей в архиве нет. */
  iconUrl: string | null;
  /** Помечен ли файл как изолируемый (песочница при запуске). */
  isolated: boolean;
}

export interface World {
  dir: string;
  name: string;
  mode: string;
  lastPlayedMs: number;
}

/** Скриншот из `minecraft/screenshots`. */
export interface Shot {
  file: string;
  size: number;
  /** unix-время изменения, мс. */
  modified: number;
}

/** Вид контента в каталоге (ТЗ §10). */
export type CatalogKind = 'mod' | 'dp' | 'rp' | 'pk' | 'sh';
export type CatalogSource = 'modrinth' | 'curseforge';

/** Аргументы поиска по каталогу (совпадают с `catalog::SearchArgs`). */
export interface SearchArgs {
  source: string;
  kind: string;
  query: string;
  loaders: string[];
  mcVersion: string | null;
  categories: string[];
  offset: number;
  sort: string;
  instanceId: string | null;
}

/** Элемент каталога (совпадает с `catalog::CatalogItem`). */
export interface CatalogItem {
  id: string;
  title: string;
  author: string;
  description: string;
  downloads: number;
  loaders: string[];
  categories: string[];
  iconUrl: string | null;
  source: string;
  kind: string;
  installed: boolean;
}

export interface CatalogSearch {
  hits: CatalogItem[];
  total: number;
}

/** Ссылка проекта на странице мода. */
export interface ProjectLink {
  label: string;
  url: string;
}

/** Подробности проекта для страницы мода. */
export interface ProjectDetails {
  id: string;
  source: string;
  title: string;
  description: string;
  body: string;
  downloads: number;
  iconUrl: string | null;
  links: ProjectLink[];
}

/** Разбор падения игры для окна «Игра упала». */
export interface CrashDiagnosis {
  found: boolean;
  source: string;
  file: string | null;
  kind: string;
  title: string;
  summary: string;
  culprit: string | null;
  culpritFile: string | null;
  suspects: string[];
  hints: string[];
  excerpt: string[];
  /** Ключи для t(): локализованный заголовок и описание. */
  titleKey: string | null;
  summaryKey: string | null;
  hintKeys: string[];
  /** Подстановки для ключей: mod, frame, error, mods. */
  vars: Record<string, string>;
}

/** Ответ ИИ-разбора падения (free-модель OpenCode Zen). */
export interface AiDiagnosis {
  model: string;
  cause: string;
  modName: string | null;
  confidence: number;
  reason: string;
  actions: string[];
  elapsedMs: number;
  tokens: number | null;
}

/** Проект каталога, подобранный ИИ-ассистентом сборки. */
export interface PlanItem {
  /** mod | rp | sh */
  kind: string;
  source: string;
  id: string;
  title: string;
  author: string;
  description: string;
  iconUrl: string | null;
  downloads: number;
  /** Зачем проект в сборке. */
  why: string;
}

/** План сборки: что ассистент предлагает поставить. */
export interface PackPlan {
  model: string;
  name: string;
  summary: string;
  description: string;
  mcVersion: string;
  loader: string;
  mods: PlanItem[];
  resourcepacks: PlanItem[];
  shaders: PlanItem[];
  /** Проекты, которые не нашлись в каталоге. */
  skipped: string[];
}

/** Профиль Modrinth App, доступный для импорта. */
export interface ModrinthProfile {
  name: string;
  path: string;
  mcVersion: string;
  loader: string;
  loaderVersion: string | null;
  mods: number;
  hasIcon: boolean;
}

/** Манифест сборки (`pack.json` в папке профиля). */
export interface PackManifest {
  name: string;
  version: string;
  author: string;
  summary: string;
  description: string;
  icon: string | null;
}

/** Сводка по сборке для вкладки «Сборка». */
export interface PackInfo {
  id: string;
  name: string;
  manifest: PackManifest;
  mcVersion: string;
  loader: string;
  loaderVersion: string | null;
  mods: number;
  modsTotal: number;
  configs: number;
  sizeBytes: number;
  icon: string | null;
}

/** Файл конфигурации мода. */
export interface ConfigEntry {
  path: string;
  size: number;
  kind: string;
  modId: string | null;
  modName: string | null;
}

/** Открытый конфиг для редактора. */
export interface ConfigDoc {
  path: string;
  content: string;
  size: number;
  hasBackup: boolean;
}

/** Замечание проверки сборки. */
export interface PackIssue {
  severity: 'error' | 'warn' | 'info' | string;
  title: string;
  detail: string;
}

/** Результат экспорта сборки. */
export interface PackExport {
  path: string;
  files: number;
  bytes: number;
}

/** Доступное обновление установленного контента. */
export interface UpdateInfo {
  file: string;
  name: string;
  newVersion: string;
}
/** Новость Minecraft с «Главной» (ТЗ §12-b). */
export interface NewsItem {
  title: string;
  version: string;
  /** ISO-8601. */
  date: string;
  imageUrl: string | null;
}

/** Текстура скина аккаунта: data-URL и её размеры в пикселях. */
export interface Skin {
  url: string;
  width: number;
  height: number;
}

/** Версия сборки, доступная к установке. */
export interface PackVersion {
  id: string;
  name: string;
  mcVersion: string;
  loader: string;
  /** ISO-8601. */
  date: string;
}

/** Ответ device-code Microsoft (поля как в структуре Rust — snake_case). */
export interface DeviceCode {
  device_code: string;
  user_code: string;
  verification_uri: string;
  /** Ссылка с `otc` — страница входа подставляет код сама. */
  verification_url: string;
  expires_in: number;
  interval: number;
}

export const api = {
  getConfig: () => invoke<Config>('get_config'),
  setConfig: (cfg: Config) => invoke<void>('set_config', { cfg }),

  listInstances: () => invoke<Instance[]>('list_instances'),
  createInstance: (name: string, mcVersion: string | null, loader: Loader) =>
    invoke<Instance>('create_instance', { name, mcVersion, loader }),
  updateInstance: (instance: Instance) => invoke<void>('update_instance', { instance }),
  deleteInstance: (id: string) => invoke<void>('delete_instance', { id }),

  setInstanceIcon: (id: string, sourcePath: string) =>
    invoke<Instance>('set_instance_icon', { id, sourcePath }),
  setInstanceIconUrl: (id: string, url: string) =>
    invoke<Instance>('set_instance_icon_url', { id, url }),
  clearInstanceIcon: (id: string) => invoke<Instance>('clear_instance_icon', { id }),

  openInstanceFolder: (id: string) => invoke<void>('open_instance_folder', { id }),

  getSystemRamMb: () => invoke<number>('get_system_ram_mb'),

  listMcVersions: () => invoke<McVersion[]>('list_mc_versions'),
  addCustomVersion: (id: string, base: string, jsonPath: string | null, jarPath: string | null) =>
    invoke<string>('add_custom_version', { id, base, jsonPath, jarPath }),
  removeCustomVersion: (id: string) => invoke<void>('remove_custom_version', { id }),
  listLoaderVersions: (loader: Loader, mcVersion: string) =>
    invoke<string[]>('list_loader_versions', { loader, mcVersion }),
  detectJavas: () => invoke<JavaInstall[]>('detect_javas'),
  installJava: (major: number) => invoke<string>('install_java', { major }),
  listAccounts: () => invoke<Account[]>('list_accounts'),
  accountStatus: (id: string) => invoke<string>('account_status', { id }),
  offlineAdd: (name: string) => invoke<Account>('offline_add', { name }),
  elybyLogin: (login: string, password: string) =>
    invoke<Account>('ely_login', { login, password }),
  littleLogin: (login: string, password: string) =>
    invoke<Account>('little_login', { login, password }),
  msBegin: () => invoke<DeviceCode>('ms_begin'),
  msPoll: (deviceCode: string, interval: number) =>
    invoke<Account>('ms_poll', { deviceCode, interval }),
  msCancel: () => invoke<void>('ms_cancel'),
  setActiveAccount: (id: string) => invoke<void>('set_active_account', { id }),
  removeAccount: (id: string) => invoke<void>('remove_account', { id }),
  launchInstance: (id: string, world: string | null) => invoke<void>('launch_instance', { id, world }),
  stopInstance: (id: string) => invoke<void>('stop_instance', { id }),

  listContent: (id: string) => invoke<ContentItem[]>('list_content', { id }),
  toggleContent: (id: string, file: string, enabled: boolean) =>
    invoke<void>('toggle_content', { id, file, enabled }),
  removeContent: (id: string, file: string) => invoke<void>('remove_content', { id, file }),
  toggleIsolation: (id: string, file: string, isolated: boolean) =>
    invoke<void>('toggle_isolation', { id, file, isolated }),
  listWorlds: (id: string) => invoke<World[]>('list_worlds', { id }),
  listScreenshots: (id: string) => invoke<Shot[]>('list_screenshots', { id }),
  openScreenshotsFolder: (id: string) => invoke<void>('open_screenshots_folder', { id }),
  openScreenshot: (id: string, file: string) => invoke<void>('open_screenshot', { id, file }),

  searchCatalog: (args: SearchArgs) => invoke<CatalogSearch>('search_catalog', { args }),
  projectDetails: (source: string, id: string) =>
    invoke<ProjectDetails>('catalog_project', { source, id }),
  installCatalogItem: (instanceId: string, source: string, projectId: string, kind: string, world: string | null) =>
    invoke<void>('install_catalog_item', { instanceId, source, projectId, kind, world }),
  checkUpdates: (id: string) => invoke<UpdateInfo[]>('check_updates', { id }),
  applyUpdates: (id: string) => invoke<number>('apply_updates', { id }),

  installModpack: (source: string, projectId: string, versionId: string | null) =>
    invoke<Instance>('install_modpack', { source, projectId, versionId }),
  listModpackVersions: (source: string, projectId: string) =>
    invoke<PackVersion[]>('list_modpack_versions', { source, projectId }),
  importPack: (path: string) => invoke<Instance>('import_pack', { path }),
  importPrismInstance: (path: string) => invoke<Instance>('import_prism_instance', { path }),
  listModrinthProfiles: () => invoke<ModrinthProfile[]>('list_modrinth_profiles'),
  importModrinthProfile: (path: string) => invoke<Instance>('import_modrinth_profile', { path }),

  diagnoseCrash: (id: string) => invoke<CrashDiagnosis>('diagnose_crash', { id }),
  aiDiagnoseCrash: (id: string, lang: string, console: string) =>
    invoke<AiDiagnosis>('ai_diagnose_crash', { id, lang, console }),
  aiPackPlan: (prompt: string, lang: string, mcVersion: string | null, loader: string | null) =>
    invoke<PackPlan>('ai_pack_plan', { prompt, lang, mcVersion, loader }),
  openCrashReports: (id: string) => invoke<void>('open_crash_reports', { id }),

  packInfo: (id: string) => invoke<PackInfo>('pack_info', { id }),
  savePackInfo: (id: string, manifest: PackManifest) =>
    invoke<PackInfo>('save_pack_info', { id, manifest }),
  packConfigs: (id: string) => invoke<ConfigEntry[]>('pack_configs', { id }),
  readPackConfig: (id: string, path: string) =>
    invoke<ConfigDoc>('read_pack_config', { id, path }),
  savePackConfig: (id: string, path: string, content: string) =>
    invoke<ConfigDoc>('save_pack_config', { id, path, content }),
  restorePackConfig: (id: string, path: string) =>
    invoke<ConfigDoc>('restore_pack_config', { id, path }),
  validatePack: (id: string) => invoke<PackIssue[]>('validate_pack', { id }),
  exportPack: (id: string, dest: string, format: string) =>
    invoke<PackExport>('export_pack', { id, dest, format }),
  openPackConfigs: (id: string) => invoke<void>('open_pack_configs', { id }),

  accountSkin: (id: string) => invoke<Skin | null>('account_skin', { id }),

  getNews: () => invoke<NewsItem[]>('get_news'),
  openDataFolder: () => invoke<void>('open_data_folder'),
  setHomeWallpaper: (sourcePath: string) => invoke<string>('set_home_wallpaper', { sourcePath }),
  clearHomeWallpaper: () => invoke<void>('clear_home_wallpaper'),
};
