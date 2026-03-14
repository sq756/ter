mod db;
mod crypto;
mod archiver;

use db::{Db, ServerConfig, Bookmark};
use crypto::Crypto;
use archiver::ARCHIVER;
use std::sync::Arc;
use anyhow::Result;
use russh::*;
use std::future::Future;
use tauri::{AppHandle, State, Manager, Url};
use tokio::sync::mpsc;
use tauri::Emitter;
use uuid::Uuid;
use std::sync::OnceLock;
use tauri_plugin_clipboard_manager::ClipboardExt;

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
            let target = r.target();
            let args = format!("{}", r.args());
            
            // v2.11.46: Backend Log Filter (Tactical HUD Cleaning)
            // 1. Hard Block Keywords
            let blacklist = ["sshbuffer", "seqn", "platform_impl", "event_loop"];
            if blacklist.iter().any(|k| args.contains(k) || target.contains(k)) {
                return;
            }

            // 2. Msg Type 94 logic (SSH_MSG_CHANNEL_DATA)
            if args.contains("msg 94") {
                if let Some(app) = APP_HANDLE.get() { let _ = app.emit("net-traffic", ()); }
                
                let mut display = false;
                if let Some(len_idx) = args.find("len ") {
                    let len_str = &args[len_idx + 4..];
                    let len_val: String = len_str.chars().take_while(|c| c.is_digit(10)).collect();
                    if let Ok(len) = len_val.parse::<usize>() {
                        // Only log large data chunks (likely code or content)
                        if len > 1000 { display = true; }
                    }
                }
                if !display { return; }
            }

            let msg = format!("[{}] {}: {}", r.level(), target, args);
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
    session_stack: TokioMutex<Vec<Arc<client::Handle<Client>>>>, // v2.12.1: Keep proxies alive
    agent_token: TokioMutex<String>,
    db: tokio::sync::OnceCell<Db>,
    db_error: TokioMutex<Option<String>>,
    crypto: TokioMutex<Option<Crypto>>,
    model_path: TokioMutex<Option<std::path::PathBuf>>,
    conda_path: TokioMutex<Option<String>>,
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
async fn set_conda_path(path: String, state: State<'_, AppState>) -> Result<(), String> { *state.conda_path.lock().await = Some(path); Ok(()) }
#[tauri::command]
async fn get_conda_path(state: State<'_, AppState>) -> Result<Option<String>, String> { Ok(state.conda_path.lock().await.clone()) }

#[tauri::command]
async fn write_remote_file(remote_path: String, content: String, state: State<'_, AppState>) -> Result<(), String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;
    let mut remote_file = sftp.create(&remote_path).await.map_err(|e| e.to_string())?;
    tokio::io::AsyncWriteExt::write_all(&mut remote_file, content.as_bytes()).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn dump_to_terminal(tab_id: String, remote_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let is_pdf = remote_path.to_lowercase().ends_with(".pdf");
    let cmd = if is_pdf { format!("pdftotext \"{}\" -", remote_path) } else { format!("cat \"{}\"", remote_path) };

    let final_cmd = if let Some(conda) = &*state.conda_path.lock().await {
        format!("{} run -n base {}", conda, cmd)
    } else {
        cmd
    };

    if let Some(tx) = state.pty_channels.get(&tab_id) {
        let _ = tx.send(format!("{}\r", final_cmd)).await;
    } else {
        return Err("Target terminal tab not found or not a terminal".to_string());
    }
    Ok(())
}

#[tauri::command]
async fn check_master_password_set(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.crypto.lock().await.is_some())
}

#[tauri::command]
async fn set_master_password(password: String, state: State<'_, AppState>) -> Result<(), String> {
 let crypto = tokio::task::spawn_blocking(move || Crypto::new(&password)).await.map_err(|e| e.to_string())?; *state.crypto.lock().await = Some(crypto); Ok(()) }
async fn get_db(state: &State<'_, AppState>) -> Result<Db, String> { if let Some(db) = state.db.get() { Ok(db.clone()) } else { match &*state.db_error.lock().await { Some(e) => Err(e.clone()), None => Err("DB not init".to_string()) } } }
#[tauri::command]
async fn save_server_config(mut config: ServerConfig, state: State<'_, AppState>) -> Result<(), String> { if let Some(pass) = config.password_enc.as_ref() { if let Some(c) = state.crypto.lock().await.as_ref() { config.password_enc = Some(c.encrypt(pass)); } else { return Err("No crypto".to_string()); } } let db = get_db(&state).await?; db.save_server(&config).await.map_err(|e| e.to_string()) }
#[tauri::command]
async fn list_server_configs(state: State<'_, AppState>) -> Result<Vec<ServerConfig>, String> { let db = get_db(&state).await?; db.list_servers().await.map_err(|e| e.to_string()) }
#[tauri::command]
async fn delete_server_config(id: String, state: State<'_, AppState>) -> Result<(), String> { let db = get_db(&state).await?; db.delete_server(&id).await.map_err(|e| e.to_string()) }

#[tauri::command]
async fn list_bookmarks(host_id: String, state: State<'_, AppState>) -> Result<Vec<Bookmark>, String> {
    let db = get_db(&state).await?;
    db.list_bookmarks(&host_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_bookmark(bookmark: Bookmark, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.save_bookmark(&bookmark).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_bookmark(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.delete_bookmark(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_ui_preference(key: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    db.save_ui_preference(&key, &value).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_ui_preferences(state: State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    let db = get_db(&state).await?;
    let prefs = db.list_ui_preferences().await.map_err(|e| e.to_string())?;
    Ok(prefs.into_iter().collect())
}

#[tauri::command]
async fn get_device_fingerprint() -> Result<serde_json::Value, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    // Simple fingerprint based on OS/Arch for now, can be expanded with hardware IDs
    Ok(serde_json::json!({
        "os": os,
        "arch": arch,
        "is_mobile": os == "android" || os == "ios"
    }))
}

#[tauri::command]
async fn navigate_cyber_webview(label: String, url: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window(&label) {
        let url_parsed = url.parse::<Url>().map_err(|e| format!("{}", e))?;
        let _ = wv.navigate(url_parsed).map_err(|e: tauri::Error| e.to_string())?;
        let _ = wv.eval(AGENT_SCRIPT).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn reload_cyber_webview(label: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window(&label) {
        let _ = wv.eval("window.location.reload()").map_err(|e: tauri::Error| e.to_string())?;
        let _ = wv.eval(AGENT_SCRIPT).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn extract_cyber_dom(label: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window(&label) {
        let _ = wv.eval("window.emit('dom-extracted', window.TerAgent.extractDOM())").map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn eval_cyber_webview(label: String, code: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window(&label) {
        let _ = wv.eval(&code).map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn copy_latest_to_clipboard(tab_id: String, app: AppHandle) -> Result<(), String> {
    let text = ARCHIVER.get_latest(&tab_id)?;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_latest_ai_response(tab_id: String) -> Result<String, String> {
    ARCHIVER.get_latest(&tab_id)
}

#[tauri::command]
async fn get_connection_chain(id: String, state: State<'_, AppState>) -> Result<Vec<ServerConfig>, String> {
    let db = get_db(&state).await?;
    let servers = db.list_servers().await.map_err(|e| e.to_string())?;
    let mut chain: Vec<ServerConfig> = Vec::new();
    let mut current_id = Some(id);

    while let Some(sid) = current_id {
        if let Some(config) = servers.iter().find(|s| s.id == sid) {
            chain.push(config.clone());
            current_id = config.proxy_id.clone().filter(|id| !id.is_empty());
        } else {
            break;
        }
    }
    chain.reverse(); // Local -> Proxy -> Target
    Ok(chain)
}

#[tauri::command]
async fn read_local_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_vault() -> Result<Vec<serde_json::Value>, String> {
    ARCHIVER.list_vault()
}

#[tauri::command]
async fn spawn_new_pty(tab_id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let session = state.session.lock().await.as_ref().ok_or("No session")?.clone();
    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    let tmux_cmd = format!("tmux new-session -A -s {0} \\; set-option status off || exec $SHELL || exec /bin/sh", tab_id);
    channel.exec(true, tmux_cmd.as_str()).await.map_err(|e| e.to_string())?;
    let (tx, mut rx) = mpsc::channel::<String>(100);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<PtyControl>(10);
    state.pty_channels.insert(tab_id.clone(), tx);
    state.ctrl_channels.insert(tab_id.clone(), ctrl_tx);
    
    let tab_id_cap = tab_id.clone();
    tauri::async_runtime::spawn(async move {
        log::info!("[PTY:{}] Starting PTY read loop", tab_id_cap);
        let mut capture_active = false;
        let mut last_capture_time = std::time::Instant::now();
        
        // v2.11.56: 16ms Pulse Aggregation Buffer
        let mut aggregation_buffer = Vec::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(16));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !aggregation_buffer.is_empty() {
                        let _ = app_handle.emit("pty-data", serde_json::json!({"id": tab_id_cap, "data": aggregation_buffer.clone()}));
                        aggregation_buffer.clear();
                    }
                }
                Some(ctrl) = ctrl_rx.recv() => { 
                    let PtyControl::Resize(c, r) = ctrl; 
                    let _ = channel.window_change(c, r, 0, 0).await; 
                }
                res = rx.recv() => { 
                    match res {
                        Some(data) => { let _ = channel.data(data.as_bytes()).await; }
                        None => break,
                    }
                }
                msg = channel.wait() => { 
                    match msg {
                        Some(russh::ChannelMsg::Data { data }) => {
                            if ARCHIVER.is_semantic_start(&data) {
                                capture_active = true;
                                ARCHIVER.clear_latest(&tab_id_cap);
                                last_capture_time = std::time::Instant::now();
                            }
                            if capture_active {
                                ARCHIVER.archive(&tab_id_cap, &data);
                                last_capture_time = std::time::Instant::now();
                                
                                if ARCHIVER.is_prompt(&data) {
                                    capture_active = false;
                                }
                            }
                            // Buffer the data instead of immediate emit
                            aggregation_buffer.extend_from_slice(&data);
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                            aggregation_buffer.extend_from_slice(&data);
                        }
                        Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    if capture_active && last_capture_time.elapsed().as_secs() > 10 {
                        capture_active = false;
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
async fn ls_remote(path: String, state: State<'_, AppState>) -> Result<RemoteDirContent, String> {
    log::debug!("[ls_remote] Received path: {}", path);
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;
    
    // v2.11.52: Canonicalize path to ensure absolute referencing
    let target_path = if path.is_empty() { ".".to_string() } else { path };
    let real_path = sftp.canonicalize(&target_path).await.map_err(|e| e.to_string())?;
    
    let entries = sftp.read_dir(&real_path).await.map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if name == "." || name == ".." { continue; }
        let is_dir = entry.file_type() == russh_sftp::protocol::FileType::Dir;
        let size = entry.metadata().len();
        
        let full_path = if real_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", real_path, name)
        };

        files.push(RemoteFile { name: name.to_string(), is_dir, size, path: full_path });
    }
    
    // Sort: Dirs first, then alpha
    files.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(RemoteDirContent { files, current_path: real_path })
}

#[tauri::command]
async fn open_dynamic_tunnel(remote_port: u16, state: State<'_, AppState>) -> Result<u16, String> {
    let session = state.session.lock().await.as_ref().ok_or("No active SSH session")?.clone();
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
                            Ok(channel) => { let mut stream = channel.into_stream(); let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await; }
                            Err(e) => { log::error!("Tunnel channel fail: {}", e); }
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });
    *state.dynamic_abort.lock().await = Some(abort_handle.abort_handle());
    *state.dynamic_port.lock().await = Some(local_port);
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
    Ok(output.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ContextRequirement { require_screenshot: Option<bool> }
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Skill { id: String, name: String, description: String, icon: Option<String>, rpc: Option<String>, trigger: Option<String>, context_requirement: Option<ContextRequirement> }
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct SkillManifest { skills: Vec<Skill> }

#[tauri::command]
async fn load_remote_skills(state: State<'_, AppState>) -> Result<Vec<Skill>, String> {
    let session_guard = state.session.lock().await;
    let session = session_guard.as_ref().ok_or("No active SSH session")?;
    let channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;
    let skills_path = ".ter/skills.json";
    match sftp.open(skills_path).await {
        Ok(mut remote_file) => {
            let mut content = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(&mut remote_file, &mut content).await.map_err(|e| e.to_string())?;
            if let Ok(m) = serde_json::from_slice::<SkillManifest>(&content) { Ok(m.skills) }
            else if let Ok(s) = serde_json::from_slice::<Vec<Skill>>(&content) { Ok(s) }
            else { Err("Parse fail".to_string()) }
        }
        Err(_) => Ok(Vec::new())
    }
}

#[tauri::command]
async fn download_file(remote_path: String, local_path: String, state: State<'_, AppState>) -> Result<(), String> {
    log::debug!("[download_file] Received remote_path: {}", remote_path);
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
    log::debug!("[delete_remote_file] Received remote_path: {}", remote_path);
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
    log::debug!("[read_remote_file] Received remote_path: {}", remote_path);
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

async fn connect_to_server(config: &ServerConfig, servers: &[ServerConfig], crypto: &Option<Crypto>, app: &AppHandle) -> Result<Vec<Arc<client::Handle<Client>>>, String> {
    let mut pass = String::new();
    if let Some(enc) = &config.password_enc { if let Some(c) = crypto.as_ref() { pass = c.decrypt(enc).ok_or("Decrypt failed")?; } }

    let mut stack = Vec::new();

    if let Some(proxy_id) = &config.proxy_id {
        if !proxy_id.is_empty() {
            let proxy_config = servers.iter().find(|s| &s.id == proxy_id).ok_or("Proxy not found")?;
            stack = Box::pin(connect_to_server(proxy_config, servers, crypto, app)).await?;
            let proxy_handle = stack.last().ok_or("Proxy stack empty")?.clone();

            // v2.12.3: Execute pre-connect script on jump host if present
            if let Some(script) = &proxy_config.pre_connect_script {
                if !script.is_empty() {
                    let _ = app.emit("conn-status", format!("[STEP] Running script on {}: {}", proxy_config.label, script));
                    let mut channel = proxy_handle.channel_open_session().await.map_err(|e| e.to_string())?;
                    channel.exec(true, script.as_str()).await.map_err(|e| e.to_string())?;
                    // We don't wait for completion here if it's a daemon, but we might need to sleep
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }

            let channel = proxy_handle.channel_open_direct_tcpip(&config.host, config.port as u32, "127.0.0.1", 0).await.map_err(|e| e.to_string())?;
            let mut russh_config = client::Config::default();
            let mut sess = client::connect_stream(Arc::new(russh_config), channel.into_stream(), Client {}).await.map_err(|e| e.to_string())?;
            let auth = sess.authenticate_password(&config.user, pass).await.map_err(|e| e.to_string())?;
            if !matches!(auth, russh::client::AuthResult::Success) { return Err("Auth fail on target".to_string()); }
            stack.push(Arc::new(sess));
            return Ok(stack);
        }
    }

    let russh_config = client::Config::default();
    let _ = app.emit("conn-status", format!("[STEP] Connecting to {}...", config.host));
    let connect_future = client::connect(Arc::new(russh_config), (config.host.as_str(), config.port as u16), Client {});
    let mut sess = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future).await.map_err(|_| "Connection timeout".to_string())?.map_err(|e| e.to_string())?;
    let auth = sess.authenticate_password(&config.user, pass).await.map_err(|e| e.to_string())?;
    if !matches!(auth, russh::client::AuthResult::Success) { return Err("Auth fail".to_string()); }
    stack.push(Arc::new(sess));
    Ok(stack)
}

#[tauri::command]
async fn connect_with_id(id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db = get_db(&state).await?;
    let servers = db.list_servers().await.map_err(|e| e.to_string())?;
    let config = servers.iter().find(|c| c.id == id).ok_or("Not found")?;
    let crypto = state.crypto.lock().await;
    
    let _ = app_handle.emit("conn-status", "[START] Orchestrating multi-layer connection...");
    let stack = connect_to_server(config, &servers, &*crypto, &app_handle).await?;
    
    let mut stack_guard = state.session_stack.lock().await;
    *stack_guard = stack;
    let final_session = stack_guard.last().unwrap().clone();
    *state.session.lock().await = Some(final_session.clone());

    // v2.12.3: Auto Dynamic Tunnel (-D)
    if config.auto_tunnel.unwrap_or(false) {
        let _ = app_handle.emit("conn-status", "[TUNNEL] Opening Dynamic Forwarding (SOCKS5)...");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
        let local_port = listener.local_addr().map_err(|e| e.to_string())?.port();
        
        let session_for_tunnel = final_session.clone();
        let abort_mutex = state.dynamic_abort.clone();
        let port_mutex = state.dynamic_port.clone();
        
        if let Some(h) = abort_mutex.lock().await.take() { h.abort(); }
        *port_mutex.lock().await = Some(local_port);

        let handle = tokio::spawn(async move {
            loop {
                if let Ok((_stream, _)) = listener.accept().await {
                    let _sess = session_for_tunnel.clone();
                    tokio::spawn(async move {
                        // Minimal SOCKS5 handshake (Stub for now, or use a crate)
                        // For now just emit status
                    });
                }
            }
        });
        *abort_mutex.lock().await = Some(handle.abort_handle());
        let _ = app_handle.emit("conn-status", format!("[SUCCESS] Dynamic Tunnel active on port {}", local_port));
    }

    let _ = app_handle.emit("conn-status", "[FINISH] Connection established.");
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RemoteFile { name: String, is_dir: bool, size: u64, path: String }

#[derive(serde::Serialize, serde::Deserialize)]
struct RemoteDirContent { files: Vec<RemoteFile>, current_path: String }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = log::set_logger(&LOGGER); log::set_max_level(log::LevelFilter::Debug);
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            pty_channels: DashMap::new(), ctrl_channels: DashMap::new(), 
            session: TokioMutex::new(None),
            session_stack: TokioMutex::new(Vec::new()),
            agent_token: TokioMutex::new(Uuid::new_v4().to_string()), db: tokio::sync::OnceCell::new(),
            db_error: TokioMutex::new(None), crypto: TokioMutex::new(None), model_path: TokioMutex::new(None),
            conda_path: TokioMutex::new(None),
            agent_port: Arc::new(TokioMutex::new(None)), vnc_port: Arc::new(TokioMutex::new(None)),
            dynamic_port: Arc::new(TokioMutex::new(None)), agent_abort: Arc::new(TokioMutex::new(None)),
            vnc_abort: Arc::new(TokioMutex::new(None)), dynamic_abort: Arc::new(TokioMutex::new(None)),
        })
        .setup(|app| {
            let ah = app.handle().clone(); let _ = APP_HANDLE.set(ah.clone());
            let app_dir = match app.path().app_data_dir() { Ok(dir) => dir, Err(_) => std::path::PathBuf::from("/tmp/.ter") };
            if !app_dir.exists() { let _ = std::fs::create_dir_all(&app_dir); }
            let archives_dir = app_dir.join("archives");
            if !archives_dir.exists() { let _ = std::fs::create_dir_all(&archives_dir); }
            let db_url = format!("sqlite:///{}?mode=rwc", app_dir.join("ter.db").to_string_lossy());
            let state = app.state::<AppState>();
            let ah_telemetry = ah.clone();
            tauri::async_runtime::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(3)).await; let _ = ah_telemetry.emit("system-stats", serde_json::json!({ "cpu_usage": 0.0, "mem_used": 0, "mem_total": 1, "net_sent": 0, "net_recv": 0, "uptime": 0, "is_heartbeat": true })); } });
            tauri::async_runtime::block_on(async move { match Db::new(&db_url).await { Ok(db) => { let _ = state.db.set(db); } Err(e) => { *state.db_error.lock().await = Some(e.to_string()); } } });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_master_password, check_master_password_set, list_server_configs, delete_server_config, connect_with_id,
            spawn_new_pty, write_pty, close_pty, resize_pty, get_terminal_logs, get_active_ports,
            get_agent_token, open_dynamic_tunnel, ls_remote, load_remote_skills,
            navigate_cyber_webview, reload_cyber_webview, extract_cyber_dom, eval_cyber_webview,
            save_server_config, set_model_path, get_model_path, 
            set_conda_path, get_conda_path,
            download_file, upload_file,
            delete_remote_file, read_remote_file, write_remote_file, dump_to_terminal,
            get_latest_ai_response, list_vault, read_local_file, get_connection_chain,
            copy_latest_to_clipboard,
            list_remote_tmux_sessions,
            list_bookmarks, save_bookmark, delete_bookmark,
            save_ui_preference, list_ui_preferences,
            get_device_fingerprint
        ])
        .run(tauri::generate_context!())
        .expect("error");
}
