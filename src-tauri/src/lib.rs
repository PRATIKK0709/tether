use std::thread;
use std::time::Duration;
use sysinfo::Disks;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use serde::{Deserialize, Serialize};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::fs;
use std::sync::Mutex;
use std::sync::Arc;

#[derive(Serialize, Clone, Debug)]
struct DriveInfo {
    name: String,
    mount_point: String,
    total_space: u64,
    available_space: u64,
    is_removable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AppConfig {
    ignored_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ignored_extensions: vec![
                "plist".into(), "log".into(), "db".into(), "ldb".into(), 
                "lock".into(), "tmp".into(), "temp".into(), "crdownload".into(), 
                "part".into(), "ini".into(), "dat".into(), "shm".into(), "wal".into()
            ],
        }
    }
}

#[tauri::command]
fn get_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    disks.list().iter().map(|disk| {
        DriveInfo {
            name: disk.name().to_string_lossy().to_string(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            is_removable: disk.is_removable(),
        }
    }).collect()
}

// State to hold the watcher
struct AppState {
    watcher: Mutex<Option<RecommendedWatcher>>,
    backup_path: Mutex<Option<String>>,
    config: Mutex<AppConfig>,
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(config: AppConfig, state: tauri::State<AppState>, app_handle: tauri::AppHandle) -> Result<(), String> {
    *state.config.lock().unwrap() = config.clone();
    
    // Persist to disk
    let app_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    if !app_dir.exists() {
        let _ = fs::create_dir_all(&app_dir);
    }
    let config_path = app_dir.join("tether_config.json");
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(config_path, json).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
fn set_backup_path(path: String, state: tauri::State<AppState>) {
    *state.backup_path.lock().unwrap() = Some(path);
}

#[tauri::command]
fn start_watching(path: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    let path = Path::new(&path);
    if !path.exists() {
        return Err("Path does not exist".to_string());
    }


    let handle = app_handle.clone();
    
    // We need to access state inside the closure, so we grab the state handle.
    // Note: State<T> is cheap to clone.
    let state_handle = app_handle.state::<AppState>(); 
    // However, we can't easily move State into the closure if it strictly references 'r.
    // Instead we'll access the backup path via the app handle or just use a shared Arc if needed.
    // For simplicity, let's use the app handle to get the state inside the closure if possible,
    // or better, wrap the backup_path in an Arc<Mutex> outside the struct if this gets complex.
    // ACTUALLY: The best way is to clone the Arc inside the Struct if we could, but struct fields are private.
    // Let's use a standard Arc<Mutex> for the backup path to pass it in.
    
    // Re-architecting slightly for thread safety in closure:
    // We will retrieve the current backup path from the state inside the closure? 
    // State is not Send/Sync in a way that allows simple moving into a long-running closure?
    // Let's rely on the handle.
    
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        match res {
            Ok(event) => {
                     match event.kind {
                         notify::EventKind::Modify(_) | notify::EventKind::Create(_) => {
                             let paths = event.paths.clone();
                             let _ = handle.emit("file-changed", &paths);
                             
                             // Auto-Sync Logic
                             let state = handle.state::<AppState>();
                             let backup_path_guard = state.backup_path.lock().unwrap();
                             let config_guard = state.config.lock().unwrap(); // Lock config
                             
                             if let Some(ref dest_root) = *backup_path_guard {
                                 let backup_folder = Path::new(dest_root).join("Tether_Backups");
                                 if !backup_folder.exists() {
                                     let _ = std::fs::create_dir_all(&backup_folder);
                                 }

                                 for path in paths {
                                     // Filter out hidden files and common system directories/files
                                     if path.components().any(|c| {
                                         let s = c.as_os_str().to_string_lossy();
                                         s.starts_with('.') || // Hidden files/dirs (.Trash, .git, .DS_Store)
                                         s == "Library" || 
                                         s == "node_modules" || 
                                         s == "target" ||
                                         s == "AppData"
                                     }) {
                                         continue;
                                     }
                                     
                                     // Filter out ignored extensions from CONFIG
                                     if let Some(ext) = path.extension() {
                                         let ext_str = ext.to_string_lossy().to_lowercase();
                                         if config_guard.ignored_extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                                            continue;
                                         }
                                     }

                                     if path.is_file() {
                                         let file_name = path.file_name().unwrap();
                                         let dest_path = backup_folder.join(file_name);

                                         // Smart Sync: Check if file exists and is identical
                                         if dest_path.exists() {
                                             if let (Ok(src_meta), Ok(dest_meta)) = (path.metadata(), dest_path.metadata()) {
                                                 if src_meta.len() == dest_meta.len() {
                                                     // For now, size match is a strong enough indicator for a quick "resume" check.
                                                     // You could also check mod times, but size is usually sufficient for checking "did we finish copying?"
                                                     // or "has the file actually changed content?".
                                                      println!("Skipped (Up to date): {:?}", file_name);
                                                      continue;
                                                 }
                                             }
                                         }

                                         match std::fs::copy(&path, &dest_path) {
                                             Ok(_) => {
                                                 println!("Synced: {:?}", file_name);
                                                 let _ = handle.emit("file-synced", file_name.to_string_lossy().to_string());
                                             },
                                             Err(e) => {
                                                  // Only log relevant errors
                                                  println!("Sync failed: {:?} -> {:?}", file_name, e);
                                             },
                                         }
                                     }
                                 }
                             }
                         }
                         _ => {}
                     }
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }).map_err(|e| e.to_string())?;

    watcher.watch(path, RecursiveMode::Recursive).map_err(|e| e.to_string())?;

    // Store watcher in state to keep it alive
    let state = app_handle.state::<AppState>();
    *state.watcher.lock().unwrap() = Some(watcher);

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { 
            watcher: Mutex::new(None),
            backup_path: Mutex::new(None),
            config: Mutex::new(AppConfig::default()), 
        })
        .invoke_handler(tauri::generate_handler![get_drives, start_watching, set_backup_path, get_config, save_config])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Load Config
            let app_dir = app.path().app_data_dir().unwrap();
            let config_path = app_dir.join("tether_config.json");
            if config_path.exists() {
               if let Ok(content) = fs::read_to_string(config_path) {
                   if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                       let state = app.state::<AppState>();
                       *state.config.lock().unwrap() = config;
                   }
               }
            }

            // System Tray Setup
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).unwrap();
            let show_i = MenuItem::with_id(app, "show", "Show Tether", true, None::<&str>).unwrap();
            let menu = Menu::with_items(app, &[&show_i, &quit_i]).unwrap();

            let _tray = TrayIconBuilder::with_id("tray")
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app);
            
            // Spawn a background thread to monitor drives
            thread::spawn(move || {
                let mut previous_disks = Disks::new_with_refreshed_list();
                loop {
                    thread::sleep(Duration::from_secs(2));
                    let current_disks = Disks::new_with_refreshed_list();
                    
                    // Simple check: if count changes, or we want more granular diff
                    // For now, let's just emit the current list every time something changes
                     if current_disks.list().len() != previous_disks.list().len() {
                        let drive_list: Vec<DriveInfo> = current_disks.list().iter().map(|disk| {
                            DriveInfo {
                                name: disk.name().to_string_lossy().to_string(),
                                mount_point: disk.mount_point().to_string_lossy().to_string(),
                                total_space: disk.total_space(),
                                available_space: disk.available_space(),
                                is_removable: disk.is_removable(),
                            }
                        }).collect();
                        
                        // Emit event to frontend
                        let _ = handle.emit("drives-changed", &drive_list);
                        previous_disks = current_disks;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
