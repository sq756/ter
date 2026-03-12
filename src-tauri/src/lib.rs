mod db;
mod crypto;

use db::{Db, ServerConfig};
use crypto::Crypto;
use std::sync::Arc;
use anyhow::Result;
use russh::*;
use std::future::Future;
use tauri::{AppHandle, State, Manager};
use tokio::sync::mpsc;
use tauri::Emitter;
use uuid::Uuid;
use std::sync::OnceLock;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Skill { name: String, description: String }

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
    agent_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    vnc_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
    dynamic_abort: Arc<TokioMutex<Option<tokio::task::AbortHandle>>>,
}

#[tauri::command]
async fn close_pty(tab_id: String, state: State<'_, AppState>) -> Result<(), String> { state.pty_channels.remove(&tab_id); state.ctrl_channels.remove(&tab_id); Ok(()) }
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
        wv.navigate(url.parse().map_err(|e| format!("{}", e))?).map_err(|e| e.to_string())?;
        wv.eval(AGENT_SCRIPT).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn reload_cyber_webview(app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        wv.eval("window.location.reload()").map_err(|e| e.to_string())?;
        wv.eval(AGENT_SCRIPT).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn extract_cyber_dom(app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        wv.eval("window.emit('dom-extracted', window.TerAgent.extractDOM())").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn eval_cyber_webview(code: String, app_handle: AppHandle) -> Result<(), String> {
    if let Some(wv) = app_handle.get_webview_window("cyber-native-view") {
        wv.eval(&code).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                Some(ctrl) = ctrl_rx.recv() => { let PtyControl::Resize(c, r) = ctrl; let _ = channel.window_change(c, r, 0, 0).await; }
                Some(data) = rx.recv() => { let _ = channel.data(data.as_bytes()).await; }
                msg = channel.wait() => { 
                    if let Some(russh::ChannelMsg::Data { data }) = msg { let _ = app_handle.emit("pty-data", serde_json::json!({"id": tab_id, "data": data.to_vec()})); }
                    else { break; }
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
#[tauri::command]
async fn ls_remote(_: String, _: State<'_, AppState>) -> Result<Vec<RemoteFile>, String> { Ok(Vec::new()) }
#[tauri::command]
async fn open_dynamic_tunnel(_: u16, _: AppHandle, _: State<'_, AppState>) -> Result<u16, String> { Ok(0) }
#[tauri::command]
async fn ai_audit_ui() -> Result<String, String> { Ok("".to_string()) }
#[tauri::command]
async fn load_remote_skills(_: State<'_, AppState>) -> Result<Vec<Skill>, String> { Ok(Vec::new()) }

#[derive(serde::Serialize)]
struct RemoteFile { name: String, is_dir: bool, size: u64 }

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
            let app_dir = app.path().app_data_dir().unwrap(); if !app_dir.exists() { std::fs::create_dir_all(&app_dir).unwrap(); }
            let db_url = format!("sqlite:///{}?mode=rwc", app_dir.join("ter.db").to_string_lossy());
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(async move { match Db::new(&db_url).await { Ok(db) => { let _ = state.db.set(db); } Err(e) => { *state.db_error.lock().await = Some(e.to_string()); } } });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_master_password, list_server_configs, delete_server_config, connect_with_id,
            spawn_new_pty, write_pty, close_pty, resize_pty, get_terminal_logs, get_active_ports,
            get_agent_token, open_dynamic_tunnel, ls_remote, load_remote_skills, ai_audit_ui,
            navigate_cyber_webview, reload_cyber_webview, extract_cyber_dom, eval_cyber_webview
        ])
        .run(tauri::generate_context!())
        .expect("error");
}
