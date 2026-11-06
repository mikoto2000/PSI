const maxFormattedLength = 8 * 1024 * 1024
const maxDepth = 120

function whitespace(char: string | undefined) {
  return char === ' ' || char === '\n' || char === '\r' || char === '\t'
}

export function formatJson(source: string): string | null {
  try {
    JSON.parse(source)
  } catch {
    return null
  }

  // Keep the original tokens, including number spellings and repeated object keys.
  let output = ''
  let depth = 0
  let index = 0
  const newline = () => '\n' + '  '.repeat(depth)

  while (index < source.length) {
    const char = source[index++]
    if (whitespace(char)) continue
    if (char === '{' || char === '[') {
      while (whitespace(source[index])) index++
      if (source[index] === (char === '{' ? '}' : ']')) {
        output += char + source[index++]
      } else {
        if (++depth > maxDepth) return null
        output += char + newline()
      }
    } else if (char === '}' || char === ']') {
      depth--
      output += newline() + char
    } else if (char === ',') {
      output += char + newline()
    } else if (char === ':') {
      output += ': '
    } else {
      const start = index - 1
      if (char === '"') {
        while (index < source.length) {
          const next = source[index++]
          if (next === '"') break
          if (next === '\\') index++
        }
      } else {
        while (index < source.length && !whitespace(source[index]) && !',]}'.includes(source[index])) index++
      }
      output += source.slice(start, index)
    }
    if (output.length > maxFormattedLength) return null
  }
  return output
}
