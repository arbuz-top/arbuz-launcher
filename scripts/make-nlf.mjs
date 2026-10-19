import fs from 'node:fs';
import path from 'node:path';

const NSIS = path.join(process.env.LOCALAPPDATA, 'tauri', 'NSIS', 'Contrib', 'Language files');
const OUT = path.join('installer', 'lang');
fs.mkdirSync(OUT, { recursive: true });

const read = (name) => fs.readFileSync(path.join(NSIS, name), 'utf8');
const english = read('English.nlf');
const russian = read('Russian.nlf');

const TARGETS = {
  Kazakh: { tl: 'kk', id: 1087 },
  Kyrgyz: { tl: 'ky', id: 1088 },
  Tajik: { tl: 'tg', id: 1064 },
  Inuktitut: { tl: 'iu', id: 1117 },
  Olbanian: { tl: null, id: 4096 },
};

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);
const protect = (s) => {
  const vars = [];
  const text = s.replace(/\$\([^)]*\)/g, (v) => {
    const token = PUA(vars.length);
    vars.push([token, v]);
    return token;
  });
  return { text, vars };
};
const restore = (s, vars) => vars.reduce((a, [t, v]) => a.split(t).join(v), s);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function trBatch(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    tl +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let a = 0; a < 6; a++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2000 * (a + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      return null;
    } catch {
      await sleep(1000 * (a + 1));
    }
  }
  return null;
}

function parseNlf(text) {
  const lines = text.split(/\r?\n/);
  const brand = lines.findIndex((l) => l.startsWith('# ^Branding'));
  const head = lines.slice(0, brand);
  const items = [];
  let i = brand;
  while (i < lines.length) {
    const l = lines[i];
    if (l.startsWith('# ^')) {
      const name = l.slice(3).trim();
      const value = lines[i + 1] ?? '';
      items.push({ name, value });
      i += 2;
    } else {
      i++;
    }
  }
  return { head, items };
}

const enParsed = parseNlf(english);
const ruParsed = parseNlf(russian);

const olbPair = (s, from, to) => s.replace(new RegExp(from, 'g'), to);
const olbize = (s) =>
  s
    .replace(/оро/g, 'ара')
    .replace(/оло/g, 'ала')
    .replace(/чн/g, 'шн')
    .replace(/жи/g, 'жы')
    .replace(/ши/g, 'шы')
    .replace(/я/g, 'йа')
    .replace(/Я/g, 'Йа')
    .split(/([^А-Яа-яЁё]+)/)
    .map((w) => {
      if (w.length < 3 || !/^[А-Яа-яЁё]+$/.test(w)) return w;
      if (/[А-ЯЁ]{2,}/.test(w)) return w;
      if (/[аеёиоуыэюяйьъ]$/i.test(w)) return w;
      return w + 'ъ';
    })
    .join('');

const CUSTOM = [
  ['RunApp', 'Run Arbuz Launcher'],
  ['Desktop', 'Create desktop shortcut'],
  ['AskData', 'Also delete settings, profiles, worlds and accounts?'],
  ['Busy', 'The launcher is running. Close it and try again.'],
];
const CUSTOM_RU = {
  RunApp: 'Запустить Arbuz Launcher',
  Desktop: 'Создать ярлык на рабочем столе',
  AskData: 'Удалить также настройки, профили, миры и аккаунты?',
  Busy: 'Лаунчер запущен. Закройте его и повторите.',
};

const customLines = [];
for (const [name, cfg] of Object.entries(TARGETS)) {
  let items;
  let custom;
  if (cfg.tl) {
    items = [];
    for (let i = 0; i < enParsed.items.length; i += 25) {
      const chunk = enParsed.items.slice(i, i + 25);
      const prot = chunk.map((x) => protect(x.value));
      const res = await trBatch(prot.map((p) => p.text), cfg.tl);
      if (!res) {
        console.log(name, 'batch failed', i);
        for (const x of chunk) items.push(x);
        continue;
      }
      chunk.forEach((x, n) => items.push({ name: x.name, value: restore(String(res[n] ?? ''), prot[n].vars) }));
      await sleep(300);
    }
    custom = [];
    for (let i = 0; i < CUSTOM.length; i += 4) {
      const chunk = CUSTOM.slice(i, i + 4);
      const res = await trBatch(chunk.map(([, v]) => v), cfg.tl);
      chunk.forEach(([k, v], n) => custom.push([k, res ? res[n] : v]));
      await sleep(300);
    }
  } else {
    items = ruParsed.items.map((x) => ({ name: x.name, value: olbize(x.value) }));
    custom = Object.entries(CUSTOM_RU).map(([k, v]) => [k, olbize(v)]);
  }

  const head = enParsed.head.slice();
  const idIndex = head.findIndex((l, i) => i > 0 && /^\d+$/.test(l.trim()) && head.slice(0, i).join('\n').includes('# Language ID'));
  if (idIndex >= 0) head[idIndex] = String(cfg.id);

  const out = [...head];
  for (const x of items) {
    out.push(`# ^${x.name}`);
    out.push(x.value);
  }
  fs.writeFileSync(path.join(OUT, `${name}.nlf`), out.join('\r\n'), 'utf8');
  console.log(name, 'items:', items.length);

  const def = name.toUpperCase();
  customLines.push(`!define LANG_${def} ${cfg.id}`);
  for (const [k, v] of custom) customLines.push(`LangString ${k} \${LANG_${def}} "${String(v).replace(/"/g, '$\\"')}"`);
}
fs.writeFileSync(path.join(OUT, 'custom.nsh'), '\uFEFF' + customLines.join('\r\n') + '\r\n', 'utf8');
console.log('done');
