# iroh-transfer

基于 [iroh](https://iroh.computer) 的 P2P 文件传输桌面应用，使用 Tauri 2 构建。

## 功能

- 📤 发送文件：拖拽文件生成票据，分享给对方
- 📥 接收文件：粘贴票据即可下载
- 🔒 P2P 直连：文件不经过服务器，端到端加密
- 🌐 跨网络：无需同一局域网，只要有网络就能传

## 前置要求

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
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

# 开发模式运行
cd src-tauri && cargo run
```

## 使用方法

1. 点击「启动节点」连接 iroh 网络
2. 发送方：拖拽文件到发送区，复制生成的票据发给对方
3. 接收方：粘贴票据到接收区，选择保存路径，点击接收

## 技术栈

- **前端**：HTML/CSS/JS（深色主题）
- **后端**：Rust + Tauri 2
- **P2P**：iroh CLI

## License

MIT
