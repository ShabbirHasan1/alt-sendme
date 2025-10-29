use crate::core::types::{SendResult, SendOptions, AddrInfoOptions, apply_options, get_or_create_secret, AppHandle};
use anyhow::Context;
use data_encoding::HEXLOWER;
use iroh::{
    discovery::pkarr::PkarrPublisher,
    Endpoint, RelayMode,
};
use iroh_blobs::{
    api::{
        blobs::{AddPathOptions, ImportMode},
        Store, TempTag,
    },
    format::collection::Collection,
    provider::{
        events::{ConnectMode, EventMask, EventSender, RequestMode},
    },
    store::fs::FsStore,
    ticket::BlobTicket,
    BlobFormat, BlobsProtocol,
};
use n0_future::{task::AbortOnDropHandle, BufferedStreamExt};
use rand::Rng;
use std::{
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::{select, sync::mpsc};
use tracing::trace;
use walkdir::WalkDir;
use n0_future::StreamExt;

// Helper function to emit events through the app handle
fn emit_event(app_handle: &AppHandle, event_name: &str) {
    if let Some(handle) = app_handle {
        if let Err(e) = handle.emit_event(event_name) {
            tracing::warn!("Failed to emit event {}: {}", event_name, e);
        }
    }
}

// Helper function to emit progress events with payload
fn emit_progress_event(app_handle: &AppHandle, bytes_transferred: u64, total_bytes: u64, speed_bps: f64) {
    if let Some(handle) = app_handle {
        // Use a consistent event name
        let event_name = "transfer-progress";
        
        // Convert speed to integer (multiply by 1000 to preserve 3 decimal places)
        let speed_int = (speed_bps * 1000.0) as i64;
        
        // Create payload data as colon-separated string
        let payload = format!("{}:{}:{}", bytes_transferred, total_bytes, speed_int);
        
        // Emit the event with proper payload
        if let Err(e) = handle.emit_event_with_payload(event_name, &payload) {
            tracing::warn!("Failed to emit progress event: {}", e);
        }
    }
}

/// Start sharing a file or directory
pub async fn start_share(path: PathBuf, options: SendOptions, app_handle: AppHandle) -> anyhow::Result<SendResult> {
    tracing::info!("🚀 Starting share for path: {}", path.display());
    
    let secret_key = get_or_create_secret()?;
    let node_id = secret_key.public();
    
    // create a magicsocket endpoint
    let relay_mode: RelayMode = options.relay_mode.clone().into();
    
    let mut builder = Endpoint::builder()
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .secret_key(secret_key)
        .relay_mode(relay_mode.clone());
    
    if options.ticket_type == AddrInfoOptions::Id {
        builder = builder.add_discovery(PkarrPublisher::n0_dns());
    }
    if let Some(addr) = options.magic_ipv4_addr {
        builder = builder.bind_addr_v4(addr);
    }
    if let Some(addr) = options.magic_ipv6_addr {
        builder = builder.bind_addr_v6(addr);
    }

    // use a flat store - todo: use a partial in mem store instead
    let suffix = rand::rng().random::<[u8; 16]>();
    let cwd = std::env::current_dir()?;
    let blobs_data_dir = cwd.join(format!(".sendme-send-{}", HEXLOWER.encode(&suffix)));
    if blobs_data_dir.exists() {
        anyhow::bail!(
            "can not share twice from the same directory: {}",
            cwd.display(),
        );
    }
    // todo: remove this as soon as we have a mem store that does not require a temp dir,
    // or create a temp dir outside the current directory.
    if cwd.join(&path) == cwd {
        anyhow::bail!("can not share from the current directory");
    }

    let path2 = path.clone();
    let blobs_data_dir2 = blobs_data_dir.clone();
    let (progress_tx, progress_rx) = mpsc::channel(32);
    let app_handle_clone = app_handle.clone();
    
    let setup = async move {
        let t0 = Instant::now();
        tokio::fs::create_dir_all(&blobs_data_dir2).await?;

        let endpoint = builder.bind().await?;
        
        let store = FsStore::load(&blobs_data_dir2).await?;
        
        let blobs = BlobsProtocol::new(
            &store,
            Some(EventSender::new(
                progress_tx,
                EventMask {
                    connected: ConnectMode::Notify,
                    get: RequestMode::NotifyLog,
                    ..EventMask::DEFAULT
                },
            )),
        );

        tracing::info!("📦 Importing files...");
        let import_result = import(path2, blobs.store()).await?;
        let dt = t0.elapsed();
        tracing::info!("✅ Import complete in {:?}", dt);

        // Start the progress handler with the total file size
        let (ref _temp_tag, size, ref _collection) = import_result;
        let progress_handle = n0_future::task::spawn(show_provide_progress_with_logging(
            progress_rx,
            app_handle_clone,
            size, // Pass the total file size
        ));

        let router = iroh::protocol::Router::builder(endpoint)
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();

        // wait for the endpoint to figure out its address before making a ticket
        let ep = router.endpoint();
        tokio::time::timeout(Duration::from_secs(30), async move {
            if !matches!(relay_mode, RelayMode::Disabled) {
                let _ = ep.online().await;
            }
        })
        .await?;

        anyhow::Ok((router, import_result, dt, blobs_data_dir2, store, progress_handle))
    };
    
    let (router, (temp_tag, size, _collection), _dt, _blobs_data_dir, store, progress_handle) = select! {
        x = setup => x?,
        _ = tokio::signal::ctrl_c() => {
            anyhow::bail!("Operation cancelled");
        }
    };
    let hash = temp_tag.hash();

    // make a ticket
    let mut addr = router.endpoint().node_addr();
    apply_options(&mut addr, options.ticket_type);
    
    let ticket = BlobTicket::new(addr, hash, BlobFormat::HashSeq);
    let entry_type = if path.is_file() { "file" } else { "directory" };
    
    tracing::info!("✅ Share started successfully! Entry type: {}, size: {} bytes, ready to accept connections", entry_type, size);

    // Return the result - CRITICAL: Keep router, temp_tag, store, and progress_handle alive
    Ok(SendResult {
        ticket: ticket.to_string(),
        hash: hash.to_hex().to_string(),
        size,
        entry_type: entry_type.to_string(),
        router,           // Keeps server running and protocols active
        temp_tag,         // Prevents data GC
        blobs_data_dir,   // For cleanup
        _progress_handle: AbortOnDropHandle::new(progress_handle), // Keeps event channel open
        _store: store,    // Keeps blob storage alive
    })
}

/// Import from a file or directory into the database.
///
/// The returned tag always refers to a collection. If the input is a file, this
/// is a collection with a single blob, named like the file.
///
/// If the input is a directory, the collection contains all the files in the
/// directory.
async fn import(
    path: PathBuf,
    db: &Store,
) -> anyhow::Result<(TempTag, u64, Collection)> {
    let parallelism = num_cpus::get();
    let path = path.canonicalize()?;
    anyhow::ensure!(path.exists(), "path {} does not exist", path.display());
    let root = path.parent().context("context get parent")?;
    // walkdir also works for files, so we don't need to special case them
    let files = WalkDir::new(path.clone()).into_iter();
    // flatten the directory structure into a list of (name, path) pairs.
    // ignore symlinks.
    let data_sources: Vec<(String, PathBuf)> = files
        .map(|entry| {
            let entry = entry?;
            if !entry.file_type().is_file() {
                // Skip symlinks. Directories are handled by WalkDir.
                return Ok(None);
            }
            let path = entry.into_path();
            let relative = path.strip_prefix(root)?;
            let name = canonicalized_path_to_string(relative, true)?;
            anyhow::Ok(Some((name, path)))
        })
        .filter_map(Result::transpose)
        .collect::<anyhow::Result<Vec<_>>>()?;
    
    // import all the files, using num_cpus workers, return names and temp tags
    let mut names_and_tags = n0_future::stream::iter(data_sources)
        .map(|(name, path)| {
            let db = db.clone();
            async move {
                let import = db.add_path_with_opts(AddPathOptions {
                    path,
                    mode: ImportMode::TryReference,
                    format: iroh_blobs::BlobFormat::Raw,
                });
                let mut stream = import.stream().await;
                let mut item_size = 0;
                let temp_tag = loop {
                    let item = stream
                        .next()
                        .await
                        .context("import stream ended without a tag")?;
                    trace!("importing {name} {item:?}");
                    match item {
                        iroh_blobs::api::blobs::AddProgressItem::Size(size) => {
                            item_size = size;
                        }
                        iroh_blobs::api::blobs::AddProgressItem::CopyProgress(_) => {
                            // Skip progress updates for library version
                        }
                        iroh_blobs::api::blobs::AddProgressItem::CopyDone => {
                            // Skip progress updates for library version
                        }
                        iroh_blobs::api::blobs::AddProgressItem::OutboardProgress(_) => {
                            // Skip progress updates for library version
                        }
                        iroh_blobs::api::blobs::AddProgressItem::Error(cause) => {
                            anyhow::bail!("error importing {}: {}", name, cause);
                        }
                        iroh_blobs::api::blobs::AddProgressItem::Done(tt) => {
                            break tt;
                        }
                    }
                };
                anyhow::Ok((name, temp_tag, item_size))
            }
        })
        .buffered_unordered(parallelism)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    
    names_and_tags.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    // total size of all files
    let size = names_and_tags.iter().map(|(_, _, size)| *size).sum::<u64>();
    // collect the (name, hash) tuples into a collection
    // we must also keep the tags around so the data does not get gced.
    let (collection, tags) = names_and_tags
        .into_iter()
        .map(|(name, tag, _)| ((name, tag.hash()), tag))
        .unzip::<_, _, Collection, Vec<_>>();
    let temp_tag = collection.clone().store(db).await?;
    // now that the collection is stored, we can drop the tags
    // data is protected by the collection
    drop(tags);
    Ok((temp_tag, size, collection))
}

/// This function converts an already canonicalized path to a string.
///
/// If `must_be_relative` is true, the function will fail if any component of the path is
/// `Component::RootDir`
///
/// This function will also fail if the path is non canonical, i.e. contains
/// `..` or `.`, or if the path components contain any windows or unix path
/// separators.
pub fn canonicalized_path_to_string(
    path: impl AsRef<Path>,
    must_be_relative: bool,
) -> anyhow::Result<String> {
    let mut path_str = String::new();
    let parts = path
        .as_ref()
        .components()
        .filter_map(|c| match c {
            Component::Normal(x) => {
                let c = match x.to_str() {
                    Some(c) => c,
                    None => return Some(Err(anyhow::anyhow!("invalid character in path"))),
                };

                if !c.contains('/') && !c.contains('\\') {
                    Some(Ok(c))
                } else {
                    Some(Err(anyhow::anyhow!("invalid path component {:?}", c)))
                }
            }
            Component::RootDir => {
                if must_be_relative {
                    Some(Err(anyhow::anyhow!("invalid path component {:?}", c)))
                } else {
                    path_str.push('/');
                    None
                }
            }
            _ => Some(Err(anyhow::anyhow!("invalid path component {:?}", c))),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let parts = parts.join("/");
    path_str.push_str(&parts);
    Ok(path_str)
}

/// Enhanced progress handler with detailed logging for debugging
async fn show_provide_progress_with_logging(
    mut recv: mpsc::Receiver<iroh_blobs::provider::events::ProviderMessage>,
    app_handle: AppHandle,
    total_file_size: u64,
) -> anyhow::Result<()> {
    use n0_future::FuturesUnordered;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    
    let mut tasks = FuturesUnordered::new();
    
    // Track transfer state per request
    #[derive(Clone)]
    struct TransferState {
        start_time: Instant,
        total_size: u64,
        last_offset: u64, // Track the last reported offset for this request
        index: u64, // Track the blob index to filter out metadata blobs
    }
    
    // Global cumulative tracking across all requests
    let cumulative_bytes: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let transfer_start_time: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let active_file_requests: Arc<Mutex<u64>> = Arc::new(Mutex::new(0)); // Count of active file (not metadata) requests
    
    let transfer_states: Arc<Mutex<std::collections::HashMap<(u64, u64), TransferState>>> = 
        Arc::new(Mutex::new(std::collections::HashMap::new()));
    
    loop {
        tokio::select! {
            biased;
            item = recv.recv() => {
                let Some(item) = item else {
                    break;
                };

                match item {
                    iroh_blobs::provider::events::ProviderMessage::ClientConnectedNotify(_msg) => {
                        // Client connected - silent
                    }
                    iroh_blobs::provider::events::ProviderMessage::ConnectionClosed(_msg) => {
                        // Connection closed - silent
                    }
                    iroh_blobs::provider::events::ProviderMessage::GetRequestReceivedNotify(msg) => {
                        let connection_id = msg.connection_id;
                        let request_id = msg.request_id;
                        
                        // Clone app_handle and state for the task
                        let app_handle_task = app_handle.clone();
                        let transfer_states_task = transfer_states.clone();
                        let cumulative_bytes_task = cumulative_bytes.clone();
                        let transfer_start_time_task = transfer_start_time.clone();
                        let active_file_requests_task = active_file_requests.clone();
                        
                        // Spawn a task to monitor this request
                        let mut rx = msg.rx;
                        tasks.push(async move {
                            
                            let mut transfer_started = false;
                            
                            while let Ok(Some(update)) = rx.recv().await {
                                match update {
                                    iroh_blobs::provider::events::RequestUpdate::Started(m) => {
                                        if !transfer_started {
                                            // Determine if this is a file blob (index >= 2) or metadata blob (index < 2)
                                            // Index 0: collection root hash
                                            // Index 1: hash sequence blob
                                            // Index 2+: actual file data
                                            let is_file_request = m.index >= 2;
                                            
                                            // Store transfer state
                                            transfer_states_task.lock().await.insert(
                                                (connection_id, request_id),
                                                TransferState {
                                                    start_time: Instant::now(),
                                                    total_size: total_file_size,
                                                    last_offset: 0,
                                                    index: m.index,
                                                }
                                            );
                                            
                                            if is_file_request {
                                                // Increment active file request counter
                                                let mut active = active_file_requests_task.lock().await;
                                                
                                                // Reset cumulative bytes when first file request of new connection starts
                                                if *active == 0 {
                                                    let mut cumulative = cumulative_bytes_task.lock().await;
                                                    *cumulative = 0;
                                                    let mut start_time = transfer_start_time_task.lock().await;
                                                    *start_time = None; // Will be set below
                                                }
                                                
                                                *active += 1;
                                            }
                                            
                                            // Set global transfer start time if not already set
                                            let mut start_time = transfer_start_time_task.lock().await;
                                            if start_time.is_none() {
                                                *start_time = Some(Instant::now());
                                                emit_event(&app_handle_task, "transfer-started");
                                            }
                                            
                                            transfer_started = true;
                                        }
                                    }
                                    iroh_blobs::provider::events::RequestUpdate::Progress(m) => {
                                        if !transfer_started {
                                            emit_event(&app_handle_task, "transfer-started");
                                            transfer_started = true;
                                        }
                                        
                                        // Update cumulative progress ONLY for file requests (index >= 2), not metadata
                                        if let Some(state) = transfer_states_task.lock().await.get_mut(&(connection_id, request_id)) {
                                            // Only count progress for actual file blobs (index >= 2)
                                            if state.index >= 2 {
                                                // Calculate bytes transferred since last update for this request
                                                let bytes_added = m.end_offset.saturating_sub(state.last_offset);
                                                state.last_offset = m.end_offset;
                                                
                                                // Add to cumulative total
                                                let mut cumulative = cumulative_bytes_task.lock().await;
                                                *cumulative += bytes_added;
                                                let current_cumulative = *cumulative;
                                                
                                                // Calculate overall speed and emit progress
                                                let start_time = transfer_start_time_task.lock().await;
                                                if let Some(start) = *start_time {
                                                    let elapsed = start.elapsed().as_secs_f64();
                                                    let speed_bps = if elapsed > 0.0 {
                                                        current_cumulative as f64 / elapsed
                                                    } else {
                                                        0.0
                                                    };
                                                    
                                                    emit_progress_event(
                                                        &app_handle_task,
                                                        current_cumulative,
                                                        total_file_size,
                                                        speed_bps
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    iroh_blobs::provider::events::RequestUpdate::Completed(_m) => {
                                        if transfer_started {
                                            // Clean up state and check if all FILE requests are complete
                                            let (had_state, is_file_request, active_file_count, _cumulative_bytes) = {
                                                let mut states = transfer_states_task.lock().await;
                                                let state = states.remove(&(connection_id, request_id));
                                                let is_file_request = state.as_ref().map(|s| s.index >= 2).unwrap_or(false);
                                                let had_state = state.is_some();
                                                
                                                // Decrement active file request counter if this was a file request
                                                let mut active = active_file_requests_task.lock().await;
                                                if is_file_request {
                                                    *active = active.saturating_sub(1);
                                                }
                                                let active_file_count = *active;
                                                
                                                let cumulative_bytes = *cumulative_bytes_task.lock().await;
                                                (had_state, is_file_request, active_file_count, cumulative_bytes)
                                            };
                                            
                                            // Emit transfer-completed when all FILE requests are done
                                            if active_file_count == 0 && had_state {
                                                tracing::info!("✅ Transfer completed");
                                                emit_event(&app_handle_task, "transfer-completed");
                                            }
                                        }
                                    }
                                    iroh_blobs::provider::events::RequestUpdate::Aborted(_m) => {
                                        tracing::warn!("⚠️  Request aborted: connection_id {}", connection_id);
                                        if transfer_started {
                                            // Clean up state and check if all FILE requests are complete
                                            let (had_state, is_file_request, active_file_count, _cumulative_bytes) = {
                                                let mut states = transfer_states_task.lock().await;
                                                let state = states.remove(&(connection_id, request_id));
                                                let is_file_request = state.as_ref().map(|s| s.index >= 2).unwrap_or(false);
                                                let had_state = state.is_some();
                                                
                                                // Decrement active file request counter if this was a file request
                                                let mut active = active_file_requests_task.lock().await;
                                                if is_file_request {
                                                    *active = active.saturating_sub(1);
                                                }
                                                let active_file_count = *active;
                                                
                                                let cumulative_bytes = *cumulative_bytes_task.lock().await;
                                                (had_state, is_file_request, active_file_count, cumulative_bytes)
                                            };
                                            
                                            // Emit transfer-completed when all FILE requests are done
                                            if active_file_count == 0 && had_state {
                                                emit_event(&app_handle_task, "transfer-completed");
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
            Some(_) = tasks.next(), if !tasks.is_empty() => {}
        }
    }
    
    // Wait for all request monitoring tasks to complete
    while tasks.next().await.is_some() {}
    
    Ok(())
}
