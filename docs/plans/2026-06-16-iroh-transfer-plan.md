# iroh-transfer 传输核心实现计划

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** 基于Tauri + iroh CLI构建P2P文件传输桌面应用的核心功能

**Architecture:** Tauri三层架构——前端Web界面（HTML/CSS/JS）、Rust薄层后端（shell调iroh CLI）、iroh CLI内置打包

**Tech Stack:** Tauri 2.x, HTML/CSS/JS, Rust, iroh CLI 0.28.1

---

### Task 1: 初始化Tauri项目

**Objective:** 创建iroh-transfer项目骨架

**Files:**
- Create: `~/Documents/iroh-transfer/` 整个项目结构

**Step 1: 用npm创建Tauri项目**

```bash
cd ~/Documents/iroh-transfer
npm create tauri-app@latest . -- --template vanilla --manager npm
```

选择：vanilla模板，npm包管理器

**Step 2: 安装前端依赖**

```bash
npm install
```

**Step 3: 验证项目能跑起来**

```bash
npm run tauri dev
```

Expected: 弹出空白Tauri窗口

**Step 4: Commit**

```bash
git init
git add .
git commit -m "feat: init tauri project"
```

---

### Task 2: 内置iroh CLI二进制

**Objective:** 将iroh CLI二进制放入项目，打包时自动包含

**Files:**
- Create: `~/Documents/iroh-transfer/src-tauri/binaries/iroh`
- Modify: `~/Documents/iroh-transfer/src-tauri/tauri.conf.json`

**Step 1: 创建binaries目录，放入iroh CLI**

```bash
mkdir -p ~/Documents/iroh-transfer/src-tauri/binaries
cp ~/.cargo/bin/iroh ~/Documents/iroh-transfer/src-tauri/binaries/
```

**Step 2: 在tauri.conf.json中配置resources**

在bundle配置中添加：
```json
"resources": [
  "binaries/*"
]
```

**Step 3: 验证二进制可执行**

```bash
~/Documents/iroh-transfer/src-tauri/binaries/iroh --version
```

Expected: `iroh-cli 0.28.1`

**Step 4: Commit**

```bash
git add .
git commit -m "feat: bundle iroh cli binary"
```

---

### Task 3: Rust后端 - 获取iroh二进制路径

**Objective:** 写Rust函数获取内置iroh二进制的绝对路径

**Files:**
- Modify: `~/Documents/iroh-transfer/src-tauri/src/main.rs`

**Step 1: 添加获取iroh路径的Tauri Command**

```rust
#[tauri::command]
fn get_iroh_path(app: tauri::AppHandle) -> Result<String, String> {
    let resource_path = app.path()
        .resource_dir()
        .map_err(|e| e.to_string())?;
    let iroh_path = resource_path.join("binaries").join("iroh");
    Ok(iroh_path.to_string_lossy().to_string())
}
```

**Step 2: 注册command到Tauri Builder**

在main函数的invoke_handler中添加：
```rust
.invoke_handler(tauri::generate_handler![get_iroh_path])
```

**Step 3: 验证**

在开发模式下运行，前端console调用：
```js
const path = await window.__TAURI__.invoke('get_iroh_path');
console.log('iroh path:', path);
```

Expected: 输出iroh二进制的绝对路径

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add get_iroh_path command"
```

---

### Task 4: Rust后端 - 启动/停止iroh节点

**Objective:** 实现iroh节点的启动和停止

**Files:**
- Modify: `~/Documents/iroh-transfer/src-tauri/src/main.rs`

**Step 1: 添加节点状态管理**

```rust
use std::sync::Mutex;
use tauri::State;

struct IrohNode {
    process: Option<std::process::Child>,
    node_id: Option<String>,
}

struct AppState {
    iroh_node: Mutex<IrohNode>,
}
```

**Step 2: 添加启动节点命令**

```rust
#[tauri::command]
fn start_node(app: tauri::AppHandle, state: State<AppState>) -> Result<String, String> {
    let iroh_path = get_iroh_path(app)?;
    
    let output = std::process::Command::new(&iroh_path)
        .args(["start", "--rpc-port", "4919"])
        .output()
        .map_err(|e| e.to_string())?;
    
    if !output.status.success() {
        return Err(format!("iroh start failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    // 获取node id
    let id_output = std::process::Command::new(&iroh_path)
        .args(["node", "id"])
        .output()
        .map_err(|e| e.to_string())?;
    
    let node_id = String::from_utf8_lossy(&id_output.stdout).trim().to_string();
    
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    node.node_id = Some(node_id.clone());
    
    Ok(node_id)
}
```

**Step 3: 添加停止节点命令**

```rust
#[tauri::command]
fn stop_node(state: State<AppState>) -> Result<(), String> {
    let mut node = state.iroh_node.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut process) = node.process {
        process.kill().map_err(|e| e.to_string())?;
    }
    node.process = None;
    node.node_id = None;
    Ok(())
}
```

**Step 4: 注册commands和state**

```rust
.invoke_handler(tauri::generate_handler![get_iroh_path, start_node, stop_node])
.manage(AppState {
    iroh_node: Mutex::new(IrohNode { process: None, node_id: None }),
})
```

**Step 5: 验证**

前端调用 `start_node` 后获取node_id，调用 `stop_node` 停止。

**Step 6: Commit**

```bash
git add .
git commit -m "feat: add start/stop iroh node commands"
```

---

### Task 5: Rust后端 - 发送文件（iroh blobs add）

**Objective:** 实现上传文件并生成票据

**Files:**
- Modify: `~/Documents/iroh-transfer/src-tauri/src/main.rs`

**Step 1: 添加发送文件命令**

```rust
#[tauri::command]
fn send_file(app: tauri::AppHandle, file_path: String) -> Result<(String, String), String> {
    let iroh_path = get_iroh_path(app)?;
    
    let output = std::process::Command::new(&iroh_path)
        .args(["blobs", "add", &file_path])
        .output()
        .map_err(|e| e.to_string())?;
    
    if !output.status.success() {
        return Err(format!("blobs add failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // 解析输出获取Blob ID和Ticket
    let mut blob_id = String::new();
    let mut ticket = String::new();
    
    for line in stdout.lines() {
        if line.starts_with("Blob:") {
            blob_id = line.split_whitespace().nth(1).unwrap_or("").to_string();
        }
        if line.starts_with("All-in-one ticket:") {
            ticket = line.split_whitespace().nth(2).unwrap_or("").to_string();
        }
    }
    
    if blob_id.is_empty() || ticket.is_empty() {
        return Err(format!("Failed to parse iroh output: {}", stdout));
    }
    
    Ok((blob_id, ticket))
}
```

**Step 2: 注册command**

添加到 `invoke_handler`。

**Step 3: 验证**

前端调用 `send_file` 传入文件路径，返回 `(blob_id, ticket)`。

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add send_file command"
```

---

### Task 6: Rust后端 - 接收文件（iroh blobs get + export）

**Objective:** 实现票据接收和文件导出

**Files:**
- Modify: `~/Documents/iroh-transfer/src-tauri/src/main.rs`

**Step 1: 添加接收文件命令**

```rust
#[tauri::command]
fn receive_file(app: tauri::AppHandle, ticket: String, save_dir: String) -> Result<String, String> {
    let iroh_path = get_iroh_path(app)?;
    
    // Step 1: blobs get
    let get_output = std::process::Command::new(&iroh_path)
        .args(["blobs", "get", &ticket])
        .output()
        .map_err(|e| e.to_string())?;
    
    if !get_output.status.success() {
        return Err(format!("blobs get failed: {}", String::from_utf8_lossy(&get_output.stderr)));
    }
    
    let get_stdout = String::from_utf8_lossy(&get_output.stdout);
    
    // 解析blob hash
    let blob_id = get_stdout.lines()
        .find(|l| l.contains("blob"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("baf")))
        .unwrap_or("")
        .to_string();
    
    // Step 2: blobs export
    let export_output = std::process::Command::new(&iroh_path)
        .args(["blobs", "export", &blob_id, &save_dir])
        .output()
        .map_err(|e| e.to_string())?;
    
    if !export_output.status.success() {
        return Err(format!("blobs export failed: {}", String::from_utf8_lossy(&export_output.stderr)));
    }
    
    Ok(format!("File saved to: {}", save_dir))
}
```

**Step 2: 注册command并验证**

**Step 3: Commit**

```bash
git add .
git commit -m "feat: add receive_file command"
```

---

### Task 7: 前端 - 基础界面布局

**Objective:** 实现主界面布局（发送区 + 接收区 + 状态栏）

**Files:**
- Modify: `~/Documents/iroh-transfer/src/index.html`
- Create: `~/Documents/iroh-transfer/src/style.css`

**Step 1: 编写HTML结构**

主界面分为三个区域：
- 顶部：节点状态栏（Node ID、在线状态）
- 左侧：发送区（拖拽区域、文件选择、票据显示、复制按钮）
- 右侧：接收区（票据输入框、保存路径选择、接收按钮）

**Step 2: 编写CSS样式**

深色主题，简洁现代风格。

**Step 3: 验证界面显示正常**

`npm run tauri dev` 查看界面。

**Step 4: Commit**

```bash
git add .
git commit -m "feat: add main UI layout"
```

---

### Task 8: 前端 - 发送功能对接

**Objective:** 前端调用Tauri Command实现文件发送

**Files:**
- Modify: `~/Documents/iroh-transfer/src/index.html`
- Create: `~/Documents/iroh-transfer/src/app.js`

**Step 1: 实现拖拽上传**

```js
const dropZone = document.getElementById('drop-zone');
dropZone.addEventListener('drop', async (e) => {
    e.preventDefault();
    const file = e.dataTransfer.files[0];
    const result = await window.__TAURI__.invoke('send_file', { filePath: file.path });
    // 显示票据
    document.getElementById('ticket-output').textContent = result[1];
});
```

**Step 2: 实现票据复制**

一键复制票据到剪贴板。

**Step 3: 验证端到端流程**

拖文件 → 生成票据 → 复制成功。

**Step 4: Commit**

```bash
git add .
git commit -m "feat: connect send functionality to frontend"
```

---

### Task 9: 前端 - 接收功能对接

**Objective:** 前端调用Tauri Command实现文件接收

**Files:**
- Modify: `~/Documents/iroh-transfer/src/app.js`

**Step 1: 实现票据粘贴接收**

```js
document.getElementById('receive-btn').addEventListener('click', async () => {
    const ticket = document.getElementById('ticket-input').value;
    const saveDir = document.getElementById('save-dir').value;
    const result = await window.__TAURI__.invoke('receive_file', { ticket, saveDir });
    // 显示接收结果
});
```

**Step 2: 实现保存路径选择**

使用Tauri的dialog API选择保存目录。

**Step 3: 验证端到端流程**

粘贴票据 → 选择路径 → 接收成功。

**Step 4: Commit**

```bash
git add .
git commit -m "feat: connect receive functionality to frontend"
```

---

### Task 10: 节点自动启动

**Objective:** 应用启动时自动启动iroh节点，显示节点ID

**Files:**
- Modify: `~/Documents/iroh-transfer/src/app.js`

**Step 1: 应用加载时调用start_node**

```js
window.addEventListener('DOMContentLoaded', async () => {
    try {
        const nodeId = await window.__TAURI__.invoke('start_node');
        document.getElementById('node-id').textContent = nodeId;
    } catch (e) {
        document.getElementById('node-status').textContent = '节点启动失败';
    }
});
```

**Step 2: 应用关闭时调用stop_node**

```js
window.addEventListener('beforeunload', async () => {
    await window.__TAURI__.invoke('stop_node');
});
```

**Step 3: 验证**

启动应用 → 自动显示Node ID → 关闭应用 → 节点自动停止。

**Step 4: Commit**

```bash
git add .
git commit -m "feat: auto start/stop iroh node"
```

---

### Task 11: 端到端测试

**Objective:** 完整的发送-接收流程测试

**Step 1: 启动应用**

```bash
cd ~/Documents/iroh-transfer
npm run tauri dev
```

**Step 2: 发送文件**

拖入一个测试文件，获取票据。

**Step 3: 接收文件**

在另一台设备或同一台设备上，粘贴票据，接收文件。

**Step 4: 验证文件内容一致**

```bash
md5sum 原文件 接收文件
```

Expected: 两个MD5值相同

**Step 5: Commit**

```bash
git add .
git commit -m "test: e2e send and receive verified"
```

---

### Task 12: 打包构建

**Objective:** 构建Linux桌面安装包

**Step 1: 安装Tauri CLI**

```bash
cargo install tauri-cli
```

**Step 2: 构建release版本**

```bash
cd ~/Documents/iroh-transfer
npm run tauri build
```

Expected: 在 `src-tauri/target/release/bundle/` 下生成 .deb 和 .AppImage

**Step 3: 测试安装包**

```bash
sudo dpkg -i src-tauri/target/release/bundle/deb/iroh-transfer_*.deb
iroh-transfer
```

Expected: 应用正常启动，功能正常

**Step 4: Commit**

```bash
git add .
git commit -m "feat: build linux installer"
```
