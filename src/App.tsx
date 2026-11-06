import { useState } from 'react'
import { desktop, type DocumentInfo } from './desktop'
import { Inspector } from './Inspector'

export default function App() {
  const [document, setDocument] = useState<DocumentInfo | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  async function pick() {
    setBusy(true)
    setError('')
    try {
      const selected = await desktop.pick()
      if (selected) setDocument(selected)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  return <main className="app-shell">
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">▧</span>
        <div><h1>PNG Structure Inspector</h1><p>PNG の構造とメタデータを調べる</p></div>
      </div>
      <button className="primary" onClick={() => void pick()} disabled={busy}>PNG を開く</button>
    </header>
    {busy && <p className="message" role="status">PNG を読み込み中…</p>}
    {error && <p className="message problem" role="alert">PNG を開けませんでした。{error}</p>}
    {document ? <Inspector key={document.id} document={document} /> :
      <section className="empty">
        <div className="empty-icon" aria-hidden="true">PNG</div>
        <p className="eyebrow">ファイルを検査</p>
        <h2>PNG ファイルを選択してください</h2>
        <p>画像のプレビュー、チャンクの構造、テキストメタデータを確認できます。</p>
      </section>}
    <footer>PNG STRUCTURE INSPECTOR <span>読み取り専用</span></footer>
  </main>
}
