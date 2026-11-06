# PNG Structure Inspector

PNG の内部構造とテキストメタデータを調べる読み取り専用のデスクトップアプリです。透過プレビュー、ファイル順のチャンク一覧、選択チャンクの詳細を表示します。

## 使い方

1. **PNG を開く** でファイルを選ぶか、PNG を 1 個ウィンドウにドロップします。
2. 一覧でチャンクを選びます。詳細にはファイル先頭からのオフセット、保存 CRC と計算 CRC、チャンク固有の値が表示されます。
3. **テキストメタデータ** では、同一 Keyword の項目もファイル順に個別表示します。JSON 本文は **整形 JSON** と **元のテキスト** を切り替えられ、コピーすると現在の表示形式の本文がクリップボードに入ります。

画像を開いた後は **←** で前の画像、**→** で次の画像へ移動できます。同じフォルダーの PNG / APNG をファイル名順にたどり、先頭と末尾で止まります。

## 対応チャンク

| 種類 | 表示内容 |
| --- | --- |
| `IHDR`, `PLTE`, `IDAT`, `IEND` | 画像ヘッダー、RGB パレット、画像データのサイズと CRC、終端 |
| `tEXt`, `zTXt`, `iTXt` | Latin-1 または UTF-8 のテキスト、Keyword、iTXt の言語タグと翻訳 Keyword、圧縮の有無 |
| `pHYs`, `gAMA`, `sRGB`, `cHRM`, `tIME` | ピクセル密度と参考 DPI、ガンマ、レンダリングインテント、色度、UTC 更新日時 |
| `iCCP`, `eXIf` | ICC プロファイルのヘッダー概要、Exif の TIFF バイト順と最初の IFD の概要 |
| その他 | 名称、サイズ、オフセット、CRC、Critical / Ancillary、Public / Private、Safe-to-copy。未知の Critical チャンクには警告を表示 |

チャンク構造とテキストの解釈は [PNG Specification, Third Edition](https://www.w3.org/TR/png-3/) に基づきます。

## 開発環境と起動

主対象は **Windows 11 x64** です。Windows 11 25H2 で検証しました。macOS と Linux も考慮した構成ですが、動作検証は Windows で行っています。

- Node.js 24、pnpm 11.25.0
- Rust stable、cargo、rustfmt、Clippy（検証時は Rust 1.99.1、`x86_64-pc-windows-msvc`）
- Windows では Visual Studio の C++ デスクトップ開発ツール、Windows SDK、Microsoft Edge WebView2

OS 別の依存関係は [Tauri 2 の開発環境ガイド](https://v2.tauri.app/start/prerequisites/) を参照してください。

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

## テストとビルド

リポジトリのルートから実行します。依存関係の取得後、テストはネットワーク接続なしで実行できます。

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked
pnpm typecheck
pnpm lint
pnpm test
pnpm build
pnpm tauri build --bundles nsis
```

Windows x64 の NSIS インストーラーは `src-tauri/target/release/bundle/nsis/` に生成されます。実行ファイルのみを作る場合は `pnpm tauri build --no-bundle` を実行します。

[fixtures/](fixtures/README.md) のサンプル 8 種を再生成するには `node scripts/generate-fixtures.mjs` を実行します。デスクトップ版では `multiple-text.png` を開き、透過プレビュー、チャンク選択、日本語 iTXt、整形 JSON のコピーを確認できます。`bad-crc.png` と `truncated.png` で診断表示を確認できます。

## 構成

| 場所 | 責務 |
| --- | --- |
| `src-tauri/png-inspector/` | Tauri 非依存の Rust クレート。チャンク走査、CRC、メタデータ解釈、プレビューの上限検査 |
| `src-tauri/src/` | OS のファイル選択とドロップの受付、読み取り専用ファイルセッション、Tauri IPC、DTO の結合テスト |
| `src/` | React、strict TypeScript、Vite、Tailwind CSS。プレビュー、一覧、選択、JSON 表示、コピー、Vitest / React Testing Library |

ファイルを 64 KiB のバッファで順次走査し、各チャンクの CRC を計算します。走査結果にはチャンクの位置と概要を保持し、選択されたチャンクの詳細だけを読み直します。OS の操作から受け取ったファイルを Rust が開き、WebView には構造の DTO と上限内のプレビューだけを渡します。画面から使う Tauri の権限はドロップ通知の購読とクリップボードへの書き込みに絞っています。

シグネチャが不正なファイルはオープンエラーになります。切り詰めや不正な Length に遭遇した場合は、走査できたチャンクと診断を表示します。CRC 不一致と構造警告は個別に確認できます。詳細のデコードや画像のプレビューが失敗しても、走査結果は閲覧できます。

## 既知の制限

- 走査は 20,000 チャンクで停止し、一覧は 200 件ずつ表示します。
- テキスト、ICC、Exif の詳細入力と圧縮ストリームの展開は、それぞれ 4 MiB が上限です。
- プレビューは静止 PNG が対象です。上限はファイル 8 MiB、1,200 万画素、各辺 16,384 ピクセル、圧縮メタデータの合計展開サイズ 8 MiB です。APNG や上限を超えた画像もチャンクは閲覧できます。
- JSON の整形はネスト 120 段、出力 8 Mi UTF-16 コード単位までです。上限を超えた本文は元のテキストを表示します。
- ICC と Exif は概要表示です。構造検査は基本的な順序、終端、CRC と対応するメタデータを対象とし、画像データの表示可否は WebView のデコーダーが判定します。
