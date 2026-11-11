import { listen } from '@tauri-apps/api/event'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { invoke } from '@tauri-apps/api/core'

import type { Detail, DocumentInfo } from './inspection'

export const desktop = {
  onDrop: (handler: () => void) => listen('png-drop', handler),
  openDrop: () => invoke<DocumentInfo | null>('open_drop'),
  copy: (text: string) => writeText(text),
  pick: () => invoke<DocumentInfo | null>('pick_png'),
  adjacent: (id: number, direction: 'previous' | 'next') => invoke<DocumentInfo | null>('adjacent_png', { id, direction }),
  detail: (id: number, index: number) => invoke<Detail>('chunk_detail', { id, index }),
  preview: (id: number) => invoke<ArrayBuffer>('png_preview', { id }),
}
