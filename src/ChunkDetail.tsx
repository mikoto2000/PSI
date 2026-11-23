import { useEffect, useMemo, useState } from 'react'
import type { Chunk, Detail, InspectionActions, TextMetadata } from './inspection'
import { Facts, byteCount, offsetHex } from './inspectionDisplay'
import { formatJson } from './json'

export function ChunkDetail({ chunk, id, index, unit, loadDetail, copyText }: {
  chunk: Chunk
  id: number
  index: number
  unit: string
  loadDetail: InspectionActions['detail']
  copyText: InspectionActions['copy']
}) {
  const [detail, setDetail] = useState<Detail | null>(null)
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    void loadDetail(id, index).then((data) => { if (active) setDetail(data) })
      .catch((cause) => { if (active) setError(String(cause)) })
    return () => { active = false }
  }, [id, index, loadDetail])
  const { attributes, storedCrc, computedCrc } = chunk
  const hasCrc = storedCrc !== undefined && computedCrc !== undefined
  const crcOk = storedCrc === computedCrc
  const crcFields: [string, string][] = hasCrc ? [
    ['保存された CRC', crcHex(storedCrc)],
    ['計算した CRC', crcHex(computedCrc)],
  ] : []

  return <section className="panel detail-panel" aria-label={`${unit}詳細`}>
    <div className="panel-title"><h2>{unit}詳細</h2><span className="pill mono">#{index + 1}</span></div>
    <div className="panel-body detail-body">
      <div className="chunk-hero">
        <h3><code>{chunk.name}</code></h3>{hasCrc && <span className={crcOk ? 'pill ok' : 'pill bad'}>{crcOk ? 'CRC 一致' : 'CRC 不一致'}</span>}
        <p>{chunk.description}</p>
      </div>
      {attributes && <div className="attributes">
        <span className="pill">{attributes.ancillary ? 'Ancillary' : 'Critical'}</span>
        <span className="pill">{attributes.private ? 'Private' : 'Public'}</span>
        <span className="pill">{attributes.safeToCopy ? 'Safe-to-copy' : 'Unsafe-to-copy'}</span>
      </div>}
      {Boolean(chunk.issues?.length) && <ul className="detail-warnings">{chunk.issues?.map((issue) => <li key={issue}>{issue}</li>)}</ul>}
      <Facts fields={[
        ['データサイズ', byteCount(chunk.length)],
        ['オフセット', `${offsetHex(chunk.offset)} (${chunk.offset})`],
        ...crcFields,
      ]} />
      {error ? <p className="problem detail-warnings" role="alert">{error}</p> : !detail ? <p className="muted" role="status">詳細を読み込み中…</p> :
        <>
          <Facts fields={detail.fields.map(({ label, value }) => [label, value])} />
          {detail.text && <MetadataText metadata={detail.text} copyText={copyText} />}
        </>}
    </div>
  </section>
}

function crcHex(crc: number) {
  return '0x' + crc.toString(16).toUpperCase().padStart(8, '0')
}

function MetadataText({ metadata, copyText }: { metadata: TextMetadata; copyText: InspectionActions['copy'] }) {
  const formatted = useMemo(() => formatJson(metadata.text), [metadata.text])
  const [raw, setRaw] = useState(false)
  const [status, setStatus] = useState('')
  const isJsonView = formatted !== null && !raw
  const content = isJsonView ? formatted : metadata.text

  async function copy() {
    try {
      await copyText(content)
      setStatus('コピーしました')
    } catch (error) {
      setStatus('コピーできませんでした。' + String(error))
    }
  }

  return <div className="text-detail">
    <Facts fields={[
      ['Keyword', metadata.keyword],
      ['Encoding', metadata.encoding],
      ['Compression', metadata.compressed ? 'Deflate (zlib)' : 'なし'],
      ...(metadata.languageTag !== null ? [['Language Tag', metadata.languageTag] as [string, string]] : []),
      ...(metadata.translatedKeyword !== null ? [['Translated Keyword', metadata.translatedKeyword] as [string, string]] : []),
    ]} />
    <div className="text-toolbar">
      <h4>Text</h4>
      <button className="secondary" onClick={() => void copy()}>{isJsonView ? 'JSON をコピー' : 'テキストをコピー'}</button>
    </div>
    {formatted !== null && <div className="view-switch text-switch" role="group" aria-label="本文の表示形式">
      <button aria-pressed={!raw} onClick={() => { setRaw(false); setStatus('') }}>整形 JSON</button>
      <button aria-pressed={raw} onClick={() => { setRaw(true); setStatus('') }}>元のテキスト</button>
    </div>}
    <pre className="metadata-body" aria-label="テキスト本文" tabIndex={0}>{content}</pre>
    {content === '' && <p className="muted">空のテキスト</p>}
    {status && <p className="copy-status" role="status">{status}</p>}
  </div>
}
