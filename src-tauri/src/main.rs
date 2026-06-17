// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::State;

struct IrohNode {
    node_id: Option<String>,
}
struct AppState {
    iroh_node: Mutex<IrohNode>,
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
    // 先启动iroh进程
    std::process::Command::new(&iroh_path)
        .args(["start"])
        .spawn()
        .map_err(|e| format!("启动iroh失败: {}", e))?;

    // 在子线程里轮询status，最多等15秒
    let node_id = tokio::task::spawn_blocking(move || {
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if let Ok(output) = std::process::Command::new(&iroh_path)
                .args(["status"])
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

#[tauri::command]
fn check_download_progress(file_path: String, blob_hash: Option<String>) -> Result<serde_json::Value, String> {
    // 查iroh blobs数据目录总大小变化来估算下载进度
    let iroh_data_dir = std::env::var("IROH_HOME_DIR")
        .unwrap_or_else(|_| dirs::data_dir()
            .map(|p| p.join("iroh").to_string_lossy().to_string())
            .unwrap_or_else(|| "~/.local/share/iroh".to_string()));
    let blobs_dir = std::path::Path::new(&iroh_data_dir).join("blobs");
    let downloaded_size = if blobs_dir.exists() {
        let output = std::process::Command::new("du")
            .args(["-sb", &blobs_dir.to_string_lossy()])
            .output()
            .ok();
        output.and_then(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.split_whitespace().next()
                .and_then(|s| s.parse::<u64>().ok())
        }).unwrap_or(0u64)
    } else {
        0u64
    };

    let file_exists = std::path::Path::new(&file_path).exists();
    let file_size = if file_exists {
        std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    Ok(serde_json::json!({
        "downloaded_size": downloaded_size,
        "file_exists": file_exists,
        "file_size": file_size
    }))
}

#[tauri::command]
async fn download_blob(ticket: String, node_id: Option<String>) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    tokio::task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&iroh_path);
        cmd.args(["blobs", "get", &ticket]);
        if let Some(ref nid) = node_id {
            if !nid.is_empty() {
                cmd.args(["--node", nid]);
            }
        }
        let output = cmd.output().map_err(|e| format!("blobs get失败: {}", e))?;
        // iroh把Fetching输出到stderr，所以stdout+stderr都要搜
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);
        if !output.status.success() {
            return Err(format!("下载失败: {}", stderr));
        }
        let blob_hash = combined
            .lines()
            .find(|l| l.starts_with("Fetching:"))
            .map(|l| l.replace("Fetching:", "").trim().to_string())
            .unwrap_or_default();
        if blob_hash.is_empty() {
            return Err("无法解析blob hash".to_string());
        }
        Ok(blob_hash)
    }).await.map_err(|e| format!("任务执行失败: {}", e))?
}

#[tauri::command]
fn export_blob(blob_hash: String, save_path: String) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    let out_path = if std::path::Path::new(&save_path).exists() {
        let stem = std::path::Path::new(&save_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = std::path::Path::new(&save_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let dir = std::path::Path::new(&save_path)
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
                break alt;
            }
            i += 1;
        }
    } else {
        save_path.clone()
    };

    let export_output = std::process::Command::new(&iroh_path)
        .args(["blobs", "export", &blob_hash, &out_path])
        .output()
        .map_err(|e| format!("blobs export失败: {}", e))?;

    if export_output.status.success() {
        Ok(format!("文件已保存到: {}", out_path))
    } else {
        Err(format!("导出失败: {}", String::from_utf8_lossy(&export_output.stderr)))
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
            download_blob,
            export_blob,
            check_download_progress
        ])
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
