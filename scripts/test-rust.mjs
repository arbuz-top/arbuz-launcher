// Кроссплатформенный запуск тестов Rust: pnpm test.
import { spawnSync } from 'node:child_process';

const r = spawnSync('cargo', ['test', '--lib', '--manifest-path', 'src-tauri/Cargo.toml'], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
process.exit(r.status ?? 1);
