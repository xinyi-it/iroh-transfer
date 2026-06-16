// iroh-transfer 前端逻辑
const { invoke } = window.__TAURI__.core;

// 传输历史
const history = [];

// === 节点管理 ===
const nodeStatusIcon = document.getElementById('node-status-icon');
const nodeStatus = document.getElementById('node-status');
const nodeId = document.getElementById('node-id');
const btnStartNode = document.getElementById('btn-start-node');

btnStartNode.addEventListener('click', async () => {
  btnStartNode.disabled = true;
  btnStartNode.textContent = '启动中...';
  try {
    const id = await invoke('start_node');
    nodeStatusIcon.textContent = '🟢';
    nodeStatus.textContent = '节点在线';
    nodeId.textContent = id.substring(0, 24) + '...';
    btnStartNode.style.display = 'none';
  } catch (e) {
    nodeStatus.textContent = '启动失败: ' + e;
    btnStartNode.disabled = false;
    btnStartNode.textContent = '重试';
  }
});

// === 发送文件 ===
const dropZone = document.getElementById('drop-zone');
const fileInput = document.getElementById('file-input');
const sendResult = document.getElementById('send-result');
const sendFileName = document.getElementById('send-file-name');
const sendFileSize = document.getElementById('send-file-size');
const ticketDisplay = document.getElementById('ticket-display');
const btnCopyTicket = document.getElementById('btn-copy-ticket');

dropZone.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', (e) => {
  if (e.target.files.length > 0) handleSend(e.target.files[0]);
});

dropZone.addEventListener('dragover', (e) => { e.preventDefault(); dropZone.classList.add('drag-over'); });
dropZone.addEventListener('dragleave', () => dropZone.classList.remove('drag-over'));
dropZone.addEventListener('drop', (e) => {
  e.preventDefault();
  dropZone.classList.remove('drag-over');
  if (e.dataTransfer.files.length > 0) handleSend(e.dataTransfer.files[0]);
});

async function handleSend(file) {
  dropZone.innerHTML = '<p>⏳ 上传中...</p>';
  try {
    const result = await invoke('send_file', { filePath: file.path || file.name });
    sendFileName.textContent = file.name;
    sendFileSize.textContent = formatSize(file.size);
    ticketDisplay.textContent = result.ticket;
    sendResult.style.display = 'block';
    dropZone.innerHTML = '<p>✅ 上传完成</p><p class="hint">可继续拖入文件</p>';
    addHistory('发送', file.name, '成功');
  } catch (e) {
    dropZone.innerHTML = '<p>❌ 上传失败: ' + e + '</p><p class="hint">重试请拖入文件</p>';
    addHistory('发送', file.name, '失败');
  }
}

btnCopyTicket.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(ticketDisplay.textContent);
    btnCopyTicket.textContent = '已复制 ✓';
    setTimeout(() => { btnCopyTicket.textContent = '复制票据'; }, 2000);
  } catch (e) {
    btnCopyTicket.textContent = '复制失败';
  }
});

// === 接收文件 ===
const ticketInput = document.getElementById('ticket-input');
const savePath = document.getElementById('save-path');
const btnReceive = document.getElementById('btn-receive');
const receiveResult = document.getElementById('receive-result');
const receiveStatus = document.getElementById('receive-status');

btnReceive.addEventListener('click', async () => {
  const ticket = ticketInput.value.trim();
  if (!ticket) { ticketInput.focus(); return; }

  btnReceive.disabled = true;
  btnReceive.textContent = '接收中...';
  receiveResult.style.display = 'none';

  try {
    const result = await invoke('receive_file', { ticket, savePath: savePath.value });
    receiveStatus.textContent = '✅ ' + result;
    receiveResult.style.display = 'block';
    addHistory('接收', savePath.value.split('/').pop(), '成功');
  } catch (e) {
    receiveStatus.textContent = '❌ ' + e;
    receiveResult.style.display = 'block';
    addHistory('接收', '-', '失败');
  } finally {
    btnReceive.disabled = false;
    btnReceive.textContent = '接收文件';
  }
});

// === 工具函数 ===
function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / 1048576).toFixed(1) + ' MB';
}

function addHistory(type, name, status) {
  const time = new Date().toLocaleTimeString('zh-CN');
  history.unshift({ type, name, status, time });
  renderHistory();
}

function renderHistory() {
  const list = document.getElementById('transfer-history');
  list.innerHTML = history.slice(0, 10).map(h =>
    `<div class="history-item">${h.type} | ${h.name} | ${h.status} | ${h.time}</div>`
  ).join('');
}
