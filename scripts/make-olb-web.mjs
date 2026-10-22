// Олбанский из интернета: строки EN-таблицы отправляются албанизатору
// javer.kiev.ua/alban.php и складываются в src/lib/olb-cache.json.
//
// Кэш нужен по двум причинам: сайт чужой (не хочется дёргать его повторно)
// и перевод строки занимает ~0.5 с. Не переведённые строки остаются
// локальному алгоритму из make-olb.mjs.
//
//   node scripts/make-olb-web.mjs          — добрать отсутствующие строки
//   node scripts/make-olb-web.mjs --write  — плюс пересобрать таблицу OLB
import fs from 'node:fs';

const FILE = 'src/lib/i18n.ts';
const CACHE = 'src/lib/olb-cache.json';
const URL_BASE = 'https://javer.kiev.ua/alban.php?input=';
const DELAY = 250;
const TRIES = 3;

// --- cp1251: сайт принимает и отдаёт windows-1251 --------------------------
const dec = new TextDecoder('windows-1251');
const CP = {};
for (let b = 0x80; b <= 0xff; b++) {
  const c = dec.decode(new Uint8Array([b]));
  if (c && c !== '\uFFFD') CP[c] = b;
}
for (let b = 0x20; b < 0x7f; b++) CP[String.fromCharCode(b)] = b;

const encQ = (s) =>
  [...s]
    .map((ch) => '%' + (CP[ch] ?? 0x3f).toString(16).toUpperCase().padStart(2, '0'))
    .join('');

// --- ключи из EN-таблицы ---------------------------------------------------
const text = fs.readFileSync(FILE, 'utf8');
const start = text.indexOf('const EN: Table = {');
const end = text.indexOf('\n};', start);
const un = (s) => s.replace(/\\(\\|'|n)/g, (_, c) => (c === 'n' ? '\n' : c));
const re = /(?:^|\n)\s*'((?:[^'\\]|\\.)*)'\s*:/g;
const keys = [];
let m;
while ((m = re.exec(text.slice(start, end)))) keys.push(un(m[1]));

// --- локальный алгоритм (запасной вариант, как в make-olb.mjs) -------------
const L = 'А-Яа-яЁё';
const pair = (s, from, to) =>
  s.replace(new RegExp(`(^|[^${L}])${from}(?=$|[^${L}])`, 'g'), (_a, p1) => p1 + to);
const PAIRS = [
  ['Что', 'Што'], ['что', 'што'],
  ['Ещё', 'Исчо'], ['ещё', 'исчо'],
  ['Привет', 'Превед'], ['привет', 'превед'],
  ['Спасибо', 'Пасиба'], ['спасибо', 'пасиба'],
  ['Пожалуйста', 'Пжалста'], ['пожалуйста', 'пжалста'],
  ['Нет', 'Ниту'], ['нет', 'ниту'],
  ['Ошибка', 'Ашипка'], ['ошибка', 'ашипка'],
  ['Удалить', 'Удолить'], ['удалить', 'удолить'],
  ['Установить', 'Устанавить'], ['установить', 'устанавить'],
  ['Настройки', 'Настройке'], ['настройки', 'настройке'],
  ['Профиль', 'Прохиль'], ['профиль', 'прохиль'],
  ['Версия', 'Версыйа'], ['версия', 'версыйа'],
  ['Играть', 'Сыграть'], ['играть', 'сыграть'],
  ['Язык', 'Йазык'], ['язык', 'йазык'],
];

function olbize(s) {
  let out = s;
  for (const [from, to] of PAIRS) out = pair(out, from, to);
  out = out
    .replace(/оро/g, 'ара')
    .replace(/оло/g, 'ала')
    .replace(/его(?=$|[^А-Яа-яЁё])/g, 'иво')
    .replace(/ого(?=$|[^А-Яа-яЁё])/g, 'ово')
    .replace(/ться/g, 'цца')
    .replace(/тся/g, 'цца')
    .replace(/чн/g, 'шн')
    .replace(/жи/g, 'жы')
    .replace(/ши/g, 'шы')
    .replace(/я/g, 'йа')
    .replace(/Я/g, 'Йа');
  return out
    .split(/([^А-Яа-яЁё]+)/)
    .map((w) => {
      if (w.length < 3 || !/^[А-Яа-яЁё]+$/.test(w)) return w;
      if (/[А-ЯЁ]{2,}/.test(w)) return w;
      if (/[аеёиоуыэюяйьъ]$/i.test(w)) return w;
      return w + 'ъ';
    })
    .join('');
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Подстановки вида {name} сайт может испортить — прячем их в маркеры. */
function protect(s) {
  const vars = [];
  const body = s.replace(/\{[a-zA-Z0-9_]+\}/g, (v) => {
    vars.push(v);
    return `%%${vars.length}%%`;
  });
  return { body, vars };
}

function restore(s, vars) {
  let out = s;
  vars.forEach((v, i) => {
    out = out.split(`%%${i + 1}%%`).join(v);
    // Сайт мог разбить маркер пробелом.
    out = out.split(`%% ${i + 1} %%`).join(v);
  });
  return out;
}

async function albanize(body) {
  const r = await fetch(URL_BASE + encQ(body), {
    headers: {
      'User-Agent':
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    },
  });
  if (!r.ok) return null;
  const html = dec.decode(Buffer.from(await r.arrayBuffer()));
  const areas = html
    .split('<textarea')
    .slice(1)
    .map((x) => x.split('</textarea>')[0].replace(/^[^>]*>/, '').trim());
  return areas.length ? areas[areas.length - 1] : null;
}

const write = process.argv.includes('--write');
let cache = {};
if (fs.existsSync(CACHE)) {
  try {
    cache = JSON.parse(fs.readFileSync(CACHE, 'utf8'));
  } catch {
    cache = {};
  }
}

const todo = keys.filter((k) => !cache[k]);
console.log(`строк: ${keys.length}, в кэше: ${keys.length - todo.length}, добрать: ${todo.length}`);

let done = 0;
const failed = [];
for (const key of todo) {
  const { body, vars } = protect(key);
  let out = null;
  for (let i = 0; i < TRIES && !out; i++) {
    try {
      out = await albanize(body);
    } catch {
      out = null;
    }
    if (!out) await sleep(500 * (i + 1));
  }
  if (out && out.trim()) {
    // Сайт мог ничего не изменить (нет знакомых слов) — это тоже его ответ.
    cache[key] = restore(out, vars);
  } else {
    failed.push(key);
  }
  done++;
  if (done % 25 === 0) {
    fs.writeFileSync(CACHE, JSON.stringify(cache, null, 1), 'utf8');
    console.log(`${done}/${todo.length}`);
  }
  await sleep(DELAY);
}
fs.writeFileSync(CACHE, JSON.stringify(cache, null, 1), 'utf8');

const have = keys.filter((k) => cache[k]).length;
console.log(`в кэше после прогона: ${have}/${keys.length}`);

if (write) {
  const esc = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n');
  const lines = ['const OLB: Table = {'];
  for (const k of keys) lines.push(`  '${esc(k)}': '${esc(cache[k] ?? olbize(k))}',`);
  lines.push('};');
  const os = text.indexOf('const OLB: Table = {');
  const oe = text.indexOf('\n};', os);
  fs.writeFileSync(FILE, text.slice(0, os) + lines.join('\n') + text.slice(oe + 3), 'utf8');
  console.log('olb table written:', keys.length);
}
