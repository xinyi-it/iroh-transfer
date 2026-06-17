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
    // 3. cargo install路径 (~/.cargo/bin/iroh)
    if let Ok(home) = std::env::var("HOME") {
        let cargo_bin = std::path::PathBuf::from(&home).join(".cargo").join("bin").join("iroh");
        if cargo_bin.exists() {
            return Ok(cargo_bin.to_string_lossy().to_string());
        }
    }
    // 4. 系统PATH
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

// 弹出文件选择对话框，返回选中的文件路径
#[tauri::command]
fn pick_file() -> Result<Option<String>, String> {
    // 用rfd弹出原生文件选择对话框
    if let Some(path) = rfd::FileDialog::new().pick_file() {
        Ok(Some(path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

// 获取用户home目录
#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    std::env::var("HOME").map_err(|e| format!("无法获取HOME: {}", e))
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

    // 如果save_path是目录，先下载到临时文件再重命名
    // 如果目标文件已存在，自动加后缀避免覆盖
    if std::path::Path::new(&save_path).is_dir() || save_path.ends_with('/') {
        // 目录：下载到临时文件，iroh会自动用blob hash命名
        let tmp_path = format!("{}iroh-download-{}", save_path, std::process::id());
        let output = std::process::Command::new(&iroh_path)
            .args(["blobs", "get", &ticket, "-o", &tmp_path])
            .output()
            .map_err(|e| format!("执行blobs get失败: {}", e))?;

        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("接收失败: {}", String::from_utf8_lossy(&output.stderr)));
        }

        // 读取iroh stdout获取blob hash作为文件名
        let stdout = String::from_utf8_lossy(&output.stdout);
        let blob_name = stdout.lines()
            .find(|l| l.starts_with("Fetching:"))
            .map(|l| l.replace("Fetching:", "").trim().to_string())
            .unwrap_or_else(|| "downloaded".to_string());

        let dest = format!("{}{}", save_path, blob_name);
        if std::path::Path::new(&dest).exists() {
            // 文件已存在，加数字后缀
            let mut i = 1;
            loop {
                let alt = format!("{}_{}", dest, i);
                if !std::path::Path::new(&alt).exists() {
                    std::fs::rename(&tmp_path, &alt).map_err(|e| format!("重命名失败: {}", e))?;
                    return Ok(format!("文件已保存到: {}", alt));
                }
                i += 1;
            }
        } else {
            std::fs::rename(&tmp_path, &dest).map_err(|e| format!("重命名失败: {}", e))?;
            return Ok(format!("文件已保存到: {}", dest));
        }
    } else {
        // 指定了具体文件路径
        let out_path = if std::path::Path::new(&save_path).exists() {
            // 文件已存在，加后缀
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

        // 使用超时机制，避免长时间卡住（最多30秒）
        let mut child = std::process::Command::new(&iroh_path)
            .args(["blobs", "get", &ticket, "-o", &out_path])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("执行blobs get失败: {}", e))?;

        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stderr = child.stderr.take().map(|mut s| {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut s, &mut buf).ok();
                        buf
                    }).unwrap_or_default();

                    if status.success() {
                        return Ok(format!("文件已保存到: {}", out_path));
                    } else {
                        return Err(format!("接收失败: {}", stderr));
                    }
                }
                Ok(None) => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Err("接收超时，对方节点可能未启动".to_string());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => return Err(format!("进程错误: {}", e)),
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
