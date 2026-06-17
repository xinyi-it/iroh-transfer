import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface HistoryItem {
  type: '发送' | '接收'
  name: string
  status: '成功' | '失败'
  time: string
}

export const useHistoryStore = defineStore('history', () => {
  const items = ref<HistoryItem[]>([])

  function add(type: '发送' | '接收', name: string, status: '成功' | '失败') {
    const time = new Date().toLocaleTimeString('zh-CN')
    items.value.unshift({ type, name, status, time })
    if (items.value.length > 20) items.value.pop()
  }

  return { items, add }
})
