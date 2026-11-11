import { useEffect, useState } from 'react'
import type { DocumentInfo, InspectionActions } from './inspection'
import { Facts, byteCount } from './inspectionDisplay'

export function Preview({ document: { id, name, size, report }, loadPreview }: { document: DocumentInfo; loadPreview: InspectionActions['preview'] }) {
  const [src, setSrc] = useState('')
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    let url = ''
    void loadPreview(id).then((bytes) => {
      if (!active) return
      url = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
      setSrc(url)
    }).catch((cause) => { if (active) setError(String(cause)) })
    return () => {
      active = false
      if (url) URL.revokeObjectURL(url)
    }
  }, [id, loadPreview])
  const header = report.header

  return <section className="panel" aria-label="画像プレビュー">
    <div className="panel-title"><h2>画像プレビュー</h2></div>
    <div className="panel-body">
      <div className="preview">
        {error ? <div className="preview-note"><strong>画像をプレビューできませんでした</strong><small>{error}</small></div>
          : src ? <img src={src} alt={name + ' のプレビュー'} onError={() => setError('PNG の画像データを表示できません。')} />
          : <span className="preview-note">プレビューを読み込み中…</span>}
      </div>
      <Facts fields={[
        ['ファイルサイズ', byteCount(size)],
        ['画像サイズ', header ? `${header.width} × ${header.height}` : '—'],
        ['色形式', header?.colorName ?? '—'],
        ['ビット深度', header ? `${header.bitDepth} bit` : '—'],
        ['チャンク数', String(report.chunks.length)],
      ]} />
    </div>
  </section>
}
