// Генерирует installer/lang/strings.nsh: переводы трёх строк нашего установщика
// (ярлык на рабочем столе, вопрос про данные, «лаунчер запущен») для штатных
// языков NSIS. Пять своих языков описаны в installer/lang/custom.nsh, потому что
// их нет в комплекте NSIS.
import fs from 'node:fs';
import path from 'node:path';

const OUT = path.join(process.cwd(), 'installer', 'lang', 'strings.nsh');

const LANGS = {
  LANG_POLISH: 'pl',
  LANG_UKRAINIAN: 'uk',
  LANG_BELARUSIAN: 'be',
  LANG_SERBIAN: 'sr',
  LANG_SLOVAK: 'sk',
  LANG_ESPERANTO: 'eo',
  LANG_ESTONIAN: 'et',
  LANG_FINNISH: 'fi',
  LANG_FRENCH: 'fr',
  LANG_GERMAN: 'de',
  LANG_LATVIAN: 'lv',
  LANG_LITHUANIAN: 'lt',
  LANG_SIMPCHINESE: 'zh-CN',
  LANG_JAPANESE: 'ja',
  LANG_UZBEK: 'uz',
  LANG_GEORGIAN: 'ka',
};

const STRINGS = [
  ['RunApp', 'Run Arbuz Launcher'],
  ['Desktop', 'Create desktop shortcut'],
  ['AskData', 'Also delete settings, profiles, worlds and accounts?'],
  ['Busy', 'The launcher is running. Close it and try again.'],
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function translateBatch(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    encodeURIComponent(tl) +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let attempt = 0; attempt < 6; attempt++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2000 * (attempt + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      if (typeof data === 'string') return [data];
      return null;
    } catch {
      await sleep(1000 * (attempt + 1));
    }
  }
  return null;
}

/** Экранирует строку под NSIS: кавычки заменяем, «$» удваиваем. */
function esc(s) {
  return String(s)
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/"/g, "'")
    .replace(/\$/g, '$$$$');
}

const lines = [
  '; Сгенерировано scripts/make-installer-strings.mjs — правьте скрипт, а не файл.',
  '',
];

for (const [define, tl] of Object.entries(LANGS)) {
  const res = await translateBatch(STRINGS.map(([, en]) => en), tl);
  if (!res) {
    console.log(`${tl}: не удалось перевести, останется английский`);
    continue;
  }
  lines.push(`!ifndef ${define}\n!define ${define} 0\n!endif`);
  STRINGS.forEach(([name], i) => {
    lines.push(`LangString ${name} \${${define}} "${esc(res[i] ?? STRINGS[i][1])}"`);
  });
  lines.push('');
  console.log(`${tl}: ok`);
  await sleep(300);
}

fs.writeFileSync(OUT, lines.join('\n'), 'utf8');
const bytes = [0xef, 0xbb, 0xbf, ...fs.readFileSync(OUT)];
fs.writeFileSync(OUT, Buffer.from(bytes));
console.log('written:', OUT);
