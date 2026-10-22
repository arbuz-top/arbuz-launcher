// Запуск и остановка профилей игры
import { api } from "./api";
import { t } from "./i18n";
import { useStore } from "../store";

/** Идёт ли процесс игры для профиля (запускается или уже запущен). */
export function isRunning(
  s: { running: Record<string, number>; live: Record<string, boolean> },
  id: string,
): boolean {
  return s.live[id] === true || s.running[id] !== undefined;
}

/** Текст этапа запуска по проценту готовности. */
export function stat(p: number): string {
  return p < 30
    ? t("Проверка файлов игры…")
    : p < 65
      ? t("Загрузка библиотек…")
      : p < 90
        ? t("Скачивание ассетов…")
        : t("Запуск Java…");
}

/** Запускает профиль или останавливает его, если он уже работает. */
export async function playInstance(
  id: string,
  world: string | null = null,
  server: string | null = null,
): Promise<void> {
  const st = useStore.getState();
  if (isRunning(st, id)) {
    try {
      await api.stopInstance(id);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
    return;
  }
  useStore.getState().set({ running: { ...st.running, [id]: 0 } });
  try {
    await api.launchInstance(id, world, server);
  } catch (e) {
    const s2 = useStore.getState();
    const running = { ...s2.running };
    delete running[id];
    const live = { ...s2.live };
    delete live[id];
    s2.set({ running, live });
    s2.toastMsg(String(e));
  }
}
