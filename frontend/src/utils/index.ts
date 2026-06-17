export function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB'
  if (bytes < 1073741824) return (bytes / 1048576).toFixed(1) + ' MB'
  return (bytes / 1073741824).toFixed(2) + ' GB'
}

export interface ParsedTicket {
  fileName: string
  nodeId: string
  fileSize: number
  ticket: string
}

export function parseTicketInput(text: string): ParsedTicket {
  const trimmed = text.trim()
  let fileName = ''
  let nodeId = ''
  let fileSize = 0
  let ticket = ''

  if (trimmed.startsWith('iroh://') && trimmed.includes('|')) {
    const afterPrefix = trimmed.substring(7)
    const parts = afterPrefix.split('|')
    if (parts.length >= 4) {
      fileName = parts[0] || ''
      nodeId = parts[1] || ''
      fileSize = parseInt(parts[2] || '0') || 0
      ticket = parts.slice(3).join('|')
    } else if (parts.length === 3) {
      fileName = parts[0] || ''
      nodeId = parts[1] || ''
      ticket = parts.slice(2).join('|')
    } else if (parts.length === 2) {
      fileName = parts[0] || ''
      ticket = parts[1] || ''
    }
  } else if (trimmed.includes('\n') && trimmed.includes('票据')) {
    const lines = trimmed.split('\n')
    for (const line of lines) {
      if (line.startsWith('文件名:') || line.startsWith('文件名：')) {
        fileName = line.replace(/文件名[:：]/, '').trim() ?? ''
      }
      if (line.startsWith('票据:') || line.startsWith('票据：')) {
        ticket = line.replace(/票据[:：]/, '').trim() ?? ''
      }
    }
  } else {
    ticket = trimmed
  }

  return { fileName, nodeId, fileSize, ticket }
}
