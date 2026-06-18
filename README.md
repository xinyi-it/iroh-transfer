# iroh-transfer

基于 [iroh](https://iroh.computer) 的 P2P 文件传输桌面应用，使用 Tauri 2 构建。

## 功能

- 发送文件：拖拽文件生成票据，分享给对方
- 接收文件：粘贴票据即可下载，实时显示进度和百分比
- P2P 直连：文件不经过服务器，端到端加密
- 跨网络：无需同一局域网，只要有网络就能传
- 节点管理：启动/停止/重启节点，重启带确认弹窗

## 前置要求

- [Rust](https://rustup.rs/) (1.76+)
- [Node.js](https://nodejs.org/) (20.19+ 或 22.12+)

> 注意：iroh CLI 不再是必须的前置依赖。应用通过 iroh Rust SDK 在进程内直接启动节点，无需单独安装 iroh CLI。CLI 仅用于依赖检测提示。

## 安装与运行

```bash
git clone https://github.com/xinyi-it/iroh-transfer.git
cd iroh-transfer

# 安装前端依赖
npm install

# 构建前端
cd frontend && npm install && npm run build && cd ..

# 开发模式运行
npm run dev
```

### 代理配置

如果需要通过代理访问网络，启动前设置环境变量：

```bash
export https_proxy=http://127.0.0.1:7897
export http_proxy=http://127.0.0.1:7897
export all_proxy=socks5://127.0.0.1:7897
```

## 使用方法

1. 点击「启动节点」连接 iroh 网络
2. 发送方：拖拽文件到发送区，复制生成的票据发给对方
3. 接收方：粘贴票据到接收区，点击接收文件，实时查看下载进度
4. 节点在线时可点击「重启节点」（带确认弹窗）或「停止节点」

## 技术栈

- **前端**：Vue 3 + Element Plus + Vite（深色主题）
- **后端**：Rust + Tauri 2 + iroh Rust SDK

## 架构说明

### 节点启动（进程内模式）

应用通过 iroh Rust SDK 的 `Node::<FsStore>::persistent()` 在应用进程内直接创建并启动 iroh 节点，使用持久化存储（数据目录位于 `~/Library/Application Support/iroh-transfer/`）。节点启动后通过 `node.client()` 获取 `Iroh` client，所有操作（发送、下载、生成 ticket）都在同一进程内完成，无需通过 CLI 子进程。

遇到 `rpc.lock` 残留文件（通常因应用异常退出导致）时，会自动清除锁文件并重试启动。

### 发送文件

通过 iroh SDK 的 `blobs().add_from_path()` 添加文件，再用 `blobs().share()` 生成 `BlobTicket`，不再依赖 CLI `iroh blobs add` 命令。

### 下载进度实现

接收文件时，后端通过 iroh Rust SDK 的 `download_with_opts()` API 获取带 `DownloadProgress` 事件的 stream，实时获取下载字节数，通过 Tauri 事件系统 (`app.emit`) 推送到前端，前端通过 `listen` 监听事件更新进度条，实现真正的实时进度显示。

## 踩坑记录

### 1. CLI 子进程启动 iroh 节点不可靠

**问题**：最初通过 `std::process::Command::new("iroh").args(["start"]).spawn()` 启动 iroh CLI 子进程，然后轮询 `iroh status` 等待节点就绪。但 `iroh start` 作为前台阻塞进程，spawn 后 `iroh status` 的 RPC 连接始终失败（返回 `rpc connect` 错误），导致轮询超时。

**原因**：`iroh start` 是前台进程，spawn 后它作为一个子进程运行，但其 RPC 服务端口绑定和 daemon 化行为在不同环境下不稳定。`iroh start --start` 参数也不是真正的 daemon 模式（只是打印状态后继续前台运行）。

**解决**：改用 iroh Rust SDK 的 `Node::<FsStore>::persistent().enable_rpc_with_addr().spawn()` 在应用进程内直接启动节点，所有操作通过 `node.client()` 完成，彻底消除了跨进程 RPC 连接问题。

### 2. MutexGuard 跨 await 导致 Send trait 报错

**问题**：Tauri 的 `#[tauri::command] async fn` 要求 future 是 `Send`，但 `std::sync::MutexGuard` 不是 `Send`，在持有锁时使用 `.await` 会导致编译错误。

**解决**：在独立的 scope 中 clone `Iroh` client（内部是 Arc，clone 轻量），释放锁后再进行 await 操作：
```rust
let iroh = {
    let guard = state.iroh_client.lock()?;
    guard.as_ref().cloned()
}; // guard 在此释放
iroh.net().node_id().await?; // 安全
```

### 3. rpc.lock 残留导致节点启动失败

**问题**：应用异常退出（如强制关闭窗口）后，数据目录中的 `rpc.lock` 文件未被清理，再次启动时报 `iroh is already running` 错误。

**解决**：在 `enable_rpc_with_addr` 遇到 "already running" 或 "rpc.lock" 相关错误时，自动删除锁文件并重试启动。

### 4. iroh SDK API 名称与预期不同

- `add_file()` 不存在，正确方法是 `add_from_path()`，且返回 `AddProgress` stream，需要 `.finish().await` 才能得到 `AddOutcome`
- `BlobFormat::Plain` 不存在，正确值是 `BlobFormat::Raw`
- 生成 ticket 应使用 `blobs().share()` 方法，而非手动构造 `BlobTicket::new()`
- `Node::<FsStore>` 即 `FsNode`，但在 crate 外部需要用完整泛型形式导入

## License

MIT
