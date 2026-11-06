import { useCallback, useEffect, useRef, useState } from 'react'
import { desktop, type DocumentInfo } from './desktop'
import { Inspector } from './Inspector'

export default function App() {
  const [document, setDocument] = useState<DocumentInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const opening = useRef(false)

  const open = useCallback(async (load: () => Promise<DocumentInfo | null>) => {
    if (opening.current) return
    opening.current = true
    setBusy(true)
    setError('')
    try {
      const selected = await load()
      if (selected) setDocument(selected)
    } catch (cause) {
      setError(String(cause))
    } finally {
      opening.current = false
      setBusy(false)
    }
  }, [])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    void desktop.onDrop(() => { if (active) void open(desktop.openDrop) })
      .then((stop) => { if (active) unlisten = stop; else stop() })
      .catch((cause) => { if (active) setError('ドロップを有効にできませんでした。' + String(cause)) })
    return () => { active = false; unlisten?.() }
  }, [open])

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
    {document ? <Inspector key={document.id} document={document} /> :
      <section className="empty">
        <div className="empty-icon" aria-hidden="true">PNG</div>
        <p className="eyebrow">ファイルを検査</p>
        <h2>PNG ファイルを選択してください</h2>
        <p>ここに PNG をドロップするか、上のボタンから開いてください。</p>
      </section>}
    <footer>PNG STRUCTURE INSPECTOR <span>読み取り専用</span></footer>
  </main>
}
