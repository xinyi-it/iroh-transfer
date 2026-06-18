<template>
  <div class="app-container">
    <!-- 依赖缺失提示 -->
    <el-alert
      v-if="depChecked && !irohFound"
      type="error"
      :closable="false"
      show-icon
      style="margin-bottom: 16px"
    >
      <template #title>
        <span>缺少 iroh 依赖</span>
      </template>
      <div style="margin-top: 8px; white-space: pre-line; font-size: 13px">{{ installGuide }}</div>
      <el-button type="primary" size="small" style="margin-top: 8px" @click="checkDeps">
        重新检测
      </el-button>
    </el-alert>
    <el-alert
      v-if="depChecked && irohFound"
      type="success"
      :closable="true"
      show-icon
      style="margin-bottom: 16px"
    >
      <template #title>
        <span>iroh 已就绪 {{ irohVersion }}</span>
      </template>
    </el-alert>

    <!-- 顶部状态栏 -->
    <el-header class="status-bar">
      <div class="status-left">
        <el-tag :type="nodeOnline ? 'success' : 'danger'" effect="dark" round>
          {{ nodeOnline ? '🟢 节点在线' : '🔴 节点离线' }}
        </el-tag>
        <span v-if="nodeId" class="node-id">{{ nodeId }}</span>
      </div>
      <el-button
        v-if="!nodeOnline"
        type="success"
        :loading="nodeStarting"
        @click="startNode"
      >
        {{ nodeStarting ? '启动中...' : '启动节点' }}
      </el-button>
      <template v-else>
        <el-button
          type="warning"
          :loading="nodeRestarting"
          @click="restartNode"
        >
          {{ nodeRestarting ? '重启中...' : '重启节点' }}
        </el-button>
        <el-button
          type="danger"
          :loading="nodeStopping"
          @click="stopNode"
        >
          {{ nodeStopping ? '停止中...' : '停止节点' }}
        </el-button>
      </template>
    </el-header>

    <!-- 主内容区 -->
    <el-main class="main-content">
      <el-row :gutter="16" class="full-height">
        <!-- 发送面板 -->
        <el-col :span="12" class="full-height">
          <el-card class="panel-card" shadow="hover">
            <template #header>
              <div class="panel-title">📤 发送文件</div>
            </template>

            <div v-if="!nodeOnline" class="disabled-hint">
              <el-icon color="#f0883e"><WarningFilled /></el-icon>
              <span>请先启动节点</span>
            </div>

            <div
              v-else-if="!sending"
              class="drop-zone"
              @click="pickAndSend"
            >
              <el-icon :size="40" color="#58a6ff"><UploadFilled /></el-icon>
              <p class="drop-text">点击选择文件</p>
              <p class="drop-hint">弹出文件选择对话框</p>
            </div>

            <div v-else class="drop-zone loading">
              <el-icon :size="40" class="is-loading"><Loading /></el-icon>
              <p class="drop-text">上传中...</p>
            </div>

            <!-- 发送结果 -->
            <div v-if="sendResult" class="result-area">
              <el-descriptions :column="1" border size="small">
                <el-descriptions-item label="文件">{{ sendResult.file_name }}</el-descriptions-item>
                <el-descriptions-item label="大小">{{ formatSize(sendResult.file_size) }}</el-descriptions-item>
              </el-descriptions>
              <p class="ticket-hint">把以下内容发给对方：</p>
              <el-input
                v-model="sendContent"
                type="textarea"
                :rows="3"
                readonly
                class="ticket-box"
              />
              <el-button type="primary" plain size="small" @click="copyTicket" style="margin-top:8px">
                {{ copyBtnText }}
              </el-button>
            </div>
          </el-card>
        </el-col>

        <!-- 接收面板 -->
        <el-col :span="12" class="full-height">
          <el-card class="panel-card" shadow="hover">
            <template #header>
              <div class="panel-title">📥 接收文件</div>
            </template>

            <div v-if="!nodeOnline" class="disabled-hint">
              <el-icon color="#f0883e"><WarningFilled /></el-icon>
              <span>请先启动节点</span>
            </div>

            <template v-else>
              <el-input
                v-model="ticketInput"
                type="textarea"
                :rows="3"
                placeholder="粘贴对方发来的内容（含文件名+票据）..."
                :disabled="receiving"
                @input="onTicketInput"
              />

              <div class="save-path-row">
                <span>保存为:</span>
                <el-input
                  v-model="saveFilename"
                  readonly
                  size="small"
                  class="save-input"
                  placeholder="自动识别文件名"
                />
              </div>

              <el-button
                type="primary"
                :disabled="!canReceive"
                :loading="receiving"
                @click="receiveFile"
                style="margin-top: 10px; width: 100%"
              >
                {{ receiving ? '接收中...' : '接收文件' }}
              </el-button>

              <!-- 进度条 -->
              <div v-if="showProgress" class="progress-area">
                <el-progress
                  :percentage="progressPercent"
                  :status="progressStatus"
                  :stroke-width="10"
                  :format="progressFormat"
                />
                <p class="progress-msg" :class="progressMsgClass">{{ progressMsg }}</p>
              </div>

              <!-- 接收结果 -->
              <el-alert
                v-if="receiveMsg"
                :type="receiveSuccess ? 'success' : 'error'"
                :title="receiveMsg"
                show-icon
                :closable="false"
                style="margin-top: 10px"
              />
            </template>
          </el-card>
        </el-col>
      </el-row>
    </el-main>

    <!-- 传输历史 -->
    <el-footer class="history-panel" height="auto">
      <div class="history-header">📋 传输历史</div>
      <div class="history-list" v-if="history.items.length">
        <el-tag
          v-for="(h, i) in history.items.slice(0, 10)"
          :key="i"
          :type="h.status === '成功' ? 'success' : 'danger'"
          effect="plain"
          size="small"
          class="history-tag"
        >
          {{ h.type }} | {{ h.name }} | {{ h.status }} | {{ h.time }}
        </el-tag>
      </div>
      <div v-else class="history-empty">暂无记录</div>
    </el-footer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { UploadFilled, Loading, WarningFilled } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke, listen } from '../api/tauri'
import type { SendFileResult, DownloadProgress } from '../api/tauri'
import { formatSize, parseTicketInput } from '../utils'
import { useHistoryStore } from '../stores/history'

const history = useHistoryStore()

// === 依赖检测 ===
const depChecked = ref(false)
const irohFound = ref(false)
const irohVersion = ref('')
const cargoFound = ref(false)
const installGuide = ref('')

async function checkDeps() {
  try {
    const info = await invoke<{
      iroh_found: boolean
      iroh_path: string
      iroh_version: string
      cargo_found: boolean
      install_guide: string
    }>('check_dependencies')
    irohFound.value = info.iroh_found
    irohVersion.value = info.iroh_version
    cargoFound.value = info.cargo_found
    installGuide.value = info.install_guide
  } catch {
    irohFound.value = false
  }
  depChecked.value = true
}

// 页面加载时自动检测
checkDeps()

// === 节点状态 ===
const nodeOnline = ref(false)
const nodeStarting = ref(false)
const nodeStopping = ref(false)
const nodeRestarting = ref(false)
const nodeId = ref('')

async function startNode() {
  nodeStarting.value = true
  try {
    console.log('[DEBUG] startNode called, invoking start_node...')
    const id = await invoke<string>('start_node')
    console.log('[DEBUG] start_node returned:', id)
    nodeId.value = id.substring(0, 24) + '...'
    nodeOnline.value = true
  } catch (e) {
    console.error('[DEBUG] start_node error:', e)
    ElMessage.error('启动失败: ' + e)
  } finally {
    nodeStarting.value = false
  }
}

async function stopNode() {
  nodeStopping.value = true
  try {
    await invoke('stop_node')
    nodeOnline.value = false
    nodeId.value = ''
  } catch (e) {
    ElMessage.error('停止失败: ' + e)
  } finally {
    nodeStopping.value = false
  }
}

async function restartNode() {
  try {
    await ElMessageBox.confirm('确定要重启节点吗？', '重启确认', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
  } catch {
    return
  }
  nodeRestarting.value = true
  try {
    await invoke('stop_node')
    nodeOnline.value = false
    nodeId.value = ''
    const id = await invoke<string>('start_node')
    nodeId.value = id.substring(0, 24) + '...'
    nodeOnline.value = true
  } catch (e) {
    ElMessage.error('重启失败: ' + e)
  } finally {
    nodeRestarting.value = false
  }
}

// === 发送 ===
const sending = ref(false)
const sendResult = ref<SendFileResult | null>(null)
const sendContent = ref('')
const copyBtnText = ref('复制发送内容')

async function pickAndSend() {
  sending.value = true
  sendResult.value = null
  try {
    const filePath = await invoke<string>('pick_file')
    if (!filePath) { sending.value = false; return }

    const result = await invoke<SendFileResult>('send_file', { filePath })
    sendResult.value = result
    sendContent.value = `iroh://${result.file_name}|${result.node_id}|${result.file_size}|${result.ticket}`
    history.add('发送', result.file_name, '成功')
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    history.add('发送', '-', '失败')
    ElMessage.error('上传失败: ' + msg)
  } finally {
    sending.value = false
  }
}

async function copyTicket() {
  try {
    await navigator.clipboard.writeText(sendContent.value)
    copyBtnText.value = '已复制 ✓'
    setTimeout(() => { copyBtnText.value = '复制发送内容' }, 2000)
  } catch {
    copyBtnText.value = '复制失败'
  }
}

// === 接收 ===
const ticketInput = ref('')
const saveFilename = ref('')
const receiving = ref(false)
const showProgress = ref(false)
const progressPercent = ref(0)
const progressStatus = ref<'' | 'success' | 'exception'>('')
const progressMsg = ref('')
const progressMsgClass = ref('')
const receiveMsg = ref('')
const receiveSuccess = ref(false)

let parsedNodeId = ''
let parsedFileSize = 0
let parsedTicket = ''
let unlisten: (() => void) | null = null

const canReceive = computed(() => nodeOnline.value && ticketInput.value.trim() && !receiving.value)

function onTicketInput() {
  const parsed = parseTicketInput(ticketInput.value)
  saveFilename.value = parsed.fileName
  parsedNodeId = parsed.nodeId
  parsedFileSize = parsed.fileSize
  parsedTicket = parsed.ticket
}

function onDownloadProgress(info: DownloadProgress) {
  if (info.status === 'connecting') {
    progressMsg.value = '正在连接对方节点...'
    progressMsgClass.value = ''
  } else if (info.status === 'downloading') {
    const downloaded = info.downloaded_size
    const total = info.total_size || parsedFileSize
    if (total > 0 && downloaded > 0) {
      const pct = Math.min(Math.round(downloaded / total * 100), 99)
      progressPercent.value = pct
      progressMsg.value = `接收中... ${formatSize(downloaded)} / ${formatSize(total)} (${pct}%)`
    } else if (downloaded > 0) {
      progressMsg.value = `接收中... ${formatSize(downloaded)}`
    } else {
      progressMsg.value = '正在连接对方节点...'
    }
  } else if (info.status === 'completed') {
    progressPercent.value = 100
    progressStatus.value = 'success'
    progressMsg.value = '✅ 文件已保存到: ' + (info.save_path || 'Downloads')
    progressMsgClass.value = 'success'
    receiveMsg.value = progressMsg.value
    receiveSuccess.value = true
    receiving.value = false
    history.add('接收', saveFilename.value.trim(), '成功')
    setTimeout(() => { showProgress.value = false }, 3000)
  } else if (info.status === 'failed') {
    progressStatus.value = 'exception'
    progressMsg.value = '❌ ' + (info.error || '下载失败')
    progressMsgClass.value = 'error'
    receiveMsg.value = progressMsg.value
    receiveSuccess.value = false
    receiving.value = false
    history.add('接收', saveFilename.value.trim(), '失败')
  }
}

onMounted(async () => {
  unlisten = await listen<DownloadProgress>('download-progress', onDownloadProgress)
  // 自动启动节点
  setTimeout(async () => {
    if (!nodeOnline.value && !nodeStarting.value) {
      await startNode()
    }
  }, 2000)
})

onUnmounted(() => {
  if (unlisten) { unlisten(); unlisten = null }
})

function progressFormat(percentage: number) {
  return percentage + '%'
}

async function receiveFile() {
  const ticket = parsedTicket || ticketInput.value.trim()
  if (!ticket) return

  const filename = saveFilename.value.trim()
  if (!filename) {
    receiveMsg.value = '无法识别文件名，请让对方重新复制发送内容'
    receiveSuccess.value = false
    return
  }

  receiving.value = true
  receiveMsg.value = ''
  showProgress.value = true
  progressPercent.value = 0
  progressStatus.value = ''
  progressMsg.value = '正在连接对方节点...'
  progressMsgClass.value = ''

  try {
    const home = await invoke<string>('get_home_dir')
    const savePath = home + '/Downloads/' + filename

    await invoke<string>('start_download', {
      ticket,
      nodeId: parsedNodeId || null,
      savePath,
      totalSize: parsedFileSize || 0
    })
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    progressStatus.value = 'exception'
    progressMsg.value = '❌ ' + msg
    progressMsgClass.value = 'error'
    receiveMsg.value = '❌ ' + msg
    receiveSuccess.value = false
    receiving.value = false
    history.add('接收', filename, '失败')
  }
}
</script>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #0d1117;
  color: #c9d1d9;
}

.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: #161b22;
  border-bottom: 1px solid #30363d;
  height: auto !important;
}

.status-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.node-id {
  color: #58a6ff;
  font-family: monospace;
  font-size: 12px;
}

.main-content {
  flex: 1;
  padding: 16px !important;
  overflow: hidden;
}

.full-height {
  height: 100%;
}

.full-height > .el-col {
  height: 100%;
  display: flex;
}

.panel-card {
  width: 100%;
  background: #161b22 !important;
  border: 1px solid #30363d !important;
  overflow-y: auto;
}

.panel-card :deep(.el-card__header) {
  border-bottom: 1px solid #30363d;
  padding: 12px 16px;
}

.panel-card :deep(.el-card__body) {
  color: #c9d1d9;
}

.panel-title {
  font-size: 16px;
  font-weight: 600;
  color: #e6edf3;
}

.disabled-hint {
  text-align: center;
  color: #f0883e;
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

.drop-zone {
  border: 2px dashed #30363d;
  border-radius: 10px;
  padding: 40px;
  text-align: center;
  cursor: pointer;
  transition: border-color 0.2s;
}

.drop-zone:hover {
  border-color: #58a6ff;
  background: rgba(88, 166, 255, 0.05);
}

.drop-zone.loading {
  cursor: wait;
}

.drop-text {
  margin-top: 10px;
  font-size: 14px;
}

.drop-hint {
  color: #484f58;
  font-size: 12px;
  margin-top: 6px;
}

.result-area {
  margin-top: 14px;
}

.ticket-hint {
  margin: 10px 0 6px;
  color: #8b949e;
  font-size: 13px;
}

.ticket-box :deep(.el-textarea__inner) {
  background: #0d1117;
  color: #58a6ff;
  font-family: monospace;
  font-size: 11px;
}

.save-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  font-size: 13px;
}

.save-input {
  flex: 1;
}

.save-input :deep(.el-input__inner) {
  color: #58a6ff !important;
}

.progress-area {
  margin-top: 12px;
}

.progress-msg {
  font-size: 13px;
  color: #8b949e;
  margin-top: 6px;
}

.progress-msg.success { color: #3fb950; }
.progress-msg.error { color: #f85149; }

.history-panel {
  background: #161b22;
  border-top: 1px solid #30363d;
  padding: 12px 20px !important;
  max-height: 150px;
  overflow-y: auto;
}

.history-header {
  font-size: 14px;
  color: #8b949e;
  margin-bottom: 8px;
}

.history-list {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.history-tag {
  font-size: 12px;
}

.history-empty {
  color: #484f58;
  font-size: 13px;
}

/* Element Plus 暗色覆盖 */
:deep(.el-card) {
  --el-card-bg-color: #161b22;
  --el-fill-color-blank: #0d1117;
}

:deep(.el-input__wrapper),
:deep(.el-textarea__inner) {
  background-color: #0d1117 !important;
  box-shadow: 0 0 0 1px #30363d inset !important;
  color: #c9d1d9 !important;
}

:deep(.el-descriptions) {
  --el-descriptions-table-border: 1px solid #30363d;
}

:deep(.el-descriptions__body) {
  background-color: #0d1117;
  color: #c9d1d9;
}

:deep(.el-progress-bar__outer) {
  background-color: #21262d;
}
</style>
