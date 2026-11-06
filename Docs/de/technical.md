<!-- translated from en/technical.md sha256:2fedbe12e4a6ee20 -->

# Technische Hinweise

Wie Apix gebaut ist, wovon es abhängt und wie man es misst. Die
[Architektur](architecture.md) erklärt den Entwurf; diese Seite ist die
praktische Seite.

## Aufbau des Repositorys

```
crates/
├── apix-core       Anfragemodell, Variablen, Dateiformat, Projekte, Signieren (sign.rs), GraphQL-Schemas, git (git.rs, diff.rs)
├── apix-http       die HTTP-Engine (reqwest), Netzwerk-Trace, OAuth-2.0-Tokens, SSE, WebSocket, Socket.IO, Abruf von GraphQL-Schemas
├── apix-import     Import aus Postman, Insomnia, OpenAPI, HAR und WSDL
├── apix-codegen    API-Dokumentation (HTML, Markdown), „Kopieren als“ (curl, curl.exe, PowerShell, Code in 10 Sprachen) und Einfügen von curl
├── apix-vault      verschlüsselte Sammlungen
├── apix-script     Pre-Request- und Testskripte (QuickJS)
├── apix-runner     Sammlungsläufe, Datendateien, JUnit/JSON/HTML/Markdown-Berichte
├── apix-grpc       gRPC-Aufrufe: .proto-Dateien, Server-Reflection oder Definitionen aus Code und JSON, Nachrichten als JSON
├── apix-proto      weitere Protokolle: MQTT
├── apix-mock       ein lokaler Mock-Server, der aus gespeicherten Beispielen antwortet
├── apix-capture    ein aufzeichnender Proxy, der durchlaufende Anfragen speichert
├── apix-route      die Route zu einem Server und wo seine Hops liegen
├── apix-plugins    WebAssembly-Plugins (wasmi): Laden, Sandbox, Auth-, Import- und Ansichts-Hooks
├── apix-plugin-sdk Plugins in Rust schreiben; seine examples/ sind die Plugins in plugins/
├── apix-cli        der Befehl `apix`
└── apix-desktop    die Desktop-App (GPUI); Oberflächentexte in locales/
tools/
├── icongen         erzeugt die App-Icons aus Logo/
└── docs-check      listet veraltete Übersetzungen der Dokumentation auf
vendor/
└── gpui-pre-windows  das Windows-Backend von gpui mit einer Korrektur für Text von rechts nach links
scripts/
├── package-*.sh/.ps1  Release-Pakete: Windows-Installer und Zip, macOS-.dmg, .deb, Tarball, Arch-Paket
└── measure.ps1     Leistungsmessungen für die README
examples/
├── jsonplaceholder eine kleine Sammlung, die die App öffnet, wenn sie aus dem Repo gestartet wird
└── ci/             Beispiele für GitHub Actions (auch geplante Monitore), GitLab CI und Azure Pipelines
plugins/            Beispiel-Plugins (.wasm), gebaut aus den examples von apix-plugin-sdk
site/               die Download-Seite (GitHub Pages), liest das neueste Release von GitHub
action.yml          die GitHub Action (`uses: 11R5/Apix@…`)
Dockerfile          die Kommandozeile `apix` als Container-Image
.github/workflows/  CI auf drei Systemen und das Image, veröffentlicht bei Versions-Tags
Docs/
├── languages.toml  Sprachen der Dokumentation
├── en/ tr/         Benutzerhandbuch, Architektur, technische Hinweise, Workflows
└── IS-PLANI.md, ROADMAP.md   der Plan (Details auf Türkisch, Zusammenfassung auf Englisch)
```

## Bauen

Voraussetzungen: [Rust](https://rustup.rs) stable (das Repo ist mit 1.99
getestet).

- **Windows:** die MSVC-Toolchain (Visual Studio oder Build Tools mit „Desktop
  development with C++“ und einem Windows-10/11-SDK).
- **macOS:** die Xcode-Kommandozeilenwerkzeuge.
- **Linux:** die Entwicklungspakete für X11/Wayland, Vulkan und fontconfig, die
  GPUI braucht (die README von gpui-kit listet sie je Distribution auf); noch
  nicht getestet.

```sh
cargo test                       # Engine, CLI, Import, Codegen, Vault, Docs-Prüfung
cargo run -p apix-desktop        # die Desktop-App (Debug)
cargo test -p apix-desktop       # Unit-Tests der Desktop-App (Übersetzungen, Pfade, …)
cargo build --release            # optimierte CLI
cargo build --release -p apix-desktop
```

`cargo build` und `cargo test` ohne `-p` lassen die Desktop-App aus, die den
ganzen GPU-Stack mitbringt; sie wird ausdrücklich gebaut. Abhängigkeiten werden
auch in Debug-Builds optimiert, damit die Oberfläche beim Entwickeln flüssig
bleibt.

Die Beispiel-Plugins in `plugins/` brauchen einmalig das WebAssembly-Target
(`rustup target add wasm32-unknown-unknown`), danach
`cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown`;
kopieren Sie `target/wasm32-unknown-unknown/release/examples/*.wasm` nach
`plugins/` (mit `-` statt `_` in den Namen). Die Tests von `apix-plugins` laden
diese Dateien. `wasmi` wird auch in Debug-Builds optimiert und ohne
Debug-Assertions gebaut (`[profile.dev.package.wasmi]`): Seine
Interpreter-Schleife setzt auf Tail Calls, die sonst den Stack überlaufen lassen.

Das Release-Profil nutzt Fat LTO, eine Codegen-Unit und entfernte Symbole. Ein
vollständiger Release-Build der Desktop-App dauert einige Minuten.

## Wichtige Abhängigkeiten

| Bereich | Crates |
|---|---|
| Oberfläche | `gpui-kit` 0.7 (GPUI und seine Komponentenbibliothek), tree-sitter-Grammatiken für JSON und GraphQL |
| HTTP | `reqwest` 0.12 mit rustls, `tokio`, `hyper-util`, `tower`-Layer für die Zeitmessung |
| gRPC | `tonic` 0.14 (Client, über dasselbe hyper und rustls), `prost-reflect` (zur Laufzeit bekannte Nachrichten, JSON), `protox` (liest .proto-Dateien ohne `protoc`), `regex` (Lesen von generiertem Code und Typen), `tonic-reflection` (der Reflection-Client), `rustls` und `tokio-rustls` für gRPC ohne Zertifikatsprüfung; `rcgen` erzeugt Testzertifikate (nur in Tests) |
| Netzwerkdetails | `x509-parser` (Zertifikate), `sha2` (Fingerabdrücke), `netdev` (Schnittstellen, Gateway, DNS-Server); TLS-Details über die eigenen Hooks von rustls (kein zusätzliches Crate); `flate2` und `brotli-decompressor` dekodieren komprimierte Bodys (für reqwest ohnehin gebaut); die Route aus `tracert`, `traceroute` oder `tracepath` des Systems, Orte aus `maxminddb` |
| Signieren | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Verschlüsselung | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize`, `keyring` (Anmeldedatenspeicher des Betriebssystems) |
| Skripte | `rquickjs` (QuickJS, aus C mit dem Compiler des Systems gebaut); `require`-Bibliotheken sind eigenes JavaScript von Apix, Hashing über `sha1`, `sha2`, `md-5`, `hmac` |
| Dateien | `serde`, `toml`, `serde_json`, `notify` (Dateiüberwachung), `trash`; git über das installierte Programm `git` (keine Bibliothek) |
| Antworten | `serde_json_path` (JSONPath, RFC 9535) und `jaq-core`, `jaq-std`, `jaq-json` (jq) für den Filter; `regex` für die Suche in großen Bodys |
| Plugins | `wasmi` 2 (ein WebAssembly-Interpreter mit Fuel- und Speichergrenzen) |
| Sprachen | `rust-i18n` (gemeinsam mit gpui-kit), `sys-locale` |

## Dateien, die Apix außerhalb von Sammlungen schreibt

| Datei | Inhalt |
|---|---|
| `<config>/Apix/settings.toml` | Sprache, Erscheinungsbild, dunkles und helles Theme, Akzentfarbe, Transparenz, Zeit bis zur automatischen Sperre, Verlauf an oder aus, wohin Umgebungsgeheimnisse kommen, ob Monitore laufen, während die App offen ist |
| `<config>/Apix/capture-ca.pem`, `capture-ca.key` | die lokale Zertifizierungsstelle des Capture-Proxys, erzeugt beim ersten Öffnen von HTTPS; nur vertrauenswürdig, wenn Sie sie selbst hinzufügen |
| `<config>/Apix/geo/*.mmdb` | die IP-Standortdatenbank für die Routenkarte (heruntergeladen aus dem Fenster Route und Karte oder dort abgelegt) |
| `<config>/Claude/claude_desktop_config.json` | nur von Mit Claude verbinden geschrieben: der Eintrag `apix-<collection>` unter `mcpServers` (das Original bleibt als `.json.bak` erhalten) |
| `<config>/Apix/monitors/` | die letzten 500 Ergebnisse jedes Monitors, eine JSON-Zeile pro Lauf (`<hash of the collection folder>-<monitor name>.jsonl`), geschrieben von `apix monitor` und der App |
| `<config>/Apix/state.toml` | zuletzt geöffnete Sammlungen, die gewählte Umgebung und die offenen Tabs je Sammlung oder Projekt (nicht für verschlüsselte Sammlungen) |
| `<config>/Apix/cookies.json` | der Cookie-Speicher (Session-Cookies werden nicht gespeichert) |
| `<config>/Apix/graphql/*.json` | das zuletzt von jedem Endpunkt abgerufene GraphQL-Schema (die Introspection-Antwort des Servers), benannt nach einem Hash der URL |
| `<config>/Apix/themes/*.toml` | Ihre eigenen Themes (nur geschrieben, wenn Sie in den Einstellungen **Ordner öffnen** drücken und der Ordner leer ist: ein Beispiel) |
| Anmeldedatenspeicher des Betriebssystems, Dienst `Apix` | nur auf Wunsch: gemerkte Passwörter verschlüsselter Sammlungen (`vault:<folder>`) und Umgebungsgeheimnisse (`env:<id>`, dazu `env:<id>#1`, … für lange Werte), Schlüssel für den KI-Assistenten (`ai:anthropic-api-key`, `ai:openai-api-key`, `ai:google-api-key`) |
| `<config>/Apix/plugins/*.wasm` | installierte Plugins (das Fenster Plugins, `apix plugins install`) |
| `<config>/Apix/history/` | gesendete Anfragen (`{{variables}}` nicht aufgelöst) und ihre Antworten: `index.jsonl`, je Eintrag eine `<id>.json` und eine `<id>.body`; die letzten 300, Bodys bei 256 KB abgeschnitten; ein gRPC-Aufruf behält sein Protokoll als Body |

`<config>` ist `%APPDATA%` unter Windows, `~/Library/Application Support` unter
macOS und `~/.config` unter Linux. OAuth-2.0-Tokens und Passwörter
verschlüsselter Sammlungen werden nie auf die Festplatte geschrieben.

## Umgebungsvariablen

| Variable | Wirkung |
|---|---|
| `APIX_PASSWORD` | Passwort einer verschlüsselten Sammlung für die CLI (`--password-env` wählt eine andere Variable). |
| `APIX_VAR_<NAME>` | Eine Variable für die CLI: `APIX_VAR_TOKEN` füllt `{{token}}` (passend zu den Namen der Sammlung, egal in welcher Schreibweise); hat Vorrang vor Umgebungen, `--var` hat Vorrang vor ihr. |
| `APIX_THEME` | eine Theme-ID (`obsidian`, `cyber`, `dracula`, `nord`, `solarized-dark`, `daylight`, `solarized-light` oder der Name einer Theme-Datei); zeigt dieses Theme, egal was in den Einstellungen steht. |
| `APIX_BENCH` | `startup` oder `json:<url>`: messen und beenden (siehe unten). |
| `APIX_BENCH_OUT` | Datei, an die die Messungen angehängt werden. |
| `APIX_BENCH_PHASES` | Mit `APIX_BENCH=startup`: meldet zusätzlich, wann jeder Schritt des Starts erreicht wurde (Zeilen `phase_…`), um zu sehen, wohin die Zeit geht. |
| `APIX_UPDATE_URL` | Wo die Update-Prüfung statt beim neuesten Release auf GitHub nachfragt (ein Fork, ein Mirror, ein Testserver). |
| `OPENAI_API_KEY`, `GEMINI_API_KEY` (oder `GOOGLE_API_KEY`), `ANTHROPIC_API_KEY` | Der Schlüssel für den KI-Assistenten, wenn für diesen Anbieter keiner im Schlüsselbund liegt. |
| `APIX_CLAUDE_CONFIG` | Die Konfigurationsdatei von Claude Desktop, die Mit Claude verbinden statt der echten schreibt (für Tests). |
| `APIX_AI_URL` | Wohin der KI-Assistent seine Anfragen an die Messages API statt an Anthropic schickt (ein Proxy, ein Testserver). |
| `APIX_GEO_URL` | Woher **DB-IP Lite herunterladen** statt von DB-IP herunterlädt. |
| `APIX_PLUGINS_DIR` | Der Plugin-Ordner statt `<config>/Apix/plugins`. |
| `GITHUB_BASE_REF` | Von GitHub Actions bei Pull Requests gesetzt: `apix run --changed` ohne Wert vergleicht mit `origin/<it>`. |

## Leistung messen

`scripts/measure.ps1` baut die Release-App und misst:

- Größe der **Release-Binärdatei**.
- **Start bis zum ersten Frame:** von `main` bis der erste Frame gezeichnet ist
  (`APIX_BENCH=startup`), Median aus mehreren Läufen.
- **Speicher im Leerlauf:** das Working Set fünf Sekunden nach dem Start mit
  geöffneter Beispielsammlung.
- **Großes JSON auf dem Bildschirm:** ein GET auf einen 100 MB großen JSON-Body
  vom lokalen Beispiel `big_json_server`, vom Senden bis die Antwort gezeichnet
  ist (`APIX_BENCH=json:<url>`), sodass das Netzwerk nicht in die Zahl eingeht.

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7, auch unter macOS/Linux
```

Das Skript gibt eine Markdown-Tabelle mit dem Rechner aus, auf dem es lief; die
README zeigt die neueste. Die Zeiten stammen aus
`crates/apix-desktop/src/bench.rs`, das nichts tut, solange `APIX_BENCH` nicht
gesetzt ist.

Weitere Helfer:

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # 100 MB JSON auf :8765
cargo run -p apix-http --example oauth_server                       # OAuth-2.0-Testserver auf :8766
cargo run -p apix-http --example sse_server                         # Server-Sent Events auf :8767 (setzt nach Last-Event-ID fort)
cargo run -p apix-http --example ws_server                          # WebSocket-Echo (auch binär) und Ticker auf :8768
cargo run -p apix-http --example graphql_server                     # ein GraphQL-Endpunkt (Schema und einige Benutzer) auf :8769
cargo run -p apix-http --example socketio_server                    # Socket.IO 4: Namespaces /, /chat, /ticker, /private (Auth) auf :8771
cargo run -p apix-grpc --example grpc_server                        # ein gRPC-Greeter mit Reflection auf :8770
cargo run -p apix-http --example soap_server                        # ein SOAP-Rechner auf :8773 (WSDL unter ?wsdl; Faults mit 500 und 200)
cargo run -p apix-proto --example mqtt_broker                       # ein MQTT-Broker auf :8772 (`clock` tickt jede Sekunde)
```

## Sprachen der Oberfläche

Die Oberflächentexte liegen in `crates/apix-desktop/locales/<code>.yml`, ein
flaches `key: value` pro Zeile, gelesen von rust-i18n über das Makro `tr!`. Ein
Unit-Test schlägt fehl, wenn einer Sprache ein Schlüssel fehlt oder sie andere
`%{placeholders}` als Englisch verwendet. Um eine Sprache hinzuzufügen,
kopieren Sie `en.yml`, übersetzen sie und fügen eine Zeile zu `LANGUAGES` in
`src/i18n.rs` hinzu.

## Sprachen der Dokumentation

`Docs/languages.toml` listet die Sprachen der Dokumentation. Übersetzungen
beginnen mit einem Stempel, der den englischen Text nennt, aus dem sie stammen:

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check                      # listet fehlende oder veraltete Übersetzungen und kaputte Links
cargo run -p docs-check -- stamps            # gibt den aktuellen Stempel jedes englischen Dokuments aus
cargo run -p docs-check -- site site/docs    # die Dokumentation als Webseiten (GitHub Pages)
```

## Das mitgelieferte gpui-Backend

Der Windows-Textrenderer von GPUI (Crate `gpui-pre-windows` 0.3.9) setzte jede
Glyphenfolge von links nach rechts, sodass arabische Wörter gespiegelt
erschienen. `vendor/gpui-pre-windows` ist dieses Crate mit korrigiertem
`DrawGlyphRun` (die Änderung ist mit „Apix patch“ markiert), und die
`Cargo.toml` des Workspace bindet es per Patch ein. Es kann entfernt werden,
sobald das Crate upstream korrigiert ist.

## Continuous Integration

`.github/workflows/ci.yml` läuft bei jedem Push und Pull Request:

| Job | Was er prüft |
|---|---|
| Format und Docs | `cargo fmt --check`; `docs-check` (Übersetzungen folgen den englischen Dokumenten) |
| Engine und CLI | `cargo clippy -D warnings` und `cargo test` für alles außer der Desktop-App, unter Windows, macOS und Linux |
| Desktop-App | dasselbe für `apix-desktop` auf den drei Systemen (Linux installiert vorher die Fenster-, Schrift- und Schlüsselbund-Bibliotheken) |
| GitHub Action | die eigene Action des Repositorys (`uses: ./`) gegen `examples/jsonplaceholder`, unter Linux und Windows, und dass sie ihre Berichte geschrieben hat |

`.github/workflows/release.yml` baut die Release-Pakete bei einem Versions-Tag
(`v1.2.3`, der zu `Cargo.toml` passen muss) und veröffentlicht sie als
GitHub-Release mit `SHA256SUMS.txt`; von Hand gestartet baut er sie nur, als
Artefakte des Laufs. Windows: `scripts/package-windows.ps1` (Inno Setup 6,
`packaging/windows/apix.iss`: Installation pro Benutzer oder pro Rechner,
optional PATH und Zuordnung von `.apix`, Deinstallationsprogramm). macOS:
`scripts/package-macos.sh` baut beide Architekturen, fügt sie mit `lipo` zu
`Apix.app` zusammen (ad hoc signiert) und erstellt eine `.dmg`. Linux (auf
Ubuntu 22.04, damit glibc 2.35 genügt): `scripts/package-linux.sh` erstellt den
Tarball mit `install.sh`, das `.deb` (Abhängigkeiten aus `dpkg-shlibdeps`, dazu
`libvulkan1`) und ein ausgefülltes `PKGBUILD`; `scripts/package-arch.sh` baut
daraus das Arch-Paket in einem `archlinux`-Container. Das `.deb` und das
Arch-Paket werden im Workflow installiert und auf fehlende Bibliotheken
geprüft. `.github/workflows/pages.yml` veröffentlicht `site/` unter
https://11r5.github.io/Apix/ .
Jedes Release enthält außerdem einen ausgefüllten Homebrew-Cask (`apix.rb`, aus
`packaging/homebrew`) und winget-Manifeste (`AhmetDemir.Apix*.yaml`, aus
`packaging/winget`), erstellt von `scripts/channel-manifests.sh`, zum Kopieren
in das Tap-Repository ([11R5/homebrew-apix](https://github.com/11R5/homebrew-apix))
und für einen Pull Request an `microsoft/winget-pkgs`. `scripts/action-repo.sh`
schreibt die Dateien von [11R5/apix-action](https://github.com/11R5/apix-action),
der Marketplace-Kopie von `action.yml`, die immer ein Release herunterlädt (der
Marketplace listet kein Repository mit Workflow-Dateien); nach jedem Release
bekommt es einen Tag `v<version>`, und `v1` wandert auf den neuen Commit.
`.github/workflows/docker.yml` veröffentlicht das Container-Image bei
Versions-Tags. Die Action speichert das von ihr gebaute `apix` unter einem Hash
der Engine-Quellen im Cache, sodass ein neuer Commit es neu baut und ein
unveränderter es wiederverwendet.

## Tests

| Crate | Was abgedeckt ist |
|---|---|
| apix-core | Hin- und Rückweg von Modell und Dateiformat, Variablen, Umgebungsgeheimnisse im Schlüsselbund (ohne Anmeldedatenspeicher übersprungen), Signieren gegen die AWS-Testsuite, RFC 7616 (Digest), RFC 7636 (PKCE) und das OAuth-1.0-Beispiel; GraphQL-Schemas (Lesen von Introspection, SDL) und was an jeder Stelle einer Abfrage vorgeschlagen und beschrieben wird (Felder, Aliase, Argumente, Enum-Werte, Input-Objekte, Listen, Fragmente, Variablentypen, Kommentare und Strings); die Tabelle `[grpc]`; Verlaufseinträge, gRPC-Aufrufe eingeschlossen; GraphQL-Variablendeklarationen und Vervollständigung in den Variablen (JSON); die Einstellung für vertrauenswürdige CAs; JSON-Filterung mit JSONPath und jq (Treffer, Ausgaben in Reihenfolge, Fehler in Worten, endlose Filter gestoppt); die Query der URL und ihre Zeilen in beide Richtungen, `:name`-Pfadvariablen gefunden und gefüllt (nur im Pfad, kodiert, ein zweites Füllen ändert nichts), Aufräumen der URL einer Anfrage; vorgeschlagene Header-Namen und -Werte (Anfang vor Enthalten, keine mehr, sobald vollständig getippt); Beispiele (Hin- und Rückweg von Text und Bytes, die Anfrage wie geschrieben, kein Set-Cookie); git gegen ein echtes Repository (Änderungen seit einem Abzweigpunkt, nicht committet, zwischen Commits, eine Datei bei einem Commit, eine unbekannte Ref), was eine geänderte Datei betrifft (Skripte, Ordner, Umgebungen, das Projekt, Dateien, die eine Anfrage nennt), Diffs von Anfragen (Zeilen nach Name, Formatierung ignoriert, Body Zeile für Zeile, Verschiebungen); Monitor-Intervalle und Hin- und Rückweg von `[[monitor]]`; Anfragen des KI-Assistenten (Anmeldedaten maskiert, Bodys gekürzt, die Sprache, Ausweichmodelle nur für das Standardmodell) und Antworten (Textblöcke, Ablehnungen, Fehlerarten, das Skript in einem Codeblock); Vergleich von Antworten (Header nach Name ohne flüchtige, JSON Feld für Feld mit Datensätzen nach ID zugeordnet, Formatierung ignoriert, Text- und Binär-Bodys); in JSON gefundene Datensätze für Tabellen (Hüllen, einfache Werte, Zahlen- und Beschriftungsspalten, ausgerichteter Text) |
| apix-http | Aufbau von Anfragen, jede Body-Art, ein lokaler Server für OAuth 2.0, Digest, AWS und OAuth 1.0, der Netzwerk-Trace; der SSE-Parser (Stücke an beliebiger Stelle geteilt), Streaming gegen einen langsam sendenden Server, WebSocket (Echo, Auth und Parameter im Handshake, Schließen von beiden Seiten, ein abgelehnter Handshake), Abruf eines GraphQL-Schemas mit der Auth der Anfrage und die Fehler, wenn keine da ist, HTTPS mit eigener Zertifizierungsstelle; WebSocket-Subprotokolle und Binärnachrichten, `wss://` mit eigener CA oder ungeprüft; was ein Event-Stream in eine neue Verbindung mitnimmt; Socket.IO-Pakete, Handshake-Adressen und binäre Anhänge, und gegen einen lokalen Socket.IO-4-Server: Events, Acks in beide Richtungen, Pings, Namespaces mit Auth und Ablehnungen, binäre Anhänge, der Server, der den Client hinauswirft, ein einfacher WebSocket-Server unterschieden; WebSocket-Pings in einem Intervall mit gemessenen Pongs, ein unbeantworteter Ping beendet die Verbindung, `ping_interval` in der Datei; TLS-Details des eigenen Handshakes der Anfrage gegen einen lokalen HTTPS-Server (Version, Cipher-Suite, Gruppe, die gesendete Kette mit Schlüssel und Fingerabdruck, Handshake-Zeit passend zur Aufteilung TCP/TLS); gzip-, zlib-, rohe deflate- und brotli-Bodys stückweise dekodiert, abgeschnittene Bodys abgelehnt, `Accept-Encoding` angefordert, sofern die Anfrage keinen setzt, `Content-Encoding` beibehalten |
| apix-grpc | Adressen, Statuscodes, Header und Auth als Metadaten; gegen einen lokalen Greeter (aus dynamischen Nachrichten gebaut, mit und ohne Reflection): Auflisten und Beschreiben des Dienstes, alle vier Methodenarten, Auth erreicht den Server, ein fehlgeschlagener Status, .proto-Dateien mit Imports, und die Fehler für keine Beschreibung, eine unbekannte Methode, eine unpassende Nachricht, eine Liste für eine unäre Methode, eine fehlende Datei und keinen Server; Beispielnachrichten; ein Aufruf als HTTP-Antwort für Läufe (Status-Zuordnung, `grpc-status`, ein Stream als Liste); TLS gegen einen Server mit einem Zertifikat seiner eigenen Zertifizierungsstelle (abgelehnt, mit der CA vertraut, ungeprüft akzeptiert); Definitionen: echte Generatorausgabe in `tests/fixtures/generated` (protoc 3.19 für Python, Java, C#, C++, PHP; protoc-gen-es; ts-proto) muss den ursprünglichen Dienst zurückgeben, Go, Rust, betterproto und Annotationen liefern Nummern und Typen, derselbe Typ aus 10 Sprachen gelesen, JSON-Beispiele, Vergleich mit einem Server, eine Vermutung weicht der Reflection, unbekannte Felder gemeldet, ein falscher Antworttyp erklärt |
| apix-proto | MQTT-Adressen, Logins und Optionen; gegen einen lokalen Broker: jede Quality of Service, Wildcards, Abonnieren und Verlassen während der Verbindung, abgelehnte Abonnements und ungültige Topics, Retained Messages, Logins und gehaltene Sessions, der Broker legt auf, `mqtts://` mit eigener CA oder ungeprüft |
| apix-codegen | API-Dokumentation (Ordner, maskierte Anmeldedaten, keine Werte aus `.local.toml`, Markdown-Beschreibungen ohne rohes HTML, Anker); Shell-Quoting für bash, cmd und PowerShell; Einfügen von curl; Hin- und Rückweg; URL, Header, Bodys, String-Escaping und Einstellungen jeder Sprache |
| apix-import | Postman-v2.0/v2.1-Fixtures; WSDL: beide SOAP-Versionen, Ports und Adressen, document- und rpc-Stil, Header, Beispiele aus dem Schema (Erweiterung, Enums, Choices, `form`, Rekursion), Imports und was nicht gelesen werden konnte, Orte, WSDL 2.0 abgelehnt |
| apix-script | Variablen und Änderungen an der Anfrage aus Pre-Request-Skripten, Ergebnisse und Meldungen von `pm.test` / `pm.expect`, die ältere API `tests[...]`, Fehler mit Dateinamen, endlose Skripte gestoppt, `setNextRequest`, `atob`/`btoa`, `pm.sendRequest` (Callback und `await`, Wartezeit nicht mitgezählt), die eingebauten Bibliotheken gegen bekannte Digests, Prüfungen ohne Code, Werte und Ausdrücke für Antwortziele |
| apix-runner | CSV- (Anführungszeichen, Zeilenumbrüche, `;`, BOM) und JSON-Datendateien, dotenv-Dateien und `APIX_VAR_*`-Namen, Anzahl der Iterationen und Datenzeilen, Timing von Pause/Stopp, JUnit/JSON/HTML/Markdown-Ausgabe und Escaping (Markdown für Pull Requests: Markierung, Escaping, was fehlschlug, eine Auswahl mit `--changed`), Datumsangaben; ganze Läufe gegen einen lokalen Server (Datenzeilen, Reihenfolge der Seitenleiste, `bail`, `setNextRequest`), Iterationen parallel gegen einen langsamen Server (schneller als nacheinander, nach Iteration aufgelistet); Monitore: wann der Zustand wechselt, Protokolle auf ihre letzten Ergebnisse begrenzt und je Sammlung getrennt, Zeitpläne, die dem Protokoll folgen, Webhook-Nachrichten |
| apix-runner (Lasttests) | Perzentile nach nächstem Rang, Parsen von Dauern; gegen einen lokalen Keep-alive-Server: eine Anzahl von Anfragen über gehaltene Verbindungen (etwa eine pro Worker bei 400 Anfragen), eine gleichmäßige Rate, ein Zeitlimit, Stoppen von außen, 500er und abgelehnte Verbindungen nach Art gezählt |
| apix-runner (Abläufe) | Ablaufdateien gelesen und geschrieben, Startknoten, Kanten ersetzt und entfernt, Probleme benannt, Gegenstände einer Antwort (Status, Zeit, Header, JSONPath, jq, nichts vorhanden), Vergleiche; gegen einen lokalen Server: ein von einer Anfrage gemerkter Wert in der URL der nächsten verwendet, eine Polling-Schleife mit Prüfung und Wartezeit, eine fehlschlagende Anfrage mit und ohne Fehlerkante, ein Wert, der nicht vorhanden ist, eine unbekannte Anfragedatei, eine endlose Schleife bei 1000 Schritten gestoppt, ein Ablauf, der nicht laufen kann |
| apix-cli (MCP) | das Protokoll (Versionen, Benachrichtigungen, unbekannte Methoden, die Tool-Liste mit und ohne `--list-only`); gegen einen lokalen Server: Auflisten von Anfragen und Umgebungen (Namen, keine Werte), Senden mit einer Umgebung und mit den Variablen des Agenten, Tests der Skripte im Ergebnis, Ausführen aller Anfragen, Pfade außerhalb der Sammlung und fehlende Dateien abgelehnt |
| apix-mock | Pfadmuster (`{{base_url}}/users/:id`, Hosts, Queries), der wörtlichste Treffer, Auswahl über `x-mock-response-name` und `-code`, 2xx zuerst, dekodierte Bodys ohne `Content-Encoding`, die 404-Liste, Preflights; ein echter Server auf einem Port, der über den HTTP-Client antwortet, mit CORS, und ein Beispiel ausliefert, das während des Laufs hinzugefügt wurde |
| apix-capture | gespeicherte Anfragen behalten keine Cookies oder Anmeldedaten, Host-Filter; über echte lokale Server: einfaches HTTP aufgezeichnet und gespeichert, HTTPS mit der lokalen Zertifizierungsstelle geöffnet (der Client vertraut nur ihr), HTTPS-Tunnel ungelesen durchgereicht |
| apix-route | Zeilen von tracert (auch einer übersetzten), traceroute und tracepath; lokale Adressen; die Kartenprojektion und Entfernungen; eine im MaxMind-DB-Format geschriebene Datenbank: Orte, Adressen, die sie nicht kennt, private, Auswahl der Datei; Installieren eines Downloads (gzip, abgelehnt, wenn keine Datenbank, die alte erst danach ersetzt); das eigene Werkzeug des Systems verfolgt 127.0.0.1 |
| apix-plugins | die Beispiel-Plugins mit ihren Manifesten geladen; die HMAC-Signatur gegen eine unabhängig berechnete; CSV-Import und Feldstatistik; als Text geschriebene Module: eine endlose Schleife gestoppt, Imports abgelehnt, ein ungültiger Name, ein fehlendes Manifest, eine Antwort außerhalb des Speichers, eine Datei, die kein WebAssembly ist; außerdem Hin- und Rückweg von `[auth.plugin]` (apix-core) und die Antwort eines Importers als Anfragedateien (apix-import) |
| apix-vault | Verschlüsselungsformat, Manipulation, Passwortänderung, Wiederherstellungsschlüssel, Schlüsselbund-Einträge (gegen den echten Anmeldedatenspeicher unter einem Test-Dienstnamen; übersprungen, wo keiner existiert) |
| apix-desktop | Übersetzungen vollständig, Sprachhelfer, Dateipfade, Hervorhebungsabfrage, Theme-Dateien (Lesen, Fehler, Hin- und Rückweg), Akzent- und Transparenzfarben, Dateinamen von GraphQL-Schemas, das Protokoll eines gRPC-Aufrufs im Verlauf |
| docs-check | Struktur und Stempel der Dokumentation |
