// Восстанавливает i18n-auto.json из готового i18n-auto.ts.
//
// translate-full.mjs хранит прогресс в JSON, но сам JSON в репозитории не
// лежит. Чтобы не переводить всё заново, вытаскиваем уже готовые строки.
import fs from 'node:fs';

const SRC = 'src/lib/i18n-auto.ts';
const OUT = 'src/lib/i18n-auto.json';
const text = fs.readFileSync(SRC, 'utf8');

const un = (s) => s.replace(/\\(\\|'|n)/g, (_, c) => (c === 'n' ? '\n' : c));
const progress = {};

const langRe = /^  (\w+): \{\n([\s\S]*?)^  \},$/gm;
let m;
while ((m = langRe.exec(text))) {
  const table = {};
  const re = /(?:^|\n)\s*'((?:[^'\\]|\\.)*)'\s*:\s*'((?:[^'\\]|\\.)*)',?/g;
  let e;
  while ((e = re.exec(m[2]))) table[un(e[1])] = un(e[2]);
  progress[m[1]] = table;
}

fs.writeFileSync(OUT, JSON.stringify(progress, null, 1), 'utf8');
for (const [lang, table] of Object.entries(progress)) {
  console.log(`${lang}: ${Object.keys(table).length}`);
}
