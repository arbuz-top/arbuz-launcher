#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod install;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Manager, State, Window};
use tauri_plugin_dialog::DialogExt;

struct AppState {
    cleanup: Mutex<Option<PathBuf>>,
    busy: AtomicBool,
}

#[tauri::command]
async fn environment() -> install::Env {
    install::environment()
}

#[tauri::command]
async fn pick_dir(app: tauri::AppHandle, current: String) -> Option<String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Куда установить Quarry");
    if !current.trim().is_empty() {
        dialog = dialog.set_directory(current);
    }
    dialog
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn install(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    opts: install::InstallOpts,
) -> install::Res<install::InstallDone> {
    state.busy.store(true, Ordering::SeqCst);
    let result = install::run_install(app, opts).await;
    state.busy.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
async fn uninstall(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    opts: install::UninstallOpts,
) -> install::Res<install::UninstallDone> {
    let dir = std::path::absolute(opts.dir.trim()).ok();
    state.busy.store(true, Ordering::SeqCst);
    let done = install::run_uninstall(app, opts).await;
    state.busy.store(false, Ordering::SeqCst);
    let done = done?;
    // Если запущен uninstall.exe из папки установки, её дочистит хвост после выхода.
    if let (Some(dir), Ok(exe)) = (dir, std::env::current_exe()) {
        if exe.parent() == Some(dir.as_path()) {
            if let Ok(mut slot) = state.cleanup.lock() {
                *slot = Some(dir);
            }
        }
    }
    Ok(done)
}

#[tauri::command]
async fn launch() -> install::Res<()> {
    install::launch_installed()
}

#[tauri::command]
async fn open_folder(path: String) -> install::Res<()> {
    install::open_folder(&path)
}

#[tauri::command]
async fn win_minimize(window: Window) -> install::Res<()> {
    window
        .minimize()
        .map_err(|e| install::Error::Msg(e.to_string()))
}

#[tauri::command]
async fn win_close(window: Window) -> install::Res<()> {
    window
        .close()
        .map_err(|e| install::Error::Msg(e.to_string()))
}

#[tauri::command]
async fn win_drag(window: Window) -> install::Res<()> {
    window
        .start_dragging()
        .map_err(|e| install::Error::Msg(e.to_string()))
}

fn main() {
    // Тихий режим из списка программ Windows: без окна и интерфейса.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--uninstall") && args.iter().any(|a| a == "--quiet") {
        let code = match install::run_uninstall_quiet() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Quarry Setup: {e}");
                1
            }
        };
        std::process::exit(code);
    }

    let built = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            cleanup: Mutex::new(None),
            busy: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            environment,
            pick_dir,
            install,
            uninstall,
            launch,
            open_folder,
            win_minimize,
            win_close,
            win_drag
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<AppState>().busy.load(Ordering::SeqCst) {
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!());

    let app = match built {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Quarry Setup: {e}");
            std::process::exit(1);
        }
    };

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            let dir = handle
                .state::<AppState>()
                .cleanup
                .lock()
                .ok()
                .and_then(|mut slot| slot.take());
            if let Some(dir) = dir {
                install::schedule_cleanup(&dir);
            }
        }
    });
}
