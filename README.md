# iroh-transfer

基于 [iroh](https://iroh.computer) 的 P2P 文件传输桌面应用，使用 Tauri 2 构建。

## 功能

- 📤 发送文件：拖拽文件生成票据，分享给对方
- 📥 接收文件：粘贴票据即可下载，实时显示进度和百分比
- 🔒 P2P 直连：文件不经过服务器，端到端加密
- 🌐 跨网络：无需同一局域网，只要有网络就能传

## 前置要求

- [Rust](https://rustup.rs/) (1.76+)
- [Node.js](https://nodejs.org/) (20.19+ 或 22.12+)
- [iroh CLI](https://iroh.computer) — 安装方式：
  ```bash
  cargo install iroh-cli
  ```

## 安装与运行

```bash
git clone https://github.com/xinyi-it/iroh-transfer.git
cd iroh-transfer

# 安装前端依赖
npm install

# 构建前端
cd frontend && npm install && npm run build && cd ..

# 开发模式运行
cd src-tauri && cargo run
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

## 技术栈

- **前端**：Vue 3 + Element Plus + Vite（深色主题）
- **后端**：Rust + Tauri 2
- **P2P**：iroh CLI + iroh Rust SDK

## 架构说明

### 下载进度实现

接收文件时，后端通过 iroh Rust SDK 的 `download_with_opts()` API 获取带 `DownloadProgress` 事件的 stream，实时获取下载字节数，通过 Tauri 事件系统 (`app.emit`) 推送到前端，前端通过 `listen` 监听事件更新进度条，实现真正的实时进度显示。

### 节点启动

启动节点时会先检查 iroh 是否已在运行（避免数据库锁冲突），如果已在运行则直接获取 Node ID，否则启动新实例并轮询等待就绪（最多 60 秒）。所有 iroh 子进程均继承代理环境变量。

## License

MIT
