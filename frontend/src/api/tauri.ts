// Tauri invoke 封装
declare global {
  interface Window {
    __TAURI_INTERNALS__: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
      listen: (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>
    }
  }
}

export function invoke<T = string>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return window.__TAURI_INTERNALS__.invoke(cmd, args) as Promise<T>
}

// 类型定义
export interface SendFileResult {
  ticket: string
  file_name: string
  node_id: string
  file_size: number
}

export interface DownloadStatus {
  status: 'idle' | 'downloading' | 'completed' | 'failed'
  blob_hash: string
  downloaded_size: number
  base_size: number
  error?: string
}
