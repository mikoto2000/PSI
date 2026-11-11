export function Facts({ fields }: { fields: [string, string][] }) {
  if (!fields.length) return null
  return <dl className="facts">{fields.map(([label, value]) =>
    <div key={label}><dt>{label}</dt><dd>{value || '—'}</dd></div>)}</dl>
}

export function byteCount(size: number) {
  return `${size.toLocaleString('ja-JP')} B`
}
export function offsetHex(offset: number) {
  return '0x' + offset.toString(16).toUpperCase().padStart(2, '0')
}
