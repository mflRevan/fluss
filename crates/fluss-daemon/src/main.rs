//! fluss-daemon: the always-running part of Fluss (tray, shortcuts, audio,
//! speech engine, text insertion, overlay). Settings UI and CLI talk to it
//! over local IPC.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod dsp;
mod engine;
mod hotkeys;
mod inject;
mod ipc_server;
mod lifecycle;
mod logging;
mod overlay;
mod platform;
mod provision;
mod sounds;
mod tray;
mod updater;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--render-overlay") {
        let dir = args.get(i + 1).cloned().unwrap_or_else(|| "overlay-gallery".into());
        if let Err(e) = overlay::render_gallery(std::path::Path::new(&dir)) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--bench") {
        // Developer benchmark: `fluss-daemon --bench <model-id> <file.wav>`
        let id = args.get(i + 1).cloned().unwrap_or_default();
        let wav = args.get(i + 2).cloned().unwrap_or_default();
        if let Err(e) = engine::bench(&id, std::path::Path::new(&wav)) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    // --autostart: started at sign-in (model load is delayed a little).
    // --background: started by the installer after an update. Neither opens a window.
    let autostart = args.iter().any(|a| a == "--autostart");
    let quiet = autostart || args.iter().any(|a| a == "--background");
    logging::init();

    // Single instance: the IPC endpoint is the lock.
    let Some(listener) = claim_endpoint(quiet) else { return };
    log::info!(
        "Fluss {} starting (pid {}, {})",
        fluss_core::VERSION,
        std::process::id(),
        args.iter().find(|a| a.starts_with("--")).map(String::as_str).unwrap_or("manual start")
    );
    platform::init_process();

    let (tx, rx) = crossbeam_channel::unbounded();
    let hub = Arc::new(ipc_server::Hub::default());
    ipc_server::serve(listener, hub.clone(), tx.clone());
    hotkeys::spawn(tx.clone());
    let audio = audio::spawn(tx.clone());
    let engine = engine::spawn(tx.clone());
    let inject = inject::spawn(tx.clone());
    let overlay = overlay::Overlay::spawn();
    let (tray, tray_rx) = tray::channel();

    let quit = Arc::new(AtomicBool::new(false));
    let q2 = quit.clone();
    let tx2 = tx.clone();
    let controller = std::thread::Builder::new()
        .name("fluss-controller".into())
        .spawn(move || {
            let handles = app::Handles { audio, engine, inject, hub, overlay, tray };
            app::App::new(tx2, rx, handles, autostart).run();
            q2.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .expect("spawn controller");

    let first_run = !fluss_core::config::Config::load().map(|c| c.general.onboarded).unwrap_or(false);
    if first_run && !quiet {
        platform::open_ui(&[]);
    }
    tray::run_main_loop(tray_rx, tx, quit);
    let _ = controller.join();
    log::info!("bye");
    platform::hard_exit(0);
}

/// Take the IPC endpoint, or hand over to the daemon that has it. A daemon
/// that is still shutting down (or hung) holds the endpoint without answering:
/// wait for it, then stop it, instead of exiting silently.
fn claim_endpoint(quiet: bool) -> Option<interprocess::local_socket::Listener> {
    let start = std::time::Instant::now();
    let mut killed = false;
    loop {
        match fluss_core::ipc::listen() {
            Ok(l) => return Some(l),
            Err(e) => {
                if let Ok(mut c) = fluss_core::ipc::Client::connect() {
                    // Already running: a manual launch means "show me Fluss".
                    if !quiet {
                        let _ = c.call("ui.open", serde_json::json!({}));
                    }
                    return None;
                }
                let waited = start.elapsed();
                if !killed && waited > std::time::Duration::from_secs(3) {
                    log::warn!("IPC endpoint is taken but nobody answers ({e:#}); stopping the stale daemon");
                    platform::kill_stale_daemons();
                    killed = true;
                } else if waited > std::time::Duration::from_secs(8) {
                    log::error!("could not start: {e:#}");
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}
