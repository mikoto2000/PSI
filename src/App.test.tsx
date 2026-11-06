import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import { desktop, type Chunk, type DocumentInfo } from './desktop'

vi.mock('./desktop', () => ({
  desktop: { pick: vi.fn(), detail: vi.fn(), preview: vi.fn() },
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
