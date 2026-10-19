import fs from 'node:fs';

const FILE = 'src/lib/i18n.ts';
const text = fs.readFileSync(FILE, 'utf8');

const start = text.indexOf('const EN: Table = {');
const end = text.indexOf('\n};', start);
const re = /(?:^|\n)\s*'((?:[^'\\]|\\.)*)'\s*:/g;
const keys = [];
let m;
const un = (s) => s.replace(/\\(\\|'|n)/g, (_, c) => (c === 'n' ? '\n' : c));
while ((m = re.exec(text.slice(start, end)))) keys.push(un(m[1]));

const L = 'А-Яа-яЁё';
const pair = (s, from, to) =>
  s.replace(new RegExp(`(^|[^${L}])${from}(?=$|[^${L}])`, 'g'), (_all, p1) => p1 + to);

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

const esc = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n');
const lines = ['const OLB: Table = {'];
for (const k of keys) lines.push(`  '${esc(k)}': '${esc(olbize(k))}',`);
lines.push('};');

const os = text.indexOf('const OLB: Table = {');
const oe = text.indexOf('\n};', os);
fs.writeFileSync(FILE, text.slice(0, os) + lines.join('\n') + text.slice(oe + 3), 'utf8');
console.log('olb keys:', keys.length);
