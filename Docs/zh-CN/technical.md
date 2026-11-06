<!-- translated from en/technical.md sha256:40c5e638c106f35d -->

# 技术说明

Apix 如何构建、依赖哪些库、如何测量性能。[架构](architecture.md)讲的是设计；本页讲的是实际操作。

## 仓库结构

```
crates/
├── apix-core       请求模型、变量、磁盘格式、项目、签名（sign.rs）、GraphQL 模式、git（git.rs、diff.rs）
├── apix-http       HTTP 引擎（reqwest）、网络跟踪、OAuth 2.0 令牌、SSE、WebSocket、Socket.IO、获取 GraphQL 模式
├── apix-import     导入 Postman、Insomnia、OpenAPI、HAR 和 WSDL
├── apix-codegen    API 文档（HTML、Markdown）、“复制为”（curl、curl.exe、PowerShell、10 种语言的代码）和粘贴 curl
├── apix-vault      加密集合
├── apix-script     预请求脚本和测试脚本（QuickJS）
├── apix-runner     集合运行、数据文件、JUnit/JSON/HTML/Markdown 报告
├── apix-grpc       gRPC 调用：.proto 文件、服务器反射或从代码和 JSON 得到的定义、以 JSON 表示的消息
├── apix-proto      其他协议：MQTT
├── apix-mock       用已保存的示例作答的本地模拟服务器
├── apix-capture    保存经过的请求的录制代理
├── apix-route      到服务器的路由及各跳所在位置
├── apix-plugins    WebAssembly 插件（wasmi）：加载、沙箱、认证、导入和视图钩子
├── apix-plugin-sdk 用 Rust 编写插件；其 examples/ 就是 plugins/ 中的插件
├── apix-cli        `apix` 命令
└── apix-desktop    桌面应用（GPUI）；界面文本在 locales/ 中
tools/
├── icongen         从 Logo/ 生成应用图标
└── docs-check      列出过期的文档翻译
vendor/
└── gpui-pre-windows  修复了从右到左文本的 gpui Windows 后端
scripts/
├── package-*.sh/.ps1  发布包：Windows 安装程序和 zip、macOS .dmg、.deb、tarball、Arch 包
└── measure.ps1     README 中的性能测量
examples/
├── jsonplaceholder 从仓库运行应用时打开的一个小集合
└── ci/             GitHub Actions（包括定时监视器）、GitLab CI 和 Azure Pipelines 示例
plugins/            示例插件（.wasm），由 apix-plugin-sdk 的示例构建
site/               下载页面（GitHub Pages），从 GitHub 读取最新发布
action.yml          GitHub Action（`uses: 11R5/Apix@…`）
Dockerfile          以容器镜像形式提供的 `apix` 命令行
.github/workflows/  三个系统上的 CI，以及在版本标签上发布的镜像
Docs/
├── languages.toml  文档语言
├── en/ tr/         用户指南、架构、技术说明、工作流程
└── IS-PLANI.md, ROADMAP.md   计划（土耳其语详细版，英语摘要）
```

## 构建

要求：[Rust](https://rustup.rs) stable（仓库用 1.99 测试）。

- **Windows：**MSVC 工具链（Visual Studio 或 Build Tools，需安装“使用 C++ 的桌面开发”和 Windows 10/11 SDK）。
- **macOS：**Xcode 命令行工具。
- **Linux：**GPUI 所需的 X11/Wayland、Vulkan 和 fontconfig 开发包（gpui-kit 的 README 按发行版列出）；尚未测试。

```sh
cargo test                       # 引擎、CLI、导入、代码生成、vault、文档检查
cargo run -p apix-desktop        # 桌面应用（debug）
cargo test -p apix-desktop       # 桌面单元测试（翻译、路径、……）
cargo build --release            # 优化后的 CLI
cargo build --release -p apix-desktop
```

不带 `-p` 的 `cargo build` 和 `cargo test` 会跳过桌面应用，因为它会引入整个 GPU 栈；请显式构建它。即使在 debug 构建中，依赖也会被优化，这样开发时界面依然流畅。

`plugins/` 中的示例插件需要先添加一次 WebAssembly 目标（`rustup target add wasm32-unknown-unknown`），然后运行 `cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown`；把 `target/wasm32-unknown-unknown/release/examples/*.wasm` 复制到 `plugins/`（文件名中的 `_` 换成 `-`）。`apix-plugins` 的测试会加载这些文件。`wasmi` 在 debug 构建中也会被优化，并且不带调试断言构建（`[profile.dev.package.wasmi]`）：它的解释器循环依赖尾调用，否则会导致栈溢出。

release 配置使用 fat LTO、单个 codegen 单元，并剥离符号。桌面应用的完整 release 构建需要几分钟。

## 主要依赖

| 领域 | Crate |
|---|---|
| 界面 | `gpui-kit` 0.7（GPUI 及其组件库），JSON 和 GraphQL 的 tree-sitter 语法 |
| HTTP | 使用 rustls 的 `reqwest` 0.12、`tokio`、`hyper-util`、用于计时的 `tower` 层 |
| gRPC | `tonic` 0.14（客户端，基于同一套 hyper 和 rustls）、`prost-reflect`（运行时才知道的消息、JSON）、`protox`（不用 `protoc` 读取 .proto 文件）、`regex`（读取生成的代码和类型）、`tonic-reflection`（反射客户端）、用于关闭证书检查的 gRPC 的 `rustls` 和 `tokio-rustls`；`rcgen` 生成测试证书（仅测试） |
| 网络详情 | `x509-parser`（证书）、`sha2`（指纹）、`netdev`（网卡、网关、DNS 服务器）；TLS 详情通过 rustls 自带的钩子获取（无需额外 crate）；`flate2` 和 `brotli-decompressor` 解码压缩的正文（reqwest 已经构建了它们）；路由来自系统的 `tracert`、`traceroute` 或 `tracepath`，位置来自 `maxminddb` |
| 签名 | `sha2`、`sha1`、`hmac`、`md-5`、`base64`、`rand_core`（AWS SigV4、OAuth 1.0、Digest、PKCE） |
| 加密 | `chacha20poly1305`（XChaCha20-Poly1305）、`argon2`、`zeroize`、`keyring`（操作系统凭据存储） |
| 脚本 | `rquickjs`（QuickJS，用系统编译器从 C 构建）；`require` 库是 Apix 自己的 JavaScript，哈希通过 `sha1`、`sha2`、`md-5`、`hmac` 计算 |
| 文件 | `serde`、`toml`、`serde_json`、`notify`（文件监视）、`trash`；git 通过已安装的 `git` 程序（不用库） |
| 响应 | 过滤用 `serde_json_path`（JSONPath，RFC 9535）和 `jaq-core`、`jaq-std`、`jaq-json`（jq）；`regex` 用于搜索大型正文 |
| 插件 | `wasmi` 2（带燃料和内存限制的 WebAssembly 解释器） |
| 语言 | `rust-i18n`（与 gpui-kit 共用）、`sys-locale` |

## Apix 在集合之外写入的文件

| 文件 | 内容 |
|---|---|
| `<config>/Apix/settings.toml` | 语言、外观、深色和浅色主题、强调色、透明度、自动锁定时间、是否开启历史、环境密钥存放位置、应用打开时是否运行监视器 |
| `<config>/Apix/capture-ca.pem`、`capture-ca.key` | 捕获代理的本地证书颁发机构，第一次打开 HTTPS 时创建；只有你自己添加后才会被信任 |
| `<config>/Apix/geo/*.mmdb` | 路由地图使用的 IP 位置数据库（在“路由与地图”窗口中下载，或自己放进去） |
| `<config>/Claude/claude_desktop_config.json` | 只由“连接到 Claude”写入：`mcpServers` 下的 `apix-<collection>` 条目（原文件保留为 `.json.bak`） |
| `<config>/Apix/monitors/` | 每个监视器最近 500 条结果，每次运行一行 JSON（`<hash of the collection folder>-<monitor name>.jsonl`），由 `apix monitor` 和应用写入 |
| `<config>/Apix/state.toml` | 最近打开的集合、所选环境，以及每个集合或项目打开的标签页（加密集合除外） |
| `<config>/Apix/cookies.json` | Cookie 存储（会话 Cookie 不会保存） |
| `<config>/Apix/graphql/*.json` | 从每个端点最后一次获取的 GraphQL 模式（服务器的内省应答），以 URL 的哈希命名 |
| `<config>/Apix/themes/*.toml` | 你自己的主题（只有在“设置”中按下 **打开文件夹** 且文件夹为空时才写入：一个示例） |
| 操作系统凭据存储，服务名 `Apix` | 仅在你要求时：加密集合记住的密码（`vault:<folder>`）和环境密钥（`env:<id>`，长值另有 `env:<id>#1`、……）、AI 助手密钥（`ai:anthropic-api-key`、`ai:openai-api-key`、`ai:google-api-key`） |
| `<config>/Apix/plugins/*.wasm` | 已安装的插件（“插件”窗口、`apix plugins install`） |
| `<config>/Apix/history/` | 已发送的请求（`{{variables}}` 未解析）及其响应：`index.jsonl`，每条记录一个 `<id>.json` 和 `<id>.body`；保留最近 300 条，正文截断到 256 KB；gRPC 调用把它的日志作为正文保存 |

`<config>` 在 Windows 上是 `%APPDATA%`，在 macOS 上是 `~/Library/Application Support`，在 Linux 上是 `~/.config`。OAuth 2.0 令牌和加密集合的密码从不写入磁盘。

## 环境变量

| 变量 | 作用 |
|---|---|
| `APIX_PASSWORD` | 供 CLI 使用的加密集合密码（`--password-env` 可选用其他变量）。 |
| `APIX_VAR_<NAME>` | 供 CLI 使用的变量：`APIX_VAR_TOKEN` 填充 `{{token}}`（与集合中的名称匹配时不区分大小写）；优先于环境，`--var` 又优先于它。 |
| `APIX_THEME` | 主题 id（`obsidian`、`cyber`、`dracula`、`nord`、`solarized-dark`、`daylight`、`solarized-light` 或主题文件名）；无论设置如何都显示该主题。 |
| `APIX_BENCH` | `startup` 或 `json:<url>`：测量后退出（见下文）。 |
| `APIX_BENCH_OUT` | 追加测量结果的文件。 |
| `APIX_BENCH_PHASES` | 与 `APIX_BENCH=startup` 一起使用：同时报告启动的每一步是何时到达的（`phase_…` 行），用来查看时间花在哪里。 |
| `APIX_UPDATE_URL` | 更新检查询问的地址，代替 GitHub 的最新发布（分叉、镜像、测试服务器）。 |
| `OPENAI_API_KEY`、`GEMINI_API_KEY`（或 `GOOGLE_API_KEY`）、`ANTHROPIC_API_KEY` | 当钥匙串中没有该提供方的密钥时使用的 AI 助手密钥。 |
| `APIX_CLAUDE_CONFIG` | “连接到 Claude”写入的 Claude Desktop 配置文件，代替真实的配置文件（用于测试）。 |
| `APIX_AI_URL` | AI 助手发送 Messages API 请求的地址，代替 Anthropic（代理、测试服务器）。 |
| `APIX_GEO_URL` | **下载 DB-IP Lite** 的下载地址，代替 DB-IP。 |
| `APIX_PLUGINS_DIR` | 插件文件夹，代替 `<config>/Apix/plugins`。 |
| `GITHUB_BASE_REF` | GitHub Actions 在拉取请求中设置：不带值的 `apix run --changed` 会与 `origin/<it>` 比较。 |

## 测量性能

`scripts/measure.ps1` 构建 release 版应用并测量：

- **Release 二进制文件**的大小。
- **从启动到第一帧：**从 `main` 开始到绘制出第一帧（`APIX_BENCH=startup`），取多次运行的中位数。
- **空闲时的内存：**打开示例集合启动后五秒时的工作集。
- **大型 JSON 上屏：**从本地 `big_json_server` 示例 GET 一个 100 MB 的 JSON 正文，从发送到响应绘制完成（`APIX_BENCH=json:<url>`），因此网络不计入该数字。

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7，也可在 macOS/Linux 上使用
```

脚本会输出一个 Markdown 表格，并注明运行它的机器；README 显示最新的一份。计时来自 `crates/apix-desktop/src/bench.rs`，只有设置了 `APIX_BENCH` 时它才起作用。

其他辅助工具：

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # 在 :8765 上提供 100 MB 的 JSON
cargo run -p apix-http --example oauth_server                       # 在 :8766 上的 OAuth 2.0 测试服务器
cargo run -p apix-http --example sse_server                         # 在 :8767 上的服务器发送事件（Last-Event-ID 之后续传）
cargo run -p apix-http --example ws_server                          # 在 :8768 上的 WebSocket 回显（也支持二进制）和计时推送
cargo run -p apix-http --example graphql_server                     # 在 :8769 上的 GraphQL 端点（模式和几个用户）
cargo run -p apix-http --example socketio_server                    # 在 :8771 上的 Socket.IO 4：命名空间 /、/chat、/ticker、/private（认证）
cargo run -p apix-grpc --example grpc_server                        # 在 :8770 上带反射的 gRPC Greeter
cargo run -p apix-http --example soap_server                        # 在 :8773 上的 SOAP 计算器（WSDL 在 ?wsdl；故障用 500 和 200）
cargo run -p apix-proto --example mqtt_broker                       # 在 :8772 上的 MQTT 代理（`clock` 每秒推送一次）
```

## 界面语言

界面文本在 `crates/apix-desktop/locales/<code>.yml` 中，每行一个扁平的 `key: value`，由 rust-i18n 通过 `tr!` 宏读取。当某种语言缺少键或使用了与英语不同的 `%{placeholders}` 时，单元测试会失败。要添加一种语言，复制 `en.yml`，翻译它，并在 `src/i18n.rs` 的 `LANGUAGES` 中加一行。

## 文档语言

`Docs/languages.toml` 列出文档语言。译文以一个标记开头，注明它来自哪份英文原文：

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check                      # 列出缺失或过期的翻译以及失效链接
cargo run -p docs-check -- stamps            # 打印每份英文文档当前的标记
cargo run -p docs-check -- site site/docs    # 将文档生成为网页（GitHub Pages）
```

用户指南中的图片由 `scripts/docs-tour.ps1` 截取。它会启动示例服务器，并以 `APIX_TOUR=Docs/tour/scenes.toml` 为每种语言运行一次桌面应用：应用自行准备每个场景（打开 `Docs/tour/demo` 中演示集合的请求、发送它们、打开窗口），脚本再把窗口保存为 `Docs/<语言>/images/<场景>.png`。导览期间，`APIX_CONFIG_DIR` 会把应用的设置、历史记录和 Cookie 放在单独的文件夹中，因此不会影响你自己的。

## 内置的 gpui 后端

GPUI 的 Windows 文本渲染器（crate `gpui-pre-windows` 0.3.9）把每一串字形都从左到右排列，因此阿拉伯语单词会显示成镜像。`vendor/gpui-pre-windows` 就是修复了 `DrawGlyphRun` 的该 crate（改动处标有 "Apix patch"），工作区的 `Cargo.toml` 用它打补丁。上游 crate 修复后即可移除。

## 持续集成

`.github/workflows/ci.yml` 在每次推送和拉取请求时运行：

| 任务 | 检查内容 |
|---|---|
| 格式和文档 | `cargo fmt --check`；`docs-check`（翻译跟上英文文档） |
| 引擎和 CLI | 在 Windows、macOS 和 Linux 上，对除桌面应用外的所有内容运行 `cargo clippy -D warnings` 和 `cargo test` |
| 桌面应用 | 在三个系统上对 `apix-desktop` 做同样的检查（Linux 先安装窗口、字体和钥匙串库） |
| GitHub Action | 在 Linux 和 Windows 上，用仓库自己的 action（`uses: ./`）运行 `examples/jsonplaceholder`，并检查它写出了报告 |

`.github/workflows/release.yml` 在版本标签（`v1.2.3`，必须与 `Cargo.toml` 一致）上构建发布包，并连同 `SHA256SUMS.txt` 作为 GitHub release 发布；手动启动时只构建，作为该次运行的产物。Windows：`scripts/package-windows.ps1`（Inno Setup 6，`packaging/windows/apix.iss`：按用户或按机器安装、可选的 PATH 和 `.apix` 关联、卸载程序）。macOS：`scripts/package-macos.sh` 构建两种架构，用 `lipo` 合并成 `Apix.app`（ad-hoc 签名），并制作 `.dmg`。Linux（在 Ubuntu 22.04 上，因此 glibc 2.35 即可）：`scripts/package-linux.sh` 制作带 `install.sh` 的 tarball、`.deb`（依赖来自 `dpkg-shlibdeps`，外加 `libvulkan1`）和一个已填好的 `PKGBUILD`；`scripts/package-arch.sh` 在 `archlinux` 容器中用它们构建 Arch 包。`.deb` 和 Arch 包会在工作流中安装，并检查是否缺少库。`.github/workflows/pages.yml` 把 `site/` 发布到 https://11r5.github.io/Apix/ 。
每个发布还附带一个已填好的 Homebrew cask（`apix.rb`，来自 `packaging/homebrew`）和 winget 清单（`AhmetDemir.Apix*.yaml`，来自 `packaging/winget`），由 `scripts/channel-manifests.sh` 生成，用于复制到 tap 仓库（[11R5/homebrew-apix](https://github.com/11R5/homebrew-apix)）和提交 `microsoft/winget-pkgs` 拉取请求。`scripts/action-repo.sh` 写出 [11R5/apix-action](https://github.com/11R5/apix-action) 的文件，它是 `action.yml` 在 Marketplace 上的副本，总是下载一个发布版（Marketplace 不会列出带有工作流文件的仓库）；每次发布后它会得到一个 `v<version>` 标签，`v1` 移到新的提交。
`.github/workflows/docker.yml` 在版本标签上发布容器镜像。action 把它构建的 `apix` 缓存在引擎源码的哈希之下，因此新的提交会重新构建，未改动的提交会复用。

## 测试

| Crate | 覆盖内容 |
|---|---|
| apix-core | 模型和文件格式的往返、变量、钥匙串中的环境密钥（没有凭据存储时跳过）、针对 AWS 测试套件、RFC 7616（Digest）、RFC 7636（PKCE）和 OAuth 1.0 示例的签名；GraphQL 模式（读取内省、SDL）以及查询中每个位置的建议和说明（字段、别名、参数、枚举值、输入对象、列表、片段、变量类型、注释和字符串）；`[grpc]` 表；历史条目，包括 gRPC 调用；GraphQL 变量声明和变量（JSON）中的补全；受信任 CA 设置；用 JSONPath 和 jq 过滤 JSON（匹配、按顺序输出、用文字描述的错误、停止无限过滤）；URL 的查询及其行的双向转换、找到并填充 `:name` 路径变量（只在路径中、经过编码、第二次填充不改变任何内容）、整理请求的 URL；建议的头名称和值（开头匹配优先于包含匹配，完整输入后不再建议）；示例（文本和字节的往返、按原样书写的请求、不含 Set-Cookie）；针对真实仓库的 git（自分支点以来的更改、未提交的更改、提交之间的更改、某次提交时的文件、未知的引用）、更改的文件涉及哪些内容（脚本、文件夹、环境、项目、请求引用的文件）、请求差异（按名称比较的行、忽略格式、逐行比较的正文、移动）；监视器间隔和 `[[monitor]]` 往返；AI 助手请求（凭据被遮盖、正文被截断、语言、仅默认模型才回退）和应答（文本块、拒绝、错误类型、代码块中的脚本）；比较响应（按名称比较的头并排除易变的头、逐字段比较的 JSON 并按 id 匹配记录、忽略格式、文本和二进制正文）；在 JSON 中找到的用于表格的记录（外层包装、普通值、数字列和标签列、对齐的文本） |
| apix-http | 请求构建、每种正文类型、用于 OAuth 2.0、Digest、AWS 和 OAuth 1.0 的本地服务器、网络跟踪；SSE 解析器（在任意位置分割的片段）、针对慢速发送服务器的流式传输、WebSocket（回显、握手中的认证和参数、任一方关闭、被拒绝的握手）、用请求的认证获取 GraphQL 模式以及没有认证时的错误、使用自有证书颁发机构的 HTTPS；WebSocket 子协议和二进制消息、使用自有 CA 或不检查的 `wss://`；事件流在重新连接时保留的内容；Socket.IO 数据包、握手地址和二进制附件，以及针对本地 Socket.IO 4 服务器：事件、双向确认、ping、带认证和拒绝的命名空间、二进制附件、服务器让客户端离开、区分普通 WebSocket 服务器；按间隔发送的 WebSocket ping 及其 pong 计时、未应答的 ping 结束连接、文件中的 `ping_interval`；针对本地 HTTPS 服务器的请求自身握手的 TLS 详情（版本、密码套件、组、它发送的证书链及其密钥和指纹、与 TCP/TLS 拆分一致的握手时间）；分段解码 gzip、zlib、原始 deflate 和 brotli 正文、拒绝被截断的正文、除非请求自己设置否则请求 `Accept-Encoding`、保留 `Content-Encoding` |
| apix-grpc | 地址、状态码、作为元数据的头和认证；针对本地 Greeter（由动态消息构建，带反射和不带反射）：列出和描述服务、全部四种方法、认证到达服务器、失败状态、带导入的 .proto 文件，以及没有描述、未知方法、不匹配的消息、一元方法收到列表、缺少文件和没有服务器时的错误；示例消息；运行时作为 HTTP 响应的调用（状态映射、`grpc-status`、作为列表的流）；针对使用自有颁发机构证书的服务器的 TLS（被拒绝、用 CA 信任、不检查而接受）；定义：保存在 `tests/fixtures/generated` 中的真实生成器输出（Python、Java、C#、C++、PHP 的 protoc 3.19；protoc-gen-es；ts-proto）必须还原出原始服务，Go、Rust、betterproto 和注解给出编号和类型，从 10 种语言读取同一类型，JSON 示例，与服务器比较，猜测让位于反射，报告未知字段，解释错误的应答类型 |
| apix-proto | MQTT 地址、登录和选项；针对本地代理：每种服务质量、通配符、连接期间订阅和退订、被拒绝的订阅和错误的主题、保留消息、登录和保持的会话、代理挂断、使用自有 CA 或不检查的 `mqtts://` |
| apix-codegen | API 文档（文件夹、遮盖的凭据、不含 `.local.toml` 的值、不含原始 HTML 的 Markdown 描述、锚点）；bash、cmd 和 PowerShell 的 shell 引号；粘贴 curl；往返；每种语言的 URL、头、正文、字符串转义和设置 |
| apix-import | Postman v2.0/v2.1 测试数据；WSDL：两个 SOAP 版本、端口和地址、document 和 rpc 风格、头、从模式生成的示例（扩展、枚举、选择、`form`、递归）、导入以及无法读取的内容、位置、拒绝 WSDL 2.0 |
| apix-script | 预请求脚本中的变量和请求更改、`pm.test` / `pm.expect` 的结果和消息、旧的 `tests[...]` API、指明文件的错误、停止无限脚本、`setNextRequest`、`atob`/`btoa`、`pm.sendRequest`（回调和 `await`，等待时间不计入）、针对已知摘要的内置库、无代码检查、响应目标的值和表达式 |
| apix-runner | CSV（引号、换行、`;`、BOM）和 JSON 数据文件、dotenv 文件和 `APIX_VAR_*` 名称、迭代次数和数据行、暂停/停止计时、JUnit/JSON/HTML/Markdown 输出和转义（用于拉取请求的 Markdown：标记、转义、失败的内容、`--changed` 选择）、日期；针对本地服务器的完整运行（数据行、侧边栏顺序、`bail`、`setNextRequest`），针对慢速服务器的并行迭代（比一个接一个更快、按迭代列出）；监视器：状态何时转变、日志只保留最近的结果并按集合分开、跟随日志的计划、webhook 消息 |
| apix-runner（负载测试） | 最近秩法百分位数、时长解析；针对本地 keep-alive 服务器：在保持的连接上发送一定数量的请求（400 个请求大约每个工作线程一个连接）、稳定速率、时间限制、从别处停止、按类型统计 500 和被拒绝的连接 |
| apix-runner（流程） | 流程文件的读写、起始节点、替换和删除边、指明问题、响应的取值对象（状态、时间、头、JSONPath、jq、不存在）、比较；针对本地服务器：一个请求保存的值用于下一个请求的 URL、带检查和等待的轮询循环、带和不带失败边的失败请求、不存在的值、未知的请求文件、在 1000 步时停止的无限循环、无法运行的流程 |
| apix-cli（MCP） | 协议（版本、通知、未知方法、带和不带 `--list-only` 的工具列表）；针对本地服务器：列出请求和环境（名称，不含值）、带环境和代理变量发送、结果中的脚本测试、运行所有请求、拒绝集合之外的路径和缺失的文件 |
| apix-mock | 路径模式（`{{base_url}}/users/:id`、主机、查询）、最字面的匹配、按 `x-mock-response-name` 和 `-code` 选择、2xx 优先、不带 `Content-Encoding` 的解码正文、404 列表、预检请求；在端口上运行的真实服务器通过 HTTP 客户端作答，支持 CORS，并提供运行期间新增的示例 |
| apix-capture | 保存的请求不含 Cookie 和凭据、主机过滤；通过真实的本地服务器：录制并保存普通 HTTP、用本地颁发机构打开 HTTPS（客户端只信任它）、HTTPS 隧道不读取直接转发 |
| apix-route | tracert（包括翻译过的版本）、traceroute 和 tracepath 的输出行；本地地址；地图投影和距离；以 MaxMind DB 格式写入的数据库：位置、它不知道的地址、私有地址、选择文件；安装下载的数据库（gzip、不是数据库时拒绝、之后才替换旧文件）；用系统自带的工具跟踪 127.0.0.1 |
| apix-plugins | 加载示例插件及其清单；HMAC 签名与独立计算的签名比对；CSV 导入和字段统计；以文本形式编写的模块：停止无限循环、拒绝导入、错误的名称、缺少清单、超出内存的应答、不是 WebAssembly 的文件；还有 `[auth.plugin]` 的往返（apix-core）和导入器应答生成的请求文件（apix-import） |
| apix-vault | 加密格式、篡改、修改密码、恢复密钥、钥匙串条目（使用测试服务名针对真实的凭据存储；没有凭据存储时跳过） |
| apix-desktop | 翻译完整、语言辅助函数、文件路径、高亮查询、主题文件（读取、错误、往返）、强调色和透明度颜色、GraphQL 模式文件名、历史中 gRPC 调用的日志 |
| docs-check | 文档结构和标记 |
