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
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let log_msg = format!("[{}] {}: {}", record.level(), record.target(), record.args());
            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("backend-log", log_msg);
            }
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

use tokio::sync::Mutex as TokioMutex;
use dashmap::DashMap;

enum PtyControl {
    Resize(u32, u32),
}

struct AppState {
    pty_channels: DashMap<String, mpsc::Sender<String>>,
    ctrl_channels: DashMap<String, mpsc::Sender<PtyControl>>,
    session: TokioMutex<Option<Arc<client::Handle<Client>>>>,
    agent_token: TokioMutex<String>,
    db: tokio::sync::OnceCell<Db>,
    db_error: TokioMutex<Option<String>>,
    crypto: TokioMutex<Option<Crypto>>,
    model_path: TokioMutex<Option<std::path::PathBuf>>,
    agent_port: Arc<TokioMutex<Option<u16>>>,
    vnc_port: Arc<TokioMutex<Option<u16>>>,
    dynamic_port: Arc<TokioMutex<Option<u16>>>,
    agent_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    vnc_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    dynamic_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
}

#[tauri::command]
async fn close_pty(tab_id: String, state: State<'_, AppState>) -> Result<(), String> {
    log::info!("Closing PTY for Tab: {}", tab_id);
    state.pty_channels.remove(&tab_id);
    state.ctrl_channels.remove(&tab_id);
    Ok(())
}

#[tauri::command]
async fn get_terminal_logs(tab_id: String, limit: i32, state: State<'_, AppState>) -> Result<Vec<Vec<u8>>, String> {
    let db = get_db(&state).await?;
    db.get_logs(&tab_id, limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_active_ports(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let agent = state.agent_port.lock().await;
    let vnc = state.vnc_port.lock().await;
    let dynamic = state.dynamic_port.lock().await;
    Ok(serde_json::json!({
        "agent": *agent,
        "vnc": *vnc,
        "dynamic": *dynamic
    }))
}

#[tauri::command]
async fn set_model_path(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut path_guard = state.model_path.lock().await;
    *path_guard = Some(std::path::PathBuf::from(path));
    Ok(())
}

#[tauri::command]
async fn get_model_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let path_guard = state.model_path.lock().await;
    Ok(path_guard.as_ref().map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn set_master_password(password: String, state: State<'_, AppState>) -> Result<(), String> {
    let crypto = tokio::task::spawn_blocking(move || Crypto::new(&password)).await.map_err(|e| e.to_string())?;
    let mut crypto_guard = state.crypto.lock().await;
    *crypto_guard = Some(crypto);
    Ok(())
}

async fn get_db(state: &State<'_, AppState>) -> Result<Db, String> {
    if let Some(db) = state.db.get() {
        Ok(db.clone())
    } else {
        let err_guard = state.db_error.lock().await;
        match &*err_guard {
            Some(e) => Err(format!("Database initialization failed: {}", e)),
            None => Err("Database not initialized".to_string()),
        }
    }
}

#[tauri::command]
async fn save_server_config(config: ServerConfig, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let mut config = config;
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
    let kill_cmd = "pkill -9 -f agent_linux_amd64 || true";
    let mut kill_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    kill_channel.exec(true, kill_cmd).await.map_err(|e| e.to_string())?;
    while let Some(_) = kill_channel.wait().await {}
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut home_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    home_channel.exec(true, "echo $HOME").await.map_err(|e| e.to_string())?;
    let mut home_dir = String::new();
    while let Some(msg) = home_channel.wait().await {
        if let russh::ChannelMsg::Data { data } = msg {
            home_dir.push_str(&String::from_utf8_lossy(&data));
        }
    }
    let home_dir = home_dir.trim();
    if home_dir.is_empty() { return Err("Failed home dir".to_string()); }

    let remote_path = format!("{}/.ter/agent_linux_amd64", home_dir);
    let mkdir_cmd = format!("mkdir -p {}/.ter", home_dir);
    let mut mkdir_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    mkdir_channel.exec(true, mkdir_cmd.as_str()).await.map_err(|e| e.to_string())?;
    while let Some(_) = mkdir_channel.wait().await {}

    let sftp_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    sftp_channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(sftp_channel.into_stream()).await.map_err(|e| e.to_string())?;

    let res_dir = app_handle.path().resource_dir().map_err(|e| e.to_string())?;
    let local_path = res_dir.join("agent_linux_amd64"); // Simplified for brevity

    let mut local_file = tokio::fs::File::open(&local_path).await.map_err(|e| e.to_string())?;
    let mut remote_file = sftp.create(&remote_path).await.map_err(|e| e.to_string())?;
    let mut buf = vec![0; 65536]; 
    while let Ok(n) = local_file.read(&mut buf).await {
        if n == 0 { break; }
        tokio::io::AsyncWriteExt::write_all(&mut remote_file, &buf[..n]).await.map_err(|e| e.to_string())?;
    }

    let run_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    let cmd = format!("chmod +x {remote_path} && TER_AGENT_TOKEN={token} nohup {remote_path} --port 34567 > /dev/null 2>&1 &", remote_path=remote_path, token=token);
    run_channel.exec(true, cmd.as_str()).await.map_err(|e| e.to_string())?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(())
}

#[tauri::command]
async fn connect_to_ssh(host: String, port: u16, user: String, pass: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, (host.as_str(), port), Client {}).await.map_err(|e| e.to_string())?;
    let auth_res = session.authenticate_password(user, pass).await.map_err(|e| e.to_string())?;
    if !matches!(auth_res, russh::client::AuthResult::Success) { return Err("Auth failed".to_string()); }

    state.pty_channels.clear(); state.ctrl_channels.clear();
    let token = Uuid::new_v4().to_string();
    *state.agent_token.lock().await = token.clone();
    deploy_agent(&session, &token, &app_handle).await?;
    *state.session.lock().await = Some(Arc::new(session));

    // Tunneling setup (Agent, VNC, etc.) - Simplified logic
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Skill { name: String, description: String }

#[tauri::command]
async fn load_remote_skills(state: State<'_, AppState>) -> Result<Vec<Skill>, String> { Ok(Vec::new()) }

#[tauri::command]
async fn spawn_new_pty(tab_id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let session = state.session.lock().await.as_ref().ok_or("No session")?.clone();
    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    let tmux_cmd = format!("tmux new-session -A -s {} \\; set-option status off", tab_id);
    channel.exec(true, &tmux_cmd).await.map_err(|e| e.to_string())?;

    let (tx, mut rx) = mpsc::channel::<String>(100);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<PtyControl>(10);
    state.pty_channels.insert(tab_id.clone(), tx);
    state.ctrl_channels.insert(tab_id.clone(), ctrl_tx);

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                Some(ctrl) = ctrl_rx.recv() => { if let PtyControl::Resize(c, r) = ctrl { let _ = channel.window_change(c, r, 0, 0).await; } }
                Some(data) = rx.recv() => { let _ = channel.data(data.as_bytes()).await; }
                msg = channel.wait() => { 
                    if let Some(russh::ChannelMsg::Data { data }) = msg {
                        let _ = app_handle.emit("pty-data", serde_json::json!({"id": tab_id, "data": data.to_vec()}));
                    } else { break; }
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn write_pty(tab_id: String, data: String, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(tx) = state.pty_channels.get(&tab_id) { let _ = tx.send(data).await; }
    Ok(())
}

#[tauri::command]
async fn resize_pty(tab_id: String, cols: u32, rows: u32, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(tx) = state.ctrl_channels.get(&tab_id) { let _ = tx.send(PtyControl::Resize(cols, rows)).await; }
    Ok(())
}

#[tauri::command]
async fn get_agent_token(state: State<'_, AppState>) -> Result<String, String> { Ok(state.agent_token.lock().await.clone()) }

#[tauri::command]
async fn ls_remote(path: String, state: State<'_, AppState>) -> Result<Vec<RemoteFile>, String> { Ok(Vec::new()) }

#[derive(serde::Serialize)]
struct RemoteFile { name: String, is_dir: bool, size: u64 }

#[tauri::command]
async fn open_dynamic_tunnel(remote_port: u16, app_handle: AppHandle, state: State<'_, AppState>) -> Result<u16, String> { Ok(0) }

#[tauri::command]
async fn ai_audit_ui() -> Result<String, String> { Ok("".to_string()) }

#[tauri::command]
async fn navigate_cyber_webview(url: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(webview) = app_handle.get_webview("cyber-native-view") {
        webview.navigate(url.parse().map_err(|e| format!("Invalid URL: {}", e))?).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn reload_cyber_webview(app_handle: AppHandle) -> Result<(), String> {
    if let Some(webview) = app_handle.get_webview("cyber-native-view") {
        webview.eval("window.location.reload()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn connect_with_id(id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let config = db.list_servers().await.map_err(|e| e.to_string())?.into_iter().find(|c| c.id == id).ok_or("Not found")?;
    let mut pass = String::new();
    if let Some(enc) = &config.password_enc {
        if let Some(crypto) = state.crypto.lock().await.as_ref() { pass = crypto.decrypt(enc).ok_or("Decrypt failed")?; }
    }
    connect_to_ssh(config.host, config.port as u16, config.user, pass, app_handle, state).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = log::set_logger(&LOGGER); log::set_max_level(log::LevelFilter::Debug);
    tauri::Builder::default()
        .manage(AppState {
            pty_channels: DashMap::new(), ctrl_channels: DashMap::new(), session: TokioMutex::new(None),
            agent_token: TokioMutex::new(Uuid::new_v4().to_string()), db: tokio::sync::OnceCell::new(),
            db_error: TokioMutex::new(None), crypto: TokioMutex::new(None), model_path: TokioMutex::new(None),
            agent_port: Arc::new(TokioMutex::new(None)), vnc_port: Arc::new(TokioMutex::new(None)),
            dynamic_port: Arc::new(TokioMutex::new(None)), agent_abort: Arc::new(TokioMutex::new(None)),
            vnc_abort: Arc::new(TokioMutex::new(None)), dynamic_abort: Arc::new(TokioMutex::new(None)),
        })
        .setup(|app| { let _ = APP_HANDLE.set(app.handle().clone()); Ok(()) })
        .invoke_handler(tauri::generate_handler![
            set_master_password, list_server_configs, delete_server_config, connect_with_id,
            spawn_new_pty, write_pty, close_pty, resize_pty, get_terminal_logs, get_active_ports,
            get_agent_token, open_dynamic_tunnel, ls_remote, load_remote_skills, ai_audit_ui,
            navigate_cyber_webview, reload_cyber_webview
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri");
}
