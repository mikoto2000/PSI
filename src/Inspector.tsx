import { useMemo, useState } from 'react'
import type { Chunk, DocumentInfo, InspectionActions } from './inspection'
import { ChunkDetail } from './ChunkDetail'
import { Preview } from './Preview'
import { byteCount, offsetHex } from './inspectionDisplay'

export function Inspector({ document, actions }: { document: DocumentInfo; actions: InspectionActions }) {
  const { report } = document
  const [index, setIndex] = useState(0)
  const [page, setPage] = useState(0)
  const [metadataOnly, setMetadataOnly] = useState(false)
  const rows = useMemo(() => report.chunks.map((chunk, item) => ({ chunk, item })), [report.chunks])
  const textRows = useMemo(() => rows.filter(({ chunk }) => ['tEXt', 'zTXt', 'iTXt'].includes(chunk.name)), [rows])
  const visible = metadataOnly ? textRows : rows
  const pageSize = 200
  const pages = Math.ceil(visible.length / pageSize)
  const pageRows = visible.slice(page * pageSize, (page + 1) * pageSize)
  const chunk = report.chunks[index]

  function changeView(textOnly: boolean) {
    if (textOnly === metadataOnly) return
    const nextRows = textOnly ? textRows : rows
    const position = nextRows.findIndex((row) => row.item === index)
    setPage(Math.floor(Math.max(0, position) / pageSize))
    setMetadataOnly(textOnly)
    if (textOnly && position < 0) {
      setIndex(textRows[0]?.item ?? -1)
    } else if (!textOnly && index < 0) {
      setIndex(0)
    }
  }
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
      <Preview document={document} loadPreview={actions.preview} />
      <section className="panel list-panel" aria-label="チャンク">
        <div className="panel-title"><h2>チャンク一覧</h2><span className="pill">{report.chunks.length} 件</span></div>
        <div className="view-switch" role="group" aria-label="チャンク表示">
          <button aria-pressed={!metadataOnly} onClick={() => changeView(false)}>すべてのチャンク</button>
          <button aria-pressed={metadataOnly} onClick={() => changeView(true)}>テキストメタデータ <span>{textRows.length}</span></button>
        </div>
        <ChunkList key={`${metadataOnly}-${page}`} rows={pageRows} index={index} onSelect={setIndex} metadataOnly={metadataOnly} />
        {pages > 1 && <nav className="pagination" aria-label="チャンク一覧のページ">
          <button className="secondary" disabled={page === 0} onClick={() => setPage(page - 1)}>前のページ</button>
          <span aria-live="polite">{page + 1} / {pages}</span>
          <button className="secondary" disabled={page + 1 === pages} onClick={() => setPage(page + 1)}>次のページ</button>
        </nav>}
      </section>
      {chunk ? <ChunkDetail key={index} id={document.id} index={index} chunk={chunk} loadDetail={actions.detail} copyText={actions.copy} /> :
        <section className="panel"><div className="panel-title"><h2>チャンク詳細</h2></div><p className="panel-body muted">表示できるチャンクがありません</p></section>}
    </div>
  </>
}

function ChunkList({ rows, index, onSelect, metadataOnly }: { rows: { chunk: Chunk; item: number }[]; index: number; onSelect: (index: number) => void; metadataOnly: boolean }) {
  if (!rows.length) return <p className="panel-body muted">{metadataOnly ? "テキストメタデータはありません" : "読み取れたチャンクはありません"}</p>
  return <div className="table-scroll">
    <table aria-label={metadataOnly ? "テキストメタデータ一覧" : "チャンク一覧"}>
      <thead><tr><th scope="col">チャンク</th><th scope="col">サイズ</th><th scope="col">オフセット</th><th scope="col">CRC</th></tr></thead>
      <tbody>{rows.map(({ chunk, item }) =>
        <tr key={chunk.offset} className={index === item ? 'selected' : ''} onClick={() => onSelect(item)}>
          <td><button className="chunk-button" aria-pressed={index === item} onClick={(event) => { event.stopPropagation(); onSelect(item) }}>{chunk.name}</button>
            <small>{metadataOnly ? chunk.textKeyword ?? "Keyword 不明" : chunk.description}{chunk.issues.length > 0 && <span className="warn-mark" title={chunk.issues.join('\n')}> ⚠</span>}</small></td>
          <td className="numeric">{byteCount(chunk.length)}</td>
          <td className="numeric">{offsetHex(chunk.offset)}</td>
          <td><Crc chunk={chunk} /></td>
        </tr>)}</tbody>
    </table>
  </div>
}

function Crc({ chunk }: { chunk: Chunk }) {
  const ok = chunk.storedCrc === chunk.computedCrc
  return <span className={ok ? 'pill ok' : 'pill bad'}>{ok ? '一致' : '不一致'}</span>
}
