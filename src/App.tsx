import { useCallback, useEffect, useRef, useState } from 'react'
import { desktop } from './desktop'
import type { DocumentInfo } from './inspection'
import { Inspector } from './Inspector'

export default function App() {
  const [document, setDocument] = useState<DocumentInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const opening = useRef(false)
  const pendingDrop = useRef(false)

  const open = useCallback(async (load: () => Promise<DocumentInfo | null>) => {
    if (opening.current) return
    opening.current = true
    setBusy(true)
    try {
      let loadNext = load
      do {
        pendingDrop.current = false
        setError('')
        try {
          const selected = await loadNext()
          if (selected) setDocument(selected)
        } catch (cause) {
          setError(String(cause))
        }
        loadNext = desktop.openDrop
      } while (pendingDrop.current)
    } finally {
      opening.current = false
      setBusy(false)
    }
  }, [])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    void desktop.onDrop(() => {
      if (!active) return
      // Tauri retains the latest drop until openDrop consumes it.
      if (opening.current) pendingDrop.current = true
      else void open(desktop.openDrop)
    })
      .then((stop) => { if (active) unlisten = stop; else stop() })
      .catch((cause) => { if (active) setError('ドロップを有効にできませんでした。' + String(cause)) })
    return () => { active = false; unlisten?.() }
  }, [open])

  const documentId = document?.id
  useEffect(() => {
    if (documentId === undefined) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return
      if (event.target instanceof HTMLElement && (event.target.isContentEditable || event.target.closest('input, textarea, select, pre'))) return
      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
      event.preventDefault()
      const direction = event.key === 'ArrowLeft' ? 'previous' : 'next'
      void open(() => desktop.adjacent(documentId, direction))
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [documentId, open])

  return <main className="app-shell">
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">▧</span>
        <div><h1>PNG Structure Inspector</h1><p>PNG の構造とメタデータを調べる</p></div>
      </div>
      <button className="primary" onClick={() => void open(desktop.pick)} disabled={busy}>PNG を開く</button>
    </header>
    {busy && <p className="message" role="status">PNG を読み込み中…</p>}
    {error && <p className="message problem" role="alert">PNG を開けませんでした。{error}</p>}
    {document ? <Inspector key={document.id} document={document} actions={desktop} /> :
      <section className="empty">
        <div className="empty-icon" aria-hidden="true">PNG</div>
        <p className="eyebrow">ファイルを検査</p>
        <h2>PNG ファイルを選択してください</h2>
        <p>ここに PNG をドロップするか、上のボタンから開いてください。</p>
      </section>}
    <footer>{document ? <span><kbd>←</kbd> 前の画像 · <kbd>→</kbd> 次の画像</span> : "PNG STRUCTURE INSPECTOR"} <span>読み取り専用</span></footer>
  </main>
}
