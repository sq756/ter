mod db;
mod crypto;

use db::{Db, ServerConfig};
use crypto::Crypto;
use std::sync::Arc;
use anyhow::Result;
use russh::*;
use russh_sftp::client::SftpSession;
use std::future::Future;
use tauri::{AppHandle, State, Manager};
use tokio::sync::mpsc;
use tauri::Emitter;
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use uuid::Uuid;
use std::sync::OnceLock;
use base64::Engine;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

struct BackendLogger;

impl log::Log for BackendLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let log_msg = format!("[{}] {}: {}", record.level(), record.target(), record.args());
            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("backend-log", log_msg);
            }
            // Also print to stderr for local debugging
            eprintln!("[{}] {}: {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: BackendLogger = BackendLogger;

#[derive(Clone)]
struct Client {}

impl client::Handler for Client {
    type Error = anyhow::Error;

    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(true) }
    }
}

enum PtyControl {
    Resize(u32, u32),
}

struct AppState {
    pty_tx: tokio::sync::Mutex<Option<mpsc::Sender<String>>>,
    ctrl_tx: tokio::sync::Mutex<Option<mpsc::Sender<PtyControl>>>,
    session: tokio::sync::Mutex<Option<Arc<client::Handle<Client>>>>,
    agent_token: tokio::sync::Mutex<String>,
    db: tokio::sync::OnceCell<Db>,
    db_error: tokio::sync::Mutex<Option<String>>,
    crypto: tokio::sync::Mutex<Option<Crypto>>,
    model_path: tokio::sync::Mutex<Option<std::path::PathBuf>>,
    agent_port: Arc<tokio::sync::Mutex<Option<u16>>>,
    vnc_port: Arc<tokio::sync::Mutex<Option<u16>>>,
    agent_abort: Arc<tokio::sync::Mutex<Option<tokio::task::AbortHandle>>>,
    vnc_abort: Arc<tokio::sync::Mutex<Option<tokio::task::AbortHandle>>>,
}

#[tauri::command]
async fn get_active_ports(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let agent = state.agent_port.lock().await;
    let vnc = state.vnc_port.lock().await;
    Ok(serde_json::json!({
        "agent": *agent,
        "vnc": *vnc
    }))
}

#[tauri::command]
async fn set_model_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut path_guard = state.model_path.lock().await;
    *path_guard = Some(std::path::PathBuf::from(path));
    log::info!("Model path updated to: {:?}", *path_guard);
    Ok(())
}

#[tauri::command]
async fn get_model_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let path_guard = state.model_path.lock().await;
    Ok(path_guard.as_ref().map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn set_master_password(password: String, state: State<'_, AppState>) -> Result<(), String> {
    log::info!("Received set_master_password command");
    let crypto = tokio::task::spawn_blocking(move || {
        Crypto::new(&password)
    }).await.map_err(|e| e.to_string())?;

    let mut crypto_guard = state.crypto.lock().await;
    *crypto_guard = Some(crypto);
    log::info!("Master password set successfully.");
    Ok(())
}

async fn get_db(state: &State<'_, AppState>) -> Result<Db, String> {
    if let Some(db) = state.db.get() {
        Ok(db.clone())
    } else {
        log::debug!("Database not found in state, checking for error...");
        let err_guard = state.db_error.lock().await;
        match &*err_guard {
            Some(e) => {
                let err_msg = format!("Database initialization failed: {}", e);
                log::error!("{}", err_msg);
                Err(err_msg)
            },
            None => {
                log::warn!("Database not initialized yet.");
                Err("Database not initialized".to_string())
            },
        }
    }
}

#[tauri::command]
async fn save_server_config(config: ServerConfig, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let mut config = config;
    
    // Encrypt password if present
    if let Some(pass) = config.password_enc.as_ref() {
        let crypto_guard = state.crypto.lock().await;
        if let Some(crypto) = crypto_guard.as_ref() {
            config.password_enc = Some(crypto.encrypt(pass));
        } else {
            return Err("Master password not set".to_string());
        }
    }

    db.save_server(&config).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn list_server_configs(state: State<'_, AppState>) -> Result<Vec<ServerConfig>, String> {
    let db = get_db(&state).await?;
    db.list_servers().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_server_config(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.delete_server(&id).await.map_err(|e| e.to_string())
}

async fn deploy_agent(session: &client::Handle<Client>, token: &str, app_handle: &AppHandle) -> Result<(), String> {
    // 1. Kill existing agent if running and wait a bit
    let kill_cmd = "pkill -9 -f agent_linux_amd64 || true";
    let mut kill_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    kill_channel.exec(true, kill_cmd).await.map_err(|e| e.to_string())?;
    while let Some(_) = kill_channel.wait().await {}
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 2. Get home directory for absolute paths
    let mut home_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    home_channel.exec(true, "echo $HOME").await.map_err(|e| e.to_string())?;
    let mut home_dir = String::new();
    while let Some(msg) = home_channel.wait().await {
        if let russh::ChannelMsg::Data { data } = msg {
            home_dir.push_str(&String::from_utf8_lossy(&data));
        }
    }
    let home_dir = home_dir.trim();
    if home_dir.is_empty() {
        return Err("Failed to determine remote home directory".to_string());
    }

    // 3. Prepare remote directory and remove old file to avoid ETXTBSY
    let remote_path = format!("{}/.ter/agent_linux_amd64", home_dir);
    let prep_cmd = format!("mkdir -p {}/.ter && rm -f {}", home_dir, remote_path);
    let mut prep_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    prep_channel.exec(true, prep_cmd.as_str()).await.map_err(|e| e.to_string())?;
    while let Some(_) = prep_channel.wait().await {}

    // 4. Upload Agent binary via SFTP
    let sftp_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    sftp_channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    
    let sftp = SftpSession::new(sftp_channel.into_stream()).await.map_err(|e| format!("SFTP init error: {}", e))?;

    let res_dir = app_handle.path().resource_dir().map_err(|e| e.to_string())?;
    
    let possible_paths = [
        res_dir.join("agent_linux_amd64"),
        res_dir.join("_up_/ter_agent/agent_linux_amd64"),
        res_dir.join("resources/agent_linux_amd64"),
        std::env::current_dir().unwrap().join("../ter_agent/agent_linux_amd64"),
        std::env::current_dir().unwrap().join("ter_agent/agent_linux_amd64"),
    ];

    let mut local_path = None;
    for path in possible_paths {
        if path.exists() {
            local_path = Some(path);
            break;
        }
    }

    let local_path = local_path.ok_or_else(|| {
        format!("Local agent not found in any of the search paths. Resource dir was: {:?}", res_dir)
    })?;

    // 1. Incremental Check: If exists and size matches, skip upload
    let local_metadata = tokio::fs::metadata(&local_path).await.map_err(|e| e.to_string())?;
    let local_size = local_metadata.len();
    
    let mut should_upload = true;
    if let Ok(remote_metadata) = sftp.metadata(&remote_path).await {
        if let Some(remote_size) = remote_metadata.size {
            if remote_size == local_size {
                log::info!("Agent already exists on remote with matching size ({}), skipping upload.", remote_size);
                should_upload = false;
            }
        }
    }

    if should_upload {
        log::info!("Uploading agent ({} bytes)...", local_size);
        let mut local_file = tokio::fs::File::open(&local_path).await.map_err(|e| format!("Failed to open agent at {:?}: {}", local_path, e))?;
        
        let mut remote_file = sftp.create(&remote_path).await.map_err(|e| {
            format!("Failed to create remote file at {}. SFTP Error: {}", remote_path, e)
        })?;
        
        let mut buf = vec![0; 65536]; // Larger buffer for faster upload
        while let Ok(n) = local_file.read(&mut buf).await {
            if n == 0 { break; }
            tokio::io::AsyncWriteExt::write_all(&mut remote_file, &buf[..n]).await.map_err(|e| format!("Write error: {}", e))?;
        }
        tokio::io::AsyncWriteExt::flush(&mut remote_file).await.map_err(|e| e.to_string())?;
    }

    // 5. Set executable permission and start agent (Always ensure +x and restart)
    let run_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    let cmd = format!("chmod +x {remote_path} && TER_AGENT_TOKEN={token} nohup {remote_path} --port 34567 > /dev/null 2>&1 &", remote_path=remote_path, token=token);
    run_channel.exec(true, cmd.as_str()).await.map_err(|e| e.to_string())?;
    
    // Give it a second to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}

#[tauri::command]
async fn connect_to_ssh(host: String, port: u16, user: String, pass: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = client::Config::default();
    let config = Arc::new(config);
    let sh = Client {};
    log::info!("Connecting to {}:{} as user {}", host, port, user);
    let mut session = client::connect(config, (host.as_str(), port), sh).await.map_err(|e| {
        log::error!("Connection failed: {}", e);
        e.to_string()
    })?;

    log::info!("Authenticating...");
    let auth_res = session.authenticate_password(user, pass).await.map_err(|e| {
        log::error!("Authentication error: {}", e);
        e.to_string()
    })?;
    if !matches!(auth_res, russh::client::AuthResult::Success) {
        log::error!("Authentication failed: Invalid credentials");
        return Err("Authentication failed".to_string());
    }
    log::info!("Authentication successful.");

    // Generate random token for agent
    let token = Uuid::new_v4().to_string();
    *state.agent_token.lock().await = token.clone();

    // Deploy Agent
    log::info!("Deploying agent to remote host...");
    deploy_agent(&session, &token, &app_handle).await?;
    log::info!("Agent deployed and started.");

    let mut channel = session.channel_open_session().await.map_err(|e| {
        log::error!("Failed to open channel: {}", e);
        e.to_string()
    })?;
    channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;
    log::info!("Shell session requested.");

    let (tx, mut rx) = mpsc::channel::<String>(100);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<PtyControl>(10);
    
    *state.pty_tx.lock().await = Some(tx);
    *state.ctrl_tx.lock().await = Some(ctrl_tx);
    *state.session.lock().await = Some(Arc::new(session));

    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                Some(ctrl) = ctrl_rx.recv() => {
                    match ctrl {
                        PtyControl::Resize(cols, rows) => {
                            let _ = channel.window_change(cols, rows, 0, 0).await;
                        }
                    }
                }
                Some(data) = rx.recv() => {
                    let _ = channel.data(data.as_bytes()).await;
                }
                msg = channel.wait() => {
                    if let Some(msg) = msg {
                        match msg {
                            russh::ChannelMsg::Data { data } => {
                                let _ = app_handle_clone.emit("pty-data", data.to_vec());
                            }
                            russh::ChannelMsg::ExitStatus { .. } => break,
                            _ => {}
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    });

    // 6. Setup Local Port Forwarding for Agent (127.0.0.1:34567)
    let session_guard = state.session.lock().await;
    if let Some(session_arc) = session_guard.as_ref() {
        let session_clone = session_arc.clone();
        let app_handle_agent = app_handle.clone();
        let agent_port_clone = state.agent_port.clone();

        // Cancel previous agent tunnel task if it exists
        let agent_abort_field = state.agent_abort.clone();
        {
            let mut agent_abort_guard = agent_abort_field.lock().await;
            if let Some(handle) = agent_abort_guard.take() { handle.abort(); }
        }

        let agent_task = tokio::spawn(async move {
            match TcpListener::bind("127.0.0.1:0").await {
                Ok(listener) => {
                    let local_port = listener.local_addr().unwrap().port();
                    log::info!("Tunnel listening on 127.0.0.1:{} -> remote 127.0.0.1:34567", local_port);
                    
                    // Update AppState with assigned port
                    *agent_port_clone.lock().await = Some(local_port);

                    let _ = app_handle_agent.emit("agent-tunnel-opened", local_port);

                    while let Ok((mut stream, _)) = listener.accept().await {
                        let session_inner = session_clone.clone();
                        tokio::spawn(async move {
                            match session_inner.channel_open_direct_tcpip("127.0.0.1", 34567, "127.0.0.1", local_port as u32).await {
                                Ok(channel) => {
                                    let (mut reader, mut writer) = stream.split();
                                    let (mut chan_reader, mut chan_writer) = tokio::io::split(channel.into_stream());
                                    let _ = tokio::join!(
                                        tokio::io::copy(&mut reader, &mut chan_writer),
                                        tokio::io::copy(&mut chan_reader, &mut writer)
                                    );
                                }
                                Err(e) => {
                                    log::error!("Failed to open direct tcpip channel: {}", e);
                                }
                            }
                        });
                    }
                }
                Err(e) => log::error!("Failed to bind to dynamic tunnel port: {}", e),
            }
        });
        
        {
            let mut agent_abort_guard = agent_abort_field.lock().await;
            *agent_abort_guard = Some(agent_task.abort_handle());
        }

        // Tunnel for VNC (127.0.0.1:5901)
        let session_clone_vnc = session_arc.clone();
        let app_handle_vnc = app_handle.clone();
        let vnc_port_clone = state.vnc_port.clone();
        let vnc_abort_field = state.vnc_abort.clone();

        // Cancel previous VNC tunnel task
        {
            let mut vnc_abort_guard = vnc_abort_field.lock().await;
            if let Some(handle) = vnc_abort_guard.take() { handle.abort(); }
        }

        let vnc_task = tokio::spawn(async move {
            match TcpListener::bind("127.0.0.1:0").await {
                Ok(listener) => {
                    let local_port = listener.local_addr().unwrap().port();
                    log::info!("VNC Tunnel listening on 127.0.0.1:{} -> remote 127.0.0.1:5901", local_port);
                    
                    *vnc_port_clone.lock().await = Some(local_port);

                    let _ = app_handle_vnc.emit("vnc-tunnel-opened", local_port);

                    while let Ok((mut stream, _)) = listener.accept().await {
                        let session_inner = session_clone_vnc.clone();
                        tokio::spawn(async move {
                            match session_inner.channel_open_direct_tcpip("127.0.0.1", 5901, "127.0.0.1", local_port as u32).await {
                                Ok(channel) => {
                                    let (mut reader, mut writer) = stream.split();
                                    let (mut chan_reader, mut chan_writer) = tokio::io::split(channel.into_stream());
                                    let _ = tokio::join!(
                                        tokio::io::copy(&mut reader, &mut chan_writer),
                                        tokio::io::copy(&mut chan_reader, &mut writer)
                                    );
                                }
                                Err(e) => log::error!("VNC tunnel error: {}", e),
                            }
                        });
                    }
                }
                Err(e) => log::error!("Failed to bind to dynamic VNC tunnel port: {}", e),
            }
        });
        
        {
            let mut vnc_abort_guard = vnc_abort_field.lock().await;
            *vnc_abort_guard = Some(vnc_task.abort_handle());
        }
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ContextRequirement {
    require_screenshot: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Skill {
    id: Option<String>,
    name: String,
    icon: Option<String>,
    description: String,
    rpc: Option<String>,
    trigger: Option<String>,
    context_requirement: Option<ContextRequirement>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct SkillManifest {
    version: String,
    skills: Vec<Skill>,
}

#[tauri::command]
async fn load_remote_skills(state: State<'_, AppState>) -> Result<Vec<Skill>, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let skills_path = ".ter/skills.json";
    
    // Safety check: verify file size before reading to prevent OOM
    if let Ok(metadata) = sftp.metadata(skills_path).await {
        if let Some(size) = metadata.size {
            if size > 1024 * 1024 { // 1MB limit
                return Err(format!("skills.json is too large: {} bytes (max 1MB)", size));
            }
        }
    }

    // Check if file exists by attempting to open it
    match sftp.open(skills_path).await {
        Ok(mut remote_file) => {
            let mut content = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut remote_file, &mut content).await.map_err(|e| e.to_string())?;
            
            // Try to parse as SkillManifest first, then fallback to Vec<Skill>
            if let Ok(manifest) = serde_json::from_slice::<SkillManifest>(&content) {
                Ok(manifest.skills)
            } else if let Ok(skills) = serde_json::from_slice::<Vec<Skill>>(&content) {
                Ok(skills)
            } else {
                Err("Failed to parse skills.json as either Manifest or List".to_string())
            }
        }
        Err(_) => {
            // File doesn't exist or other error, return empty list gracefully
            log::info!("No skills.json found at {}", skills_path);
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
async fn write_pty(data: String, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(tx) = state.pty_tx.lock().await.as_ref() {
        let _ = tx.send(data).await;
    }
    Ok(())
}

#[tauri::command]
async fn resize_pty(cols: u32, rows: u32, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(tx) = state.ctrl_tx.lock().await.as_ref() {
        let _ = tx.send(PtyControl::Resize(cols, rows)).await;
    }
    Ok(())
}

#[tauri::command]
async fn get_agent_token(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.agent_token.lock().await.clone())
}

#[tauri::command]
async fn ls_remote(path: String, state: State<'_, AppState>) -> Result<Vec<RemoteFile>, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let entries = sftp.read_dir(&path).await.map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in entries {
        files.push(RemoteFile {
            name: entry.file_name(),
            is_dir: entry.file_type().is_dir(),
            size: entry.metadata().size.unwrap_or(0),
        });
    }
    Ok(files)
}

#[tauri::command]
async fn download_file(remote_path: String, local_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let mut remote_file = sftp.open(&remote_path).await.map_err(|e| e.to_string())?;
    let mut local_file = tokio::fs::File::create(&local_path).await.map_err(|e| e.to_string())?;

    tokio::io::copy(&mut remote_file, &mut local_file).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn upload_file(local_path: String, remote_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let mut local_file = tokio::fs::File::open(&local_path).await.map_err(|e| e.to_string())?;
    let mut remote_file = sftp.create(&remote_path).await.map_err(|e| e.to_string())?;

    tokio::io::copy(&mut local_file, &mut remote_file).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize)]
struct RemoteFile {
    name: String,
    is_dir: bool,
    size: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct PluginParameter {
    name: String,
    #[serde(rename = "type")]
    param_type: String,
    description: String,
    default: Option<serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct PluginManifest {
    name: String,
    description: String,
    command: String,
    parameters: Vec<PluginParameter>,
}

#[tauri::command]
async fn upload_ui_snapshot(base64_data: String, state: State<'_, AppState>) -> Result<String, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    // 1. Decode base64
    let raw_data = base64::engine::general_purpose::STANDARD
        .decode(base64_data.replace("data:image/png;base64,", ""))
        .map_err(|e: base64::DecodeError| e.to_string())?;

    // 2. Open SFTP session
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    // 3. Upload to /tmp/current_ui.png
    let remote_path = "/tmp/current_ui.png";
    let mut remote_file = sftp.create(remote_path).await.map_err(|e| e.to_string())?;
    tokio::io::AsyncWriteExt::write_all(&mut remote_file, &raw_data).await.map_err(|e| e.to_string())?;
    
    log::info!("UI Snapshot uploaded to remote: {}", remote_path);
    Ok(remote_path.to_string())
}

#[tauri::command]
async fn write_remote_text(text: String, remote_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().cloned().ok_or("No active SSH session")?;
    drop(session_guard); // Release lock early

    // Spawn the SFTP write operation to avoid blocking the caller
    tokio::spawn(async move {
        match session.channel_open_session().await {
            Ok(channel) => {
                if let Ok(_) = channel.request_subsystem(true, "sftp").await {
                    if let Ok(sftp) = SftpSession::new(channel.into_stream()).await {
                        if let Ok(mut remote_file) = sftp.create(&remote_path).await {
                            let _ = tokio::io::AsyncWriteExt::write_all(&mut remote_file, text.as_bytes()).await;
                        }
                    }
                }
            }
            Err(e) => log::error!("Failed to open session for write_remote_text: {}", e),
        }
    });
    
    Ok(())
}

#[tauri::command]
async fn get_skill_manifest() -> Result<serde_json::Value, String> {
    let project_root = std::env::current_dir().unwrap_or_default();
    let manifest_path = project_root.join(".ter/skills.json");
    
    if !manifest_path.exists() {
        return Err("Skill manifest not found".to_string());
    }

    let content = std::fs::read_to_string(manifest_path).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(json)
}

#[tauri::command]
async fn run_plugin(name: String, app: AppHandle) -> Result<String, String> {
    use std::process::Command;
    let home = std::env::var("HOME").map_err(|_| "Could not find HOME dir".to_string())?;
    let plugin_path = std::path::Path::new(&home).join(".ter/plugins").join(&name).join("manifest.yaml");
    
    if !plugin_path.exists() {
        return Err(format!("Plugin {} not found", name));
    }

    let content = std::fs::read_to_string(&plugin_path).map_err(|e| e.to_string())?;
    let manifest: PluginManifest = serde_yaml::from_str(&content).map_err(|e| e.to_string())?;

    log::info!("Executing plugin: {}", manifest.name);
    
    // Execute the command (Simple version for Dummy)
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &manifest.command]).output()
    } else {
        Command::new("sh").args(["-c", &manifest.command]).output()
    }.map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    // Attempt to parse stdout as JSON to support Flow B (Complex UI injection)
    let payload = serde_json::from_str::<serde_json::Value>(&stdout).unwrap_or(serde_json::json!(stdout));
    
    // Determine type from payload if it's an object, else default to 'text'
    let ui_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("text");
    let ui_message = payload.get("message").unwrap_or(&payload);

    let ui_payload = serde_json::json!({
        "type": ui_type,
        "title": manifest.name,
        "message": ui_message,
        "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
    });

    app.emit("plugin-ui-event", ui_payload).map_err(|e| e.to_string())?;

    Ok(stdout)
}

#[tauri::command]
async fn list_plugins() -> Result<Vec<PluginManifest>, String> {
    use std::fs;
    let home = std::env::var("HOME").map_err(|_| "Could not find HOME dir".to_string())?;
    let plugin_dir = std::path::Path::new(&home).join(".ter/plugins");
    
    if !plugin_dir.exists() {
        fs::create_dir_all(&plugin_dir).map_err(|e| e.to_string())?;
    }

    log::info!("Scanning for plugins in: {:?}", plugin_dir);
    let mut plugins = Vec::new();

    for entry in fs::read_dir(plugin_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("manifest.yaml");
            if manifest_path.exists() {
                let content = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
                match serde_yaml::from_str::<PluginManifest>(&content) {
                    Ok(manifest) => {
                        log::info!("Loaded plugin: {} - {}", manifest.name, manifest.description);
                        plugins.push(manifest);
                    }
                    Err(e) => log::error!("Failed to parse manifest at {:?}: {}", manifest_path, e),
                }
            }
        }
    }

    Ok(plugins)
}

async fn perform_ui_audit() -> Result<String, String> {
    use headless_chrome::{Browser, LaunchOptions};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    log::info!("Initiating Visual Audit (Sandbox Disabled)...");
    
    // 1. Force absolute path to snap directory in project root
    let project_root = std::env::current_dir().unwrap_or_default();
    let snap_dir = project_root.join("snap");
    if !snap_dir.exists() {
        fs::create_dir_all(&snap_dir).map_err(|e| format!("Failed to create snap dir: {}", e))?;
    }

    // 2. Launch Browser with Sandbox disabled (Crucial for Linux/CI/Xvfb)
    let options = LaunchOptions::default_builder()
        .sandbox(false)
        .build()
        .map_err(|e| format!("Launch options error: {}", e))?;

    let browser = Browser::new(options).map_err(|e| format!("Chrome launch failed: {}", e))?;
    let tab = browser.new_tab().map_err(|e| e.to_string())?;

    // 3. Navigate to Dev Server
    tab.navigate_to("http://localhost:5173").map_err(|e| e.to_string())?;
    tab.wait_until_navigated().map_err(|e| e.to_string())?;

    // Wait for rendering (UI Toasts, Charts)
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 4. Capture
    let png_data = tab.capture_screenshot(
        headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
        None,
        None,
        true
    ).map_err(|e| e.to_string())?;

    // 5. Save with timestamp to absolute path
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let filename = format!("ter_audit_{}.png", now);
    let path = snap_dir.join(&filename);
    
    fs::write(&path, png_data).map_err(|e| format!("Save failed to {:?}: {}", path, e))?;

    let path_str = path.to_string_lossy().into_owned();
    log::info!("UI Audit SUCCESS. Snapshot saved: {}", path_str);
    Ok(path_str)
}

#[tauri::command]
async fn ai_audit_ui() -> Result<String, String> {
    perform_ui_audit().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize our custom logger
    if let Err(e) = log::set_logger(&LOGGER) {
        eprintln!("Failed to set logger: {}", e);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    tauri::Builder::default()
        .manage(AppState {
            pty_tx: tokio::sync::Mutex::new(None),
            ctrl_tx: tokio::sync::Mutex::new(None),
            session: tokio::sync::Mutex::new(None),
            agent_token: tokio::sync::Mutex::new(String::new()),
            db: tokio::sync::OnceCell::new(),
            db_error: tokio::sync::Mutex::new(None),
            crypto: tokio::sync::Mutex::new(Option::None),
            model_path: tokio::sync::Mutex::new(None),
            agent_port: Arc::new(tokio::sync::Mutex::new(None)),
            vnc_port: Arc::new(tokio::sync::Mutex::new(None)),
            agent_abort: Arc::new(tokio::sync::Mutex::new(None)),
            vnc_abort: Arc::new(tokio::sync::Mutex::new(None)),
        })
        .register_uri_scheme_protocol("ter-model", |_, request| {
            // In Tauri v2, we can't easily get state from the first closure param if it's UriSchemeContext
            // Use our static APP_HANDLE instead
            let app = match APP_HANDLE.get() {
                Some(app) => app,
                None => return tauri::http::Response::builder().status(503).body(Vec::new()).unwrap(),
            };
            let state = app.state::<AppState>();
            
            // Use block_on for simple sync protocol handler
            let model_path_guard = tauri::async_runtime::block_on(async { state.model_path.lock().await });
            
            let base_path: std::path::PathBuf = match &*model_path_guard {
                Some(p) => p.clone(),
                None => return tauri::http::Response::builder().status(404).body(Vec::new()).unwrap(),
            };

            let uri = request.uri().to_string();
            // Remove protocol and host parts
            let path_str = uri
                .strip_prefix("ter-model://localhost/")
                .or_else(|| uri.strip_prefix("ter-model://"))
                .unwrap_or(&uri);
            
            // Basic path traversal protection
            if path_str.contains("..") {
                return tauri::http::Response::builder().status(403).body(Vec::new()).unwrap();
            }

            let file_path = base_path.join(path_str);
            log::debug!("AI Protocol serving: {:?}", file_path);

            match std::fs::read(&file_path) {
                Ok(content) => {
                    let mime = match file_path.extension().and_then(|s: &std::ffi::OsStr| s.to_str()) {
                        Some("json") => "application/json",
                        Some("bin") => "application/octet-stream",
                        Some("wasm") => "application/wasm",
                        Some("js") => "application/javascript",
                        _ => "application/octet-stream",
                    };
                    tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(content)
                        .unwrap()
                }
                Err(e) => {
                    log::error!("AI Protocol error reading {:?}: {}", file_path, e);
                    tauri::http::Response::builder().status(404).body(Vec::new()).unwrap()
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Store AppHandle for logger
            let _ = APP_HANDLE.set(app.handle().clone());

            // --- AI TRIGGER API (Port 1414) ---
            tauri::async_runtime::spawn(async move {
                match tiny_http::Server::http("127.0.0.1:1414") {
                    Ok(server) => {
                        log::info!("AI Trigger API listening on http://127.0.0.1:1414");
                        for request in server.incoming_requests() {
                            log::info!("AI Audit requested via Local API...");
                            // Run the async audit
                            let res = tauri::async_runtime::block_on(perform_ui_audit());
                            match res {
                                Ok(path) => {
                                    let _ = request.respond(tiny_http::Response::from_string(format!("OK: {}", path)));
                                }
                                Err(e) => {
                                    let _ = request.respond(tiny_http::Response::from_string(format!("ERR: {}", e)).with_status_code(500));
                                }
                            }
                        }
                    }
                    Err(e) => log::error!("Failed to start AI Trigger API: {}", e),
                }
            });
            // ----------------------------------

            log::info!("Starting Ter application setup...");
            let app_handle = app.handle().clone();
            let app_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    log::error!("CRITICAL ERROR: Failed to get app data dir: {}", e);
                    return Err(Box::new(e));
                }
            };
            
            log::info!("App data directory: {:?}", app_dir);
            if !app_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&app_dir) {
                    log::error!("CRITICAL ERROR: Failed to create app data dir: {}", e);
                    return Err(Box::new(e));
                }
            }
            
            let db_path = app_dir.join("ter.db");
            // Use 3 slashes for absolute path in sqlite:// URL
            let db_url = format!("sqlite:///{}?mode=rwc", db_path.to_string_lossy());
            log::info!("Database URL: {}", db_url);

            let state = app_handle.state::<AppState>();
            
            tauri::async_runtime::block_on(async move {
                log::info!("Initializing database...");
                match Db::new(&db_url).await {
                    Ok(db) => {
                        log::info!("Database initialized successfully.");
                        if let Err(_) = state.db.set(db) {
                            log::warn!("Failed to set database in state (already set?)");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to initialize database: {}", e);
                        *state.db_error.lock().await = Some(e.to_string());
                    }
                }
            });

            log::info!("Setup completed.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect_to_ssh, 
            write_pty, 
            resize_pty, 
            get_agent_token,
            ls_remote,
            download_file,
            upload_file,
            set_master_password,
            save_server_config,
            list_server_configs,
            delete_server_config,
            connect_with_id,
            get_model_path,
            set_model_path,
            ai_audit_ui,
            list_plugins,
            run_plugin,
            get_skill_manifest,
            load_remote_skills,
            upload_ui_snapshot,
            write_remote_text,
            get_active_ports
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn connect_with_id(id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let configs = db.list_servers().await.map_err(|e| e.to_string())?;
    let config = configs.into_iter().find(|c| c.id == id).ok_or("Server config not found")?;

    let mut password = String::new();
    if let Some(enc_pass) = &config.password_enc {
        let crypto_guard = state.crypto.lock().await;
        if let Some(crypto) = crypto_guard.as_ref() {
            password = crypto.decrypt(enc_pass).ok_or("Failed to decrypt password")?;
        } else {
            return Err("Master password not set".to_string());
        }
    }

    connect_to_ssh(config.host, config.port as u16, config.user, password, app_handle, state).await
}
