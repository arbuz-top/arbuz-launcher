// Версия для CI: из тега (v1.2.3) или из поля release у workflow_dispatch.
// Пишет значение в $GITHUB_OUTPUT, чтобы его взяли шаги сборки.
import { appendFileSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';

const raw = (
  process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : process.env.RELEASE_INPUT || ''
).trim();
const v = raw.replace(/^v/i, '');

if (v) {
  const r = spawnSync(process.execPath, ['scripts/set-version.mjs', v], { stdio: 'inherit' });
  if (r.status !== 0) process.exit(r.status ?? 1);
}

const version = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
console.log(`version=${version}`);
