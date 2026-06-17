// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

// 应用状态
struct IrohNode {
    node_id: Option<String>,
}

struct AppState {
    iroh_node: Mutex<IrohNode>,
}

// 获取iroh二进制路径（优先环境变量，再找PATH）
fn get_iroh_path() -> Result<String, String> {
    if let Ok(path) = std::env::var("IROH_PATH") {
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }
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
    if let Ok(home) = std::env::var("HOME") {
        let cargo_bin = std::path::PathBuf::from(&home).join(".cargo").join("bin").join("iroh");
        if cargo_bin.exists() {
            return Ok(cargo_bin.to_string_lossy().to_string());
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
    Err("未找到iroh，请先安装: cargo install iroh-cli 或从 https://iroh.computer 下载".to_string())
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

// 启动iroh节点
#[tauri::command]
fn start_node(state: State<AppState>) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    std::process::Command::new(&iroh_path)
        .args(["start"])
        .spawn()
        .map_err(|e| format!("启动iroh失败: {}", e))?;
    std::thread::sleep(std::time::Duration::from_secs(3));
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

#[tauri::command]
fn stop_node(state: State<AppState>) -> Result<(), String> {
    let iroh_path = get_iroh_path()?;
    std::process::Command::new(&iroh_path)
        .args(["shutdown"])
        .output()
        .map_err(|e| format!("执行iroh shutdown失败: {}", e))?;
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
            if let Some(idx) = line.find("ticket:") {
                ticket = line[idx + 7..].trim().to_string();
            }
        }
    }
    if blob_id.is_empty() || ticket.is_empty() {
        return Err(format!("解析iroh输出失败: {}", stdout));
    }
    let file_name = file_path.split('/').last().unwrap_or("downloaded").to_string();
    Ok(serde_json::json!({
        "blob_id": blob_id,
        "ticket": ticket,
        "file_name": file_name
    }))
}

// 接收文件（带进度推送）
#[tauri::command]
fn receive_file(app: tauri::AppHandle, ticket: String, save_path: String) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;

    let out_path = if std::path::Path::new(&save_path).exists() {
        let mut i = 1;
        loop {
            let alt = format!("{}_{}", save_path, i);
            if !std::path::Path::new(&alt).exists() {
                break alt;
            }
            i += 1;
        }
    } else {
        save_path.clone()
    };

    // 发送初始状态
    let _ = app.emit("receive-progress", serde_json::json!({
        "status": "connecting",
        "message": "正在连接对方节点..."
    }));

    let mut child = std::process::Command::new(&iroh_path)
        .args(["blobs", "get", &ticket, "-o", &out_path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行blobs get失败: {}", e))?;

    let timeout = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();
    let mut last_progress = String::new();
    let mut last_emit = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok();
                    buf
                }).unwrap_or_default();
                let stderr = child.stderr.take().map(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok();
                    buf
                }).unwrap_or_default();

                if status.success() {
                    // 解析最终传输结果
                    let transferred = stdout.lines()
                        .find(|l| l.starts_with("Transferred"))
                        .unwrap_or("");
                    let _ = app.emit("receive-progress", serde_json::json!({
                        "status": "done",
                        "message": format!("✅ 接收完成 {}", transferred),
                        "path": out_path
                    }));
                    return Ok(format!("文件已保存到: {}", out_path));
                } else {
                    let _ = app.emit("receive-progress", serde_json::json!({
                        "status": "error",
                        "message": format!("❌ 接收失败: {}", stderr.trim())
                    }));
                    return Err(format!("接收失败: {}", stderr));
                }
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = app.emit("receive-progress", serde_json::json!({
                        "status": "error",
                        "message": "❌ 接收超时，对方节点可能未启动"
                    }));
                    return Err("接收超时，对方节点可能未启动".to_string());
                }

                // 读取当前输出看有没有进度信息
                // iroh blobs get 在传输过程中会更新stderr
                // 尝试非阻塞读取stderr
                if let Some(stderr) = child.stderr.as_mut() {
                    // 用set_nonblocking尝试读取
                    // 由于无法直接set_nonblocking on ChildStderr，用时间间隔推送进度
                }

                // 每500ms推送一次进度（避免太频繁）
                let elapsed = start.elapsed();
                if last_emit.elapsed() >= std::time::Duration::from_millis(500) {
                    let secs = elapsed.as_secs();
                    let msg = if secs < 3 {
                        "正在连接对方节点...".to_string()
                    } else if secs < 10 {
                        format!("接收中... 已等待{}秒", secs)
                    } else {
                        format!("接收中... 已等待{}秒，请确保对方节点在线", secs)
                    };
                    let _ = app.emit("receive-progress", serde_json::json!({
                        "status": "downloading",
                        "message": msg,
                        "elapsed": secs
                    }));
                    last_emit = std::time::Instant::now();
                }

                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => {
                let _ = app.emit("receive-progress", serde_json::json!({
                    "status": "error",
                    "message": format!("❌ 进程错误: {}", e)
                }));
                return Err(format!("进程错误: {}", e));
            }
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            iroh_node: Mutex::new(IrohNode { node_id: None }),
        })
        .invoke_handler(tauri::generate_handler![
            get_iroh_binary_path,
            pick_file,
            get_home_dir,
            start_node,
            stop_node,
            send_file,
            receive_file,
        ])
        .setup(|app| {
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
