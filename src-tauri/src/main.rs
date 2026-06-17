// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::State;

struct IrohNode { node_id: Option<String> }
struct AppState { iroh_node: Mutex<IrohNode> }

fn get_iroh_path() -> Result<String, String> {
    if let Ok(path) = std::env::var("IROH_PATH") {
        if std::path::Path::new(&path).exists() { return Ok(path); }
    }
    if let Ok(exe_dir) = std::env::current_exe() {
        let bin_path = exe_dir.parent().and_then(|p| p.parent()).map(|p| p.join("binaries").join("iroh"));
        if let Some(ref bp) = bin_path { if bp.exists() { return Ok(bp.to_string_lossy().to_string()); } }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(&home).join(".cargo").join("bin").join("iroh");
        if p.exists() { return Ok(p.to_string_lossy().to_string()); }
    }
    let output = std::process::Command::new("which").arg("iroh").output().map_err(|e| format!("查找iroh失败: {}", e))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() { return Ok(path); }
    }
    Err("未找到iroh，请先安装: cargo install iroh-cli".to_string())
}

#[tauri::command]
fn get_iroh_binary_path() -> Result<String, String> { get_iroh_path() }

#[tauri::command]
fn pick_file() -> Result<Option<String>, String> {
    if let Some(path) = rfd::FileDialog::new().pick_file() { Ok(Some(path.to_string_lossy().to_string())) } else { Ok(None) }
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> { std::env::var("HOME").map_err(|e| format!("无法获取HOME: {}", e)) }

#[tauri::command]
fn start_node(state: State<AppState>) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    std::process::Command::new(&iroh_path).args(["start"]).spawn().map_err(|e| format!("启动iroh失败: {}", e))?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    let id_output = std::process::Command::new(&iroh_path).args(["status"]).output().map_err(|e| format!("获取节点状态失败: {}", e))?;
    let stdout = String::from_utf8_lossy(&id_output.stdout);
    let node_id = stdout.lines().find(|l| l.starts_with("Node ID:")).map(|l| l.replace("Node ID:", "").trim().to_string()).unwrap_or_default();
    if node_id.is_empty() { return Err("无法获取Node ID".to_string()); }
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = Some(node_id.clone());
    Ok(node_id)
}

#[tauri::command]
fn stop_node(state: State<AppState>) -> Result<(), String> {
    let iroh_path = get_iroh_path()?;
    std::process::Command::new(&iroh_path).args(["shutdown"]).output().map_err(|e| format!("shutdown失败: {}", e))?;
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = None;
    Ok(())
}

#[tauri::command]
fn send_file(file_path: String) -> Result<serde_json::Value, String> {
    let iroh_path = get_iroh_path()?;
    let output = std::process::Command::new(&iroh_path).args(["blobs", "add", &file_path]).output().map_err(|e| format!("blobs add失败: {}", e))?;
    if !output.status.success() { return Err(format!("blobs add失败: {}", String::from_utf8_lossy(&output.stderr))); }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut blob_id = String::new();
    let mut ticket = String::new();
    for line in stdout.lines() {
        if line.starts_with("Blob:") { blob_id = line.split_whitespace().nth(1).unwrap_or("").to_string(); }
        if line.contains("ticket:") { if let Some(idx) = line.find("ticket:") { ticket = line[idx + 7..].trim().to_string(); } }
    }
    if blob_id.is_empty() || ticket.is_empty() { return Err(format!("解析iroh输出失败: {}", stdout)); }
    let file_name = file_path.split('/').last().unwrap_or("downloaded").to_string();
    Ok(serde_json::json!({ "blob_id": blob_id, "ticket": ticket, "file_name": file_name }))
}

#[tauri::command]
fn receive_file(ticket: String, save_path: String) -> Result<String, String> {
    let iroh_path = get_iroh_path()?;
    let out_path = if std::path::Path::new(&save_path).exists() {
        let mut i = 1;
        loop { let alt = format!("{}_{}", save_path, i); if !std::path::Path::new(&alt).exists() { break alt; } i += 1; }
    } else { save_path.clone() };

    let output = std::process::Command::new(&iroh_path).args(["blobs", "get", &ticket, "-o", &out_path]).output().map_err(|e| format!("blobs get失败: {}", e))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let transferred = stdout.lines().find(|l| l.starts_with("Transferred")).unwrap_or("");
        Ok(format!("文件已保存到: {} {}", out_path, transferred))
    } else {
        Err(format!("接收失败: {}", String::from_utf8_lossy(&output.stderr)))
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState { iroh_node: Mutex::new(IrohNode { node_id: None }) })
        .invoke_handler(tauri::generate_handler![get_iroh_binary_path, pick_file, get_home_dir, start_node, stop_node, send_file, receive_file])
        .setup(|_app| { Ok(()) })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
