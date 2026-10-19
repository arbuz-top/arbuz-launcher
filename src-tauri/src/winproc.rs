//! Детали запуска дочерних процессов на Windows.

/// Запрещает дочернему процессу открывать отдельное консольное окно.
///
/// В release-сборке лаунчер — GUI-приложение без собственной консоли
/// (`windows_subsystem = "windows"` в `main.rs`). Поэтому `java.exe`, `cmd.exe`
/// и другие консольные программы, запущенные без `CREATE_NO_WINDOW`, получают
/// каждый своё чёрное окно. В debug-сборке этого не видно: все пишут в общую
/// консоль родителя.
pub fn hide_console(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}
