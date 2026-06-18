// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::str::FromStr;
use tauri::{AppHandle, Emitter, State};
use iroh::client::Iroh;
use iroh::node::Node;
use iroh::blobs::store::fs::Store as FsStore;
use iroh::blobs::get::db::DownloadProgress;
use iroh::blobs::get::progress::BlobProgress;
use iroh::blobs::BlobFormat;
use iroh::base::ticket::BlobTicket;
use iroh::client::blobs::{DownloadMode, DownloadOptions, WrapOption};
use iroh::blobs::util::SetTagOption;
use iroh::base::node_addr::AddrInfoOptions;
use futures_lite::StreamExt;

struct AppState {
    // 存储在应用内启动的 iroh 节点
    node: Mutex<Option<Node<FsStore>>>,
    // 存储 RPC client（可能来自内部节点或外部 CLI）
    iroh_client: Mutex<Option<Iroh>>,
    node_id: Mutex<Option<String>>,
    download_active: Mutex<bool>,
}

fn get_iroh_path() -> Result<String, String> {
    if let Ok(path) = std::env::var("IROH_PATH") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }
    if let Ok(exe_dir) = std::env::current_exe() {
        let bin_path = exe_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("binaries").join("iroh"));
        if let Some(ref bp) = bin_path {
            if bp.exists() {
                return Ok(bp.to_string_lossy().to_string());
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(&home)
            .join(".cargo")
            .join("bin")
            .join("iroh");
        if p.exists() {
            return Ok(p.to_string_lossy().to_string());
        }
    }
    let output = std::process::Command::new("which")
        .arg("iroh")
        .output()
        .map_err(|e| format!("查找iroh失败: {}", e))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    Err("未找到iroh，请先安装: cargo install iroh-cli".to_string())
}

#[tauri::command]
fn check_dependencies() -> Result<serde_json::Value, String> {
    let iroh_found = get_iroh_path().ok();

    let cargo_found = std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let iroh_version = if let Some(ref path) = iroh_found {
        std::process::Command::new(path)
            .arg("--version")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    };

    let install_guide = if cfg!(target_os = "macos") {
        if !cargo_found {
            "1. 安装Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n2. 安装iroh: cargo install iroh-cli"
        } else {
            "在终端运行: cargo install iroh-cli"
        }
    } else if cfg!(target_os = "windows") {
        if !cargo_found {
            "1. 安装Rust: 访问 https://rustup.rs 下载安装\n2. 安装iroh: cargo install iroh-cli"
        } else {
            "在终端运行: cargo install iroh-cli"
        }
    } else {
        if !cargo_found {
            "1. 安装Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n2. 安装iroh: cargo install iroh-cli\n3. 安装依赖: sudo apt install build-essential pkg-config libssl-dev"
        } else {
            "在终端运行: cargo install iroh-cli"
        }
    };

    Ok(serde_json::json!({
        "iroh_found": iroh_found.is_some(),
        "iroh_path": iroh_found.unwrap_or_default(),
        "iroh_version": iroh_version.unwrap_or_default(),
        "cargo_found": cargo_found,
        "install_guide": install_guide
    }))
}

#[tauri::command]
fn get_iroh_binary_path() -> Result<String, String> {
    get_iroh_path()
}

#[tauri::command]
fn pick_file() -> Result<Option<String>, String> {
    if let Some(path) = rfd::FileDialog::new().pick_file() {
        Ok(Some(path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    std::env::var("HOME").map_err(|e| format!("无法获取HOME: {}", e))
}

#[tauri::command]
async fn start_node(state: State<'_, AppState>) -> Result<String, String> {
    eprintln!("[DEBUG] start_node called!");
    // 先检查是否已有内部连接
    let existing_client = {
        let guard = state.iroh_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref().cloned()
    };
    if let Some(iroh) = existing_client {
        let node_id = iroh.net().node_id().await.map_err(|e| format!("获取node_id失败: {}", e))?;
        let id_str = node_id.to_string();
        let mut nid = state.node_id.lock().map_err(|e| e.to_string())?;
        *nid = Some(id_str.clone());
        return Ok(id_str);
    }

    // 用 iroh Rust SDK 直接在应用内启动节点（持久化存储）
    let data_dir = dirs::data_local_dir()
        .ok_or("无法获取本地数据目录")?
        .join("iroh-transfer");
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("创建数据目录失败: {}", e))?;

    // 用 Builder 创建持久化 iroh 节点
    let builder = Node::<FsStore>::persistent(&data_dir)
        .await
        .map_err(|e| format!("创建iroh节点失败: {}", e))?;

    let builder = match builder.enable_rpc_with_addr("127.0.0.1:0".parse().unwrap()).await {
        Ok(b) => b,
        Err(e) => {
            let err_msg = format!("{}", e);
            if err_msg.contains("already running") || err_msg.contains("rpc.lock") {
                // 清除残留的锁文件后重试
                let _ = std::fs::remove_file(data_dir.join("rpc.lock"));
                Node::<FsStore>::persistent(&data_dir)
                    .await
                    .map_err(|e2| format!("重试创建iroh节点失败: {}", e2))?
                    .enable_rpc_with_addr("127.0.0.1:0".parse().unwrap())
                    .await
                    .map_err(|e2| format!("重试启用RPC失败: {}", e2))?
            } else {
                return Err(format!("启用RPC失败: {}", e));
            }
        }
    };

    let node = builder.spawn().await.map_err(|e| format!("启动iroh节点失败: {}", e))?;

    let node_id = node.node_id();
    let id_str = node_id.to_string();
    let client = node.client().clone();

    let mut node_guard = state.node.lock().map_err(|e| e.to_string())?;
    *node_guard = Some(node);
    drop(node_guard);
    let mut client_guard = state.iroh_client.lock().map_err(|e| e.to_string())?;
    *client_guard = Some(client);
    drop(client_guard);
    let mut nid = state.node_id.lock().map_err(|e| e.to_string())?;
    *nid = Some(id_str.clone());
    Ok(id_str)
}

#[tauri::command]
async fn stop_node(state: State<'_, AppState>) -> Result<(), String> {
    // 先清除客户端引用
    {
        let mut client_guard = state.iroh_client.lock().map_err(|e| e.to_string())?;
        *client_guard = None;
    }
    // 取出 node（take 后锁自动释放），再 shutdown
    let node_opt = {
        let mut node_guard = state.node.lock().map_err(|e| e.to_string())?;
        node_guard.take()
    };
    if let Some(node) = node_opt {
        // 给shutdown加超时，防止卡住
        match tokio::time::timeout(std::time::Duration::from_secs(10), node.shutdown()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("[WARN] 关闭节点出错: {}, 继续清理", e);
            }
            Err(_) => {
                eprintln!("[WARN] 关闭节点超时(10秒), 继续清理");
            }
        }
    }
    {
        let mut nid = state.node_id.lock().map_err(|e| e.to_string())?;
        *nid = None;
    }
    // 清理残留的rpc.lock，防止下次启动失败
    let data_dir = dirs::data_local_dir()
        .unwrap_or_default()
        .join("iroh-transfer");
    let _ = std::fs::remove_file(data_dir.join("rpc.lock"));
    Ok(())
}

#[tauri::command]
async fn send_file(file_path: String, state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let iroh = {
        let guard = state.iroh_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref()
            .ok_or("iroh节点未启动")
            .map(|i| i.clone())?
    };
    let node_id = state
        .node_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .unwrap_or_default();

    let abs_path = std::path::Path::new(&file_path).to_path_buf();
    let file_name = abs_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or("downloaded".to_string());
    let file_size = std::fs::metadata(&abs_path).map(|m| m.len()).unwrap_or(0);

    // 用 iroh SDK 添加文件
    eprintln!("[DEBUG] send_file: adding file from path: {}", abs_path.display());
    let outcome = iroh.blobs().add_from_path(
        abs_path,
        false,
        SetTagOption::Auto,
        WrapOption::NoWrap,
    ).await.map_err(|e| format!("添加文件失败: {}", e))?
    .finish().await.map_err(|e| format!("添加文件完成失败: {}", e))?;

    let hash = outcome.hash;
    eprintln!("[DEBUG] send_file: hash = {}", hash);

    // 用 iroh SDK 生成 ticket
    let ticket = iroh.blobs().share(
        hash,
        BlobFormat::Raw,
        AddrInfoOptions::RelayAndAddresses,
    ).await.map_err(|e| format!("生成ticket失败: {}", e))?;
    eprintln!("[DEBUG] send_file: ticket generated successfully");

    Ok(serde_json::json!({
        "blob_id": hash.to_string(),
        "ticket": ticket.to_string(),
        "file_name": file_name,
        "node_id": node_id,
        "file_size": file_size
    }))
}

/// 解析ticket字符串为BlobTicket
fn parse_ticket(ticket_str: &str) -> Result<BlobTicket, String> {
    let ticket_part = if ticket_str.starts_with("iroh://") {
        ticket_str.rsplit_once('|').map(|(_, t)| t).unwrap_or(ticket_str)
    } else {
        ticket_str
    };
    BlobTicket::from_str(ticket_part)
        .map_err(|e| format!("解析ticket失败: {}", e))
}

/// 计算不冲突的保存路径
fn compute_save_path(save_path: &str) -> String {
    if std::path::Path::new(save_path).exists() {
        let stem = std::path::Path::new(save_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = std::path::Path::new(save_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let dir = std::path::Path::new(save_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut i = 1;
        loop {
            let alt = if ext.is_empty() {
                format!("{}/{}_{}", dir, stem, i)
            } else {
                format!("{}/{}_{}.{}", dir, stem, i, ext)
            };
            if !std::path::Path::new(&alt).exists() {
                return alt;
            }
            i += 1;
        }
    }
    save_path.to_string()
}

#[tauri::command]
async fn start_download(
    ticket: String,
    _node_id: Option<String>,  // 保留参数兼容前端，但不再使用
    save_path: String,
    total_size: u64,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    *state.download_active.lock().unwrap() = true;

    let iroh = {
        let guard = state.iroh_client.lock().map_err(|e| e.to_string())?;
        guard.as_ref()
            .ok_or("iroh节点未启动")
            .map(|i| i.clone())?
    };

    let blob_ticket = parse_ticket(&ticket)?;
    let (node_addr, hash, format) = blob_ticket.into_parts();
    eprintln!("[DEBUG] start_download: node_addr={}, hash={}, format={}", node_addr.node_id, hash, format);

    // 直接使用 ticket 中的 node_addr（已包含正确的 relay + direct_addresses）
    // 不再用前端传入的 nodeId 覆盖，避免地址不匹配

    let final_path = compute_save_path(&save_path);
    let effective_total_size = if total_size > 0 { total_size } else { 1 }; // 避免除零

    // 启动下载，获取带进度的stream
    let mut stream = iroh.blobs()
        .download_with_opts(
            hash,
            DownloadOptions {
                format,
                nodes: vec![node_addr],
                tag: SetTagOption::Auto,
                mode: DownloadMode::Direct,
            },
        )
        .await
        .map_err(|e| format!("启动下载失败: {}", e))?;

    // 发出连接中事件
    let _ = app.emit("download-progress", serde_json::json!({
        "status": "connecting",
        "downloaded_size": 0,
        "total_size": effective_total_size
    }));

    // 在后台任务中消费进度stream（带超时）
    tokio::spawn(async move {
        let mut downloaded_size: u64 = 0;
        let mut blob_total_size: u64 = effective_total_size;
        let mut last_progress_time = std::time::Instant::now();
        // 连接超时：60秒内没有进度就判定失败
        let connect_timeout = std::time::Duration::from_secs(60);
        // 整体超时：2小时
        let overall_timeout = std::time::Duration::from_secs(7200);
        let start_time = std::time::Instant::now();

        loop {
            // 检查整体超时
            if start_time.elapsed() > overall_timeout {
                let _ = app.emit("download-progress", serde_json::json!({
                    "status": "failed",
                    "error": "下载超时（2小时）",
                    "downloaded_size": downloaded_size,
                    "total_size": blob_total_size
                }));
                return;
            }

            // 用tokio::select!给stream.next()加超时
            let item = tokio::time::timeout(std::time::Duration::from_secs(30), stream.next()).await;

            match item {
                Ok(Some(progress_item)) => {
                    last_progress_time = std::time::Instant::now();
                    match progress_item {
                        Ok(progress) => {
                            eprintln!("[DEBUG] DownloadProgress event: {:?}", progress);
                            match progress {
                                DownloadProgress::InitialState(state) => {
                                    if state.connected {
                                        let _ = app.emit("download-progress", serde_json::json!({
                                            "status": "downloading",
                                            "downloaded_size": downloaded_size,
                                            "total_size": blob_total_size
                                        }));
                                    }
                                    if let Some(blob) = state.get_current() {
                                        if let Some(size) = blob.size {
                                            blob_total_size = size.value();
                                        }
                                        match blob.progress {
                                            BlobProgress::Progressing(offset) => {
                                                downloaded_size = offset;
                                            }
                                            BlobProgress::Done => {
                                                downloaded_size = blob_total_size;
                                            }
                                            BlobProgress::Pending => {}
                                        }
                                    }
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": downloaded_size,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::Connected => {
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": 0,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::FoundHashSeq { children, .. } => {
                                    blob_total_size = effective_total_size.max(children as u64);
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": downloaded_size,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::Found { size, .. } => {
                                    blob_total_size = size;
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": 0,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::Progress { offset, .. } => {
                                    downloaded_size = offset;
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": downloaded_size,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::FoundLocal { .. } => {
                                    eprintln!("[DEBUG] FoundLocal - blob already exists locally");
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": blob_total_size,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::Abort(e) => {
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "failed",
                                        "error": format!("下载中止: {}", e),
                                        "downloaded_size": downloaded_size,
                                        "total_size": blob_total_size
                                    }));
                                    return;
                                }
                                DownloadProgress::Done { .. } => {
                                    downloaded_size = blob_total_size;
                                    let _ = app.emit("download-progress", serde_json::json!({
                                        "status": "downloading",
                                        "downloaded_size": downloaded_size,
                                        "total_size": blob_total_size
                                    }));
                                }
                                DownloadProgress::AllDone(stats) => {
                                    eprintln!("[DEBUG] AllDone - bytes_read: {}, stats: {:?}", stats.bytes_read, stats);
                                    downloaded_size = stats.bytes_read;
                                    let export_result = export_downloaded_file(&iroh, hash, &final_path, format).await;
                                    eprintln!("[DEBUG] export result: {:?}", export_result);
                                    match export_result {
                                        Ok(path) => {
                                            let _ = app.emit("download-progress", serde_json::json!({
                                                "status": "completed",
                                                "save_path": path,
                                                "downloaded_size": downloaded_size,
                                                "total_size": blob_total_size
                                            }));
                                        }
                                        Err(e) => {
                                            let _ = app.emit("download-progress", serde_json::json!({
                                                "status": "failed",
                                                "error": e,
                                                "downloaded_size": downloaded_size,
                                                "total_size": blob_total_size
                                            }));
                                        }
                                    }
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = app.emit("download-progress", serde_json::json!({
                                "status": "failed",
                                "error": format!("下载出错: {}", e),
                                "downloaded_size": 0,
                                "total_size": blob_total_size
                            }));
                            return;
                        }
                    }
                }
                Ok(None) => {
                    // stream结束但没有AllDone
                    let _ = app.emit("download-progress", serde_json::json!({
                        "status": "failed",
                        "error": "下载意外结束",
                        "downloaded_size": downloaded_size,
                        "total_size": blob_total_size
                    }));
                    return;
                }
                Err(_) => {
                    // 30秒内没有新进度事件
                    // 如果还在连接阶段（0字节下载），检查连接超时
                    if downloaded_size == 0 && last_progress_time.elapsed() > connect_timeout {
                        let _ = app.emit("download-progress", serde_json::json!({
                            "status": "failed",
                            "error": "连接超时（60秒无响应），对方节点可能离线或网络不通",
                            "downloaded_size": 0,
                            "total_size": blob_total_size
                        }));
                        return;
                    }
                    // 如果已经在下载中，可能是暂时卡住，继续等
                    // 但如果超过5分钟没进度，也判定失败
                    if last_progress_time.elapsed() > std::time::Duration::from_secs(300) {
                        let _ = app.emit("download-progress", serde_json::json!({
                            "status": "failed",
                            "error": "下载停滞超时（5分钟无进度）",
                            "downloaded_size": downloaded_size,
                            "total_size": blob_total_size
                        }));
                        return;
                    }
                    // 否则继续等待
                }
            }
        }
    });

    Ok("downloading".to_string())
}

async fn export_downloaded_file(
    iroh: &Iroh,
    hash: iroh::blobs::Hash,
    save_path: &str,
    format: BlobFormat,
) -> Result<String, String> {
    use iroh::blobs::store::{ExportFormat, ExportMode};

    let recursive = format == BlobFormat::HashSeq;
    let export_format = if recursive {
        ExportFormat::Collection
    } else {
        ExportFormat::Blob
    };

    let absolute = std::path::Path::new(save_path).to_path_buf();
    let stream = iroh.blobs()
        .export(hash, absolute.clone(), export_format, ExportMode::Copy)
        .await
        .map_err(|e| format!("导出失败: {}", e))?;

    stream.await.map_err(|e| format!("导出写入失败: {}", e))?;
    Ok(save_path.to_string())
}

#[tauri::command]
fn check_download_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let active = *state.download_active.lock().unwrap();
    Ok(serde_json::json!({
        "status": if active { "downloading" } else { "idle" },
        "downloaded_size": 0,
        "total_size": 0
    }))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            node: Mutex::new(None),
            iroh_client: Mutex::new(None),
            node_id: Mutex::new(None),
            download_active: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            check_dependencies,
            get_iroh_binary_path,
            pick_file,
            get_home_dir,
            start_node,
            stop_node,
            send_file,
            start_download,
            check_download_status
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
