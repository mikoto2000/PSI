# 開発ガイド

[PNG Structure Inspector](README.md) の開発環境、テスト、ビルドの手順です。

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
| `src-tauri/src/` | OS のファイル選択とドロップの受付、読み取り専用ファイルセッション、JPEG のマーカー走査と詳細表示、Tauri IPC、結合テスト |
| `src/` | React、strict TypeScript、Vite、Tailwind CSS。プレビュー、一覧、選択、JSON 表示、コピー、Vitest / React Testing Library |

PNG は 64 KiB のバッファで順次走査し、各チャンクの CRC を計算します。JPEG はマーカーを走査し、画像データ中のバイトスタッフィングとリスタートマーカーを処理します。走査結果には各項目の位置と概要を保持し、選択された項目の詳細だけを読み直します。OS の操作から受け取ったファイルを Rust が開き、WebView には構造の DTO と上限内のプレビューだけを渡します。画面から使う Tauri の権限はドロップ通知の購読とクリップボードへの書き込みに絞っています。

## CI

GitHub Actions の [ビルドワークフロー](.github/workflows/build.yml) は、`main` へのプッシュまたは手動実行で、Windows x64（NSIS と単体の実行ファイル）、Linux x64（DEB）、macOS の arm64 と x64（DMG）の成果物を作成します。

## 参照仕様

- PNG のチャンク構造とテキストメタデータ。[PNG Specification, Third Edition](https://www.w3.org/TR/png-3/)
- JPEG のマーカー構造。[ITU-T T.81](https://www.w3.org/Graphics/JPEG/itu-t81.pdf)
