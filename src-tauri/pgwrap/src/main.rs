//! Stands in for `postgres.exe` so the database never opens a terminal.
//!
//! Postgres is a console program. Windows gives it a terminal when it is
//! started from the shop window, and closing that terminal stops the database.
//! Every launch of postgres goes through this program, which starts the real
//! server with no window.
//!
//! initdb feeds the post-bootstrap SQL to that process on standard input.
//! Hiding the window without passing those handles makes postgres crash with
//! 0xC0000005 at "performing post-bootstrap initialization".

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
    hide_without_dropping_stdio(&mut command);
    command.status().ok()?.code()
}

fn server_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "postgres-server.exe"
    } else {
        "postgres-server"
    }
}

#[cfg(target_os = "windows")]
fn hide_without_dropping_stdio(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    // Explicit handles make Rust set STARTF_USESTDHANDLES. CREATE_NO_WINDOW
    // alone leaves the server with no standard input, and initdb's SQL pipe
    // never arrives.
    unsafe {
        command
            .stdin(stdio_from(STD_INPUT_HANDLE))
            .stdout(stdio_from(STD_OUTPUT_HANDLE))
            .stderr(stdio_from(STD_ERROR_HANDLE));
    }
    command.creation_flags(0x0800_0000);
}

#[cfg(target_os = "windows")]
const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
#[cfg(target_os = "windows")]
const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5;
#[cfg(target_os = "windows")]
const STD_ERROR_HANDLE: u32 = 0xFFFF_FFF4;
#[cfg(target_os = "windows")]
const DUPLICATE_SAME_ACCESS: u32 = 0x0000_0002;

#[cfg(target_os = "windows")]
unsafe extern "system" {
    fn GetStdHandle(kind: u32) -> *mut core::ffi::c_void;
    fn GetCurrentProcess() -> *mut core::ffi::c_void;
    fn DuplicateHandle(
        source_process: *mut core::ffi::c_void,
        source: *mut core::ffi::c_void,
        target_process: *mut core::ffi::c_void,
        target: *mut *mut core::ffi::c_void,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
}

#[cfg(target_os = "windows")]
unsafe fn stdio_from(kind: u32) -> std::process::Stdio {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    let invalid = -1isize as *mut core::ffi::c_void;
    let source = GetStdHandle(kind);
    if source.is_null() || source == invalid {
        return std::process::Stdio::null();
    }
    let mut duplicated = std::ptr::null_mut();
    let copied = DuplicateHandle(
        GetCurrentProcess(),
        source,
        GetCurrentProcess(),
        &mut duplicated,
        0,
        1,
        DUPLICATE_SAME_ACCESS,
    );
    if copied == 0 || duplicated.is_null() {
        return std::process::Stdio::null();
    }
    std::process::Stdio::from(OwnedHandle::from_raw_handle(duplicated))
}
