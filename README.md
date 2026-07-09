# Iroh Transfer

基于 [iroh](https://iroh.computer) 的 P2P 文件传输桌面应用，使用 Tauri 2 构建。

## 功能特性

- **P2P 直连传输**：文件不经过服务器，端到端加密
- **跨网络**：无需同一局域网，有网络就能传
- **发送文件**：选择文件生成票据，分享给对方
- **接收文件**：粘贴票据即可下载，实时显示进度和百分比
- **节点管理**：启动 / 停止 / 重启节点（重启带确认弹窗）
- **传输历史**：本地记录发送和接收历史

## 技术栈

- **前端**：Vue 3 + Element Plus + Vite + Pinia（深色主题）
- **后端**：Rust + Tauri 2 + iroh Rust SDK

## 前置要求

- [Rust](https://rustup.rs/) 1.76+
- [Node.js](https://nodejs.org/) 20.19+ 或 22.12+

> 应用通过 iroh Rust SDK 在进程内直接启动节点，无需单独安装 iroh CLI。

## 开发模式

```bash
# 安装依赖
npm install
cd frontend && npm install && cd ..

# 启动开发模式（热重载）
npm run dev
```

## 打包构建

```bash
npm run build
```

产物位于 `src-tauri/target/release/bundle/`：

- **macOS**：`Iroh Transfer.app` 和 `Iroh Transfer_0.1.0_aarch64.dmg`（或 `_x64.dmg`，取决于构建机器架构）
- **Windows**：`.msi` / `.exe`（NSIS）
- **Linux**：`.deb`

## 自动发布

项目配置了 GitHub Actions（`.github/workflows/release.yml`），推送 `v*` 格式的 tag 时自动构建多平台安装包并发布 Release。

```bash
# 打 tag 触发自动打包发布
git tag v0.1.0
git push origin v0.1.0
```

构建矩阵：

| 平台 | 架构 | 产物 | 适用设备 |
|------|------|------|----------|
| macOS | arm64 | `Iroh Transfer_0.1.0_aarch64.dmg` | M1/M2/M3/M4 |
| macOS | x86_64 | `Iroh Transfer_0.1.0_x64.dmg` | Intel Mac |
| Linux | x86_64 | `iroh-transfer_0.1.0_amd64.deb` | Debian/Ubuntu |
| Windows | x86_64 | `.msi` / `.exe` | Windows |

> macOS 两个架构都用 macos-14 runner 构建以避免 Intel runner 排队，Release 默认为草稿状态，需在 GitHub Release 页面手动发布。

## 使用方法

1. 启动应用后自动连接 iroh 网络（或点击「启动节点」）
2. **发送方**：点击发送区选择文件，复制生成的票据发给对方
3. **接收方**：粘贴票据到接收区，点击「接收文件」，实时查看下载进度
4. 节点在线时可点击「重启节点」（带确认弹窗）或「停止节点」

## 架构说明

### 节点启动（进程内模式）

通过 iroh Rust SDK 的 `Node::<FsStore>::persistent()` 在应用进程内直接创建并启动 iroh 节点，使用持久化存储（数据目录位于 `~/Library/Application Support/iroh-transfer/`）。节点启动后通过 `node.client()` 获取 `Iroh` client，所有操作（发送、下载、生成 ticket）都在同一进程内完成。

遇到 `rpc.lock` 残留文件（通常因应用异常退出导致）时，会自动清除锁文件并重试启动。

### 发送文件

通过 iroh SDK 的 `blobs().add_from_path()` 添加文件，再用 `blobs().share()` 生成 `BlobTicket`。

### 下载进度实现

接收文件时，后端通过 iroh Rust SDK 的 `download_with_opts()` API 获取带 `DownloadProgress` 事件的 stream，实时获取下载字节数，通过 Tauri 事件系统 (`app.emit`) 推送到前端，前端通过 `listen` 监听事件更新进度条。

## License

MIT
