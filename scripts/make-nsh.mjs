import fs from 'node:fs';
import path from 'node:path';

const NSIS = path.join(process.env.LOCALAPPDATA, 'tauri', 'NSIS', 'Contrib', 'Language files');
const OUT = path.join('installer', 'lang');
fs.mkdirSync(OUT, { recursive: true });

const english = fs.readFileSync(path.join(NSIS, 'English.nsh'), 'utf8');
const russian = fs.readFileSync(path.join(NSIS, 'Russian.nsh'), 'utf8');

const TARGETS = {
  Kazakh: 'kk',
  Kyrgyz: 'ky',
  Tajik: 'tg',
  Inuktitut: 'iu',
  Olbanian: null,
};

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);
function protect(s) {
  const vars = [];
  const text = s.replace(/\$[^\s"]+/g, (v) => {
    const token = PUA(vars.length);
    vars.push([token, v]);
    return token;
  });
  return { text, vars };
}
const restore = (s, vars) => vars.reduce((a, [t, v]) => a.split(t).join(v), s);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function batch(list, tl) {
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
      const d = await r.json();
      if (Array.isArray(d) && d.length === list.length) return d;
      return null;
    } catch {
      await sleep(1000 * (a + 1));
    }
  }
  return null;
}

function olbize(s) {
  return s
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
}

function stringsOf(text) {
  return [...text.matchAll(/\$\{LangFileString\}\s+(\w+)\s+"((?:[^"\\]|\\.)*)"/g)].map((m) => ({
    name: m[1],
    value: m[2],
  }));
}

for (const [name, tl] of Object.entries(TARGETS)) {
  const base = tl ? english : russian;
  const items = stringsOf(base);
  let translated;
  if (tl) {
    translated = [];
    for (let i = 0; i < items.length; i += 25) {
      const chunk = items.slice(i, i + 25);
      const prot = chunk.map((x) => protect(x.value));
      const res = await batch(prot.map((p) => p.text), tl);
      if (!res) {
        chunk.forEach((x) => translated.push(x));
      } else {
        chunk.forEach((x, n) => translated.push({ name: x.name, value: restore(String(res[n] ?? ''), prot[n].vars) }));
      }
      await sleep(300);
    }
  } else {
    translated = items.map((x) => ({ name: x.name, value: olbize(x.value) }));
  }

  let out = base.replace(/\$\{LangFileString\}\s+(\w+)\s+"((?:[^"\\]|\\.)*)"/g, (all, key) => {
    const found = translated.find((x) => x.name === key);
    return found ? `\${LangFileString} ${key} "${found.value.replace(/"/g, '$\\"')}"` : all;
  });
  out = out.replace(/;Language:.*/, `;Language: ${name}`);
  out = out.replace(/!insertmacro LANGFILE [^\r\n]+/, `!insertmacro LANGFILE "${name}" "${name}" "${name}" "${name}"`);
  fs.writeFileSync(path.join(OUT, `${name}.nsh`), out, 'utf8');
  console.log(name, 'strings:', translated.length);
}
