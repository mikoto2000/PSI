import { mkdir, writeFile } from 'node:fs/promises'
import { crc32, deflateSync } from 'node:zlib'

const directory = new URL('../fixtures/', import.meta.url)
const signature = Buffer.from('89504e470d0a1a0a', 'hex')
const header = Buffer.from('00000002000000020806000000', 'hex')
const pixels = Buffer.from([
  0, 128, 230, 214, 255, 99, 147, 255, 128,
  0, 99, 147, 255, 128, 128, 230, 214, 0,
])

function chunk(name, data = Buffer.alloc(0)) {
  const body = Buffer.concat([Buffer.from(name, 'ascii'), data])
  const framing = Buffer.alloc(8)
  framing.writeUInt32BE(data.length)
  framing.writeUInt32BE(crc32(body), 4)
  return Buffer.concat([framing.subarray(0, 4), body, framing.subarray(4)])
}

function png(...metadata) {
  return Buffer.concat([signature, chunk('IHDR', header), ...metadata,
    chunk('IDAT', deflateSync(pixels)), chunk('IEND')])
}

const latin = chunk('tEXt', Buffer.from('Title\0Café', 'latin1'))
const compressed = chunk('zTXt', Buffer.concat([Buffer.from('Title\0\0'), deflateSync('Compressed text')]))
const international = chunk('iTXt', Buffer.from('Title\0\0\0ja\0表題\0日本語のメタデータ'))
const json = chunk('tEXt', Buffer.from('prompt\0{"id":9007199254740993,"nodes":[{"type":"example","enabled":true}]}'))
const badCrc = Buffer.from(latin)
badCrc[badCrc.length - 1] ^= 1

const samples = {
  'plain.png': png(),
  'text.png': png(latin),
  'compressed-text.png': png(compressed),
  'japanese.png': png(international),
  'json.png': png(json),
  'multiple-text.png': png(latin, compressed, international, json),
  'bad-crc.png': png(badCrc),
  'truncated.png': png(latin).subarray(0, -3),
}

await mkdir(directory, { recursive: true })
await Promise.all(Object.entries(samples).map(([name, bytes]) => writeFile(new URL(name, directory), bytes)))
