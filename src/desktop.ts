import { listen } from '@tauri-apps/api/event'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { invoke } from '@tauri-apps/api/core'

export interface Header {
  width: number
  height: number
  bitDepth: number
  colorName: string
}

export interface Chunk {
  name: string
  description: string
  length: number
  offset: number
  storedCrc: number
  computedCrc: number
  issues: string[]
  textKeyword: string | null
  attributes: {
    ancillary: boolean
    private: boolean
    reservedBitSet: boolean
    safeToCopy: boolean
  } | null
}

export interface DocumentInfo {
  id: number
  name: string
  size: number
  report: { header: Header | null; chunks: Chunk[]; issues: string[] }
}

export interface TextMetadata {
  keyword: string
  text: string
  languageTag: string | null
  translatedKeyword: string | null
  compressed: boolean
  encoding: string
}

export interface Detail {
  fields: { label: string; value: string }[]
  text: TextMetadata | null
}

export const desktop = {
  onDrop: (handler: () => void) => listen('png-drop', handler),
  openDrop: () => invoke<DocumentInfo | null>('open_drop'),
  copy: (text: string) => writeText(text),
  pick: () => invoke<DocumentInfo | null>('pick_png'),
  adjacent: (id: number, direction: 'previous' | 'next') => invoke<DocumentInfo | null>('adjacent_png', { id, direction }),
  detail: (id: number, index: number) => invoke<Detail>('chunk_detail', { id, index }),
  preview: (id: number) => invoke<ArrayBuffer>('png_preview', { id }),
}
