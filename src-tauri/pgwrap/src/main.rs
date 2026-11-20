//! Stands in for `postgres.exe` so the database never opens a terminal.
//!
//! Postgres is a console program. Windows gives it a terminal when it is
//! started from the shop window, and closing that terminal stops the database.
//! Every launch of postgres goes through this program, which starts the real
//! server with no window.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    let code = relay().unwrap_or(1);
    std::process::exit(code);
}

fn relay() -> Option<i32> {
    let exe = std::env::current_exe().ok()?;
    let real = exe.parent()?.join(server_name());
    let mut command = std::process::Command::new(real);
    command.args(std::env::args_os().skip(1));
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW. The real server, and every backend it starts by
        // calling this program again, stays off the desktop.
        command.creation_flags(0x0800_0000);
    }
    command.status().ok()?.code()
}

fn server_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "postgres-server.exe"
    } else {
        "postgres-server"
    }
}
