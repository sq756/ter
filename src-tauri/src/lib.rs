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
    crypto: tokio::sync::Mutex<Option<Crypto>>,
}

#[tauri::command]
async fn set_master_password(password: String, state: State<'_, AppState>) -> Result<(), String> {
    let crypto = tokio::task::spawn_blocking(move || {
        Crypto::new(&password)
    }).await.map_err(|e| e.to_string())?;

    let mut crypto_guard = state.crypto.lock().await;
    *crypto_guard = Some(crypto);
    Ok(())
}

#[tauri::command]
async fn save_server_config(config: ServerConfig, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.get().ok_or("Database not initialized")?;
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
    let db = state.db.get().ok_or("Database not initialized")?;
    db.list_servers().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_server_config(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.get().ok_or("Database not initialized")?;
    db.delete_server(&id).await.map_err(|e| e.to_string())
}

async fn deploy_agent(session: &client::Handle<Client>, token: &str, app_handle: &AppHandle) -> Result<(), String> {
    // 1. Prepare remote directory
    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.exec(true, "mkdir -p ~/.ter").await.map_err(|e| e.to_string())?;
    
    // Wait for command completion
    while let Some(_) = channel.wait().await {}

    // 2. Upload Agent binary via SFTP
    let sftp_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    sftp_channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
    
    let sftp = SftpSession::new(sftp_channel.into_stream()).await.map_err(|e| format!("SFTP init error: {}", e))?;

    let res_dir = app_handle.path().resource_dir().map_err(|e| e.to_string())?;
    
    // In Tauri v2 bundled resources, the path is often flattened or prefixed with _up_
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

    let mut local_file = tokio::fs::File::open(&local_path).await.map_err(|e| format!("Failed to open agent at {:?}: {}", local_path, e))?;
    let remote_path = ".ter/agent_linux_amd64";
    let mut remote_file = sftp.create(remote_path).await.map_err(|e| format!("Failed to create remote file: {}", e))?;
    
    let mut buf = vec![0; 16384];
    while let Ok(n) = local_file.read(&mut buf).await {
        if n == 0 { break; }
        // Use explicit trait method call to resolve type inference issues on Windows/Linux
        tokio::io::AsyncWriteExt::write_all(&mut remote_file, &buf[..n]).await.map_err(|e| format!("Write error: {}", e))?;
    }
    // Ensure data is flushed
    tokio::io::AsyncWriteExt::flush(&mut remote_file).await.map_err(|e| e.to_string())?;
    drop(remote_file);

    // 3. Set executable permission and start agent
    let run_channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    let cmd = format!("chmod +x ~/.ter/agent_linux_amd64 && TER_AGENT_TOKEN={} nohup ~/.ter/agent_linux_amd64 --port 34567 > /dev/null 2>&1 &", token);
    run_channel.exec(true, cmd).await.map_err(|e| e.to_string())?;
    
    // Give it a second to start
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}

#[tauri::command]
async fn connect_to_ssh(host: String, user: String, pass: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let config = client::Config::default();
    let config = Arc::new(config);
    let sh = Client {};
    let mut session = client::connect(config, (host.as_str(), 22), sh).await.map_err(|e| e.to_string())?;

    let auth_res = session.authenticate_password(user, pass).await.map_err(|e| e.to_string())?;
    if !matches!(auth_res, russh::client::AuthResult::Success) {
        return Err("Authentication failed".to_string());
    }

    // Generate random token for agent
    let token = Uuid::new_v4().to_string();
    *state.agent_token.lock().await = token.clone();

    // Deploy Agent
    deploy_agent(&session, &token, &app_handle).await?;

    let mut channel = session.channel_open_session().await.map_err(|e| e.to_string())?;
    channel.request_pty(true, "xterm-256color", 80, 24, 0, 0, &[]).await.map_err(|e| e.to_string())?;
    channel.request_shell(true).await.map_err(|e| e.to_string())?;

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

    // Setup Local Port Forwarding for Agent (127.0.0.1:34567)
    let session_guard = state.session.lock().await;
    if let Some(session_arc) = session_guard.as_ref() {
        let session_clone = session_arc.clone();
        tauri::async_runtime::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:54321").await.unwrap();
            println!("Tunnel listening on 127.0.0.1:54321 -> remote 127.0.0.1:34567");
            while let Ok((mut stream, _)) = listener.accept().await {
                let session_inner = session_clone.clone();
                tokio::spawn(async move {
                    match session_inner.channel_open_direct_tcpip("127.0.0.1", 34567, "127.0.0.1", 54321).await {
                        Ok(channel) => {
                            let (mut reader, mut writer) = stream.split();
                            let (mut chan_reader, mut chan_writer) = tokio::io::split(channel.into_stream());
                            let _ = tokio::join!(
                                tokio::io::copy(&mut reader, &mut chan_writer),
                                tokio::io::copy(&mut chan_reader, &mut writer)
                            );
                        }
                        Err(e) => {
                            eprintln!("Failed to open direct tcpip channel: {}", e);
                        }
                    }
                });
            }
        });

        // Tunnel for VNC (127.0.0.1:5901) -> localhost:55901
        let session_clone_vnc = session_arc.clone();
        tauri::async_runtime::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:55901").await.unwrap();
            println!("VNC Tunnel listening on 127.0.0.1:55901 -> remote 127.0.0.1:5901");
            while let Ok((mut stream, _)) = listener.accept().await {
                let session_inner = session_clone_vnc.clone();
                tokio::spawn(async move {
                    match session_inner.channel_open_direct_tcpip("127.0.0.1", 5901, "127.0.0.1", 55901).await {
                        Ok(channel) => {
                            let (mut reader, mut writer) = stream.split();
                            let (mut chan_reader, mut chan_writer) = tokio::io::split(channel.into_stream());
                            let _ = tokio::join!(
                                tokio::io::copy(&mut reader, &mut chan_writer),
                                tokio::io::copy(&mut chan_reader, &mut writer)
                            );
                        }
                        Err(e) => eprintln!("VNC tunnel error: {}", e),
                    }
                });
            }
        });
    }

    Ok(())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            eprintln!("Starting Ter application setup...");
            let app_handle = app.handle().clone();
            let app_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("CRITICAL ERROR: Failed to get app data dir: {}", e);
                    return Err(Box::new(e));
                }
            };
            
            eprintln!("App data directory: {:?}", app_dir);
            if !app_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&app_dir) {
                    eprintln!("CRITICAL ERROR: Failed to create app data dir: {}", e);
                    return Err(Box::new(e));
                }
            }
            
            let db_path = app_dir.join("ter.db");
            let db_url = format!("sqlite://{}?mode=rwc", db_path.to_str().ok_or("Invalid path encoding")?);
            eprintln!("Database URL: {}", db_url);

            let state = app_handle.state::<AppState>();
            let db_state = state.db.clone();
            
            tauri::async_runtime::block_on(async move {
                eprintln!("Initializing database...");
                match Db::new(&db_url).await {
                    Ok(db) => {
                        eprintln!("Database initialized successfully.");
                        let _ = db_state.set(db);
                    }
                    Err(e) => {
                        eprintln!("ERROR: Failed to initialize database: {}", e);
                    }
                }
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            eprintln!("Setup completed.");
            Ok(())
        })
        .manage(AppState {
            pty_tx: tokio::sync::Mutex::new(None),
            ctrl_tx: tokio::sync::Mutex::new(None),
            session: tokio::sync::Mutex::new(None),
            agent_token: tokio::sync::Mutex::new(String::new()),
            db: tokio::sync::OnceCell::new(),
            crypto: tokio::sync::Mutex::new(None),
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
            connect_with_id
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn connect_with_id(id: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db = state.db.get().ok_or("Database not initialized")?;
    let configs = db.list_servers().await.map_err(|e| e.to_string())?;
    let config = configs.into_iter().find(|c| c.id == id).ok_or("Server config not found")?;

    let mut password = String::new();
    if let Some(enc_pass) = config.password_enc {
        let crypto_guard = state.crypto.lock().await;
        if let Some(crypto) = crypto_guard.as_ref() {
            password = crypto.decrypt(&enc_pass).ok_or("Failed to decrypt password")?;
        } else {
            return Err("Master password not set".to_string());
        }
    }

    connect_to_ssh(config.host, config.user, password, app_handle, state).await
}
