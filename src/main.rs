#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use eframe::egui;
use enigo::*;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task;
use crossbeam_channel::{bounded, Sender};
use tracing::{info, debug, error};

const MOUSE_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(5);
const KEY_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_secs(10);

static RUNNING: AtomicBool = AtomicBool::new(false);

fn setup_logging() {
    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_target(false)
            .with_thread_ids(true)
            .with_line_number(true)
            .with_file(true)
            .init();
    }
}

struct AppState {
    enigo: Arc<Mutex<Enigo>>,
    stop_sender: Option<Sender<()>>,
    core_ids: Vec<core_affinity::CoreId>,
}

struct App {
    state: AppState,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(debug_assertions)]
        info!("Initializing App");
        let core_ids = core_affinity::get_core_ids().unwrap_or_default();
        Self {
            state: AppState {
                enigo: Arc::new(Mutex::new(Enigo::new(&Settings::default()).unwrap())),
                stop_sender: None,
                core_ids,
            },
        }
    }

    #[inline(always)]
    async fn move_mouse(enigo: Arc<Mutex<Enigo>>, stop_rx: crossbeam_channel::Receiver<()>) {
        while RUNNING.load(Ordering::Acquire) {
            if stop_rx.try_recv().is_ok() {
                #[cfg(debug_assertions)]
                info!("Stopping mouse movement loop");
                break;
            }

            {
                let mut enigo = enigo.lock();
                #[cfg(debug_assertions)]
                debug!("Moving mouse +1,+1");
                let _ = enigo.move_mouse(1, 1, Coordinate::Rel);
            } // MutexGuard automatically drops here
            
            spin_sleep::sleep(MOUSE_SLEEP_DURATION);
            
            {
                let mut enigo = enigo.lock();
                #[cfg(debug_assertions)]
                debug!("Moving mouse -1,-1");
                let _ = enigo.move_mouse(-1, -1, Coordinate::Rel);
            }
        }
    }

    #[inline(always)]
    async fn press_key(enigo: Arc<Mutex<Enigo>>, stop_rx: crossbeam_channel::Receiver<()>) {
        #[cfg(debug_assertions)]
        info!("Starting key press task");
        
        while RUNNING.load(Ordering::Acquire) {
            if stop_rx.try_recv().is_ok() {
                #[cfg(debug_assertions)]
                info!("Stopping key press task");
                break;
            }

            {
                let mut enigo = enigo.lock();
                #[cfg(debug_assertions)]
                debug!("Attempting to press F15 key");
                
                match enigo.key(Key::F15, Direction::Click) {
                    Ok(_) => {
                        #[cfg(debug_assertions)]
                        debug!("Successfully pressed F15 key");
                    },
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        error!("Failed to press F15 key: {:?}", e);
                    }
                }
            }
            
            #[cfg(debug_assertions)]
            debug!("Sleeping for {} seconds", KEY_SLEEP_DURATION.as_secs());
            
            spin_sleep::sleep(KEY_SLEEP_DURATION);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                let is_running = RUNNING.load(Ordering::Acquire);
                let button_text = if is_running { "Stop" } else { "Start" };
                
                if ui.button(egui::RichText::new(button_text).size(20.0)).clicked() {
                    if is_running {
                        if RUNNING.compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                            #[cfg(debug_assertions)]
                            info!("Stopping automation");
                            if let Some(sender) = self.state.stop_sender.take() {
                                let _ = sender.send(());
                            }
                        }
                    } else {
                        if RUNNING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                            #[cfg(debug_assertions)]
                            info!("Starting automation tasks");
                            let enigo = self.state.enigo.clone();
                            let enigo2 = self.state.enigo.clone();
                            let (stop_tx, stop_rx1) = bounded(1);
                            let stop_rx2 = stop_rx1.clone();
                            
                            self.state.stop_sender = Some(stop_tx);

                            let core_ids1 = self.state.core_ids.clone();
                            let core_ids2 = self.state.core_ids.clone();
                            
                            // Spawn mouse movement task
                            task::spawn(async move {
                                if !core_ids1.is_empty() {
                                    core_affinity::set_for_current(core_ids1[0]);
                                }
                                App::move_mouse(enigo, stop_rx1).await;
                            });

                            // Spawn key press task separately
                            task::spawn(async move {
                                if core_ids2.len() > 1 {
                                    core_affinity::set_for_current(core_ids2[1]);
                                }
                                App::press_key(enigo2, stop_rx2).await;
                            });
                        }
                    }
                }
            });
        });
    }
}

fn main() {
    setup_logging();
    #[cfg(debug_assertions)]
    info!("Application starting");
    
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    let options = eframe::NativeOptions {
        centered: true,
        multisampling: 0,
        vsync: true,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        ..Default::default()
    };

    eframe::run_native(
        "Mouse Shaker",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    ).unwrap();
}
