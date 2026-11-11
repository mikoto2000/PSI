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

export interface InspectionActions {
  preview: (id: number) => Promise<ArrayBuffer>
  detail: (id: number, index: number) => Promise<Detail>
  copy: (text: string) => Promise<void>
}
