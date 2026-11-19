use std::sync::{Arc, Mutex};

use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

struct Running {
    inner: Arc<Mutex<Option<(tokio::runtime::Runtime, shop_api::Till)>>>,
}

fn main() {
    block_stop_signals();
    std::env::set_var("API_ADDR", "127.0.0.1:8788");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let till = runtime.block_on(shop_api::open()).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });
    let url: tauri::Url = till.url().parse().expect("till address");
    let slot = Arc::new(Mutex::new(Some((runtime, till))));
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
    let running = Running { inner: slot };

    tauri::Builder::default()
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
        .expect("start the till")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                let Some(running) = app.try_state::<Running>() else {
                    return;
                };
                stop_till(&running.inner);
            }
        });
}

fn stop_till(slot: &Mutex<Option<(tokio::runtime::Runtime, shop_api::Till)>>) {
    let Some((runtime, till)) = slot.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    runtime.block_on(till.stop());
}

fn block_stop_signals() {
    unsafe {
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}
