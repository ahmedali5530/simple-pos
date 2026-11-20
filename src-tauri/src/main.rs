#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

struct Running {
    inner: Arc<Mutex<Option<(tokio::runtime::Runtime, shop_api::Till)>>>,
}

fn main() {
    #[cfg(unix)]
    block_stop_signals();
    std::env::set_var("API_ADDR", "127.0.0.1:8788");
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => fail(&format!("Could not start the shop.\n\n{err}")),
    };
    let till = match runtime.block_on(shop_api::open()) {
        Ok(till) => till,
        Err(err) => fail(&format!("Could not open the shop window.\n\n{err}")),
    };
    let url: tauri::Url = match till.url().parse() {
        Ok(url) => url,
        Err(_) => fail("Could not open the shop window.\n\nThe till address is not a URL."),
    };
    let slot = Arc::new(Mutex::new(Some((runtime, till))));
    #[cfg(unix)]
    {
        let watched = Arc::clone(&slot);
        std::thread::spawn(move || {
            let mut received = 0;
            unsafe {
                let mut set = std::mem::zeroed();
                libc::sigemptyset(&mut set);
                libc::sigaddset(&mut set, libc::SIGINT);
                libc::sigaddset(&mut set, libc::SIGTERM);
                libc::sigwait(&set, &mut received);
            }
            stop_till(&watched);
            std::process::exit(0);
        });
    }
    let running = Running { inner: slot };

    let app = match tauri::Builder::default()
        .manage(running)
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Shop")
                .inner_size(1280.0, 800.0)
                .resizable(true)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(err) => fail(&format!("Could not open the shop window.\n\n{err}")),
    };
    app.run(|app, event| {
        if let RunEvent::Exit = event {
            let Some(running) = app.try_state::<Running>() else {
                return;
            };
            stop_till(&running.inner);
        }
    });
}

fn fail(message: &str) -> ! {
    let log = write_startup_log(message);
    eprintln!("{message}");
    #[cfg(target_os = "windows")]
    {
        let shown = match &log {
            Some(path) => format!("{message}\n\nSaved to {}", path.display()),
            None => message.to_string(),
        };
        message_box("Shop", &clip(&shown, 1800));
    }
    let _ = log;
    std::process::exit(1);
}

#[cfg(target_os = "windows")]
fn clip(message: &str, max_chars: usize) -> String {
    if message.chars().count() <= max_chars {
        return message.to_string();
    }
    let end = message
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(message.len());
    format!("{}\n…", &message[..end])
}

fn write_startup_log(message: &str) -> Option<std::path::PathBuf> {
    let dir = std::env::var("TILL_HOME")
        .ok()
        .filter(|dir| !dir.trim().is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("LOCALAPPDATA")
                .ok()
                .filter(|dir| !dir.trim().is_empty())
                .map(|dir| std::path::PathBuf::from(dir).join("shop-till"))
        })
        .or_else(|| {
            std::env::var("XDG_DATA_HOME")
                .ok()
                .filter(|dir| !dir.trim().is_empty())
                .map(|dir| std::path::PathBuf::from(dir).join("shop-till"))
        })
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| std::path::PathBuf::from(home).join(".local/share/shop-till"))
        })?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("startup.log");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    use std::io::Write;
    let _ = writeln!(file, "{message}\n");
    Some(path)
}

#[cfg(target_os = "windows")]
fn message_box(title: &str, message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let mut text: Vec<u16> = OsStr::new(message).encode_wide().collect();
    text.push(0);
    let mut caption: Vec<u16> = OsStr::new(title).encode_wide().collect();
    caption.push(0);
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x10);
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(
        hwnd: *mut std::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        kind: u32,
    ) -> i32;
}

fn stop_till(slot: &Mutex<Option<(tokio::runtime::Runtime, shop_api::Till)>>) {
    let Some((runtime, till)) = slot.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    runtime.block_on(till.stop());
}

#[cfg(unix)]
fn block_stop_signals() {
    unsafe {
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}
