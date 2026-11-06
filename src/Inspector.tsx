import { useEffect, useState } from 'react'
import { desktop, type Chunk, type Detail, type DocumentInfo } from './desktop'

export function Inspector({ document }: { document: DocumentInfo }) {
  const { report } = document
  const [index, setIndex] = useState(0)
  const chunk = report.chunks[index]
  const crcErrors = report.chunks.filter((chunk) => chunk.storedCrc !== chunk.computedCrc).length
  const structureWarnings = report.issues.length + report.chunks.reduce((sum, chunk) => sum + chunk.issues.length, 0)

  return <>
    <div className="filebar">
      <div className="file-id"><span className="file-tag">PNG</span><strong title={document.name}>{document.name}</strong></div>
      <span className={crcErrors || structureWarnings ? 'pill bad' : 'pill ok'}>
        CRC 不一致 {crcErrors} 件 · 構造警告 {structureWarnings} 件
      </span>
    </div>
    {report.issues.length > 0 && <section className="message problem" aria-label="PNG 構造の問題">
      <strong>PNG 構造の問題</strong><ul>{report.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>
    </section>}
    <div className="workspace">
      <Preview document={document} />
      <section className="panel list-panel" aria-label="チャンク">
        <div className="panel-title"><h2>チャンク一覧</h2><span className="pill">{report.chunks.length} 件</span></div>
        <ChunkList chunks={report.chunks} index={index} onSelect={setIndex} />
      </section>
      {chunk ? <ChunkDetail key={index} id={document.id} index={index} chunk={chunk} /> :
        <section className="panel"><div className="panel-title"><h2>チャンク詳細</h2></div><p className="panel-body muted">表示できるチャンクがありません</p></section>}
    </div>
  </>
}

function Preview({ document: { id, name, size, report } }: { document: DocumentInfo }) {
  const [src, setSrc] = useState('')
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    let url = ''
    void desktop.preview(id).then((bytes) => {
      if (!active) return
      url = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }))
      setSrc(url)
    }).catch((cause) => { if (active) setError(String(cause)) })
    return () => {
      active = false
      if (url) URL.revokeObjectURL(url)
    }
  }, [id])
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

function ChunkList({ chunks, index, onSelect }: { chunks: Chunk[]; index: number; onSelect: (index: number) => void }) {
  if (!chunks.length) return <p className="panel-body muted">読み取れたチャンクはありません</p>
  return <div className="table-scroll">
    <table aria-label="チャンク一覧">
      <thead><tr><th scope="col">チャンク</th><th scope="col">サイズ</th><th scope="col">オフセット</th><th scope="col">CRC</th></tr></thead>
      <tbody>{chunks.map((chunk, item) =>
        <tr key={chunk.offset} className={index === item ? 'selected' : ''} onClick={() => onSelect(item)}>
          <td><button className="chunk-button" aria-pressed={index === item} onClick={(event) => { event.stopPropagation(); onSelect(item) }}>{chunk.name}</button>
            <small>{chunk.description}{chunk.issues.length > 0 && <span className="warn-mark" title={chunk.issues.join('\n')}> ⚠</span>}</small></td>
          <td className="numeric">{byteCount(chunk.length)}</td>
          <td className="numeric">{offsetHex(chunk.offset)}</td>
          <td><Crc chunk={chunk} /></td>
        </tr>)}</tbody>
    </table>
  </div>
}

function ChunkDetail({ chunk, id, index }: { chunk: Chunk; id: number; index: number }) {
  const [detail, setDetail] = useState<Detail | null>(null)
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    void desktop.detail(id, index).then((data) => { if (active) setDetail(data) })
      .catch((cause) => { if (active) setError(String(cause)) })
    return () => { active = false }
  }, [id, index])
  const attributes = chunk.attributes
  const crcOk = chunk.storedCrc === chunk.computedCrc

  return <section className="panel detail-panel" aria-label="チャンク詳細">
    <div className="panel-title"><h2>チャンク詳細</h2><span className="pill mono">#{index + 1}</span></div>
    <div className="panel-body detail-body">
      <div className="chunk-hero">
        <h3><code>{chunk.name}</code></h3><span className={crcOk ? 'pill ok' : 'pill bad'}>{crcOk ? 'CRC 一致' : 'CRC 不一致'}</span>
        <p>{chunk.description}</p>
      </div>
      {attributes && <div className="attributes">
        <span className="pill">{attributes.ancillary ? 'Ancillary' : 'Critical'}</span>
        <span className="pill">{attributes.private ? 'Private' : 'Public'}</span>
        <span className="pill">{attributes.safeToCopy ? 'Safe-to-copy' : 'Unsafe-to-copy'}</span>
      </div>}
      {chunk.issues.length > 0 && <ul className="detail-warnings">{chunk.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
      <Facts fields={[
        ['データサイズ', byteCount(chunk.length)],
        ['オフセット', `${offsetHex(chunk.offset)} (${chunk.offset})`],
        ['保存された CRC', crcHex(chunk.storedCrc)],
        ['計算した CRC', crcHex(chunk.computedCrc)],
      ]} />
      {error ? <p className="problem detail-warnings" role="alert">{error}</p> : !detail ? <p className="muted" role="status">詳細を読み込み中…</p> :
        <Facts fields={detail.fields.map(({ label, value }) => [label, value])} />}
    </div>
  </section>
}

function Crc({ chunk }: { chunk: Chunk }) {
  const ok = chunk.storedCrc === chunk.computedCrc
  return <span className={ok ? 'pill ok' : 'pill bad'}>{ok ? '一致' : '不一致'}</span>
}

export function Facts({ fields }: { fields: [string, string][] }) {
  if (!fields.length) return null
  return <dl className="facts">{fields.map(([label, value]) =>
    <div key={label}><dt>{label}</dt><dd>{value || '—'}</dd></div>)}</dl>
}

function byteCount(size: number) {
  return `${size.toLocaleString('ja-JP')} B`
}
function offsetHex(offset: number) {
  return '0x' + offset.toString(16).toUpperCase().padStart(2, '0')
}
function crcHex(crc: number) {
  return '0x' + crc.toString(16).toUpperCase().padStart(8, '0')
}
