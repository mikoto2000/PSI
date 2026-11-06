import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import { desktop, type Chunk, type DocumentInfo } from './desktop'

vi.mock('./desktop', () => ({
  desktop: { pick: vi.fn(), detail: vi.fn(), preview: vi.fn(), onDrop: vi.fn(), openDrop: vi.fn(), adjacent: vi.fn(), copy: vi.fn() },
}))

function chunk(name: string, description: string, offset: number, crcOk = true, textKeyword: string | null = null): Chunk {
  return {
    name, description, offset, length: 13, storedCrc: 42, computedCrc: crcOk ? 42 : 43,
    issues: [], textKeyword, attributes: { ancillary: textKeyword !== null, private: false, reservedBitSet: false, safeToCopy: false },
  }
}

const sample: DocumentInfo = {
  id: 1, name: 'sample.png', size: 425,
  report: {
    header: { width: 32, height: 24, bitDepth: 8, colorName: 'Truecolor with alpha' },
    issues: [],
    chunks: [chunk('IHDR', 'Image header', 8), chunk('IDAT', 'Compressed image data', 33, false), chunk('IEND', 'End of datastream', 58)],
  },
}

beforeEach(() => {
  vi.resetAllMocks()
  vi.mocked(desktop.onDrop).mockResolvedValue(vi.fn())
  vi.mocked(desktop.pick).mockResolvedValue(sample)
  vi.mocked(desktop.detail).mockImplementation(async (_, index) => ({
    fields: index === 0 ? [{ label: 'Width', value: '32' }] : [],
    text: null,
  }))
  vi.mocked(desktop.preview).mockResolvedValue(new ArrayBuffer(8))
  vi.stubGlobal('URL', { createObjectURL: vi.fn(() => 'blob:preview'), revokeObjectURL: vi.fn() })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('PNG inspection', () => {
  it('shows the empty state before a file is chosen', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'PNG Structure Inspector' })).toBeInTheDocument()
    expect(screen.getByText('PNG ファイルを選択してください')).toBeInTheDocument()
  })

  it('opens a PNG, lists chunks in file order and changes details when selected', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
    expect(await screen.findByText('sample.png')).toBeInTheDocument()
    expect(screen.getByText('32 × 24')).toBeInTheDocument()
    expect(await screen.findByRole('img', { name: 'sample.png のプレビュー' })).toBeInTheDocument()
    const table = screen.getByRole('table', { name: 'チャンク一覧' })
    const rows = within(table).getAllByRole('row').slice(1)
    expect(rows.map((row) => within(row).getByRole('button').textContent)).toEqual(['IHDR', 'IDAT', 'IEND'])
    expect(await screen.findByText('Width')).toBeInTheDocument()
    await user.click(within(table).getByRole('button', { name: 'IDAT' }))
    const detail = screen.getByRole('region', { name: 'チャンク詳細' })
    expect(within(detail).getByText('Compressed image data')).toBeInTheDocument()
    expect(within(detail).getByText('CRC 不一致')).toBeInTheDocument()
    expect(screen.queryByText('Width')).not.toBeInTheDocument()
    expect(vi.mocked(desktop.detail)).toHaveBeenLastCalledWith(1, 1)
  })

  it('reports an open failure and allows retry', async () => {
    vi.mocked(desktop.pick).mockRejectedValueOnce('Invalid PNG signature')
    const user = userEvent.setup()
    render(<App />)
    const open = screen.getByRole('button', { name: 'PNG を開く' })
    await user.click(open)
    expect(await screen.findByRole('alert')).toHaveTextContent('Invalid PNG signature')
    await user.click(open)
    expect(await screen.findByText('sample.png')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('keeps the chunk list when the image cannot be displayed', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
    fireEvent.error(await screen.findByRole('img', { name: 'sample.png のプレビュー' }))
    expect(screen.getByText('画像をプレビューできませんでした')).toBeInTheDocument()
    expect(screen.getByRole('table', { name: 'チャンク一覧' })).toBeInTheDocument()
  })
})

describe('text metadata', () => {
  const metadataSample: DocumentInfo = {
    ...sample,
    report: {
      ...sample.report,
      chunks: [
        sample.report.chunks[0],
        chunk('tEXt', 'Latin-1 text', 33, true, 'prompt'),
        chunk('iTXt', 'International text', 58, true, 'prompt'),
        ...sample.report.chunks.slice(1).map((item) => ({ ...item, offset: item.offset + 50 })),
      ],
    },
  }
  const raw = '{"id":9007199254740993,"nodes":[{"key":"a","key":"b"}]}'
  const formatted = '{\n  "id": 9007199254740993,\n  "nodes": [\n    {\n      "key": "a",\n      "key": "b"\n    }\n  ]\n}'
  const text = { keyword: 'prompt', languageTag: 'ja', translatedKeyword: '指示', compressed: true, encoding: 'UTF-8' }

  beforeEach(() => {
    vi.mocked(desktop.pick).mockResolvedValue(metadataSample)
    vi.mocked(desktop.detail).mockImplementation(async (_, index) => ({
      fields: [],
      text: index === 1 || index === 2 ? { ...text, text: index === 1 ? raw : '日本語 {broken' } : null,
    }))
  })

  it('shows each duplicate keyword, opens details and falls back to plain text', async () => {
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
    await user.click(await screen.findByRole('button', { name: /テキストメタデータ/ }))
    const table = screen.getByRole('table', { name: 'テキストメタデータ一覧' })
    expect(within(table).getAllByText('prompt')).toHaveLength(2)
    expect(within(table).queryByText('IHDR')).not.toBeInTheDocument()
    await user.click(within(table).getByRole('button', { name: 'iTXt' }))
    expect(await screen.findByText('日本語 {broken')).toBeInTheDocument()
    expect(screen.getByText('指示')).toBeInTheDocument()
    expect(screen.getByText('ja')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '整形 JSON' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /すべてのチャンク/ }))
    expect(screen.getByRole('table', { name: 'チャンク一覧' })).toBeInTheDocument()
  })

  it('formats nested JSON without changing numbers or duplicate keys and copies the active view', async () => {
    const user = userEvent.setup()
    const copy = vi.mocked(desktop.copy)
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
    await user.click(await screen.findByRole('button', { name: 'tEXt' }))
    const body = await screen.findByLabelText('テキスト本文')
    expect(body.textContent).toBe(formatted)
    await user.click(screen.getByRole('button', { name: 'JSON をコピー' }))
    expect(copy).toHaveBeenLastCalledWith(formatted)
    expect(screen.getByText('コピーしました')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '元のテキスト' }))
    expect(body.textContent).toBe(raw)
    await user.click(screen.getByRole('button', { name: 'テキストをコピー' }))
    expect(copy).toHaveBeenLastCalledWith(raw)
  })
})

it('opens a dropped PNG and replaces the previous document', async () => {
  const user = userEvent.setup()
  render(<App />)
  await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
  expect(await screen.findByText('sample.png')).toBeInTheDocument()
  vi.mocked(desktop.openDrop).mockResolvedValue({ ...sample, id: 2, name: 'dropped.png' })
  const onDrop = vi.mocked(desktop.onDrop).mock.lastCall?.[0]
  expect(onDrop).toBeTypeOf('function')
  await act(async () => { onDrop?.() })
  expect(await screen.findByText('dropped.png')).toBeInTheDocument()
  expect(screen.queryByText('sample.png')).not.toBeInTheDocument()
})

it('pages through large chunk lists in file order', async () => {
  const user = userEvent.setup()
  const chunks = Array.from({ length: 202 }, (_, index) => chunk('vpAg', 'Unknown chunk', 8 + index * 12))
  vi.mocked(desktop.pick).mockResolvedValue({ ...sample, report: { ...sample.report, chunks } })
  render(<App />)
  await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
  const table = await screen.findByRole('table', { name: 'チャンク一覧' })
  expect(within(table).getAllByRole('row')).toHaveLength(201)
  await user.click(screen.getByRole('button', { name: '次のページ' }))
  const nextTable = screen.getByRole('table', { name: 'チャンク一覧' })
  expect(within(nextTable).getAllByRole('row')).toHaveLength(3)
  await user.click(within(nextTable).getAllByRole('button')[1])
  expect(vi.mocked(desktop.detail)).toHaveBeenLastCalledWith(1, 201)
})

it('uses left and right to navigate from the current document', async () => {
  const user = userEvent.setup()
  const adjacent = vi.mocked(desktop.adjacent)
  adjacent.mockResolvedValueOnce({ ...sample, id: 2, name: 'next.png' }).mockResolvedValueOnce(null)
  render(<App />)
  await user.keyboard('{ArrowRight}')
  expect(adjacent).not.toHaveBeenCalled()
  await user.click(screen.getByRole('button', { name: 'PNG を開く' }))
  await screen.findByText('sample.png')
  await user.keyboard('{ArrowRight}')
  expect(await screen.findByText('next.png')).toBeInTheDocument()
  expect(adjacent).toHaveBeenLastCalledWith(1, 'next')
  await user.keyboard('{ArrowLeft}')
  expect(adjacent).toHaveBeenLastCalledWith(2, 'previous')
  expect(screen.getByText('next.png')).toBeInTheDocument()
})
