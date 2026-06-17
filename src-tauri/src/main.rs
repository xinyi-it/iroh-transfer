// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::str::FromStr;
use tauri::{AppHandle, Emitter, State};
use iroh::client::Iroh;
use iroh::blobs::get::db::DownloadProgress;
use iroh::blobs::get::progress::BlobProgress;
use iroh::blobs::BlobFormat;
use iroh::base::ticket::BlobTicket;
use iroh::net::key::PublicKey;
use iroh::client::blobs::{DownloadMode, DownloadOptions};
use futures_lite::StreamExt;

struct IrohNode {
    node_id: Option<String>,
}

struct AppState {
    iroh_node: Mutex<IrohNode>,
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

async fn connect_iroh() -> Result<Iroh, String> {
    // 获取iroh RPC地址
    let iroh_path = get_iroh_path()?;
    let output = std::process::Command::new(&iroh_path)
        .args(["status"])
        .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
        .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
        .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
        .output()
        .map_err(|e| format!("获取iroh状态失败: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 解析 RPC Addr: 127.0.0.1:xxxx
    let rpc_addr = stdout
        .lines()
        .find(|l| l.starts_with("RPC Addr:"))
        .and_then(|l| l.split(':').nth(1).map(|s| s.trim()))
        .and_then(|l| l.parse::<std::net::SocketAddr>().ok())
        .ok_or("无法获取iroh RPC地址，请确认iroh已启动")?;
    Iroh::connect_addr(rpc_addr).await.map_err(|e| format!("连接iroh节点失败: {}", e))
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
    let iroh_path = get_iroh_path()?;

    // 先检查iroh是否已经在运行
    if let Ok(output) = std::process::Command::new(&iroh_path)
        .args(["status"])
        .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
        .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
        .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(id) = stdout
                .lines()
                .find(|l| l.starts_with("Node ID:"))
                .map(|l| l.replace("Node ID:", "").trim().to_string())
            {
                if !id.is_empty() {
                    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
                    node.node_id = Some(id.clone());
                    return Ok(id);
                }
            }
        }
    }

    // iroh未运行，先关闭可能残留的进程，再启动
    let _ = std::process::Command::new(&iroh_path)
        .args(["shutdown"])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    std::process::Command::new(&iroh_path)
        .args(["start"])
        .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
        .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
        .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
        .spawn()
        .map_err(|e| format!("启动iroh失败: {}", e))?;

    let iroh_path_clone = iroh_path.clone();
    let node_id = tokio::task::spawn_blocking(move || {
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if let Ok(output) = std::process::Command::new(&iroh_path_clone)
                .args(["status"])
                .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
                .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
                .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(id) = stdout
                    .lines()
                    .find(|l| l.starts_with("Node ID:"))
                    .map(|l| l.replace("Node ID:", "").trim().to_string())
                {
                    if !id.is_empty() {
                        return Ok(id);
                    }
                }
            }
        }
        Err("启动超时，请检查iroh是否正常".to_string())
    }).await.map_err(|e| format!("任务执行失败: {}", e))??;

    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = Some(node_id.clone());
    Ok(node_id)
}

#[tauri::command]
fn stop_node(state: State<AppState>) -> Result<(), String> {
    let iroh_path = get_iroh_path()?;
    std::process::Command::new(&iroh_path)
        .args(["shutdown"])
        .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
        .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
        .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
        .output()
        .map_err(|e| format!("shutdown失败: {}", e))?;
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = None;
    Ok(())
}

#[tauri::command]
fn send_file(file_path: String, state: State<AppState>) -> Result<serde_json::Value, String> {
    let iroh_path = get_iroh_path()?;
    let output = std::process::Command::new(&iroh_path)
        .args(["blobs", "add", &file_path])
        .env("https_proxy", std::env::var("https_proxy").unwrap_or_default())
        .env("http_proxy", std::env::var("http_proxy").unwrap_or_default())
        .env("all_proxy", std::env::var("all_proxy").unwrap_or_default())
        .output()
        .map_err(|e| format!("blobs add失败: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "blobs add失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut blob_id = String::new();
    let mut ticket = String::new();
    for line in stdout.lines() {
        if line.starts_with("Blob:") {
            blob_id = line.split_whitespace().nth(1).unwrap_or("").to_string();
        }
        if line.contains("ticket:") {
            if let Some(idx) = line.find("ticket:") {
                ticket = line[idx + 7..].trim().to_string();
            }
        }
    }
    if blob_id.is_empty() || ticket.is_empty() {
        return Err(format!("解析iroh输出失败: {}", stdout));
    }
    let file_name = file_path
        .split('/')
        .last()
        .unwrap_or("downloaded")
        .to_string();
    let file_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
    let node_id = state
        .iroh_node
        .lock()
        .map_err(|e| e.to_string())?
        .node_id
        .clone()
        .unwrap_or_default();
    Ok(serde_json::json!({
        "blob_id": blob_id,
        "ticket": ticket,
        "file_name": file_name,
        "node_id": node_id,
        "file_size": file_size
    }))
}

/// 解析ticket字符串为BlobTicket
fn parse_ticket(ticket_str: &str) -> Result<BlobTicket, String> {
    // ticket可能是纯ticket，也可能是iroh://filename|nodeid|size|ticket格式
    let ticket_part = if ticket_str.starts_with("iroh://") {
        // 从iroh://格式中提取ticket部分（最后一个|后的内容）
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
    node_id: Option<String>,
    save_path: String,
    total_size: u64,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    // 标记下载开始
    *state.download_active.lock().unwrap() = true;

    let iroh = connect_iroh().await?;
    let blob_ticket = parse_ticket(&ticket)?;
    let (node_addr, hash, format) = blob_ticket.into_parts();

    // 如果有额外的node_id参数，构建node_addr
    let node_addr = if let Some(ref nid) = node_id {
        if !nid.is_empty() {
            match nid.parse::<PublicKey>() {
                Ok(pk) => {
                    // 使用ticket中的地址信息，但替换node_id
                    iroh::net::NodeAddr::from_parts(pk, node_addr.info.relay_url, node_addr.info.direct_addresses)
                }
                Err(_) => node_addr,
            }
        } else {
            node_addr
        }
    } else {
        node_addr
    };

    let final_path = compute_save_path(&save_path);

    // 启动下载，获取带进度的stream
    let mut stream = iroh.blobs()
        .download_with_opts(
            hash,
            DownloadOptions {
                format,
                nodes: vec![node_addr],
                tag: iroh::blobs::util::SetTagOption::Auto,
                mode: DownloadMode::Direct,
            },
        )
        .await
        .map_err(|e| format!("启动下载失败: {}", e))?;

    // 在后台任务中消费进度stream，通过Tauri事件推送
    tokio::spawn(async move {
        let mut downloaded_size: u64 = 0;
        let mut blob_total_size: u64 = total_size;

        while let Some(item) = stream.next().await {
            match item {
                Ok(progress) => match progress {
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
                        blob_total_size = total_size.max(children as u64);
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
                    DownloadProgress::FoundLocal { .. } => {}
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
                        downloaded_size = stats.bytes_read;
                        // 下载完成，导出文件
                        let export_result = export_downloaded_file(&iroh, hash, &final_path, format).await;
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
                },
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

        // stream结束但没收到AllDone
        let _ = app.emit("download-progress", serde_json::json!({
            "status": "failed",
            "error": "下载意外结束",
            "downloaded_size": downloaded_size,
            "total_size": blob_total_size
        }));
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
            iroh_node: Mutex::new(IrohNode { node_id: None }),
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
