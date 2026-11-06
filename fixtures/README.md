# テスト用 PNG

`node scripts/generate-fixtures.mjs` で 2 × 2 ピクセルのサンプルを生成します。Rust の結合テストとデスクトップ版の手動確認で使えます。

| ファイル | 内容 |
| --- | --- |
| `plain.png` | 透明・半透明を含む RGBA |
| `text.png` | Latin-1 の tEXt |
| `compressed-text.png` | zTXt |
| `japanese.png` | 日本語の iTXt |
| `json.png` | 大きな整数と配列を含む JSON |
| `multiple-text.png` | 同じ Keyword を持つ 3 種類のテキストと JSON |
| `bad-crc.png` | tEXt の CRC が不一致 |
| `truncated.png` | IEND の CRC の途中で切断 |
