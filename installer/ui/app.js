import { mix, monet } from './theme.js';

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const PHASES_INSTALL = [
  ['Подготовка', 'Папка и доступ'],
  ['Файлы лаунчера', 'quarry.exe и uninstall.exe'],
  ['Ярлыки', 'Рабочий стол и «Пуск»'],
  ['Реестр', 'Список программ'],
];
const PHASES_REMOVE = [
  ['Подготовка', 'Проверка установки'],
  ['Ярлыки', 'Рабочий стол и «Пуск»'],
  ['Файлы', 'quarry.exe и uninstall.exe'],
  ['Реестр и данные', 'Список программ'],
];

const $ = (id) => document.getElementById(id);
const win = $('win');
const S = {
  dir: '',
  installedDir: null,
  env: null,
  view: 'opts',
  mode: 'install',
  busy: false,
  phases: [],
};

// Палитра установщика фиксирована: выбор акцента и темы убран из интерфейса.
const theme = { accent: '#5fb339', dark: true };

function applyTheme(animate = true) {
  if (!animate) win.classList.add('no-t');
  const roles = monet(theme.dark, theme.accent);
  for (const key of Object.keys(roles)) {
    win.style.setProperty('--' + key, roles[key]);
  }
  win.style.setProperty('--pri-soft', mix(roles.pri, roles.bg, 0.82));
  win.style.setProperty('--pri-dim', mix(roles.pri, roles.bg, 0.62));
  win.dataset.mode = theme.dark ? 'dark' : 'light';
  document.documentElement.style.colorScheme = theme.dark ? 'dark' : 'light';
  if (!animate) {
    requestAnimationFrame(() => win.classList.remove('no-t'));
  }
}

function show(view) {
  clearError();
  document.querySelectorAll('.step').forEach((el) => {
    el.classList.toggle('on', el.dataset.view === view);
  });
  S.view = view;
  $('main').scrollTop = 0;
  updateSteps(view);
}

function updateSteps(view) {
  const steps = [...document.querySelectorAll('#steps li')];
  const active = view === 'opts' || view === 'remove' ? 1 : view === 'progress' ? 2 : 3;
  steps.forEach((li, i) => {
    const n = i + 1;
    li.classList.toggle('on', n === active && view !== 'done');
    li.classList.toggle('done', n < active || view === 'done');
  });
}

function setPhases(labels) {
  const wrap = $('phases');
  wrap.textContent = '';
  S.phases = labels.map(([title, sub]) => {
    const row = document.createElement('div');
    row.className = 'ph';
    const dot = document.createElement('span');
    dot.className = 'dot';
    dot.innerHTML = '<svg viewBox="0 0 12 12"><path d="M2 6.2 4.8 9 10 3.4"/></svg>';
    const name = document.createElement('span');
    name.className = 'ph-t';
    name.textContent = title;
    const desc = document.createElement('span');
    desc.className = 'ph-s';
    desc.textContent = sub;
    row.append(dot, name, desc);
    wrap.appendChild(row);
    return row;
  });
}

function resetProgress(title, sub, labels) {
  $('pct').textContent = '0';
  $('bar').style.width = '0%';
  $('prog-title').textContent = title;
  $('prog-sub').textContent = sub;
  $('now-text').textContent = labels[0][0];
  setPhases(labels);
}

function onProgress(p) {
  const percent = Math.max(0, Math.min(100, Number(p.percent) || 0));
  $('pct').textContent = String(percent);
  $('bar').style.width = percent + '%';
  $('now-text').textContent = p.text || '';
  S.phases.forEach((row, i) => {
    row.classList.toggle('done', p.stage > i || percent >= 100);
    row.classList.toggle('on', p.stage === i && percent < 100);
  });
}

function showError(message) {
  const box = $('err');
  $('err-text').textContent = message;
  box.hidden = false;
  clearTimeout(showError.timer);
  showError.timer = setTimeout(() => {
    box.hidden = true;
  }, 9000);
}

function clearError() {
  clearTimeout(showError.timer);
  $('err').hidden = true;
}

function updatePath() {
  $('path').textContent = S.dir;
  $('rm-path').textContent = S.installedDir || S.dir;
}

function renderEnv(env) {
  $('ver').textContent = 'Версия ' + env.version;
  S.env = env;
  S.installedDir = env.installedDir || null;
  S.dir = env.defaultDir;
  updatePath();

  $('hint').textContent = 'Займёт около ' + env.payloadMb + ' МБ';

  if (env.uninstallMode) {
    S.mode = 'uninstall';
    $('tb-mode').textContent = 'Удаление';
    $('brand-eyebrow').textContent = 'Удаление';
    $('brand-sub').textContent = 'Удаляем лаунчер и данные';
    $('st1').textContent = 'Подтверждение';
    $('st1s').textContent = 'Проверка установки';
    $('st2').textContent = 'Удаление';
    $('st2s').textContent = 'Файлы и ярлыки';
    $('st3').textContent = 'Готово';
    $('st3s').textContent = 'Очистка завершена';
    $('install').hidden = true;
    $('rm-cancel').textContent = 'Отмена';
    show('remove');
    if (!env.installed) {
      $('rm-go').disabled = true;
      showError('Quarry не найден в списке установленных программ.');
    }
    return;
  }

  S.mode = 'install';
  if (env.installed) {
    const badge = $('badge');
    badge.hidden = false;
    badge.textContent =
      env.installedVersion === env.version
        ? 'Установлен · ' + env.installedVersion
        : 'Обновление · ' + env.installedVersion;
    $('install-label').textContent =
      env.installedVersion === env.version ? 'Переустановить' : 'Обновить';
    $('remove').hidden = false;
  } else {
    $('badge').hidden = true;
    $('install-label').textContent = 'Установить';
    $('remove').hidden = true;
  }
  show('opts');
}

async function startInstall() {
  if (S.busy) return;
  S.busy = true;
  resetProgress(
    'Устанавливаем Quarry',
    'Копируем файлы и готовим ярлыки. Это займёт пару секунд.',
    PHASES_INSTALL,
  );
  $('prog-eyebrow').textContent = 'Установка';
  show('progress');
  try {
    const done = await invoke('install', {
      opts: {
        dir: S.dir,
        desktop: $('desktop').checked,
        startMenu: $('start').checked,
        launch: $('run').checked,
      },
    });
    S.busy = false;
    S.installedDir = done.dir;
    S.dir = done.dir;
    updatePath();
    $('done-eyebrow').textContent = 'Установка завершена';
    $('done-title').textContent = 'Quarry установлен';
    $('done-sub').textContent = done.launched
      ? 'Лаунчер уже запущен. Приятной игры!'
      : 'Лаунчер готов к работе.';
    $('done-path').textContent = done.dir;
    $('done-path').hidden = false;
    $('launch').hidden = Boolean(done.launched);
    $('launch').disabled = false;
    $('launch').textContent = 'Запустить Quarry';
    $('open-folder').hidden = false;
    $('finish').textContent = 'Закрыть';
    show('done');
  } catch (e) {
    S.busy = false;
    show('opts');
    showError(String(e));
  }
}

async function startUninstall() {
  if (S.busy || !S.installedDir) return;
  S.busy = true;
  resetProgress(
    'Удаляем Quarry',
    'Убираем файлы, ярлыки и запись в списке программ.',
    PHASES_REMOVE,
  );
  $('prog-eyebrow').textContent = 'Удаление';
  show('progress');
  try {
    const done = await invoke('uninstall', {
      opts: {
        dir: S.installedDir,
        data: $('rm-data').checked,
      },
    });
    S.busy = false;
    S.installedDir = null;
    $('done-eyebrow').textContent = 'Удаление завершено';
    $('done-title').textContent = 'Quarry удалён';
    $('done-sub').textContent = done.dataRemoved
      ? 'Файлы, ярлыки, настройки и данные удалены.'
      : 'Файлы и ярлыки удалены. Настройки и миры сохранены.';
    $('done-path').textContent = S.dir;
    $('done-path').hidden = true;
    $('launch').hidden = true;
    $('open-folder').hidden = true;
    $('finish').textContent = 'Закрыть';
    show('done');
  } catch (e) {
    S.busy = false;
    show(S.mode === 'uninstall' ? 'remove' : 'opts');
    showError(String(e));
  }
}

function showRemove() {
  if (!S.installedDir) return;
  $('rm-path').textContent = S.installedDir;
  $('rm-data').checked = false;
  $('rm-cancel').textContent = S.mode === 'uninstall' ? 'Отмена' : 'Назад';
  show('remove');
}

async function closeApp() {
  if (S.busy) return;
  try {
    await invoke('win_close');
  } catch (e) {
    showError(String(e));
  }
}

function bind() {
  $('min').addEventListener('click', () => {
    void invoke('win_minimize').catch((e) => showError(String(e)));
  });
  $('close').addEventListener('click', () => void closeApp());
  $('finish').addEventListener('click', () => void closeApp());
  $('err-x').addEventListener('click', clearError);

  $('tb').addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    if (e.target.closest('.wc')) return;
    void invoke('win_drag').catch(() => {});
  });

  $('browse').addEventListener('click', () => {
    void (async () => {
      try {
        const picked = await invoke('pick_dir', { current: S.dir });
        if (picked) {
          S.dir = picked;
          updatePath();
        }
      } catch (e) {
        showError(String(e));
      }
    })();
  });

  $('install').addEventListener('click', () => void startInstall());
  $('remove').addEventListener('click', showRemove);
  $('rm-go').addEventListener('click', () => void startUninstall());
  $('rm-cancel').addEventListener('click', () => {
    if (S.mode === 'uninstall') {
      void closeApp();
    } else {
      show('opts');
    }
  });

  $('launch').addEventListener('click', () => {
    void (async () => {
      try {
        await invoke('launch');
        $('launch').disabled = true;
        $('launch').textContent = 'Quarry запущен';
      } catch (e) {
        showError(String(e));
      }
    })();
  });

  $('open-folder').addEventListener('click', () => {
    void invoke('open_folder', { path: S.installedDir || S.dir }).catch((e) =>
      showError(String(e)),
    );
  });

  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    if (S.busy) return;
    if (S.view === 'remove' && S.mode !== 'uninstall') {
      show('opts');
    } else if (S.view === 'opts' || S.view === 'done') {
      void closeApp();
    }
  });
}

async function boot() {
  applyTheme(false);
  bind();
  try {
    await listen('progress', (event) => onProgress(event.payload));
  } catch (e) {
    showError(String(e));
  }
  try {
    const env = await invoke('environment');
    renderEnv(env);
  } catch (e) {
    showError(String(e));
  }
}

void boot();
