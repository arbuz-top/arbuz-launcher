// Ставит одну версию сразу везде: package.json, tauri.conf.json, Cargo.toml.
//
//   node scripts/set-version.mjs 1.2.3
//   node scripts/set-version.mjs v1.2.3     (префикс v отбрасывается)
//
// Версия из Cargo.toml попадает в бинарник (env!("CARGO_PKG_VERSION")) и по ней
// лаунчер сравнивает себя с релизом на GitHub, поэтому менять её нужно целиком,
// а не только в tauri.conf.json.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const raw = (process.argv[2] ?? '').trim().replace(/^v/i, '');
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(raw)) {
  console.error('usage: node scripts/set-version.mjs 1.2.3');
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const files = {
  pkg: join(root, 'package.json'),
  conf: join(root, 'src-tauri', 'tauri.conf.json'),
  cargo: join(root, 'src-tauri', 'Cargo.toml'),
};

// package.json и tauri.conf.json — обычный JSON.
for (const file of [files.pkg, files.conf]) {
  const text = readFileSync(file, 'utf8');
  const json = JSON.parse(text);
  json.version = raw;
  writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`, 'utf8');
}

// Cargo.toml: трогаем только version в секции [package], не зависимости.
const cargo = readFileSync(files.cargo, 'utf8');
const pkgAt = cargo.indexOf('[package]');
if (pkgAt < 0) {
  console.error('Cargo.toml: не найдена секция [package]');
  process.exit(1);
}
const nextAt = cargo.indexOf('\n[', pkgAt + '[package]'.length);
const head = cargo.slice(0, pkgAt);
const body = cargo.slice(pkgAt, nextAt < 0 ? undefined : nextAt);
const tail = nextAt < 0 ? '' : cargo.slice(nextAt);
const updated = body.replace(/^version\s*=\s*"[^"]*"/m, `version = "${raw}"`);
if (updated === body && !body.includes(`version = "${raw}"`)) {
  console.error('Cargo.toml: не удалось заменить version в [package]');
  process.exit(1);
}
writeFileSync(files.cargo, head + updated + tail, 'utf8');

console.log(`версия: ${raw}`);
console.log(`  ${files.pkg}`);
console.log(`  ${files.conf}`);
console.log(`  ${files.cargo}`);
