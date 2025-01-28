#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use atomic_once_cell::AtomicLazy as Lazy;
use atomic_once_cell::AtomicOnceCell;
use eframe::egui::{self};
use enigo::*;
use async_std::task; // Import async-std task
use futures::FutureExt; // Import FutureExt for fuse
use std::sync::atomic::{AtomicBool, Ordering};

static RUNNER: AtomicOnceCell<async_std::task::JoinHandle<()>> = AtomicOnceCell::new();
static ENIGO: Lazy<async_std::sync::Mutex<Enigo>> = Lazy::new(App::create_enigo);
static RUNNING: AtomicBool = AtomicBool::new(false);
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false); // New flag for stopping

struct App;

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> App {
        App {}
    }

    fn create_enigo() -> async_std::sync::Mutex<Enigo> {
        async_std::sync::Mutex::new(Enigo::new(&Settings::default()).unwrap())
    }
}

async fn move_mouse() {
    while RUNNING.load(Ordering::Relaxed) {
        if STOP_REQUESTED.load(Ordering::Relaxed) {
            break; // Exit if stop is requested
        }
        {
            let mut enigo_lock = ENIGO.lock().await; // Lock once
            if let Err(e) = enigo_lock.move_mouse(1, 1, Coordinate::Rel) {
                println!("Error moving mouse: {:?}", e);
            }
        }
        async_std::task::sleep(std::time::Duration::from_secs(5)).await; // Sleep outside the lock
        {
            let mut enigo_lock = ENIGO.lock().await; // Lock once
            if let Err(e) = enigo_lock.move_mouse(-1, -1, Coordinate::Rel) {
                println!("Error moving mouse back: {:?}", e);
            }
        }
        println!("Mouse movement completed.");
    }
}

async fn press_key() {
    while RUNNING.load(Ordering::Relaxed) {
        if STOP_REQUESTED.load(Ordering::Relaxed) {
            break; // Exit if stop is requested
        }
        {
            let mut enigo_lock = ENIGO.lock().await; // Lock once
            if let Err(e) = enigo_lock.key(Key::F15, Direction::Click) {
                println!("Error pressing key: {:?}", e);
            }
        }
        async_std::task::sleep(std::time::Duration::from_secs(10)).await; // Sleep outside the lock
        println!("Key press completed.");
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Each frame:
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(
                egui::Layout::top_down_justified(egui::Align::Center),
                |ui| {
                    if ui.button(egui::RichText::new("Start").size(20.0)).clicked()
                        && RUNNING
                            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                            .is_ok()
                    {
                        STOP_REQUESTED.store(false, Ordering::Relaxed); // Reset stop request
                        let _ = RUNNER.set(task::spawn(async {
                            futures::select! {
                                _ = move_mouse().fuse() => {},
                                _ = press_key().fuse() => {},
                            }
                        }));
                    }
                    if ui.button(egui::RichText::new("Stop").size(20.0)).clicked()
                        && RUNNING
                            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                            .is_ok()
                    {
                        STOP_REQUESTED.store(true, Ordering::Relaxed); // Request to stop
                        println!("Stopped");
                    }
                },
            );
        });
    }
}

fn main() {
    // Initialize the static variables
    let _ = async_std::task::block_on(async {
        let view = eframe::egui::viewport::ViewportBuilder {
            resizable: Some(true),
            transparent: Some(false),
            drag_and_drop: Some(false),
            decorations: Some(true),
            active: Some(true),
            close_button: Some(true),
            mouse_passthrough: Some(false),
            maximize_button: Some(false),
            ..Default::default()
        };
        let options = eframe::NativeOptions {
            viewport: view,
            centered: true,
            multisampling: 8,
            vsync: true,
            ..Default::default()
        };

        let _ = eframe::run_native(
            "Mouse Shacker",
            options,
            Box::new(|cc| Ok(Box::new(App::new(cc)))),
        );
    });
}
