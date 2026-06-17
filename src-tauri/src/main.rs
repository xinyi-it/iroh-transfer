// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, State};

// 应用状态
struct IrohNode {
    node_id: Option<String>,
}

struct AppState {
    iroh_node: Mutex<IrohNode>,
}

// 获取iroh二进制路径（优先环境变量，再找PATH）
fn get_iroh_path() -> Result<String, String> {
    // 1. 环境变量指定
    if let Ok(path) = std::env::var("IROH_PATH") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }
    // 2. 内置binaries目录
    if let Ok(exe_dir) = std::env::current_exe() {
        let bin_path = exe_dir.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("binaries").join("iroh"));
        if let Some(ref bp) = bin_path {
            if bp.exists() {
                return Ok(bp.to_string_lossy().to_string());
            }
        }
    }
    // 3. 系统PATH
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
    
    Err("未找到iroh，请先安装: cargo install iroh-cli 或从 https://iroh.computer 下载".to_string())
}

// 获取iroh路径（暴露给前端）
#[tauri::command]
fn get_iroh_binary_path() -> Result<String, String> {
    get_iroh_path()
}

// 启动iroh节点
#[tauri::command]
fn start_node(state: State<AppState>) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    
    // 后台spawn启动iroh节点（不阻塞等待）
    std::process::Command::new(&iroh_path)
        .args(["start"])
        .spawn()
        .map_err(|e| format!("启动iroh失败: {}", e))?;
    
    // 等待节点启动
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    // 获取node id
    let id_output = std::process::Command::new(&iroh_path)
        .args(["status"])
        .output()
        .map_err(|e| format!("获取节点状态失败: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&id_output.stdout);
    let node_id = stdout.lines()
        .find(|l| l.starts_with("Node ID:"))
        .map(|l| l.replace("Node ID:", "").trim().to_string())
        .unwrap_or_default();
    
    if node_id.is_empty() {
        return Err("无法获取Node ID，节点可能还在启动中，请稍后重试".to_string());
    }
    
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = Some(node_id.clone());
    
    Ok(node_id)
}

// 停止iroh节点
#[tauri::command]
fn stop_node(state: State<AppState>) -> Result<(), String> {
    let iroh_path = get_iroh_path()?;
    
    std::process::Command::new(&iroh_path)
        .args(["stop"])
        .output()
        .map_err(|e| format!("执行iroh stop失败: {}", e))?;
    
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = None;
    
    Ok(())
}

// 发送文件
#[tauri::command]
fn send_file(file_path: String) -> Result<serde_json::Value, String> {
    let iroh_path = get_iroh_path()?;
    
    let output = std::process::Command::new(&iroh_path)
        .args(["blobs", "add", &file_path])
        .output()
        .map_err(|e| format!("执行blobs add失败: {}", e))?;
    
    if !output.status.success() {
        return Err(format!("blobs add失败: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let mut blob_id = String::new();
    let mut ticket = String::new();
    
    for line in stdout.lines() {
        if line.starts_with("Blob:") {
            blob_id = line.split_whitespace().nth(1).unwrap_or("").to_string();
        }
        if line.contains("ticket:") {
            // 票据在"All-in-one ticket:"后面
            if let Some(idx) = line.find("ticket:") {
                ticket = line[idx + 7..].trim().to_string();
            }
        }
    }
    
    if blob_id.is_empty() || ticket.is_empty() {
        return Err(format!("解析iroh输出失败: {}", stdout));
    }
    
    Ok(serde_json::json!({
        "blob_id": blob_id,
        "ticket": ticket
    }))
}

// 接收文件
#[tauri::command]
fn receive_file(ticket: String, save_path: String) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    
    // Step 1: blobs get
    let get_output = std::process::Command::new(&iroh_path)
        .args(["blobs", "get", &ticket])
        .output()
        .map_err(|e| format!("执行blobs get失败: {}", e))?;
    
    if !get_output.status.success() {
        return Err(format!("blobs get失败: {}", String::from_utf8_lossy(&get_output.stderr)));
    }
    
    let get_stdout = String::from_utf8_lossy(&get_output.stdout);
    
    // 解析blob hash（从输出中找）
    let blob_id = get_stdout.lines()
        .find(|l| l.contains("blob") || l.contains("Blob"))
        .and_then(|l| {
            l.split_whitespace()
                .find(|w| w.starts_with("baf") || w.len() > 30)
                .map(|w| w.to_string())
        })
        .unwrap_or_default();
    
    if blob_id.is_empty() {
        return Err(format!("无法从输出解析Blob ID: {}", get_stdout));
    }
    
    // Step 2: blobs export
    let export_output = std::process::Command::new(&iroh_path)
        .args(["blobs", "export", &blob_id, &save_path])
        .output()
        .map_err(|e| format!("执行blobs export失败: {}", e))?;
    
    if !export_output.status.success() {
        return Err(format!("blobs export失败: {}", String::from_utf8_lossy(&export_output.stderr)));
    }
    
    Ok(format!("文件已保存到: {}", save_path))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            iroh_node: Mutex::new(IrohNode { node_id: None }),
        })
        .invoke_handler(tauri::generate_handler![
            get_iroh_binary_path,
            start_node,
            stop_node,
            send_file,
            receive_file,
        ])
        .setup(|app| {
            // 在开发模式下打开devtools
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
