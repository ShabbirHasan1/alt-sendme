use crate::state::{AppStateMutex, ShareHandle};
use sendme::{start_share, download, SendOptions, ReceiveOptions, RelayModeOption, AddrInfoOptions, AppHandle, EventEmitter};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{State, Emitter};

struct TauriEventEmitter {
    app_handle: tauri::AppHandle,
}

impl EventEmitter for TauriEventEmitter {
    fn emit_event(&self, event_name: &str) -> Result<(), String> {
        tracing::debug!("📡 Emitting event: {}", event_name);
        self.app_handle
            .emit(event_name, ())
            .map_err(|e| {
                tracing::error!("❌ Failed to emit event '{}': {}", event_name, e);
                e.to_string()
            })
    }
    
    fn emit_event_with_payload(&self, event_name: &str, payload: &str) -> Result<(), String> {
        tracing::debug!("📡 Emitting event '{}' with payload: {}...", event_name, &payload[..50.min(payload.len())]);
        self.app_handle
            .emit(event_name, payload)
            .map_err(|e| {
                tracing::error!("❌ Failed to emit event '{}' with payload: {}", event_name, e);
                e.to_string()
            })
    }
}

#[tauri::command]
pub async fn get_file_size(path: String) -> Result<u64, String> {
    tracing::info!("📏 Getting file size for path: {}", path);
    let path = PathBuf::from(path);
    
    if !path.exists() {
        tracing::warn!("❌ Path does not exist: {}", path.display());
        return Err("Path does not exist".to_string());
    }
    
    if path.is_file() {
        match std::fs::metadata(&path) {
            Ok(metadata) => {
                let size = metadata.len();
                tracing::info!("📄 File size: {} bytes ({:.2} MB)", size, size as f64 / 1_048_576.0);
                Ok(size)
            }
            Err(e) => {
                tracing::error!("❌ Failed to get file metadata: {}", e);
                Err(format!("Failed to get file metadata: {}", e))
            }
        }
    } else if path.is_dir() {
        tracing::info!("📁 Calculating directory size...");
        let mut total_size = 0u64;
        let mut file_count = 0u64;
        
        for entry in walkdir::WalkDir::new(&path) {
            match entry {
                Ok(entry) => {
                    if entry.file_type().is_file() {
                        if let Ok(metadata) = entry.metadata() {
                            total_size += metadata.len();
                            file_count += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("⚠️  Error walking directory: {}", e);
                }
            }
        }
        
        tracing::info!("📁 Directory size: {} bytes ({:.2} MB) across {} files", 
                      total_size, total_size as f64 / 1_048_576.0, file_count);
        Ok(total_size)
    } else {
        tracing::warn!("❌ Path is neither a file nor a directory: {}", path.display());
        Err("Path is neither a file nor a directory".to_string())
    }
}

#[tauri::command]
pub async fn start_sharing(
    path: String,
    state: State<'_, AppStateMutex>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!("🚀 Starting file sharing for path: {}", path);
    let path = PathBuf::from(path);
    
    let mut app_state = state.lock().await;
    if app_state.current_share.is_some() {
        tracing::warn!("⚠️  Already sharing a file. Please stop current share first.");
        return Err("Already sharing a file. Please stop current share first.".to_string());
    }
    
    if !path.exists() {
        tracing::error!("❌ Path does not exist: {}", path.display());
        return Err(format!("Path does not exist: {}", path.display()));
    }
    
    tracing::info!("📋 Configuring send options...");
    let options = SendOptions {
        relay_mode: RelayModeOption::Default,
        ticket_type: AddrInfoOptions::RelayAndAddresses,
        magic_ipv4_addr: None,
        magic_ipv6_addr: None,
    };
    
    tracing::info!("📡 Setting up event emitter...");
    let emitter = Arc::new(TauriEventEmitter {
        app_handle: app_handle.clone(),
    });
    let boxed_handle: AppHandle = Some(emitter);
    
    tracing::info!("🔄 Initiating share operation...");
    match start_share(path.clone(), options, boxed_handle).await {
        Ok(result) => {
            let ticket = result.ticket.clone();
            tracing::info!("✅ Share started successfully");
            tracing::info!("🎫 Generated ticket: {}...", &ticket[..50.min(ticket.len())]);
            app_state.current_share = Some(ShareHandle::new(ticket.clone(), path, result));
            Ok(ticket)
        }
        Err(e) => {
            tracing::error!("❌ Failed to start sharing: {}", e);
            Err(format!("Failed to start sharing: {}", e))
        },
    }
}

#[tauri::command]
pub async fn stop_sharing(
    state: State<'_, AppStateMutex>,
) -> Result<(), String> {
    tracing::info!("🛑 Stopping file sharing...");
    let mut app_state = state.lock().await;
    
    if let Some(mut share) = app_state.current_share.take() {
        tracing::info!("🔄 Stopping share session...");
        if let Err(e) = share.stop().await {
            tracing::error!("❌ Failed to stop sharing: {}", e);
            return Err(e);
        }
        tracing::info!("✅ Share session stopped successfully");
    } else {
        tracing::warn!("⚠️  No active share session to stop");
    }
    
    Ok(())
}

#[tauri::command]
pub async fn receive_file(
    ticket: String,
    output_path: String,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    tracing::info!("📥 receive_file command called");
    tracing::info!("🎫 Ticket: {}...", &ticket[..50.min(ticket.len())]);
    tracing::info!("📁 Output path: {}", output_path);
    
    let output_dir = PathBuf::from(output_path);
    let options = ReceiveOptions {
        output_dir: Some(output_dir),
        relay_mode: RelayModeOption::Default,
        magic_ipv4_addr: None,
        magic_ipv6_addr: None,
    };
    
    tracing::info!("📁 Output directory: {:?}", options.output_dir);
    tracing::info!("🚀 Starting download...");
    
    let emitter = Arc::new(TauriEventEmitter {
        app_handle: app_handle.clone(),
    });
    let boxed_handle: AppHandle = Some(emitter);
    
    match download(ticket, options, boxed_handle).await {
        Ok(result) => {
            tracing::info!("✅ Download completed successfully: {}", result.message);
            Ok(result.message)
        },
        Err(e) => {
            tracing::error!("❌ Failed to receive file: {}", e);
            Err(format!("Failed to receive file: {}", e))
        },
    }
}

#[tauri::command]
pub async fn get_sharing_status(
    state: State<'_, AppStateMutex>,
) -> Result<Option<String>, String> {
    tracing::debug!("📊 Getting sharing status...");
    let app_state = state.lock().await;
    let status = app_state.current_share.as_ref().map(|share| share.ticket.clone());
    
    if status.is_some() {
        tracing::debug!("✅ Active share session found");
    } else {
        tracing::debug!("❌ No active share session");
    }
    
    Ok(status)
}

#[tauri::command]
pub async fn check_path_type(path: String) -> Result<String, String> {
    tracing::debug!("🔍 Checking path type for: {}", path);
    let path = PathBuf::from(path);
    
    if !path.exists() {
        tracing::warn!("❌ Path does not exist: {}", path.display());
        return Err("Path does not exist".to_string());
    }
    
    if path.is_dir() {
        tracing::debug!("📁 Path is a directory");
        Ok("directory".to_string())
    } else if path.is_file() {
        tracing::debug!("📄 Path is a file");
        Ok("file".to_string())
    } else {
        tracing::warn!("❌ Path is neither a file nor a directory: {}", path.display());
        Err("Path is neither a file nor a directory".to_string())
    }
}

#[tauri::command]
pub async fn get_transport_status(
    state: State<'_, AppStateMutex>,
) -> Result<bool, String> {
    tracing::debug!("🚚 Getting transport status...");
    let app_state = state.lock().await;
    let is_transporting = app_state.is_transporting;
    
    if is_transporting {
        tracing::debug!("🔄 Transport is active");
    } else {
        tracing::debug!("⏸️  Transport is inactive");
    }
    
    Ok(is_transporting)
}
