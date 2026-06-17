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

export function listen<T = unknown>(event: string, handler: (event: T) => void): Promise<() => void> {
  return window.__TAURI_INTERNALS__.listen(event, (e: { payload: unknown }) => handler(e.payload as T))
}

// 类型定义
export interface SendFileResult {
  ticket: string
  file_name: string
  node_id: string
  file_size: number
}

export interface DownloadProgress {
  status: 'downloading' | 'completed' | 'failed'
  downloaded_size: number
  total_size: number
  save_path?: string
  error?: string
}
