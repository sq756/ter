mod db;
mod crypto;

use db::{Db, ServerConfig};
use crypto::Crypto;
use std::sync::Arc;
use anyhow::Result;
use russh::*;
use std::future::Future;
use tauri::{AppHandle, State, Manager, Url};
use tokio::sync::mpsc;
use tauri::Emitter;
use uuid::Uuid;
use std::sync::OnceLock;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

const AGENT_SCRIPT: &str = r####"
(function() {
  window.TerAgent = {
    extractDOM: function() {
      const selectors = "a, button, input, textarea, [role='button'], [onclick]";
      const elements = document.querySelectorAll(selectors);
      let idCounter = 1;
      let markdown = "### CYBER_DOM_SNAPSHOT\n\n";
      elements.forEach(el => {
        const rect = el.getBoundingClientRect();
        const style = window.getComputedStyle(el);
        if (rect.width > 0 && rect.height > 0 && style.display !== "none" && style.visibility !== "hidden" && style.opacity !== "0") {
          const id = idCounter++;
          el.setAttribute("data-ter-id", id);
          const tag = el.tagName.toLowerCase();
          const text = (el.innerText || el.value || el.placeholder || el.ariaLabel || "").trim().substring(0, 40);
          markdown += "- [" + tag.toUpperCase() + " #" + id + ": " + (text || "NODE") + "]\n";
        }
      });
      return markdown;
    },
    click: function(id) {
      const el = document.querySelector("[data-ter-id='" + id + "']");
      if (el) { el.click(); return "OK"; }
      return "FAIL";
    },
    type: function(id, text) {
      const el = document.querySelector("[data-ter-id='" + id + "']");
      if (el) {
        el.focus();
        el.value = text;
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
        return "OK";
      }
      return "FAIL";
    }
  };
})();
"####;

struct BackendLogger;
impl log::Log for BackendLogger {
    fn enabled(&self, m: &log::Metadata) -> bool { m.level() <= log::Level::Debug }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            let msg = format!("[{}] {}: {}", r.level(), r.target(), r.args());
            if let Some(app) = APP_HANDLE.get() { let _ = app.emit("backend-log", msg); }
        }
    }
    fn flush(&self) {}
}
static LOGGER: BackendLogger = BackendLogger;

#[derive(Clone)]
struct Client {}
impl client::Handler for Client {
    type Error = anyhow::Error;
    fn check_server_key(&mut self, _: &russh::keys::PublicKey) -> impl Future<Output = Result<bool, Self::Error>> + Send { async { Ok(true) } }
}

use tokio::sync::Mutex as TokioMutex;
use dashmap::DashMap;
enum PtyControl { Resize(u32, u32) }
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
    #[allow(dead_code)]
    agent_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    #[allow(dead_code)]
    vnc_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    #[allow(dead_code)]
    dynamic_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
}

#[tauri::command]
async fn close_pty(tab_id: String, state: State<'_, AppState>) -> Result<(), String> {
    log::info!("Closing PTY and killing remote session for Tab: {}", tab_id);
    
    // v2.3.11: Fix P0-3 Zombie Process Leak
    // Attempt to kill the remote tmux session before removing channels
    if let Some(session) = state.session.lock().await.as_ref() {
        if let Ok(channel) = session.channel_open_session().await {
            let kill_cmd = format!("tmux kill-session -t {} || exit", tab_id);
            let _ = channel.exec(true, kill_cmd.as_str()).await;
        }

    }

    state.pty_channels.remove(&tab_id);
    state.ctrl_channels.remove(&tab_id);
    Ok(())
}

#[tauri::command]
async fn get_terminal_logs(tab_id: String, limit: i32, state: State<'_, AppState>) -> Result<Vec<Vec<u8>>, String> { let db = get_db(&state).await?; db.get_logs(&tab_id, limit).await.map_err(|e| e.to_string()) }
#[tauri::command]
async fn get_active_ports(state: State<'_, AppState>) -> Result<serde_json::Value, String> { Ok(serde_json::json!({ "agent": *state.agent_port.lock().await, "vnc": *state.vnc_port.lock().await, "dynamic": *state.dynamic_port.lock().await })) }
#[tauri::command]
async fn set_model_path(path: String, state: State<'_, AppState>) -> Result<(), String> { *state.model_path.lock().await = Some(std::path::PathBuf::from(path)); Ok(()) }
#[tauri::command]
async fn get_model_path(state: State<'_, AppState>) -> Result<Option<String>, String> { Ok(state.model_path.lock().await.as_ref().map(|p| p.to_string_lossy().into_owned())) }
#[tauri::command]
async fn set_master_password(password: String, state: State<'_, AppState>) -> Result<(), String> { let crypto = tokio::task::spawn_blocking(move || Crypto::new(&password)).await.map_err(|e| e.to_string())?; *state.crypto.lock().await = Some(crypto); Ok(()) }
async fn get_db(state: &State<'_, AppState>) -> Result<Db, String> { if let Some(db) = state.db.get() { Ok(db.clone()) } else { match &*state.db_error.lock().await { Some(e) => Err(e.clone()), None => Err("DB not init".to_string()) } } }
#[tauri::command]
async fn save_server_config(mut config: ServerConfig, state: State<'_, AppState>) -> Result<(), String> { if let Some(pass) = config.password_enc.as_ref() { if let Some(c) = state.crypto.lock().await.as_ref() { config.password_enc = Some(c.encrypt(pass)); } else { return Err("No crypto".to_string()); } } let db = get_db(&state).await?; db.save_server(&config).await.map_err(|e| e.to_string()) }
#[tauri::command]
async fn list_server_configs(state: State<'_, AppState>) -> Result<Vec<ServerConfig>, String> { let db = get_db(&state).await?; db.list_servers().await.map_err(|e| e.to_string()) }
#[tauri::command]
async fn delete_server_config(id: String, state: State<'_, AppState>) -> Result<(), String> { let db = get_db(&state).await?; db.delete_server(&id).await.map_err(|e| e.to_string()) }

#[tauri::command]
async fn navigate_cyber_webview(url: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        let url_parsed = url.parse::<Url>().map_err(|e| format!("{}", e))?;
        let _ = wv.navigate(url_parsed).map_err(|e: tauri::Error| e.to_string())?;
        let _ = wv.eval(AGENT_SCRIPT).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn reload_cyber_webview(app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        let _ = wv.eval("window.location.reload()").map_err(|e: tauri::Error| e.to_string())?;
        let _ = wv.eval(AGENT_SCRIPT).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn extract_cyber_dom(app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        let _ = wv.eval("window.emit('dom-extracted', window.TerAgent.extractDOM())").map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn eval_cyber_webview(code: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        let _ = wv.eval(&code).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

// v2.11.32: Ghost Archiver Logic
struct GhostArchiver {
    archives_dir: std::path::PathBuf,
}

impl GhostArchiver {
    fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let archives_dir = std::path::PathBuf::from(home).join(".ter/archives");
        if !archives_dir.exists() { let _ = std::fs::create_dir_all(&archives_dir); }
        Self { archives_dir }
    }

    fn strip_ansi(data: &[u8]) -> String {
        let s = String::from_utf8_lossy(data);
        // Regex to strip ANSI color codes
        let re = regex::Regex::new(r"\x1B\[([0-9;]*[a-zA-Z])").unwrap();
        re.replace_all(&s, "").to_string()
    }

    fn archive(&self, tab_id: &str, data: &[u8]) {
        let clean_text = Self::strip_ansi(data);
        if clean_text.trim().is_empty() { return; }
        
        let file_path = self.archives_dir.join(format!("{}_latest.md", tab_id));
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let content = format!("\n\n### ARCHIVE_BLOCK [{}]\n\n{}", timestamp, clean_text);
        
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(file_path) {
            use std::io::Write;
            let _ = file.write_all(content.as_bytes());
        }
    }
}

lazy_static::lazy_static! {
    static ref ARCHIVER: GhostArchiver = GhostArchiver::new();
}

#[tauri::command]
async fn get_latest_ai_response(tab_id: String) -> Result<String, String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let file_path = std::path::PathBuf::from(home).join(".ter/archives").join(format!("{}_latest.md", tab_id));
    if !file_path.exists() { return Err("No ghost archive found for this session.".to_string()); }
    std::fs::read_to_string(file_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn spawn_new_pty(tab_id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let session = state.session.lock().await.as_ref().ok_or("No session")?.clone();
    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    let tmux_cmd = format!("tmux new-session -A -s {} \\; set-option status off", tab_id);
    channel.exec(true, tmux_cmd.as_str()).await.map_err(|e| e.to_string())?;
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<PtyControl>(10);
    state.pty_channels.insert(tab_id.clone(), tx);
    state.ctrl_channels.insert(tab_id.clone(), ctrl_tx);
    
    let tab_id_cap = tab_id.clone();
    tauri::async_runtime::spawn(async move {
        log::info!("[PTY:{}] Starting PTY read loop", tab_id_cap);
        let mut capture_active = false;
        loop {
            tokio::select! {
                Some(ctrl) = ctrl_rx.recv() => { 
                    let PtyControl::Resize(c, r) = ctrl; 
                    let _ = channel.window_change(c, r, 0, 0).await; 
                }
                Some(data) = rx.recv() => { 
                    // v2.11.32: Logic to trigger capture on AI keywords
                    if data.to_lowercase().contains("gemini") || data.to_lowercase().contains("claude") {
                        capture_active = true;
                    }
                    let _ = channel.data(data.as_bytes()).await; 
                }
                msg = channel.wait() => { 
                    match msg {
                        Some(russh::ChannelMsg::Data { data }) => {
                            if capture_active {
                                ARCHIVER.archive(&tab_id_cap, &data);
                            }
                            let _ = app_handle.emit("pty-data", serde_json::json!({"id": tab_id_cap, "data": data.to_vec()}));
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                            let _ = app_handle.emit("pty-data", serde_json::json!({"id": tab_id_cap, "data": data.to_vec()}));
                        }
                        Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) => break,
                        _ => {}
                    }
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
async fn write_pty(tab_id: String, data: String, state: State<'_, AppState>) -> Result<(), String> { if let Some(tx) = state.pty_channels.get(&tab_id) { let _ = tx.send(data).await; } Ok(()) }
#[tauri::command]
async fn resize_pty(tab_id: String, cols: u32, rows: u32, state: State<'_, AppState>) -> Result<(), String> { if let Some(tx) = state.ctrl_channels.get(&tab_id) { let _ = tx.send(PtyControl::Resize(cols, rows)).await; } Ok(()) }
#[tauri::command]
async fn get_agent_token(state: State<'_, AppState>) -> Result<String, String> { Ok(state.agent_token.lock().await.clone()) }
use russh_sftp::client::SftpSession;

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
        let name = entry.file_name();
        // skip . and ..
        if name == "." || name == ".." {
            continue;
        }
        let is_dir = entry.file_type() == russh_sftp::protocol::FileType::Dir;
        let size = entry.metadata().len();
        files.push(RemoteFile {
            name: name.to_string(),
            is_dir,
            size,
        });
    }
    Ok(files)
}
#[tauri::command]
async fn open_dynamic_tunnel(remote_port: u16, state: State<'_, AppState>) -> Result<u16, String> {
    log::info!("Opening dynamic tunnel for remote port: {}", remote_port);
    let session = state.session.lock().await.as_ref().ok_or("No active SSH session")?.clone();
    
    // Close existing tunnel if any
    if let Some(handle) = state.dynamic_abort.lock().await.take() { handle.abort(); }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let local_port = listener.local_addr().map_err(|e| e.to_string())?.port();
    
    let abort_handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, _)) => {
                    let session = session.clone();
                    tokio::spawn(async move {
                        match session.channel_open_direct_tcpip("127.0.0.1", remote_port as u32, "127.0.0.1", 0).await {
                            Ok(channel) => {
                                let mut channel_stream = channel.into_stream();
                                let _ = tokio::io::copy_bidirectional(&mut socket, &mut channel_stream).await;
                            }
                            Err(e) => { log::error!("Failed to open SSH channel for tunnel: {}", e); }
                        }
                    });
                }
                Err(e) => { 
                    log::error!("Tunnel listener accept error: {}", e);
                    break;
                }
            }
        }
    });

    *state.dynamic_abort.lock().await = Some(abort_handle.abort_handle());
    *state.dynamic_port.lock().await = Some(local_port);

    log::info!("Tunnel active: 127.0.0.1:{} -> remote:{}", local_port, remote_port);
    Ok(local_port)
}
#[tauri::command]
async fn list_remote_tmux_sessions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    channel.exec(true, "tmux ls -F '#S'").await.map_err(|e| e.to_string())?;
    let mut stream = channel.into_stream();
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut data).await.map_err(|e| e.to_string())?;
    
    let output = String::from_utf8_lossy(&data);
    let sessions = output.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    Ok(sessions)
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ContextRequirement {
    require_screenshot: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Skill {
    id: String,
    name: String,
    description: String,
    icon: Option<String>,
    rpc: Option<String>,
    trigger: Option<String>,
    context_requirement: Option<ContextRequirement>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct SkillManifest {
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
async fn download_file(remote_path: String, local_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let mut remote_file = sftp.open(&remote_path).await.map_err(|e| e.to_string())?;
    let mut local_file = tokio::fs::File::create(local_path).await.map_err(|e| e.to_string())?;

    tokio::io::copy(&mut remote_file, &mut local_file).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn upload_file(remote_path: String, local_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let mut local_file = tokio::fs::File::open(local_path).await.map_err(|e| e.to_string())?;
    let mut remote_file = sftp.create(&remote_path).await.map_err(|e| e.to_string())?;

    tokio::io::copy(&mut local_file, &mut remote_file).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn delete_remote_file(remote_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    sftp.remove_file(&remote_path).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn read_remote_file(remote_path: String, state: State<'_, AppState>) -> Result<String, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;

    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;

    let mut remote_file = sftp.open(&remote_path).await.map_err(|e| e.to_string())?;
    let mut buffer = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut remote_file, &mut buffer).await.map_err(|e| e.to_string())?;
    
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

#[tauri::command]
async fn connect_with_id(id: String, _app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let config = db.list_servers().await.map_err(|e| e.to_string())?.into_iter().find(|c| c.id == id).ok_or("Not found")?;
    let mut pass = String::new();
    if let Some(enc) = &config.password_enc { if let Some(c) = state.crypto.lock().await.as_ref() { pass = c.decrypt(enc).ok_or("Decrypt failed")?; } }
    let mut sess = client::connect(Arc::new(client::Config::default()), (config.host.as_str(), config.port as u16), Client {}).await.map_err(|e| e.to_string())?;
    let auth = sess.authenticate_password(config.user, pass).await.map_err(|e| e.to_string())?;
    if !matches!(auth, russh::client::AuthResult::Success) { return Err("Auth fail".to_string()); }
    *state.session.lock().await = Some(Arc::new(sess));
    Ok(())
}

#[derive(serde::Serialize)]
struct RemoteFile { name: String, is_dir: bool, size: u64 }

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
        .setup(|app| {
            let ah = app.handle().clone(); let _ = APP_HANDLE.set(ah.clone());
            
            // v2.3.11: Fix P0-2 Startup Panic Risk
            let app_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(_) => {
                    eprintln!("[ERROR] Failed to get app data dir, falling back to /tmp/.ter");
                    std::path::PathBuf::from("/tmp/.ter")
                }
            };
            if !app_dir.exists() { let _ = std::fs::create_dir_all(&app_dir); }
            
            let db_url = format!("sqlite:///{}?mode=rwc", app_dir.join("ter.db").to_string_lossy());
            let state = app.state::<AppState>();
            
            // v2.11.22: Telemetry Push Loop (Heartbeat)
            let ah_telemetry = ah.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    // Emit a heartbeat/dummy stats if no agent is active to keep the link alive
                    // In a production scenario, this would fetch from the remote agent.
                    let _ = ah_telemetry.emit("system-stats", serde_json::json!({
                        "cpu_usage": 0.0,
                        "mem_used": 0,
                        "mem_total": 1,
                        "net_sent": 0,
                        "net_recv": 0,
                        "uptime": 0,
                        "is_heartbeat": true
                    }));
                }
            });

            tauri::async_runtime::block_on(async move { match Db::new(&db_url).await { Ok(db) => { let _ = state.db.set(db); } Err(e) => { *state.db_error.lock().await = Some(e.to_string()); } } });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_master_password, list_server_configs, delete_server_config, connect_with_id,
            spawn_new_pty, write_pty, close_pty, resize_pty, get_terminal_logs, get_active_ports,
            get_agent_token, open_dynamic_tunnel, ls_remote, load_remote_skills,
            navigate_cyber_webview, reload_cyber_webview, extract_cyber_dom, eval_cyber_webview,
            save_server_config, set_model_path, get_model_path, download_file, upload_file,
            delete_remote_file, read_remote_file, get_latest_ai_response,
            list_remote_tmux_sessions
        ])
        .run(tauri::generate_context!())
        .expect("error");
}
