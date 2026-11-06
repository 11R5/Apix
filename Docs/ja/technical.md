<!-- translated from en/technical.md sha256:2fedbe12e4a6ee20 -->

# 技術ノート

Apix がどのようにビルドされ、何に依存し、どう計測するか。
[アーキテクチャ](architecture.md)は設計を説明します。このページは実務的な面を扱います。

## リポジトリ構成

```
crates/
├── apix-core       リクエストモデル、変数、ディスク上の形式、プロジェクト、署名（sign.rs）、GraphQL スキーマ、git（git.rs、diff.rs）
├── apix-http       HTTP エンジン（reqwest）、ネットワークトレース、OAuth 2.0 トークン、SSE、WebSocket、Socket.IO、GraphQL スキーマ取得
├── apix-import     Postman、Insomnia、OpenAPI、HAR、WSDL のインポート
├── apix-codegen    API ドキュメント（HTML、Markdown）、「形式を指定してコピー」（curl、curl.exe、PowerShell、10 言語のコード）と curl の貼り付け
├── apix-vault      暗号化コレクション
├── apix-script     プリリクエストスクリプトとテストスクリプト（QuickJS）
├── apix-runner     コレクション実行、データファイル、JUnit/JSON/HTML/Markdown レポート
├── apix-grpc       gRPC 呼び出し：.proto ファイル、サーバーリフレクションまたはコードと JSON からの定義、JSON としてのメッセージ
├── apix-proto      その他のプロトコル：MQTT
├── apix-mock       保存済みの例から応答するローカルモックサーバー
├── apix-capture    通過するリクエストを保存する記録用プロキシ
├── apix-route      サーバーまでの経路と各ホップの場所
├── apix-plugins    WebAssembly プラグイン（wasmi）：読み込み、サンドボックス、認証・インポート・ビューのフック
├── apix-plugin-sdk Rust でプラグインを書くためのもの。その examples/ が plugins/ のプラグイン
├── apix-cli        `apix` コマンド
└── apix-desktop    デスクトップアプリ（GPUI）。インターフェースのテキストは locales/ に
tools/
├── icongen         Logo/ からアプリアイコンを作る
└── docs-check      古くなったドキュメント翻訳を一覧にする
vendor/
└── gpui-pre-windows  右から左へのテキストを修正した gpui の Windows バックエンド
scripts/
├── package-*.sh/.ps1  リリースパッケージ：Windows インストーラーと zip、macOS .dmg、.deb、tarball、Arch パッケージ
└── measure.ps1     README 用の性能計測
examples/
├── jsonplaceholder リポジトリから起動したときにアプリが開く小さなコレクション
└── ci/             GitHub Actions（定期実行のモニターも）、GitLab CI、Azure Pipelines の例
plugins/            サンプルプラグイン（.wasm）。apix-plugin-sdk の examples からビルド
site/               ダウンロードページ（GitHub Pages）。最新リリースを GitHub から読む
action.yml          GitHub Action（`uses: 11R5/Apix@…`）
Dockerfile          コンテナイメージとしての `apix` コマンドライン
.github/workflows/  3 つのシステムでの CI と、バージョンタグでのイメージ公開
Docs/
├── languages.toml  ドキュメントの言語
├── en/ tr/         ユーザーガイド、アーキテクチャ、技術ノート、ワークフロー
└── IS-PLANI.md, ROADMAP.md   計画（トルコ語の詳細版、英語の要約版）
```

## ビルド

必要なもの：[Rust](https://rustup.rs) stable（リポジトリは 1.99 でテストしています）。

- **Windows：** MSVC ツールチェーン（"Desktop development with C++" と Windows 10/11 SDK を
  含む Visual Studio または Build Tools）。
- **macOS：** Xcode コマンドラインツール。
- **Linux：** GPUI が必要とする X11/Wayland、Vulkan、fontconfig の開発パッケージ
  （gpui-kit の README にディストリビューションごとに載っています）。まだテストしていません。

```sh
cargo test                       # エンジン、CLI、インポート、コード生成、vault、ドキュメントチェック
cargo run -p apix-desktop        # デスクトップアプリ（デバッグ）
cargo test -p apix-desktop       # デスクトップの単体テスト（翻訳、パスなど）
cargo build --release            # 最適化した CLI
cargo build --release -p apix-desktop
```

`-p` を付けない `cargo build` と `cargo test` は、GPU スタック全体を取り込むデスクトップアプリを
対象外にします。デスクトップアプリは明示的にビルドしてください。開発中も UI が滑らかに動くよう、
依存クレートはデバッグビルドでも最適化されます。

`plugins/` のサンプルプラグインには、一度だけ WebAssembly ターゲットが必要です
（`rustup target add wasm32-unknown-unknown`）。その後
`cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown` を実行し、
`target/wasm32-unknown-unknown/release/examples/*.wasm` を `plugins/` にコピーします
（名前の `_` は `-` にします）。`apix-plugins` のテストはこれらのファイルを読み込みます。
`wasmi` はデバッグビルドでも最適化され、デバッグアサーションなしでビルドされます
（`[profile.dev.package.wasmi]`）。そのインタプリタのループは末尾呼び出しに頼っており、
そうしないとスタックがあふれるためです。

リリースプロファイルは fat LTO、コード生成ユニット 1 つ、シンボル削除を使います。
デスクトップアプリのリリースビルドを最初から行うと数分かかります。

## 主な依存関係

| 分野 | クレート |
|---|---|
| UI | `gpui-kit` 0.7（GPUI とそのコンポーネントライブラリ）、JSON と GraphQL の tree-sitter 文法 |
| HTTP | rustls 付きの `reqwest` 0.12、`tokio`、`hyper-util`、タイミング計測用の `tower` レイヤー |
| gRPC | `tonic` 0.14（クライアント、同じ hyper と rustls の上で動作）、`prost-reflect`（実行時に分かるメッセージ、JSON）、`protox`（`protoc` なしで .proto ファイルを読む）、`regex`（生成コードと型を読む）、`tonic-reflection`（リフレクションクライアント）、証明書チェックを切った gRPC 用の `rustls` と `tokio-rustls`。`rcgen` はテスト用証明書を作ります（テストのみ） |
| ネットワークの詳細 | `x509-parser`（証明書）、`sha2`（フィンガープリント）、`netdev`（インターフェース、ゲートウェイ、DNS サーバー）。TLS の詳細は rustls 自身のフック経由（追加クレートなし）。`flate2` と `brotli-decompressor` が圧縮されたボディを展開（reqwest 用にすでにビルド済み）。経路はシステムの `tracert`、`traceroute`、`tracepath` から、場所は `maxminddb` から |
| 署名 | `sha2`、`sha1`、`hmac`、`md-5`、`base64`、`rand_core`（AWS SigV4、OAuth 1.0、Digest、PKCE） |
| 暗号化 | `chacha20poly1305`（XChaCha20-Poly1305）、`argon2`、`zeroize`、`keyring`（OS の資格情報ストア） |
| スクリプト | `rquickjs`（QuickJS。システムのコンパイラで C からビルド）。`require` のライブラリは Apix 独自の JavaScript で、ハッシュは `sha1`、`sha2`、`md-5`、`hmac` 経由 |
| ファイル | `serde`、`toml`、`serde_json`、`notify`（ファイル監視）、`trash`。git はインストール済みの `git` プログラム経由（ライブラリなし） |
| レスポンス | フィルター用の `serde_json_path`（JSONPath、RFC 9535）と `jaq-core`、`jaq-std`、`jaq-json`（jq）。大きなボディの検索に `regex` |
| プラグイン | `wasmi` 2（燃料とメモリの上限を持つ WebAssembly インタプリタ） |
| 言語 | `rust-i18n`（gpui-kit と共有）、`sys-locale` |

## Apix がコレクションの外に書くファイル

| ファイル | 内容 |
|---|---|
| `<config>/Apix/settings.toml` | 言語、外観、ダークテーマとライトテーマ、アクセントカラー、透明度、自動ロックの時間、履歴のオン・オフ、環境のシークレットの保存先、アプリを開いている間にモニターを実行するかどうか |
| `<config>/Apix/capture-ca.pem`、`capture-ca.key` | キャプチャプロキシのローカル認証局。初めて HTTPS を開いたときに作られます。自分で追加した場合にだけ信頼されます |
| `<config>/Apix/geo/*.mmdb` | 経路の地図用の IP 位置データベース（**経路と地図**ウィンドウからダウンロードするか、そこに置きます） |
| `<config>/Claude/claude_desktop_config.json` | **Claude に接続**だけが書き込みます：`mcpServers` の下の `apix-<collection>` エントリ（元のファイルは `.json.bak` として残します） |
| `<config>/Apix/monitors/` | 各モニターの直近 500 件の結果。1 回の実行につき JSON 1 行（`<hash of the collection folder>-<monitor name>.jsonl`）。`apix monitor` とアプリが書き込みます |
| `<config>/Apix/state.toml` | 最近開いたコレクション、選んだ環境、コレクションまたはプロジェクトごとの開いているタブ（暗号化コレクションは除く） |
| `<config>/Apix/cookies.json` | Cookie ジャー（セッション Cookie は保存しません） |
| `<config>/Apix/graphql/*.json` | 各エンドポイントから最後に取得した GraphQL スキーマ（サーバーのイントロスペクションの応答）。名前は URL のハッシュ |
| `<config>/Apix/themes/*.toml` | 自分のテーマ（**設定**で **フォルダーを開く** を押し、フォルダーが空のときにだけ書き込みます：例が 1 つ） |
| OS の資格情報ストア、サービス `Apix` | 求められたときだけ：暗号化コレクションの記憶したパスワード（`vault:<folder>`）、環境のシークレット（`env:<id>`、長い値は `env:<id>#1` なども）、AI アシストのキー（`ai:anthropic-api-key`、`ai:openai-api-key`、`ai:google-api-key`） |
| `<config>/Apix/plugins/*.wasm` | インストールしたプラグイン（**プラグイン**ウィンドウ、`apix plugins install`） |
| `<config>/Apix/history/` | 送信したリクエスト（`{{variables}}` は未解決のまま）とそのレスポンス：`index.jsonl`、エントリごとに `<id>.json` と `<id>.body` が 1 つずつ。直近 300 件、ボディは 256 KB で切り詰め。gRPC 呼び出しはログをボディとして保存します |

`<config>` は Windows では `%APPDATA%`、macOS では `~/Library/Application Support`、
Linux では `~/.config` です。OAuth 2.0 トークンと暗号化コレクションのパスワードは
ディスクに書き込まれません。

## 環境変数

| 変数 | 効果 |
|---|---|
| `APIX_PASSWORD` | CLI 用の暗号化コレクションのパスワード（`--password-env` で別の変数を指定できます）。 |
| `APIX_VAR_<NAME>` | CLI 用の変数：`APIX_VAR_TOKEN` は `{{token}}` を埋めます（大文字小文字に関係なくコレクションの名前と照合）。環境より優先され、`--var` がこれより優先されます。 |
| `APIX_THEME` | テーマ ID（`obsidian`、`cyber`、`dracula`、`nord`、`solarized-dark`、`daylight`、`solarized-light`、またはテーマファイルの名前）。設定に関係なくそのテーマで表示します。 |
| `APIX_BENCH` | `startup` または `json:<url>`：計測して終了します（下記参照）。 |
| `APIX_BENCH_OUT` | 計測結果を追記するファイル。 |
| `APIX_BENCH_PHASES` | `APIX_BENCH=startup` と一緒に使います：起動の各ステップに到達した時刻も報告し（`phase_…` 行）、時間がどこにかかっているかを確認できます。 |
| `APIX_UPDATE_URL` | 更新確認の問い合わせ先。GitHub の最新リリースの代わりに使います（フォーク、ミラー、テストサーバー）。 |
| `OPENAI_API_KEY`、`GEMINI_API_KEY`（または `GOOGLE_API_KEY`）、`ANTHROPIC_API_KEY` | そのプロバイダーのキーがキーチェーンにないときの AI アシストのキー。 |
| `APIX_CLAUDE_CONFIG` | **Claude に接続**が書き込む Claude Desktop の設定ファイル。本物の代わりに使います（テスト用）。 |
| `APIX_AI_URL` | AI アシストが Messages API リクエストを送る先。Anthropic の代わりに使います（プロキシ、テストサーバー）。 |
| `APIX_GEO_URL` | **DB-IP Lite をダウンロード**のダウンロード元。DB-IP の代わりに使います。 |
| `APIX_PLUGINS_DIR` | `<config>/Apix/plugins` の代わりに使うプラグインフォルダー。 |
| `GITHUB_BASE_REF` | プルリクエストで GitHub Actions が設定します：値なしの `apix run --changed` は `origin/<it>` と比較します。 |

## 性能の計測

`scripts/measure.ps1` はリリース版のアプリをビルドし、次を計測します。

- **リリースバイナリ**のサイズ。
- **起動から最初のフレームまで：** `main` から最初のフレームが描画されるまで
  （`APIX_BENCH=startup`）。数回実行した中央値です。
- **アイドル時のメモリ：** サンプルコレクションを開いて起動してから 5 秒後のワーキングセット。
- **画面上の大きな JSON：** ローカルの `big_json_server` サンプルから 100 MB の JSON ボディを
  GET し、送信してからレスポンスが描画されるまで（`APIX_BENCH=json:<url>`）。
  そのためネットワークはこの数値に含まれません。

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7、macOS/Linux でも
```

スクリプトは実行したマシンの情報とともに Markdown の表を出力します。README には最新のものが載っています。
タイミングは `crates/apix-desktop/src/bench.rs` から取られ、`APIX_BENCH` が設定されていなければ
何もしません。

その他の補助ツール：

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # :8765 で 100 MB の JSON
cargo run -p apix-http --example oauth_server                       # :8766 で OAuth 2.0 テストサーバー
cargo run -p apix-http --example sse_server                         # :8767 で server-sent events（Last-Event-ID の後から再開）
cargo run -p apix-http --example ws_server                          # :8768 で WebSocket エコー（バイナリも）とティッカー
cargo run -p apix-http --example graphql_server                     # :8769 で GraphQL エンドポイント（スキーマと数人のユーザー）
cargo run -p apix-http --example socketio_server                    # :8771 で Socket.IO 4：名前空間 /、/chat、/ticker、/private（認証）
cargo run -p apix-grpc --example grpc_server                        # :8770 でリフレクション付きの gRPC Greeter
cargo run -p apix-http --example soap_server                        # :8773 で SOAP 電卓（WSDL は ?wsdl、フォールトは 500 と 200）
cargo run -p apix-proto --example mqtt_broker                       # :8772 で MQTT ブローカー（`clock` が毎秒更新）
```

## インターフェースの言語

インターフェースのテキストは `crates/apix-desktop/locales/<code>.yml` にあり、1 行に 1 つの
フラットな `key: value` で書かれ、rust-i18n が `tr!` マクロを通して読み込みます。
ある言語にキーが足りないとき、または英語と異なる `%{placeholders}` を使っているとき、
単体テストが失敗します。言語を追加するには、`en.yml` をコピーして翻訳し、
`src/i18n.rs` の `LANGUAGES` に 1 行追加します。

## ドキュメントの言語

`Docs/languages.toml` にドキュメントの言語が並んでいます。翻訳の先頭には、
元にした英語の本文を示すスタンプがあります。

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check                      # 欠けている翻訳や古くなった翻訳、壊れたリンクを一覧にする
cargo run -p docs-check -- stamps            # 英語の各ドキュメントの現在のスタンプを出力する
cargo run -p docs-check -- site site/docs    # ドキュメントを Web ページとして書き出す（GitHub Pages）
```

## 同梱した gpui バックエンド

GPUI の Windows 用テキストレンダラー（クレート `gpui-pre-windows` 0.3.9）はグリフの並びを
すべて左から右に配置していたため、アラビア語の単語が反転して表示されていました。
`vendor/gpui-pre-windows` は `DrawGlyphRun` を修正したそのクレートで（変更箇所には
"Apix patch" の印があります）、ワークスペースの `Cargo.toml` がこれでパッチを当てます。
上流のクレートが修正されれば削除できます。

## 継続的インテグレーション

`.github/workflows/ci.yml` はすべての push とプルリクエストで実行されます。

| ジョブ | チェック内容 |
|---|---|
| Format and docs | `cargo fmt --check`、`docs-check`（翻訳が英語のドキュメントに追従しているか） |
| Engine and CLI | デスクトップアプリ以外すべてに対する `cargo clippy -D warnings` と `cargo test`。Windows、macOS、Linux で |
| Desktop app | `apix-desktop` に対して同じことを 3 つのシステムで（Linux では先にウィンドウ、フォント、キーチェーンのライブラリをインストール） |
| GitHub Action | リポジトリ自身のアクション（`uses: ./`）を `examples/jsonplaceholder` に対して Linux と Windows で実行し、レポートが書き出されたかを確認 |

`.github/workflows/release.yml` はバージョンタグ（`v1.2.3`。`Cargo.toml` と一致する必要があります）で
リリースパッケージをビルドし、`SHA256SUMS.txt` 付きの GitHub リリースとして公開します。
手動で起動した場合はビルドだけを行い、実行のアーティファクトとして残します。Windows：
`scripts/package-windows.ps1`（Inno Setup 6、`packaging/windows/apix.iss`：ユーザー単位またはマシン単位の
インストール、PATH と `.apix` の関連付けは任意、アンインストーラー）。macOS：`scripts/package-macos.sh`
が両方のアーキテクチャをビルドし、`lipo` で `Apix.app`（アドホック署名）にまとめ、`.dmg` を作ります。
Linux（Ubuntu 22.04 上でビルドするので glibc 2.35 で足ります）：`scripts/package-linux.sh` が
`install.sh` 付きの tarball、`.deb`（依存関係は `dpkg-shlibdeps` から、加えて `libvulkan1`）、
記入済みの `PKGBUILD` を作ります。`scripts/package-arch.sh` はそれらから `archlinux` コンテナ内で
Arch パッケージをビルドします。`.deb` と Arch パッケージはワークフロー内でインストールされ、
足りないライブラリがないか確認されます。`.github/workflows/pages.yml` は `site/` を
https://11r5.github.io/Apix/ に公開します。
各リリースには、記入済みの Homebrew cask（`apix.rb`、`packaging/homebrew` から）と
winget マニフェスト（`AhmetDemir.Apix*.yaml`、`packaging/winget` から）も付きます。
これらは `scripts/channel-manifests.sh` が作り、tap リポジトリ
（[11R5/homebrew-apix](https://github.com/11R5/homebrew-apix)）へのコピーと
`microsoft/winget-pkgs` へのプルリクエストに使います。`scripts/action-repo.sh` は
[11R5/apix-action](https://github.com/11R5/apix-action) のファイルを書き出します。これは
常にリリースをダウンロードする `action.yml` の Marketplace 用コピーです（Marketplace は
ワークフローファイルを含むリポジトリを掲載しないため）。リリースのたびに `v<version>` タグが付き、
`v1` が新しいコミットに移ります。
`.github/workflows/docker.yml` はバージョンタグでコンテナイメージを公開します。
アクションは自分でビルドした `apix` をエンジンのソースのハッシュでキャッシュするので、
新しいコミットでは再ビルドし、変更のないコミットでは再利用します。

## テスト

| クレート | 対象 |
|---|---|
| apix-core | モデルとファイル形式の往復変換、変数、キーチェーン内の環境のシークレット（資格情報ストアがなければスキップ）、AWS テストスイートに対する署名、RFC 7616（Digest）、RFC 7636（PKCE）、OAuth 1.0 の例。GraphQL スキーマ（イントロスペクションの読み込み、SDL）と、クエリ内の各位置で候補に出るものと説明されるもの（フィールド、エイリアス、引数、列挙値、入力オブジェクト、リスト、フラグメント、変数の型、コメントと文字列）。`[grpc]` テーブル。gRPC 呼び出しを含む履歴エントリ。GraphQL の変数宣言と変数（JSON）での補完。信頼する CA の設定。JSONPath と jq による JSON のフィルター（一致、順序どおりの出力、言葉で示すエラー、終わらないフィルターの停止）。URL のクエリとその行の双方向の変換、`:name` パス変数の検出と埋め込み（パス内だけ、エンコード済み、2 回目の埋め込みでは何も変わらない）、リクエストの URL の整理。ヘッダーの名前と値の候補（前方一致を部分一致より先に、完全に入力したら候補なし）。例（テキストとバイトの往復変換、書いたとおりのリクエスト、Set-Cookie なし）。実際のリポジトリに対する git（分岐点以降の変更、未コミット、コミット間、あるコミットでのファイル、不明な ref）、変更されたファイルが影響するもの（スクリプト、フォルダー、環境、プロジェクト、リクエストが参照するファイル）、リクエストの差分（名前ごとの行、書式の違いは無視、ボディは行ごと、移動）。モニターの間隔と `[[monitor]]` の往復変換。AI アシストのリクエスト（資格情報をマスク、ボディを切り詰め、言語、フォールバックは既定のモデルのときだけ）と回答（テキストブロック、拒否、エラーの種類、コードブロック内のスクリプト）。レスポンスの比較（ヘッダーは名前ごとで変わりやすいものは除外、JSON はフィールドごとでレコードは id で対応付け、書式の違いは無視、テキストとバイナリのボディ）。テーブル用に JSON から見つけるレコード（エンベロープ、単純な値、数値と見出しの列、揃えたテキスト） |
| apix-http | リクエストの組み立て、すべてのボディの種類、OAuth 2.0・Digest・AWS・OAuth 1.0 用のローカルサーバー、ネットワークトレース。SSE パーサー（どこで分割されても可）、ゆっくり送るサーバーに対するストリーミング、WebSocket（エコー、ハンドシェイクでの認証とパラメーター、どちら側からの切断、拒否されたハンドシェイク）、リクエストの認証による GraphQL スキーマの取得と認証がないときのエラー、独自の認証局による HTTPS。WebSocket のサブプロトコルとバイナリメッセージ、独自の CA またはチェックなしの `wss://`。イベントストリームが再接続に引き継ぐもの。Socket.IO のパケット、ハンドシェイクのアドレスとバイナリ添付、ローカルの Socket.IO 4 サーバーに対して：イベント、双方向の ack、ping、認証と拒否のある名前空間、バイナリ添付、サーバーによるクライアントの退出、普通の WebSocket サーバーとの区別。一定間隔の WebSocket ping と pong の計時、応答のない ping による接続の終了、ファイル内の `ping_interval`。ローカルの HTTPS サーバーに対するリクエスト自身のハンドシェイクの TLS の詳細（バージョン、暗号スイート、グループ、送られてきた証明書チェーンと鍵とフィンガープリント、TCP/TLS の分割と一致するハンドシェイク時間）。gzip、zlib、raw deflate、brotli のボディを分割して展開、途中で切れたボディは拒否、リクエストが指定しない限り `Accept-Encoding` を要求、`Content-Encoding` は保持 |
| apix-grpc | アドレス、ステータスコード、メタデータとしてのヘッダーと認証。ローカルの Greeter（動的メッセージで構築、リフレクションありとなし）に対して：サービスの一覧と説明、4 種類すべてのメソッド、サーバーに届く認証、失敗ステータス、インポートを含む .proto ファイル、そして説明がない・不明なメソッド・合わないメッセージ・単項メソッドへのリスト・ファイルがない・サーバーがないときのエラー。メッセージの例。実行用の HTTP レスポンスとしての呼び出し（ステータスの対応付け、`grpc-status`、リストとしてのストリーム）。独自の認証局の証明書を持つサーバーに対する TLS（拒否、CA で信頼、チェックなしで受け入れ）。定義：`tests/fixtures/generated` に置いた実際のジェネレーターの出力（Python、Java、C#、C++、PHP 用の protoc 3.19、protoc-gen-es、ts-proto）から元のサービスを復元できること、Go、Rust、betterproto、アノテーションから番号と型が得られること、10 言語から読んだ同じ型、JSON の例、サーバーとの比較、推測よりリフレクションを優先、不明なフィールドの報告、誤った応答の型の説明 |
| apix-proto | MQTT のアドレス、ログイン、オプション。ローカルのブローカーに対して：すべての QoS、ワイルドカード、接続中の購読と購読解除、拒否された購読と不正なトピック、保持メッセージ、ログインと保持されるセッション、ブローカーからの切断、独自の CA またはチェックなしの `mqtts://` |
| apix-codegen | API ドキュメント（フォルダー、マスクした資格情報、`.local.toml` の値なし、生の HTML を含まない Markdown の説明、アンカー）。bash、cmd、PowerShell 用のシェルのクォート。curl の貼り付け。往復変換。各言語の URL、ヘッダー、ボディ、文字列のエスケープ、設定 |
| apix-import | Postman v2.0/v2.1 のフィクスチャ。WSDL：両方の SOAP バージョン、ポートとアドレス、document と rpc のスタイル、ヘッダー、スキーマからの例（extension、列挙、choice、`form`、再帰）、インポートと読めなかったもの、location、WSDL 2.0 は拒否 |
| apix-script | プリリクエストスクリプトからの変数とリクエストの変更、`pm.test` / `pm.expect` の結果とメッセージ、古い `tests[...]` API、ファイル名を示すエラー、終わらないスクリプトの停止、`setNextRequest`、`atob`/`btoa`、`pm.sendRequest`（コールバックと `await`、待ち時間は計上しない）、既知のダイジェストに対する組み込みライブラリ、コードなしのチェック、レスポンスの対象の値と式 |
| apix-runner | CSV（引用符、改行、`;`、BOM）と JSON のデータファイル、dotenv ファイルと `APIX_VAR_*` の名前、反復回数とデータ行、一時停止・停止のタイミング、JUnit/JSON/HTML/Markdown の出力とエスケープ（プルリクエスト用の Markdown：マーカー、エスケープ、失敗したもの、`--changed` による選択）、日付。ローカルサーバーに対する実行全体（データ行、サイドバーの順序、`bail`、`setNextRequest`）、遅いサーバーに対する並行した反復（順番に実行するより速く、反復ごとに一覧）。モニター：状態が変わるとき、直近の結果に限って保持し、コレクションごとに分けたログ、ログに従うスケジュール、Webhook のメッセージ |
| apix-runner（負荷テスト） | 最近順位法によるパーセンタイル、期間の解析。ローカルの keep-alive サーバーに対して：維持した接続での一定数のリクエスト（400 リクエストでワーカーあたり約 1 接続）、一定のレート、時間制限、外部からの停止、500 と拒否された接続を種類ごとに集計 |
| apix-runner（フロー） | フローファイルの読み書き、開始ノード、エッジの置き換えと削除、問題の指摘、レスポンスの対象（ステータス、時間、ヘッダー、JSONPath、jq、何もない場合）、比較。ローカルサーバーに対して：あるリクエストが保持した値を次のリクエストの URL で使う、チェックと待機を含むポーリングループ、失敗エッジありとなしでの失敗するリクエスト、存在しない値、不明なリクエストファイル、1000 ステップで止まる終わらないループ、実行できないフロー |
| apix-cli（MCP） | プロトコル（バージョン、通知、不明なメソッド、`--list-only` ありとなしのツール一覧）。ローカルサーバーに対して：リクエストと環境の一覧（名前のみ、値なし）、環境とエージェントの変数を使った送信、結果に含まれるスクリプトのテスト、すべてのリクエストの実行、コレクション外のパスと存在しないファイルの拒否 |
| apix-mock | パスのパターン（`{{base_url}}/users/:id`、ホスト、クエリ）、最も文字どおりの一致、`x-mock-response-name` と `-code` による選択、2xx を優先、`Content-Encoding` なしの展開済みボディ、404 の一覧、プリフライト。ポート上の実際のサーバーが HTTP クライアント経由で応答、CORS 付き、実行中に追加された例も返す |
| apix-capture | 保存したリクエストには Cookie も資格情報も残さない、ホストのフィルター。実際のローカルサーバーを通して：普通の HTTP の記録と保存、ローカル認証局で開いた HTTPS（クライアントはそれだけを信頼）、中身を読まずに渡す HTTPS トンネル |
| apix-route | tracert（翻訳されたものも）、traceroute、tracepath の行。ローカルアドレス。地図の投影と距離。MaxMind DB 形式で書いたデータベース：場所、知らないアドレス、プライベートアドレス、ファイルの選択。ダウンロードのインストール（gzip、データベースでなければ拒否、古いものは成功した後にだけ置き換え）。システム自身のツールによる 127.0.0.1 のトレース |
| apix-plugins | マニフェスト付きで読み込んだサンプルプラグイン。独立に計算したものと比べた HMAC 署名。CSV のインポートとフィールドの統計。テキストで書いたモジュール：終わらないループの停止、インポートの拒否、不正な名前、マニフェストがない、メモリ外の応答、WebAssembly でないファイル。さらに `[auth.plugin]` の往復変換（apix-core）と、インポーターの応答をリクエストファイルにすること（apix-import） |
| apix-vault | 暗号化形式、改ざん、パスワード変更、回復キー、キーチェーンのエントリ（テスト用のサービス名で実際の資格情報ストアに対して。ない環境ではスキップ） |
| apix-desktop | 翻訳がそろっていること、言語のヘルパー、ファイルパス、ハイライトのクエリ、テーマファイル（読み込み、エラー、往復変換）、アクセントカラーと透明度の色、GraphQL スキーマのファイル名、履歴内の gRPC 呼び出しのログ |
| docs-check | ドキュメントの構造とスタンプ |
