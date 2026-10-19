// Форматирование значений для интерфейса
import { monthName, t, tp } from './i18n';

export const gb = (mb: number): string => (mb / 1024).toFixed(mb % 1024 ? 1 : 0) + ' ' + t('ГБ');

export function fmtBytes(n: number): string {
  if (n >= 1024 * 1024 * 1024) return (n / 1024 / 1024 / 1024).toFixed(1) + ' ' + t('ГБ');
  if (n >= 1024 * 1024) return (n / 1024 / 1024).toFixed(1) + ' ' + t('МБ');
  if (n >= 1024) return Math.round(n / 1024) + ' ' + t('КБ');
  return n + ' ' + t('Б');
}

export function fmtDate(unixSec: number | null): string {
  if (!unixSec) return t('Никогда');
  const d = new Date(unixSec * 1000);
  return `${d.getDate()} ${monthName(d.getMonth())} ${d.getFullYear()}`;
}

/** Дата новости в виде «13 окт» (ТЗ §12-b). */
export function fmtShortDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  return `${d.getDate()} ${monthName(d.getMonth())}`;
}

/** Относительное время в формах макета: «Сегодня», «Вчера», «3 дня назад». */
export function fmtRelative(unixSec: number | null): string {
  if (!unixSec) return t('Никогда');
  const now = new Date();
  const d = new Date(unixSec * 1000);
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(now) - day(d)) / 86400000);
  if (diff <= 0) return t('Сегодня');
  if (diff === 1) return t('Вчера');
  if (diff < 7) return tp('day', diff);
  if (diff < 14) return t('Неделю назад');
  if (diff < 31) {
    const w = Math.round(diff / 7);
    return tp('week', w);
  }
  return fmtDate(unixSec);
}

export function fmtHours(sec: number): string {
  const h = sec / 3600;
  const num = h >= 10 ? Math.round(h) + '' : h.toFixed(1).replace(/\.0$/, '');
  return num + ' ' + t('ч');
}
