// Машинные формы множественного числа для языков, у которых их не было.
// Ручные формы (ru/en/pl/uk/eo/zh) остаются в i18n.ts и имеют приоритет.
import fs from 'node:fs';

const LANGS = {
  de: 'de',
  fr: 'fr',
  fi: 'fi',
  et: 'et',
  be: 'be',
  sr: 'sr',
  sk: 'sk',
  ja: 'ja',
  tg: 'tg',
  uz: 'uz',
  ky: 'ky',
  lt: 'lt',
  lv: 'lv',
  iu: 'iu',
  kk: 'kk',
  ka: 'ka',
};

// one / many; форма few для этих языков не запрашивается — pluralForm её не вернёт.
const KEYS = {
  update: ['{n} update', '{n} updates'],
  profile: ['{n} profile', '{n} profiles'],
  day: ['{n} day ago', '{n} days ago'],
  week: ['{n} week ago', '{n} weeks ago'],
  download: ['{n} download', '{n} downloads'],
  homeStats: ['{n} profile · {h} played in total', '{n} profiles · {h} played in total'],
};

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);
function protect(s) {
  const vars = [];
  const text = s.replace(/\{[a-zA-Z0-9_]+\}/g, (v) => {
    const token = PUA(vars.length);
    vars.push([token, v]);
    return token;
  });
  return { text, vars };
}
const restore = (s, vars) => vars.reduce((acc, [t, v]) => acc.split(t).join(v), s);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function tr(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    encodeURIComponent(tl) +
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

const out = {};
for (const [lang, tl] of Object.entries(LANGS)) {
  const entries = [];
  for (const [key, [one, many]] of Object.entries(KEYS)) {
    entries.push({ key, form: 0, text: one }, { key, form: 1, text: many });
  }
  const prot = entries.map((e) => protect(e.text));
  const res = await tr(prot.map((p) => p.text), tl);
  if (!res) {
    console.log(`${lang}: failed`);
    continue;
  }
  out[lang] = {};
  entries.forEach((e, i) => {
    const val = restore(String(res[i] ?? ''), prot[i].vars).trim();
    if (!val) return;
    out[lang][e.key] ??= ['', '', ''];
    out[lang][e.key][e.form] = val;
  });
  for (const key of Object.keys(out[lang])) out[lang][key][2] = out[lang][key][1];
  console.log(`${lang}: done`);
  await sleep(500);
}

const esc = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n');
const lines = [
  'export const PLURALS_AUTO: Record<string, Partial<Record<string, [string, string, string]>>> = {',
];
for (const key of Object.keys(KEYS)) {
  lines.push(`  ${key}: {`);
  for (const [lang, table] of Object.entries(out)) {
    const f = table[key];
    if (!f || !f[0] || !f[1]) continue;
    lines.push(`    ${lang}: ['${esc(f[0])}', '${esc(f[1])}', '${esc(f[2])}'],`);
  }
  lines.push('  },');
}
lines.push('};');
lines.push('');
fs.writeFileSync('src/lib/i18n-plurals.ts', lines.join('\n'), 'utf8');
console.log('written src/lib/i18n-plurals.ts');
