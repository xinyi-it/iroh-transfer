<template>
  <div class="about-container">
    <div class="about-card">
      <img src="/favicon.ico" alt="logo" class="about-logo" />
      <h1 class="about-title">Iroh Transfer</h1>
      <p class="about-version">版本 {{ version }}</p>
      <p class="about-desc">基于 iroh 的 P2P 文件传输桌面应用</p>

      <div class="update-section">
        <el-button
          type="primary"
          :loading="checking"
          @click="checkUpdate"
          style="width: 100%"
        >
          {{ checking ? '检查中...' : '检查更新' }}
        </el-button>

        <div v-if="updateStatus" class="update-status" :class="updateStatusClass">
          {{ updateStatus }}
        </div>

        <el-button
          v-if="hasUpdate"
          type="success"
          :loading="updating"
          @click="downloadAndInstall"
          style="width: 100%; margin-top: 10px"
        >
          {{ updating ? '下载安装中...' : '下载并安装更新' }}
        </el-button>

        <el-progress
          v-if="updateProgress > 0"
          :percentage="updateProgress"
          :stroke-width="8"
          style="margin-top: 10px"
        />
      </div>

      <div class="about-links">
        <a href="https://github.com/xinyi-it/iroh-transfer" target="_blank">GitHub 仓库</a>
        <span class="link-divider">|</span>
        <a href="https://github.com/xinyi-it/iroh-transfer/releases" target="_blank">下载页面</a>
      </div>

      <el-button @click="goBack" style="margin-top: 16px; width: 100%">
        返回
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { check } from '@tauri-apps/plugin-updater'
import { invoke } from '../api/tauri'

const router = useRouter()
const version = ref('0.1.6')
const checking = ref(false)
const updating = ref(false)
const hasUpdate = ref(false)
const updateStatus = ref('')
const updateStatusClass = ref('')
const updateProgress = ref(0)

async function checkUpdate() {
  checking.value = true
  updateStatus.value = ''
  hasUpdate.value = false
  try {
    const update = await check()
    if (update) {
      hasUpdate.value = true
      updateStatus.value = `发现新版本 ${update.version}！${update.body}`
      updateStatusClass.value = 'success'
    } else {
      updateStatus.value = '当前已是最新版本'
      updateStatusClass.value = 'info'
    }
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    updateStatus.value = '检查更新失败: ' + msg
    updateStatusClass.value = 'error'
  } finally {
    checking.value = false
  }
}

async function downloadAndInstall() {
  updating.value = true
  updateProgress.value = 0
  try {
    const update = await check()
    if (!update) {
      ElMessage.info('没有可用的更新')
      return
    }
    await update.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        updateProgress.value = 0
      } else if (event.event === 'Progress') {
        updateProgress.value = Math.min(updateProgress.value + 5, 99)
      } else if (event.event === 'Finished') {
        updateProgress.value = 100
      }
    })
    updateStatus.value = '更新下载完成，即将重启安装...'
    updateStatusClass.value = 'success'
    await update.install()
    await invoke('relaunch')
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    updateStatus.value = '更新失败: ' + msg
    updateStatusClass.value = 'error'
  } finally {
    updating.value = false
  }
}

function goBack() {
  router.push('/')
}
</script>

<style scoped>
.about-container {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: #0d1117;
}

.about-card {
  background: #161b22;
  border: 1px solid #30363d;
  border-radius: 12px;
  padding: 32px;
  width: 360px;
  text-align: center;
}

.about-logo {
  width: 64px;
  height: 64px;
  margin-bottom: 12px;
}

.about-title {
  color: #e6edf3;
  font-size: 22px;
  margin: 0 0 4px;
}

.about-version {
  color: #58a6ff;
  font-size: 14px;
  margin: 0 0 8px;
}

.about-desc {
  color: #8b949e;
  font-size: 13px;
  margin: 0 0 20px;
}

.update-section {
  margin-bottom: 20px;
}

.update-status {
  margin-top: 10px;
  font-size: 13px;
  padding: 8px;
  border-radius: 4px;
}

.update-status.success {
  color: #3fb950;
  background: rgba(63, 185, 80, 0.1);
}

.update-status.info {
  color: #58a6ff;
  background: rgba(88, 166, 255, 0.1);
}

.update-status.error {
  color: #f85149;
  background: rgba(248, 81, 73, 0.1);
}

.about-links {
  margin-top: 16px;
  font-size: 13px;
}

.about-links a {
  color: #58a6ff;
  text-decoration: none;
}

.about-links a:hover {
  text-decoration: underline;
}

.link-divider {
  color: #30363d;
  margin: 0 8px;
}
</style>
