<!-- translated from en/architecture.md sha256:75b4d247b3b61d0d -->

# Architektur von Apix

Apix ist ein nativer API-Client, geschrieben in Rust. Die Oberfläche wird mit
[GPUI](https://github.com/zed-industries/zed) (der UI-Engine von Zed) und der
Komponentenbibliothek [gpui-kit](https://github.com/longbridge/gpui-kit) auf der
GPU gezeichnet. Es gibt keine WebView und kein Electron.

## Grundsätze

1. **Lokal zuerst, git-freundlich.** Eine Sammlung ist ein Ordner mit kleinen
   TOML-Dateien. Es gibt kein Konto und keinen Zwang zur Cloud. Sammlungen
   werden wie Code committet, verglichen und geprüft.
2. **Die Oberfläche blockiert nie.** Netzwerk, Festplatte, Parsen und Skripte
   laufen außerhalb des UI-Threads. Der UI-Thread macht nur Layout und
   Zeichnen.
3. **Eine Engine, viele Frontends.** Die Desktop-App, die CLI `apix` und
   künftige Frontends (eine TUI, ein MCP-Server) teilen dieselben Crates,
   sodass sich eine Anfrage überall gleich verhält, auch in CI.
4. **Auch riesige Antworten bewältigen.** Eine JSON-Antwort von 100 MB muss
   flüssig scrollen. Nur die sichtbaren Zeilen werden gelayoutet (virtuelle
   Listen), und der Body wird erst bei Bedarf geparst.
5. **Leicht und schnell beim Start.** Geschwindigkeit und Funktionen gehen vor
   Größe. Die Release-Binärdatei bleibt unter 100 MB (heute 54.4 MB;
   Electron-Apps haben 300 MB+) und öffnet ein Fenster in unter 150 ms.

## Crates

```
crates/
├── apix-core     Anfragemodell, {{variables}}- und {{$dynamic}}-Werte, Dateiformat,
│                 Projekte, Umgebungen und Geheimnisse, Baum- und Dateioperationen,
│                 GraphQL-Schemas (Introspection, SDL) und Vervollständigung von Abfragen,
│                 git: Branch, geänderte Dateien, was eine Änderung betrifft, Diffs auf Anfrageebene
├── apix-http     HTTP-Engine: reqwest + rustls, Zeitmessung, Erfassen der Antwort, Netzwerk-Trace,
│                 Server-Sent Events, WebSocket (tokio-tungstenite), Socket.IO, Abruf von GraphQL-Schemas
│                 (DNS, Verbindungsaufbau, Weiterleitungen, Zertifikat, lokale Schnittstelle)
├── apix-import   Postman, Insomnia, OpenAPI, HAR und WSDL → Apix-Sammlungen
├── apix-codegen  Anfragen ↔ Shell-Befehle: curl (bash), curl.exe (cmd), PowerShell; parst eingefügtes curl; Code in 10 Sprachen
├── apix-vault    verschlüsselte Sammlungen: XChaCha20-Poly1305 pro Datei, Argon2id, Wiederherstellungsschlüssel
├── apix-script   Pre-Request- und Testskripte: QuickJS mit einer pm.*-kompatiblen API
├── apix-runner   führt Sammlungen aus: Reihenfolge, Iterationen (auch parallel), Datendateien, Verzögerung, Bail,
│                 Berichte (auch Markdown für Pull Requests), Monitore: Zeitplan, Protokoll, Webhooks
├── apix-grpc     gRPC-Aufrufe: tonic, Nachrichten als JSON (prost-reflect), .proto-Dateien (protox), Definitionen aus Code oder JSON
│                 oder Server-Reflection
├── apix-proto    weitere Protokolle: MQTT (rumqttc)
├── apix-mock     ein lokaler Mock-Server, der aus gespeicherten Beispielen antwortet (hyper)
├── apix-capture  ein aufzeichnender Proxy: einfaches HTTP, CONNECT-Tunnel, HTTPS mit lokaler CA geöffnet (rcgen, rustls)
├── apix-route    die Route zu einem Server (das traceroute des Systems) und wo seine Hops liegen (eine Offline-.mmdb-Datenbank)
├── apix-plugins  WebAssembly-Plugins (wasmi): Auth-Arten, Importer, Antwortansichten; apix-plugin-sdk schreibt sie in Rust
├── apix-cli      Binärdatei `apix`: send / run / diff / schema / import / shell aus Terminal & CI
└── apix-desktop  GPUI-Desktop-App (Windows, macOS und Linux aus einem Crate)
tools/
└── icongen       erzeugt jedes App-Icon (ico, icns, hicolor-PNGs) aus Logo/LogoFirst.jpg
```

In `apix-desktop/src`: `workspace.rs` (Anfrage-Editor, Antwortbereich,
Aktionen und Tastenbelegungen), `workspace/collections.rs` (Seitenleiste,
Projekte, Dateioperationen, Import, Dateiüberwachung),
`workspace/env_editor.rs` (der Variablendialog), `workspace/auth_editor.rs`
(der Reiter Auth und der Auth-Dialog für Ordner/Sammlung/Projekt),
`workspace/palette.rs` (die Befehlspalette und ihre unscharfe Suche),
`workspace/network.rs` (der Reiter Netzwerk der Antwort),
`workspace/runner_ui.rs` (der Runner), `workspace/vault_ui.rs` (Entsperren,
Sperren und automatisches Sperren, Verschlüsseln, Entschlüsseln, Passwort
ändern, Wiederherstellungsschlüssel), `assets.rs` (gpui-kit-Icons plus die zwei
Schloss-Icons), `workspace/menu_bar.rs` (Datei/Anfrage/Umgebung/Ansicht/Hilfe in
der Titelleiste; die Schaltflächen rufen `occlude()` für den Ziehbereich der
Titelleiste auf, damit Windows ihnen den Klick übergibt), `workspace/tabs.rs`
(offene Anfragen; der vordere Reiter lebt in den Editor-Widgets, die anderen
behalten ihren Entwurf, ihre Antwort und ihre laufende Aufgabe),
`workspace/drag.rs` (Anfragen und Ordner zum Verschieben ziehen),
`workspace/shell_copy.rs` (Kopieren als / curl einfügen), `workspace/var_chips.rs`
(`{{variable}}`-Chips in der URL-Leiste und ihre Hover-Karte),
`workspace/graphql_ui.rs` (Abruf, Cache, Vervollständigung und Hover von
GraphQL-Schemas), `workspace/grpc_editor.rs` und `workspace/grpc_ui.rs` (der
Reiter Nachricht einer gRPC-Anfrage und der Aufruf im Antwortbereich),
`large_view.rs` (zeilenbasierter Betrachter für riesige Bodys), `net.rs`
(Brücke zu tokio), `bidi.rs` (Helfer für Layout von rechts nach links, siehe
Sprachen), `state.rs` (zuletzt verwendete Elemente und Umgebung pro Ordner im
Konfigurationsordner des Benutzers), `theme.rs` und `fonts.rs`.

### Ein Desktop-Crate für drei Betriebssysteme

GPUI und gpui-kit laufen unter Windows, macOS und Linux mit demselben Code, so
wie Zed ausgeliefert wird. Es gibt also keine UI-Ordner pro Betriebssystem.
Code, der sich je Betriebssystem wirklich unterscheidet, steht hinter `cfg` in
`crates/apix-desktop/src/platform/{windows,macos,linux}.rs`: Titelleiste und
Fenstersteuerung, Blur/Vibrancy, die macOS-Menüleiste, Dateizuordnungen für
`.apix` und der Zugriff auf den Schlüsselbund. Installer und Icons liegen in
`packaging/{windows,macos,linux}/`. Eine eigene Oberfläche würde nur
abgetrennt, wenn ein Betriebssystem einmal eine wirklich andere Oberfläche
braucht.

Abhängigkeitsrichtung: `core` ← `http`/`script`/`proto` ← `cli`/`desktop`.
Nichts hängt von `desktop` ab.

## Datenfluss einer Anfrage

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                   Pre-Request-Skript (später) ◀────────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio-Runtime, Hintergrund-Thread)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                         Testskript (später) ◀──────────┤
                                                        ▼
            Oberfläche: Statuszeile, virtualisierter Body-Betrachter
```

`{{variables}}` werden in Schichten aufgelöst. Spätere Schichten haben Vorrang:
**globale Variablen → Sammlung (`apix.toml`) → Umgebung → Überschreibungen (CLI `--env-file`, `APIX_VAR_*` und `--var`, passend zu den Namen der Sammlung, egal in welcher Schreibweise; Skript `pm.variables.set`)**.
Undefinierte Namen bleiben stehen und werden gemeldet, sodass die Oberfläche
sie unterstreichen und die CLI das Senden verweigern kann.

### Der Netzwerk-Trace

Jedes Senden baut seinen eigenen kurzlebigen reqwest-Client, sodass alles, was
er aufzeichnet, zu dieser Anfrage gehört (`apix_http::trace`). Ein Resolver
misst die DNS-Zeit, ein Tower-Layer um den Connector von reqwest misst jede
Verbindung, und die Weiterleitungsrichtlinie notiert jeden Hop. reqwest gibt
seine TLS-Verbindung weder heraus, noch lässt es einen eigenen Connector eine
zurückgeben; daher stammen die TLS-Details aus dem Inneren von rustls, über
eine Client-Konfiguration, die Apix selbst baut (`use_preconfigured_tls`,
`tls::traced_config`, Hooks in `tls_trace.rs`):

- ein umhüllender Zertifikatsprüfer zeichnet die Kette auf, die der Server
  gesendet hat, und welche Version er für die Signatur prüft (TLS 1.2 oder
  1.3), und lässt dann den üblichen Prüfer (oder „alles akzeptieren“ bei
  ausgeschalteter Prüfung) entscheiden;
- ein Session-Speicher, der nichts behält, wird beim Schreiben des Client
  Hello nach einem Hinweis zum Schlüsselaustausch gefragt; das markiert den
  Beginn des TLS-Handshakes: Die Zeit der Verbindung davor (nach DNS) ist TCP,
  danach TLS; ihm wird die Gruppe mitgeteilt, die der Server gewählt hat
  (TLS 1.3);
- die Cipher-Suites sind die eigenen von rustls, einmal erzeugt mit einer
  dünnen Hülle um ihre Verschlüsselung, die notiert, welche Suite eingerichtet
  wurde. Suites sind `'static` und gemeinsam genutzt, daher findet die Hülle
  die Anfrage über einen Thread-lokalen Wert, der gesetzt ist, während das
  Future der Anfrage gepollt wird (`Scoped`); der Handshake läuft innerhalb
  dieses Polls.

ALPN ergibt sich aus der Version der Antwort (HTTP/2 über TLS nur, nachdem `h2`
vereinbart wurde). Komprimierte Bodys dekodiert Apix selbst (`decode.rs`, gzip,
zlib oder rohes deflate, brotli, während sie ankommen) statt reqwest, das
`Content-Encoding` und die Größe auf der Leitung verwerfen würde.

## Dateiformat

```
my-api/
├── apix.toml               Name + [vars] und [auth] auf Ebene der Sammlung
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         optional: [auth] für die Anfragen in diesem Ordner
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Vererbung der Auth.** Eine Anfrage ohne `auth` erbt sie vom nächsten Ordner,
dessen `folder.toml` eine festlegt, dann von der Sammlung (`apix.toml`), dann
vom Projekt (`apix-project.toml`); gibt es keine, wird nichts gesendet.
`auth = "none"` in einer Anfrage (oder einem Ordner) beendet die Vererbung.
`apix_core::project::inherited_auth` übernimmt die Suche für die Desktop-App,
die CLI und „Kopieren als“ gleichermaßen.

**Anfrage-Einstellungen.** Eine Tabelle `[settings]` (Timeout,
Weiterleitungen, TLS-Prüfung, Proxy, Client-Zertifikat, vertrauenswürdige CA,
Cookies) gibt es auf denselben vier Ebenen. Jedes Feld ist eine `Option`:
`None` erbt feldweise über dieselbe Kette
(`apix_core::project::inherited_settings`), und was dann noch nicht gesetzt
ist, fällt auf die Standardwerte in `RequestSettings::effective` zurück — kein
Timeout, bis zu 10 Weiterleitungen folgen, TLS prüfen, Proxy aus der Umgebung,
Cookies an. Die Engine wendet das Ergebnis an, wenn sie den `reqwest::Client`
jedes Sendens baut (eine Datei mit vertrauenswürdiger CA fügt ihre Zertifikate
als Wurzeln hinzu); der gemeinsame Cookie-Speicher (ein `cookie_store`, den der
Cookie-Manager auflisten und bearbeiten kann) wird an diesen Client übergeben,
sofern die Anfrage Cookies nicht ausschaltet.

**Signierte Auth.** AWS Signature V4, OAuth 1.0a und Digest sind reine
Funktionen in `apix_core::sign`, geprüft gegen die AWS-Testsuite, RFC 7616,
RFC 7636 und das klassische OAuth-1.0-Beispiel, sodass die Engine und „Kopieren
als“ auf dieselbe Weise signieren. Die Engine baut zuerst den
`reqwest::Request` und signiert ihn dann (der AWS-Payload-Hash braucht den
endgültigen Body; ein Multipart-Stream wird als `UNSIGNED-PAYLOAD` gesendet).
Digest sendet einmal, liest die Challenge der 401 und beantwortet sie auf
derselben Verbindung. OAuth 2.0 lebt in `apix_http::oauth2`: Das Token wird vor
dem Senden geholt (Client Credentials, Passwort oder Authorization Code mit
PKCE, wobei der Browser zu einem einmaligen Listener auf 127.0.0.1
zurückkehrt), im Speicher pro Server, Client, Grant, Scope und Benutzer
gehalten, mit seinem Refresh-Token erneuert und nie auf die Festplatte
geschrieben.

Eine Anfragedatei:

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # keine Query: die steht in [[query]]

[[query]]
key = "notify"
value = "yes"

[[path]]                 # der Wert des Segments :org
key = "org"
value = "{{org}}"

[[example]]              # eine mit der Anfrage aufbewahrte Antwort
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # woraus sie gesendet wurde, wie geschrieben
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # in der Datei behalten, nicht gesendet

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # eine Prüfung ohne Code, nach den Testskripten ausgeführt
target = "status"
op = "=="
value = "201"
```

**Prüfungen.** `[[assert]]`-Zeilen vergleichen ein Ziel (`status`, `time`,
`size`, `body`, `body.<path>`, `header.<name>`) mit einem Wert über `==`, `!=`,
`>`, `>=`, `<`, `<=`, `contains`, `not contains`, `exists`, `not exists`,
`matches` oder `is` (ein JSON-Typ). `apix_script::assertions_script` macht
daraus ein weiteres Testskript, sodass sie überall wie `pm.test` berichten.

**Bodys.** `[body]` enthält eines von `json`, `text`, `xml`, `form`
(URL-kodierte Zeilen), `multipart` (Zeilen mit `file = true` für Dateifelder
und einem optionalen `content_type`), `binary = { file = "…" }` oder `graphql`
(`query` und `variables`). Dateipfade dürfen Variablen enthalten und sind
relativ zur Anfragedatei; `Request::with_files_relative_to` macht sie nach dem
Auflösen der Variablen absolut, in der App wie in der CLI. Die Engine liest
Dateien erst beim Senden, sodass eine fehlende Datei ein Sendefehler ist, kein
Ladefehler. Ein GraphQL-Body wird als JSON `{"query", "variables"}` gesendet;
ungültige Variablen stoppen das Senden. Bei einem Multipart-Body wird ein
Header `Content-Type: multipart/…` ohne die Boundary der Engine (häufig in
Postman-Exporten) verworfen, damit der Body lesbar bleibt.

**gRPC.** Eine Anfrage, deren URL `grpc://host:port` ist (oder `grpcs://` für
TLS), nennt ihre Methode in `[grpc]`; der JSON-Body ist die Nachricht, und eine
JSON-Liste ist ein Strom von Nachrichten für eine Methode, die einen annimmt.
Ohne `protos` wird der Server gefragt (Reflection). Pfade sind relativ zur
Anfragedatei.

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # optional
import_paths = ["../protos"]              # optional
```

Statt Dateien hält `definition` die Beschreibung in der Anfrage: `.proto`-Text
oder ein Descriptor-Set als JSON. Sie hat Vorrang vor `protos`. `guessed = true`
markiert eine, die aus Typen oder JSON erschlossen wurde: Server-Reflection hat
Vorrang vor ihr, und die Unterschiede werden gemeldet. `protos` nimmt auch
Descriptor-Sets an, binär oder als `.json`.

```toml
[grpc]
method = "shop.v1.Orders/Create"
guessed = true
definition = """
syntax = "proto3";
package shop.v1;
service Orders { rpc Create(CreateRequest) returns (CreateResponse); }
message CreateRequest { string customer = 1; }
message CreateResponse {}
"""
```

Offene Fragen:
- **Geheimnisse.** Umgebungswerte wie Tokens sollten nicht committet werden.
  `environments/<name>.local.toml` liegt als Schicht über `<name>.toml`. Wird
  eine geschrieben, kommt `*.local.toml` in die `.gitignore` neben
  `environments/`, falls es dort fehlt. Ein Geheimnis darf einen Platzhalter in
  der gemeinsamen Datei behalten, sodass Teammitglieder den Namen sehen.
  Wahlweise (Einstellungen › Geheime Umgebungswerte) kommen die Geheimnisse
  stattdessen in den Schlüsselbund des Betriebssystems: `<name>.local.toml`
  enthält dann nur `keychain = "<id>"`, und die Werte liegen im Eintrag
  `env:<id>` des Dienstes `Apix` (`apix_vault::keychain::save_text`, wegen der
  Grenze von 2560 Byte unter Windows auf mehrere Einträge verteilt).
  `apix_core::secrets::use_keychain` entscheidet, wohin neue Geheimnisse
  geschrieben werden; das Lesen folgt der Datei, sodass die CLI keine
  Einstellung braucht. Die ID wandert mit der Datei, sodass Umbenennen der
  Umgebung oder Verschieben des Ordners die Verknüpfung erhält. Innerhalb einer
  verschlüsselten Sammlung bleiben Geheimnisse im Tresor.
- **Dynamische Variablen.** `{{$name}}` wird von `apix_core::dynamic` gefüllt,
  wenn keine Variable dieses Namens definiert ist, mit einem neuen Wert für
  jedes Vorkommen (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …).
  Keine zusätzlichen Crates: ein kleiner xorshift-Generator und ein von Hand
  geschriebener UTC-Datumsformatierer.
- **Reihenfolge in der Seitenleiste.** Ohne gespeicherte Reihenfolge kommen
  zuerst die Ordner, dann die Anfragen, jeweils nach Name. Ziehen speichert
  eine Liste `order` mit den Namen der Kinder in der `folder.toml` des Ordners
  (oder der `apix.toml` der Sammlung); `store::place` setzt ein Element vor oder
  hinter ein anderes, und Umbenennen und Verschieben halten die Liste aktuell.
  Namen, die die Liste nicht kennt, folgen in der Standardreihenfolge, sodass
  eine neue Datei nie verschwindet. Sammlungen eines Projekts verwenden `order`
  in `apix-project.toml` (`Project::place_collection`). Der Postman-Importer
  schreibt `order` nur, wenn die Reihenfolge in Postman von der Standardreihenfolge
  abweicht.

### Projekte

Ein Projekt gruppiert Sammlungen und besitzt Umgebungen, die alle gemeinsam
nutzen:

```
shop/
├── apix-project.toml       Name, gemeinsame [vars], Reihenfolge der Sammlungen, group_by, pins, colors
├── environments/           test.toml, prod.toml (+ *.local.toml-Geheimnisse)
├── users/                  eine Sammlung (apix.toml + .apix-Dateien)
└── payments/               eine Sammlung; kann an bestimmte Umgebungen angeheftet sein
```

`[colors]` ordnet einem Sammlungsordner einen Farbnamen zu (`red`, `orange`,
`yellow`, `green`, `teal`, `blue`, `purple`, `pink`); die App zeichnet ihn als
Punkt in der Seitenleiste und auf den Reitern. `project::move_collection_into`
verschiebt einen Sammlungsordner in ein Projekt (Kopieren und Löschen über
Laufwerke hinweg).

Variablenschichten, die spätere hat Vorrang: Projektvariablen →
Sammlungsvariablen → die Umgebung des Projekts → die gleichnamige Umgebung der
Sammlung (jeweils mit ihren `.local.toml`-Geheimnissen). Die CLI
(`apix run --env`) und die Desktop-App verwenden dieselbe Funktion,
`apix_core::project::variables`. `group_by = "environment"` ordnet die
Seitenleiste als Projekt → Umgebung → Sammlungen an, und `[pins]` beschränkt
eine Sammlung auf bestimmte Umgebungen.

### Verschlüsselte Sammlungen (M4)

Format und Engine liegen in `crates/apix-vault`. Der Rest von Apix erreicht sie
über `apix_core::vfs`, eine dünne Schicht mit denselben Aufrufen wie `std::fs`:
Ein Pfad unter der Wurzel eines entsperrten Tresors wird aus dem Tresor
bedient, im Speicher entschlüsselt; jeder andere Pfad geht auf die Festplatte.
So verwenden Speicher, Seitenleiste, Reiter und CLI weiter gewöhnliche Pfade,
und nichts Entschlüsseltes wird je auf die Festplatte geschrieben. Entsperrte
Tresore leben in einem prozessweiten Register; Sperren (von Hand, nach 15
Minuten Leerlauf oder beim Beenden) verwirft den Tresor und löscht seinen
Schlüssel. Löschen innerhalb einer verschlüsselten Sammlung ist endgültig: Ein
Papierkorb des Systems würde entschlüsselte Dateien enthalten.

```
my-api/
├── apix-vault.toml   Klartext-Kopf: Format, Argon2id-Kosten und Salts,
│                     der Datenschlüssel, umhüllt vom Passwort und vom Wiederherstellungsschlüssel
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   die echten Pfade aller Dateien, verschlüsselt
    ├── 3f9c…e1.apixe je eine verschlüsselte Datei; zufällige Namen, flach (keine Ordnerstruktur)
    └── local/        *.local.toml-Geheimnisse, gleiches Format, nie committet
```

- Ein zufälliger 256-Bit-**Datenschlüssel** verschlüsselt jede Datei mit
  XChaCha20-Poly1305 und einer neuen Nonce bei jedem Schreiben. Die Objekt-ID
  sind die zugehörigen Daten (Associated Data), sodass Blobs nicht unbemerkt
  vertauscht oder umbenannt werden können.
- Der Datenschlüssel wird zweimal gespeichert: umhüllt von einem aus dem
  **Passwort** abgeleiteten Schlüssel (Argon2id, 64 MiB, 3 Durchläufe) und von
  einem aus dem **Wiederherstellungsschlüssel** abgeleiteten (160 zufällige
  Bits, einmal angezeigt als `XXXX-XXXX-…` in Crockford-Base32). Eine
  Passwortänderung umhüllt den Schlüssel neu; Dateien werden nicht neu
  verschlüsselt.
- Umbenennen und Verschieben berühren nur den verschlüsselten Index.
- Der entsperrte Schlüssel lebt nur im Speicher (`zeroize` löscht ihn beim
  Sperren).
- Git sieht weiterhin, welches Objekt sich geändert hat, aber nie, was darin
  steht. Die Sicherheit kommt von der Chiffre und dem Passwort, nicht davon,
  dass das Format geheim ist; Apix ist Open Source.
- Das Passwort kann im Anmeldedatenspeicher des Betriebssystems gemerkt werden
  (`apix_vault::keychain`, über das Crate `keyring`: Anmeldeinformationsverwaltung,
  macOS-Schlüsselbund, Secret Service). Ein Eintrag pro Tresor unter dem Dienst
  `Apix`, benannt `vault:<absolute folder>` (unter Windows in Kleinbuchstaben).
  Es ist eine Option in der Entsperrabfrage; App und CLI probieren es, bevor
  sie fragen, ein Passwort, das den Tresor nicht mehr öffnet, wird gelöscht und
  erneut abgefragt, eine Passwortänderung aktualisiert es, und das
  Entschlüsseln der Sammlung löscht es. Gibt es keinen Speicher, fragt Apix
  einfach wie bisher.

## Skripte (M5)

`apix_core::scripts` findet die Skripte einer Anfrage (`_collection.<event>.js`,
das `_folder.<event>.js` jedes Ordners, dann `<request>.<event>.js`; Events sind
`prerequest` und `test`) und hält `ScriptVars`, was Skripte pro Geltungsbereich
setzen oder entfernen. `ScriptVars::apply` legt sie über die `Variables` der
Anfrage, sodass das Auflösen wie zuvor funktioniert. Nichts, was Skripte
setzen, wird auf die Festplatte geschrieben.

`apix-script` führt die Skripte eines Events in einer neuen QuickJS-Runtime aus
(`rquickjs`; Grenzen von 10 s und 256 MB). Das Objekt `pm` ist JavaScript
(`prelude.js`): Rust übergibt den Kontext als JSON (die nicht aufgelöste
Anfrage, die Antwort für Tests, die Werte der Geltungsbereiche aus
`project::scope_values`, frühere `ScriptVars`, `pm.info`), die Skripte laufen
nacheinander, jeweils in eine Funktion gehüllt, und `__apix_output()` gibt die
geänderte Anfrage und Variablen, Testergebnisse, Konsolenzeilen,
`setNextRequest` und `skipRequest` als JSON zurück. Die API in JavaScript zu
halten, beschränkt die Rust-Seite auf wenige Aufrufe und macht es leicht, das
Verhalten von Postman nachzubilden.

`pm.sendRequest` ruft ein natives `__apix_send` mit der Anfrage als JSON auf;
wer `apix_script::run` aufruft, übergibt einen `Sender`, der sie sendet und
blockiert, bis die Antwort da ist (die CLI auf einer neuen Runtime im
Skript-Thread, die App auf ihrer Netzwerk-Runtime). Skripte werden in
async-Funktionen gehüllt, sodass sowohl der Callback-Stil als auch `await`
funktionieren, und die Wartezeit wird nicht auf die Grenze angerechnet.
`require` kommt aus `libs.js`: Teilmengen von crypto-js (Digests und HMACs in
Rust über `__apix_hash` / `__apix_hmac`), uuid, lodash, moment, ajv, tv4 und
chai, für Apix geschrieben statt heruntergeladen.

Ein Senden mit Skripten: Pre-Request-Skripte, dann `ScriptVars::apply` und
Auflösen, die HTTP-Anfrage, dann Testskripte mit einer `ResponseView`.
`apix_runner::exchange` tut das für `apix send`, `apix run` und den Runner; das
einzelne Senden der Desktop-App tut es in `script_run.rs`, mit den Skripten in
einem Hintergrund-Thread und den eigenen Skripten der Anfrage aus dem Editor,
gespeichert oder nicht.

`apix_runner::run` nimmt einen `Plan` (Anfragedateien in Reihenfolge, Variablen
und Skript-Geltungsbereiche, Datenzeilen, Iterationen, Verzögerung, `bail`,
Transport-Überschreibungen, der Skriptzustand, mit dem begonnen wird) und geht
ihn durch: Pro Iteration legt er die Datenzeile (dann die `--var`-Werte) über
die Variablen und `pm.iterationData`, folgt `setNextRequest` nach dem Namen der
Anfrage, wartet zwischen Anfragen und meldet `Progress` (Iteration, Senden,
Element fertig) an einen Callback. Ein `Control` pausiert oder stoppt ihn aus
einem anderen Thread. Er gibt einen `RunReport` aus `ItemResult`s zurück, den
`report.rs` als JSON, JUnit-XML (eine Suite pro Anfrage und Iteration, ein Fall
pro Test, ein Statusfall für Anfragen ohne Tests) und als in sich geschlossene
HTML-Seite schreibt. `data.rs` liest CSV (RFC 4180, BOM, `;`-Kopfzeilen) und
JSON-Arrays. Die CLI gibt den Fortschritt aus, sobald er kommt; der
Desktop-Runner (`workspace/runner_ui.rs`) führt ihn auf der Netzwerk-Runtime
aus und kopiert den Fortschritt einige Male pro Sekunde aus einer gemeinsamen
Struktur `Live`; die Ansicht bleibt im Workspace, sodass ein Lauf seinen Dialog
überdauert.

## Streams und WebSocket (M6)

`HttpClient::send_with` meldet `Progress::Head` (Status und Header), sobald
sie da sind, und danach jeden `Progress::Chunk` des Bodys; `send` ist
`send_with` ohne Zuhörer, und die Antwort enthält trotzdem den ganzen Body.
`apix_http::sse` liest `text/event-stream`-Bodys Stück für Stück (`SseParser`:
jedes Zeilenende, ein Stück darf an beliebiger Stelle geteilt sein, `id` wird
übernommen, ein unvollständiges letztes Event wird verworfen). Der Parser
merkt sich außerdem die letzte ID und das letzte `retry:` für das erneute
Verbinden: `reconnected()` startet einen Parser für die nächste Verbindung mit
beidem, und `reconnect_request` fügt `Last-Event-ID` hinzu. App und CLI
übernehmen das Warten und erneute Senden selbst, sodass Esc oder Strg+C es
immer stoppt.

`apix_http::ws::connect` baut den Handshake aus der Anfrage so, wie die
HTTP-Seite sie senden würde (`prepare`: Parameter, Header, Auth; OAuth 2.0 wird
zuerst zu einem Bearer-Token), fügt `Sec-WebSocket-Protocol` aus `[websocket]
subprotocols` hinzu und verbindet sich mit tokio-tungstenite. `wss://` bekommt
eine rustls-Konfiguration aus `apix_http::tls::client_config` (die üblichen
Wurzeln plus eine vertrauenswürdige CA, ein Client-Zertifikat oder ein Prüfer,
der jedes Zertifikat akzeptiert), dasselbe Modul, das gRPC ohne Prüfung nutzt.
`WsConnection::run` sendet, was ein Kanal von `WsCommand`s verlangt, und meldet
`WsEvent`s (gesendet, empfangen, geschlossen mit Code und Grund, Fehler), bis
eine der Seiten schließt; die Pings des Servers beantwortet tungstenite. Mit
`[websocket] ping_interval` pingt es auch selbst, mit einem Zähler als
Nutzdaten, meldet die Roundtrip-Zeit des passenden Pongs (`WsEvent::Pong`) und
endet mit einem Fehler, wenn ein Ping noch unbeantwortet ist, wenn der nächste
fällig wird. Das `MessageFormat` einer Nachricht (Text, base64, hex) macht aus
ihrem Text `WsData`, nachdem die Variablen eingesetzt sind.

In der Desktop-App füllt die Netzwerk-Runtime gemeinsame Strukturen
(`net::Incoming` für ein Senden, `net::SocketIncoming` für einen Socket), und
eine UI-Aufgabe liest sie alle 60–80 ms. Eine Event-Stream-Antwort wird zu
`ResponseState::Streaming` (`workspace/stream_ui.rs`) mit einer Live-Liste der
Events, die neuen Events folgt, sofern nicht nach oben gescrollt wurde; Esc
macht aus dem Angekommenen eine fertige Antwort. Eine `ws://`-URL schaltet den
Anfragebereich auf den Reiter Nachrichten (`workspace/ws_editor.rs`) und den
Antwortbereich auf `ResponseState::Socket` (`workspace/socket_ui.rs`). Beide
Zustände werden wie eine fertige Antwort an ihren Reiter geleitet, sodass ein
Stream oder Socket in einem Reiter im Hintergrund weiterläuft. Die CLI gibt
Events und Nachrichten aus denselben Callbacks aus.

## Socket.IO (M6)

`apix_http::socketio` spricht Socket.IO 3/4 (Engine.IO-Protokoll 4) auf der
oben beschriebenen WebSocket-Verbindung, sodass Handshake, Header, Auth und
TLS-Einstellungen dieselben sind. Eine Anfrage ist Socket.IO, wenn sie eine
Tabelle `[socketio]` hat (`SocketIoOptions`: Namespace, Pfad, Auth);
`handshake_request` macht aus `http(s)://` ein `ws(s)://`, nimmt den Namespace
aus den Optionen oder dem Pfad der URL, setzt den Pfad des Servers
(`/socket.io/`) an seine Stelle und fügt `EIO=4&transport=websocket` hinzu.
`connect` wartet auf das Open-Paket von Engine.IO, sendet das CONNECT des
Namespace mit dem Auth-JSON und wartet auf die Antwort (`40…` mit der
Socket-ID oder `44…` mit der Ablehnung), bevor es zurückkehrt, sodass ein
falscher Server, Namespace oder Token wie ein fehlgeschlagener Handshake
scheitert.

`SioConnection::run` führt `WsConnection::run` neben einem Übersetzer zwischen
zwei Kanälen aus: `SioCommand::Emit` wird zu einem `EVENT`-Paket (`42…`, mit
einer Ack-ID, wenn verlangt, IDs ab 0 gezählt) oder einem `BINARY_EVENT`
(`45n-…`), dessen `{"$binary": "<base64>"}`-Werte durch Platzhalter ersetzt und
danach als Binär-Frames gesendet werden; empfangene Pakete werden zu
`SioEvent`s (empfangene Events mit der Ack-ID, die der Server wünscht, unsere
Acks, der Server, der uns hinauswirft, unlesbare Frames), wobei binäre Anhänge
gesammelt und dort wieder eingesetzt werden, wo ihre Platzhalter standen.
Engine.IO-Pings (`2`) bekommen ein Pong (`3`).

Die Desktop-App hat für beides einen Reiter Nachrichten: `WsEditor` hat einen
Schalter WebSocket/Socket.IO und die Socket.IO-Felder, und `WsMessage` trägt
`event` und `ack`. `net::SocketIncoming` ist generisch über den Event-Typ, und
`socket_ui.rs` verfolgt beide Verbindungsarten mit einer Schleife und macht aus
`SioEvent`s Protokolleinträge mit dem Event-Namen und Ack-Chips. Der Testserver
in `apix-http/tests/common/socketio_server.rs` ist zugleich das Beispiel
`socketio_server` (:8771).

## MQTT (M6)

`apix_proto::mqtt` umhüllt die Event-Schleife von rumqttc (MQTT 3.1.1; sein TLS
nur über die Konfiguration aus `apix_http::tls::client_config`, sodass es keinen
eigenen Krypto-Provider mitbringt). Eine `mqtt://`- oder `mqtts://`-URL macht
eine Anfrage zu MQTT, wie `grpc://` es für gRPC tut; `[mqtt]` enthält
Client-ID, Session, Keep-alive und Abonnements. `connect` pollt die
Event-Schleife bis zum CONNACK des Brokers, sodass ein abgelehnter Login wie
ein fehlgeschlagener Handshake scheitert.

`MqttConnection::run` führt zwei Futures nebeneinander aus und bricht das Pollen
der Event-Schleife nie ab: Eines übergibt `MqttCommand`s an den Client (der
warten kann, wenn seine Warteschlange voll ist, und diese Warteschlange leert
das Pollen), das andere verfolgt die Verbindung und meldet `MqttEvent`s.
rumqttc meldet die Paket-ID jedes ausgehenden Pakets in der Reihenfolge der
Übergabe, sodass eine Warteschlange Veröffentlichungen und Abonnements ihren
IDs zuordnet und PUBACK, PUBCOMP, SUBACK und UNSUBACK des Brokers dem, was sie
bestätigen. Eine Nachricht gilt als gesendet, wenn ihr Paket hinausgegangen
ist. Ein ungültiges Topic oder ein abgelehntes Abonnement ist ein Event, und die
Verbindung läuft weiter; Schließen und das Auflegen des Brokers beenden sie.

In der Desktop-App hält `WsEditor` auch die MQTT-Felder (die URL entscheidet,
und `Workspace` teilt es ihm mit), und `socket_ui.rs` verfolgt die Verbindung
mit derselben Schleife wie WebSocket und Socket.IO und macht aus `MqttEvent`s
Protokollzeilen nach Topic. Der Test-Broker in
`apix-proto/tests/common/broker.rs` (Wildcards, QoS 0–2, Retained Messages,
Ablehnungen, TLS über jeden Stream) ist zugleich das Beispiel `mqtt_broker`
(:8772).

## SOAP (M6)

SOAP fügt keine Anfrageart hinzu: `apix_import::wsdl` macht aus einer WSDL 1.1
gewöhnliche POST-Anfragen mit XML-Bodys, sodass alles, was mit einer Anfrage
funktioniert, auch mit ihnen funktioniert. `apix_core::soap` enthält, worin
sich die Versionen unterscheiden (Header, Envelope), und liest Faults;
`Response::soap_fault` wendet es auf XML-Antworten an, und der Runner lässt
eine Anfrage ohne Tests bei einem Fault scheitern, egal welcher Status.

Der Importer ruft nichts ab. `Loader` geht `wsdl:import`, `xsd:import` und
`xsd:include` durch: Er sagt, welcher Ort als Nächstes gelesen werden soll, und
nimmt den Text zurück, sodass CLI und Desktop-App Dateien und Netzwerk jeweils
auf ihre eigene Weise lesen. Orte werden relativ zu ihrem Dokument aufgelöst,
ein Dokument aus dem Netzwerk kann nicht auf lokale Dateien verweisen, und ein
Lesevorgang stoppt bei 64 Dokumenten. `Model` sammelt dann Messages, Port Types,
Bindings, Ports und globale Schema-Elemente aus allen Dokumenten, nach
Namespace und Name geordnet. `Writer` baut Beispiele aus den XSD-Knoten
(roxmltree behält sie und löst `prefix:name` gegen die eigenen Deklarationen des
Knotens auf) und entscheidet für jedes Element anhand von `form`,
`elementFormDefault` und ob es global ist, ob es qualifiziert ist. Namespaces,
die er verwendet, kommen auf das Envelope. Ein Stapel der gerade geschriebenen
benannten Typen stoppt Rekursion.

Der URL-Import in App und CLI liest den Text und nimmt dann dieselben Wege wie
eine Datei, sodass er jedes Format bedient. Die Test-WSDLs in
`apix-import/tests/fixtures/` speisen auch das Beispiel `soap_server` (:8773).

## GraphQL-Schemas (M6)

`apix_core::graphql` enthält die Standard-`INTROSPECTION_QUERY`, ein `Schema`,
das aus ihrer Antwort gelesen wird (Typen mit Feldern, Argumenten,
Input-Feldern, Enum-Werten, Interfaces und möglichen Typen; `TypeRef` behält
Listen und `!`), und `to_sdl`, das zuerst die Wurzeltypen schreibt und
eingebaute Skalare und `__`-Typen weglässt. Für den Editor liest es eine
Abfrage nur bis zum Cursor: `spot_at` zerlegt den Text in Tokens (Kommentare,
Strings und `$variables` entfallen) und geht einen Stapel von Selektionen,
Argumentlisten, Input-Objekten und Listen durch, folgt Aliasen,
`... on Type`, Fragmenten und Variablendefinitionen, um zu sagen, was dort
passt (`Spot`). `suggestions` macht daraus Beschriftungen mit ihren Typen und
Beschreibungen, sortiert nach Präfix, dann Teilstring, veraltete zuletzt;
`describe_at` liefert das Markdown für Hover. Nichts wird validiert: Eine
kaputte Abfrage wird trotzdem so gut wie möglich vervollständigt. Auch der
Variablen-Editor vervollständigt: `variable_types` liest die Deklarationen
`$name: Type`, ein kleiner JSON-Scanner findet den Pfad aus Schlüsseln und
Listenelementen bis zum Cursor und ob dort ein Schlüssel oder ein Wert hingehört,
und `variable_suggestions` geht diesen Pfad durch Input-Felder und
Listenelemente, um die noch nicht angegebenen Variablen, Input-Felder,
Enum-Werte (als JSON-Strings) und `true`/`false` vorzuschlagen; ein gerade
getippter String wird samt Anführungszeichen ersetzt, auch eines, das der
Editor geschlossen hat.

`apix_http::graphql::fetch_schema` sendet die Introspection-Abfrage als POST
mit URL, Headern, Auth und Einstellungen der Anfrage (die Anfrage wird zuerst
aufgelöst) und liest das Schema oder sagt, warum es keines gibt (ein Status
außerhalb von 2xx mit dem Anfang des Bodys oder das `errors[0].message` des
Servers).

In der App implementiert `workspace/graphql_ui.rs` den `CompletionProvider` und
den `HoverProvider` von gpui-kit über ein gemeinsames
`Rc<RefCell<Option<Rc<Schema>>>>`, das der Abfrage-Editor hält;
Vervollständigungen ersetzen das gerade getippte Wort über eine LSP-Textänderung.
**Schema abrufen** im Body-Editor sendet `FetchSchema`; der Workspace baut die
Anfrage wie für ein Senden (geerbte Auth und Einstellungen, Variablen) und ruft
es auf der Netzwerk-Runtime ab. Die Antwort jedes Endpunkts wird in
`<config>/Apix/graphql/<FNV-1a of the URL>.json` aufbewahrt und geladen, wenn
eine GraphQL-Anfrage geöffnet wird. Die SDL-Ansicht ist ein schreibgeschützter
Editor, der beim Zeichnen gefüllt wird, und erst, wenn er angezeigt wird.

## gRPC (M6)

`apix-grpc` macht Aufrufe ohne generierten Code. Die Dienstbeschreibung ist ein
`prost_reflect::DescriptorPool`: aus den `.proto`-Dateien der Anfrage über
protox (Rust, kein `protoc`; Imports werden in `import_paths` und neben jeder
Datei gesucht, die Well-Known Types von Google sind eingebaut) oder vom Server
über gRPC-Reflection (v1, dann v1alpha; Dateien, die der Server auslässt, werden
nach Namen angefragt und dann in der Reihenfolge der Abhängigkeiten
hinzugefügt). Nachrichten sind `DynamicMessage`s: JSON hinein über die
serde-Unterstützung von prost-reflect, JSON heraus mit den Feldnamen aus der
`.proto` und angezeigten Standardwerten. `DynamicCodec` steckt sie in tonic,
sodass `tonic::client::Grpc` alle vier Methodenarten (unär, Server-Streaming,
Client-Streaming, bidirektional) über einen `Channel` ausführt (HTTP/2 auf dem
hyper und rustls, die die HTTP-Engine schon nutzt).

`apix_grpc::convert` erstellt eine Beschreibung aus dem, was sonst vorhanden
ist, und probiert der Reihe nach: ein Descriptor-Set in JSON; `.proto`-Text;
das `FileDescriptorProto`, das generierter Code einbettet (`embedded.rs` weiß,
wie jeder Generator diese Bytes schreibt, und die von PHP sind nicht einmal
unbedingt gültiger Text); dann Deklarationen in einer Programmiersprache
(`source.rs`: Kommentare entfernt, Blöcke gefunden, Member je nach
Sprachfamilie gelesen, Typen normalisiert, Nummern aus Annotationen und den
Encodern von ts-proto übernommen) oder ein JSON-Beispiel (`json.rs`, das die
Reihenfolge der Schlüssel behält, die `serde_json::Value` verlieren würde). Die
letzten beiden gehen über ein kleines `Schema`, das als `.proto` ausgegeben
wird. `printer.rs` schreibt eine exakte Beschreibung als `.proto`-Text zurück
und gilt nur dann, wenn dieser Text zu derselben Beschreibung kompiliert;
andernfalls, oder bei mehreren Dateien, bleibt die Definition ein
Descriptor-Set in JSON. `compare` gleicht eine Definition mit der Beschreibung
eines Servers ab und ordnet Felder nach Namen zu, da Nachrichtennamen nie über
die Leitung gehen. `described` hält die Reihenfolge: die Definition der Anfrage,
ihre Dateien, Reflection; eine Vermutung probiert zuerst Reflection und weicht
ihr. `call` meldet Felder, die eine `DynamicMessage` nicht zuordnen konnte
(`CallEvent::Note`), weil es sonst nichts tun würde.

Header und Auth der Anfrage werden zu Metadaten (Bearer, Basic, ein
API-Key-Header, OAuth 2.0 über `apix_http::oauth2`; `-bin`-Schlüssel sind
base64); Auth-Arten, die HTTP-Anfragen signieren, werden mit Begründung
abgelehnt. Timeout, Client-Zertifikat und vertrauenswürdige CA kommen aus den
Anfrage-Einstellungen. Bei ausgeschalteter TLS-Prüfung bekommt tonic (das keinen
solchen Schalter hat) eine einfache `http://`-Adresse und den echten
`https`-Ursprung, und ein eigener Connector (`insecure.rs`) erledigt TLS: rustls
mit ALPN h2 und einem Prüfer, der jedes Zertifikat akzeptiert, aber trotzdem die
Signaturen des Handshakes prüft. `call` meldet die Events `Sent`, `Headers` und
`Received`, sobald sie geschehen, und endet mit einem `CallOutcome`: Statuscode
und -meldung, Antwort-Metadaten, Trailer und jede Nachricht. Ein anderer Status
als OK ist ein Ergebnis, kein Fehler; Fehler sind Dinge, die einen Aufruf
verhindern (keine Verbindung, keine Beschreibung, eine Nachricht, die nicht zu
ihrem Typ passt).

In der App macht eine `grpc://`-URL aus dem Reiter Body den Reiter Nachricht
(`workspace/grpc_editor.rs`): eine durchsuchbare Methodenauswahl, gefüllt über
**Methoden laden**, die `.proto`-Dateien und die Nachricht mit einem aus dem
Eingabetyp gebauten Beispiel. Der Aufruf läuft auf der Netzwerk-Runtime und
füllt eine gemeinsame Struktur, die die Oberfläche alle 60 ms liest; der
Antwortbereich ist ein Protokoll (`ResponseState::Grpc`,
`workspace/grpc_ui.rs`) wie bei einem WebSocket, und Esc bricht den Aufruf ab.
Die CLI gibt eine unäre Antwort vollständig und Streams Zeile für Zeile aus, und
`apix schema` schreibt den Dienst als `.proto`-ähnlichen Text.

Läufe rufen gRPC-Anfragen über `apix_grpc::send_for_tests` auf, das den
`CallOutcome` in eine `apix_http::Response` umwandelt
(`CallOutcome::to_response`): der HTTP-Status, für den der Code steht (die
Zuordnung von Google), mit dem Namen des Codes als Grund, Metadaten und Trailer
plus `grpc-status` und `grpc-message` als Header, und die Nachricht (eine
JSON-Liste bei einem Server-Stream) als Body. Testskripte, Prüfungen und
Berichte brauchen dann nichts gRPC-Spezifisches. Der Verlauf hält einen Aufruf
mit gesetztem `Summary::grpc`: Der Status ist der gRPC-Code, Trailer und
Statusmeldung haben eigene Felder, und der Body ist das Protokoll
(`[{"at_ms", "sent", "message"}]`), das `grpc_ui::restored` wieder in die
Protokollansicht verwandelt.

## Parallele Läufe und Monitore (M7)

`Plan::parallel` über eins führt so viele Iterationen gleichzeitig aus
(`futures_util` `buffer_unordered` in der eigenen Aufgabe des Laufs, keine
Threads): Jede Iteration sendet ihre Anfragen der Reihe nach mit ihrer eigenen
Kopie des Skriptzustands, ein Fehlschlag unter `bail` stoppt die anderen, und
der Bericht wird danach nach Iteration sortiert. Der Fortschritt läuft über
einen Mutex um den Callback des Aufrufers.

`apix_core::monitor` beschreibt `[[monitor]]` in `apix.toml` (Name, `every`,
`run`, Umgebung, Webhook, aktiviert) und liest Intervalle;
`apix_runner::monitor` führt einen aus (`plan` löst Variablen wie `apix run`
auf: die Umgebung des Monitors, `APIX_VAR_*`, dann angeheftete Werte), fasst ihn
zusammen (`Outcome`), führt pro Monitor ein `Log` im JSON-Lines-Format
(`<config>/Apix/monitors/<hash of the collection>-<name>.jsonl`, die letzten
500), entscheidet, wann jeder fällig ist (`Scheduler`: Ein zum ersten Mal
gesehener Monitor ist fällig, sobald sein Intervall seit seinem letzten
protokollierten Lauf verstrichen ist), und benachrichtigt einen Webhook, wenn
der Zustand wechselt (`change`, `notify`). `apix monitor` und die Desktop-App
(`workspace/monitors_ui.rs`: ein Ticker jede Sekunde, solange die Einstellung
an ist, ein Lauf nach dem anderen auf der Netzwerk-Runtime) nutzen es beide und
teilen sich so das Protokoll.

## Git (M7)

`apix_core::git` führt das bereits installierte Programm `git` aus (keine
git-Bibliothek, sodass der Binärdatei nichts hinzugefügt wird und die eigene
Konfiguration, Hooks und Anmeldedaten des Benutzers gelten). Ein `Repo` wird am
Sammlungs- oder Projektordner geöffnet, und jeder Befehl läuft dort mit
`--relative`, sodass Pfade relativ zu diesem Ordner sind und eine Sammlung in
einem größeren Repository nur ihre eigenen Dateien sieht.
`changes_since(rev)` ist `git diff --merge-base rev` (wo der Branch `rev`
verlassen hat, wie bei einem Pull Request) plus nicht verfolgte Dateien; ohne
Historie (ein flacher Klon) fällt es auf `rev` selbst zurück und sagt das.
`show(rev, path)` liest eine Datei bei einem Commit.

`git::affected` ordnet geänderte Dateien nach ihrer Rolle Anfragen zu
(`git::role_of`): eine Anfragedatei oder ihre Skripte `<stem>.*.js` → diese
Anfrage; `folder.toml` oder `_folder.*.js` → die Anfragen unter diesem Ordner;
`apix.toml`, `_collection.*.js`, `environments/*.toml`, die Projektdatei oder
die Umgebungen des Projekts → alle; jede andere Datei → die Anfragen, deren
Datei sie nennt. `apix run --changed` behält diese Anfragen, und der Bericht
hält die Auswahl fest (`RunReport::selection`).

`apix_core::diff` vergleicht zwei Versionen einer Datei nach ihrer Bedeutung:
Anfragen werden in `Request` geparst und als JSON serialisiert (sodass
Standardwerte und Formatierung keine Rolle spielen), anderes TOML in Werte,
Skripte und anderer Text Zeile für Zeile (längste gemeinsame Teilfolge, mit
abgeschnittenem gemeinsamem Anfang und Ende). Listen von Zeilen mit einem
eindeutigen `key` oder `name` (Header, Query, `[[example]]`) werden zu danach
geordneten Tabellen, sodass Zeilen nach Namen zusammenpassen; Felder werden
nach ihrem Pfad benannt (`headers.Accept`, `body.json`). Eine entfernte und
eine hinzugefügte Anfrage mit demselben Inhalt werden zu einer Verschiebung.
`apix diff` gibt das Ergebnis als Text, Markdown oder JSON aus.

`RunReport::to_markdown` schreibt den Lauf für GitHub (eine Tabellenzeile pro
Anfrage, Fehlschläge in einem geöffneten `<details>`) und beginnt mit einer
versteckten Markierung `<!-- apix-run: <name> -->`, nach der der
Kommentar-Schritt der Action sucht, um seinen früheren Kommentar zu bearbeiten.
Er bleibt unter der Grenze von GitHub von 65 536 Zeichen.

Die Desktop-App (`workspace/git_ui.rs`) liest den Branch und `git status` für
den offenen Ordner im Hintergrund nach jedem Neuladen und wenn das Fenster
aktiviert wird, und hält eine Zuordnung geänderter Pfade für die Markierungen in
der Seitenleiste; eine Anfrage, deren eigene Skripte sich geändert haben, gilt
als geändert.

Das Fenster Git (`workspace/git_panel.rs`) verwendet dasselbe `Repo`:
`working_changes` für die Liste, `show("HEAD", path)` gegen die Datei auf der
Festplatte über `diff::file` für die ausgewählte Datei, und `commit_folder`
(`git add -A -- .`, dann `git commit -- .`, also nur der offene Ordner),
`branches`, `switch`, `create_branch` (`switch -c`), `pull` (`--ff-only`),
`push` (`-u origin <branch>`, wenn es keinen Upstream gibt) und `log` (Hash,
Zeit, Betreff). Git läuft mit `GIT_TERMINAL_PROMPT=0`, sodass ein Remote, das
ein Passwort braucht, mit der Meldung von git scheitert, statt auf eine Eingabe
zu warten; Credential-Helper funktionieren weiterhin. Jede Operation läuft im
Hintergrund und aktualisiert die Statusleiste.

## KI-Assistent

`apix_core::ai` baut die Anfrage für einen von drei Anbietern von Hand (Rust hat
kein offizielles SDK für sie) und liest die Antwort. `Provider` (`anthropic`,
`openai`, `google`) liefert das Standardmodell, das Schlüsselbund-Konto des
Schlüssels (`ai:<provider>-api-key`, über `apix_vault::keychain::save_text`),
seine Umgebungsvariable und die URL. Anthropic: `POST /v1/messages` mit
`x-api-key`, `anthropic-version: 2026-10-21`, `claude-opus-5-5` und, für das
Standardmodell, serverseitigen Fallbacks (`fallbacks: "default"`, Beta
`server-side-fallback-2026-07-01`); ein Stop-Grund `refusal` wird zu einer
Meldung. OpenAI: `POST /v1/chat/completions` mit einem Bearer-Schlüssel, einer
System- und einer Benutzernachricht; ein Feld `refusal` wird als Ablehnung
behandelt. Google: `POST /v1beta/models/{model}:generateContent` mit
`x-goog-api-key`, `systemInstruction` und `contents`; `SAFETY`-Abschlüsse und
blockierte Prompts werden als Ablehnung behandelt, Gedankenteile übersprungen.
Jeder Prompt enthält den Austausch (`Context::describe`: Anmeldedaten-Header
maskiert, Bodys bei 12 KB abgeschnitten), die Aufgabe und den Sprachcode der
App. `models_request` / `models` listen, was ein Schlüssel verwenden kann (bei
OpenAI nur Chat-Modelle, bei Google Modelle, die `generateContent` können).
`APIX_AI_URL` ersetzt die Basis-URL für Tests. Die App (`workspace/ai_ui.rs`)
sendet über `net::send`, hängt Tests mit `ScriptEditor::append_test` an und
zeigt Erklärungen in einem Dialog; die Einstellungen behalten `ai_provider` und
`ai_model`.

## MCP-Server

`apix mcp` (`apix-cli/src/mcp.rs`) liest eine JSON-RPC-2.0-Nachricht pro Zeile
von der Standardeingabe und schreibt eine Antwort pro Zeile auf die
Standardausgabe (der stdio-Transport von MCP; eine Byte-Order-Mark vor der
ersten Zeile wird übersprungen). `initialize` antwortet mit der angefragten
Protokollversion, wenn es sie kennt (2025-06-18, 2025-03-26, 2024-11-05), und
sonst mit der neuesten, und bietet nur Tools an; Benachrichtigungen bekommen
keine Antwort, unbekannte Methoden den Fehler -32601. Ergebnisse von
`tools/call` tragen das JSON sowohl als Text als auch als `structuredContent`,
und ein fehlschlagendes Tool ist ein Ergebnis mit `isError`, kein
Protokollfehler, sodass der Agent lesen kann, warum. Pfade vom Agenten werden an
die Wurzel der Sammlung angehängt, `..` aufgelöst und außerhalb davon
abgelehnt. `send_request` läuft über `apix_runner::exchange` (Skripte,
Variablen, Tests), `run_requests` über `apix_runner::run`; Variablen kommen aus
demselben `VarArgs::resolve` wie bei den anderen Befehlen, mit den `variables`
des Agenten als `--var`s. Die CLI läuft in einem Thread mit 16 MB, da das
gemeinsame Future der Befehle unter Windows den Stack des Haupt-Threads
überschritt.

## Einrichtung und Claude

`workspace/setup_ui.rs` öffnet die Einrichtung einmal
(`settings.setup_shown`) und über Hilfe. Die Datenbank-Schaltfläche führt
`route_ui::download_geo` aus (die DB-IP-Datei dieses Monats, sonst die des
letzten Monats, über den Client der App, außerhalb des UI-Threads entpackt);
`apix geo download|status|remove` tut dasselbe in einem Terminal, und genau das
wird ein optionaler Schritt eines Installers aufrufen. `apix_core::claude_mcp`
findet die Konfiguration von Claude Desktop (`%APPDATA%\Claude`,
`~/Library/Application Support/Claude`, `~/.config/Claude`;
`APIX_CLAUDE_CONFIG` für Tests), benennt den Server `apix-<collection>`, lässt
ihn auf das `apix` neben dem laufenden Programm (oder im PATH) mit
`mcp <folder> [--list-only]` zeigen und fügt ihn in `mcpServers` ein, wobei
alles andere erhalten bleibt (das Original wird einmal nach `.bak` kopiert);
`claude_code_command` ist `claude mcp add --scope user`.

## Abläufe

`apix_core::flow` ist die Datei: Ein `Flow` hat Knoten (`NodeKind::Request`,
`Check`, `Set`, `Delay`, abgeflacht in jede `[[node]]`-Tabelle mit ihrer ID und
Position) und Kanten vom Ergebnis eines Knotens (`ok`/`fail`, `yes`/`no`,
`next`) zu einem anderen Knoten. `problems()` listet, was einen Ablauf am
Laufen hindert; `Seen` ist die letzte Antwort, und `Seen::subject` liest
`status`, `time`, `body`, `header:Name` oder einen JSONPath- oder jq-Filter über
`apix_core::filter`. `.flow`-Dateien können überall in der Sammlung liegen; die
Seitenleiste ignoriert sie.

`apix_runner::flow::run` durchläuft den Graphen vom Startknoten aus. Ein
Anfrageknoten geht wie ein Lauf über `exchange`, sodass Skripte, Tests und
`pm.*`-Variablen funktionieren; was er sich merkt, wird im lokalen
Geltungsbereich des Skriptzustands gesetzt, der für die folgenden Knoten Vorrang
vor der Umgebung hat, so wie es `pm.variables.set` täte. Eine Prüfung, deren
Gegenstand Text ist, löst `{{variables}}` auf und hat keinen Wert, wenn eine
davon undefiniert ist. Der Lauf meldet jeden Schritt (`FlowProgress`) und stoppt
nach 1000 Schritten oder über `Control`. `FlowReport::to_run_report` verwendet
die Laufberichte (JUnit, HTML, Markdown) wieder. Der Desktop-Editor
(`workspace/flows_ui.rs`) zeichnet Knoten als positionierte Elemente und Kanten
auf einer Zeichenfläche: Vorwärtskanten als Kurven, rückwärts laufende Kanten
als Linien um die Knoten herum; er führt den Ablauf auf der Netzwerk-Runtime aus
und liest seinen Fortschritt per Timer, wie der Runner.

## Plugins

`apix-plugins` führt WebAssembly-Plugins mit `wasmi` 2 aus, einem Interpreter:
kein JIT, also wird aus einem Plugin kein nativer Code erzeugt, und es baut
überall, wo Rust baut. Ein Modul darf nichts importieren, sodass ein Plugin
überhaupt keine Host-Funktionen hat (kein Netzwerk, keine Dateien, keine Uhr,
kein Zufall); es exportiert `memory`, `apix_alloc`, `apix_manifest` und
`apix_call`, und Antworten sind JSON, übergeben als `ptr << 32 | len`. Jeder
Aufruf instanziiert das Modul neu (kein Zustand zwischen Aufrufen) mit 2·10⁹
Einheiten Fuel und einer Speichergrenze von 256 MB, in einem Thread mit 64 MB
Stack; geht der Fuel aus, wird „lief zu lange“ gemeldet. Das Manifest wird
einmal beim Laden des Plugins gelesen; sein `name` muss `[a-z0-9-]` sein.
`installed()` speichert den Plugin-Ordner (`<config>/Apix/plugins` oder
`APIX_PLUGINS_DIR`) bis `reload()` zwischen.

Die Hooks treffen den Rest von Apix an drei Stellen: `Auth::Plugin { plugin,
config }` im Modell, aufgelöst wie andere Auth (Variablen in der Konfiguration)
und von `apix_http::prepare` auf die endgültige Anfrage angewendet;
`apix_import::plugin::plan`, das die Antwort eines Importers in Anfragedateien
umwandelt; und der Visualisierer der Antwort (`VisualMode::Plugin`), der das
Plugin außerhalb des UI-Threads ausführt und eine Tabelle oder Text zeichnet.
wasmi wird auch in Debug-Builds mit `opt-level = 3` und ohne Debug-Assertions
gebaut, da sein Dispatch auf Tail Calls setzt, die sonst den Stack aufbrauchen.

## Antworten vergleichen

`apix_core::compare` nimmt zwei `Snapshot`s (Beschriftung, Status, Header, Body,
Zeit). Header werden nach kleingeschriebenem Namen verglichen, mehrere mit
demselben Namen zusammengefügt; die in `VOLATILE` (Datumsangaben, Caching,
Anfrage- und Trace-IDs, Ratenbegrenzungen, wie der Body gesendet wurde) werden
nur als ausgelassen gezählt. Zwei JSON-Bodys gehen durch `by_id`, das Listen von
Datensätzen mit eindeutigen Werten `id` / `_id` / `uuid` / `key` in Tabellen mit
dem Schlüssel `[id=…]` umwandelt, und dann durch `diff::fields_as_they_are` (der
Durchlauf des Anfrage-Diffs ohne seine Zuordnung nach key/name, die zu
Anfragedateien passt, aber nicht zu Daten); anderer Text wird Zeile für Zeile
verglichen, alles andere nur nach Größe. Das Fenster Antworten vergleichen der
App (`workspace/compare_ui.rs`) nimmt die frühere Seite aus dem Verlauf
(Einträge mit derselben Anfragedatei oder derselben Methode und URL) oder sendet
die Anfrage, aufgelöst mit den Variablen einer anderen Umgebung;
`apix compare` sendet sie in zwei Umgebungen.

## Lasttests

`apix_http::load::Load` baut einen reqwest-Client für eine aufgelöste Anfrage
(die TLS-Einstellungen der Anfrage über `tls::client_config`, ihr Proxy, Timeout
und ihre Weiterleitungsrichtlinie, ein unbegrenzter Leerlauf-Pool), und `hit()`
sendet sie und zählt die Bytes des Bodys, ohne zu dekodieren; OAuth 2.0 wird
einmal zu einem Bearer-Token, Digest wird abgelehnt.
`apix_runner::bench::Bench::run` startet `concurrency` Worker in einem
`FuturesUnordered`; jeder nimmt die nächste Anfragenummer aus einem atomaren
Zähler, bis die Anzahl, die Frist oder `stop()` Nein sagt, und wartet mit einer
Rate bis `start + n / rate`. Jeder Treffer wird unter einem Mutex
festgehalten: seine Latenz in Mikrosekunden (ein `u32` pro Anfrage, also sind
eine Million Anfragen 4 MB), der Status, der Fehler nach Art („could not
connect“, „timed out“, …), die Bytes und ein Eimer pro Sekunde. `summary()` kann
während des Laufs aufgerufen werden: Es sortiert eine Kopie der Latenzen für
Perzentile nach nächstem Rang. Die CLI führt den Test auf einer eigenen Runtime
mit einem Thread pro Kern aus und gibt jede Sekunde eine Zeile aus; die App
führt ihn auf ihrer Netzwerk-Runtime aus und zeichnet viermal pro Sekunde neu.

## Route und Karte

`apix-route` startet das Werkzeug des Systems, so wie `apix_core::git` git
startet: `tracert -d -h N -w 1000` unter Windows, `traceroute -n -q 1 -w 1 -m N`
unter macOS und Linux, `tracepath -n -m N`, wenn traceroute fehlt. Keines davon
braucht Administratorrechte, die das eigene Senden von ICMP unter Windows
bräuchte. `parse_line` liest eine Zeile jedes dieser Werkzeuge nur anhand ihrer
Zahlen und Adressen (zuerst die Hop-Nummer, jedes `N ms`, das erste Wort, das
sich als Adresse parsen lässt), sodass ein übersetztes Windows in seiner eigenen
Codepage genauso gelesen wird; die Zeit eines Hops ist die beste seiner
Versuche, `<1 ms` zählt als eine halbe Millisekunde. `Trace` iteriert die Hops,
wie das Werkzeug sie ausgibt, und beendet es, wenn es verworfen oder gestoppt
wird.

`Geo` öffnet eine MaxMind-DB-Datei (`maxminddb`) und liefert einen `Place`
(Breitengrad, Längengrad, englischer Stadtname, ISO-Land) für öffentliche
Adressen; private, Loopback-, Link-Local-, CGNAT- und Multicast-Adressen werden
nie nachgeschlagen. Die Datenbank ist die `.mmdb`, die in `<config>/Apix/geo`
zuerst kommt; `install` entpackt einen Download (gzip), prüft, dass er sich
öffnen lässt, und ersetzt erst dann die alte. `dbip_urls` liefert die Adresse
von DB-IP Lite für diesen und den letzten Monat (`APIX_GEO_URL` ersetzt sie).
Die Karte ist eine abstandstreue Zylinderprojektion von `land::LAND`, Kontinente
und große Inseln, von Hand auf etwa zwei Grad genau gezeichnet (mit dem
Schwarzen und dem Kaspischen Meer in `land::WATER`), sodass keine Kartendaten
heruntergeladen oder lizenziert werden. Das Desktop-Fenster
(`workspace/route_ui.rs`) führt den Trace in einem Thread aus, platziert Hops,
sobald sie kommen, liest die Datenbank einmal pro Programmlauf und zeichnet die
Karte mit GPUI-Pfaden. `apix trace` gibt die Hops mit ihren Orten und der
Entfernung jedes Sprungs über 100 km aus.

## Update-Prüfung

`apix_core::update` baut die Anfrage (`releases/latest` von GitHub für
11R5/Apix, `User-Agent: Apix/<version>`, ein Timeout von 10 s;
`APIX_UPDATE_URL` lenkt sie woandershin) und liest die Antwort: `Version` parst
`v1.2.3`, `1.2` und Vorabversionen (`1.0.0-rc.1` wird vor `1.0.0` sortiert,
numerische Teile nach Wert), 404 bedeutet, dass noch nichts veröffentlicht ist,
andere Fehler tragen die Meldung von GitHub. Die CLI (`apix update`) sendet sie
mit ihrem eigenen Client; die App (`workspace/update_ui.rs`) über `net::send`
beim Start, wenn `settings.check_updates` an ist (nur ein neueres Release wird
angezeigt), und über Hilfe › Nach Updates suchen… (jede Antwort wird
angezeigt). Nichts wird heruntergeladen oder installiert.

## Themes

`theme.rs` enthält die Farben. Ein `ThemeDef` ist ein Theme (eingebaut oder aus
`<config>/Apix/themes/*.toml` gelesen; Farben, die eine Datei auslässt, kommen
aus Obsidian oder Daylight) mit `Colors` und einer Markierung dunkel/hell.
`Palette::build` macht daraus das, womit Ansichten zeichnen: die
Akzent-Einstellung (eine benannte Farbe mit einem dunklen und einem tieferen
hellen Ton oder `#RRGGBB`), der Text auf dem Akzent, gewählt nach relativer
Leuchtdichte, Hover- und Eingabetönungen für dunkel oder hell, und die
Transparenz-Einstellung: Das Alpha des Fensterhintergrunds ist `1 - t`, Panels
werden mit 60 % dieser Rate durchsichtig, Ränder werden mit wachsendem `t`
heller in Richtung Weiß, und helle Themes verwenden 70 % von `t`.
`from_settings` wählt das dunkle oder helle Theme nach der Einstellung des
Erscheinungsbilds und, bei „dem System folgen“, nach `App::window_appearance`.

Die Wurzelansicht von gpui-kit zeichnet `Theme::background` unter das ganze
Fenster, was den Blur des Betriebssystems verdecken würde; daher wird er bei
jeder Transparenz durchsichtig gesetzt, und Dialoge bekommen ihren eigenen
Hintergrund über `bidi::DialogBidi`, das jeder Dialog verwendet. Den Blur
übernimmt das Betriebssystem: `WindowBackgroundAppearance::Blurred` ist Acryl
unter Windows (`SetWindowCompositionAttribute`) und Vibrancy unter macOS.

Ansichten behalten nie eine Kopie der Palette: Sie rufen beim Zeichnen
`theme::current()` auf. `theme::set` speichert die neue Palette, richtet das
Komponenten-Theme von gpui-kit darauf aus (und schaltet seinen Modus um, sodass
die Farben des Code-Editors folgen), setzt den Hintergrund jedes Fensters auf
verschwommen oder deckend und aktualisiert die Fenster, sodass eine Änderung
sofort sichtbar ist. Der Workspace beobachtet das Erscheinungsbild des Fensters
und ruft es erneut auf, wenn das System zwischen hell und dunkel wechselt.

## Sprachen (M4)

Jeder Text in der Desktop-Oberfläche läuft über einen Schlüssel:
`tr!("menu.file")` (eine dünne Hülle um `rust_i18n::t!`, die einen
`SharedString` zum Anzeigen zurückgibt; `tr_plain!` liefert denselben Text für
Gespeichertes, etwa den Standardnamen einer neuen Anfrage). rust-i18n ist
dasselbe System, das gpui-kit verwendet, sodass beide einer prozessweiten
Locale folgen; Chinesisch verwendet `zh-CN`, den Code, den die eigenen Texte von
gpui-kit verwenden. Jede Sprache ist eine flache Datei,
`crates/apix-desktop/locales/<code>.yml` (ein `key: value` pro Zeile, sortiert,
Platzhalter `%{name}`), in die Binärdatei kompiliert; `build.rs` baut neu, wenn
sich eine Datei ändert. Ein fehlender Text fällt auf Englisch zurück, und ein
Unit-Test schlägt fehl, wenn einer Sprache ein Schlüssel fehlt, sie einen
unbekannten hat oder andere Platzhalter als Englisch verwendet. Sätze sind
ganze Schlüssel mit Platzhaltern, nie aus Stücken zusammengeklebt, weil sich die
Wortstellung zwischen Sprachen unterscheidet.

Die Sprache wird in den Einstellungen gewählt (oder folgt beim ersten Start der
Systemsprache) und wechselt sofort: Texte werden bei jedem Rendern gelesen, und
die wenigen Texte, die in Widgets gehalten werden (Platzhalter von Eingaben,
Einträge von Auswahllisten), setzt `Workspace::retranslate` neu. Glyphen für
Japanisch, Chinesisch und Arabisch kommen aus dem Schrift-Fallback der Plattform
(DirectWrite, Core Text, fontconfig) neben der eingebetteten Inter. Arabischer
Text wird von rechts nach links geformt und platziert; unter Windows braucht das
`vendor/gpui-pre-windows`, das DirectWrite-Backend von gpui 0.3.9 mit
korrigierter Platzierung der Glyphenfolgen (aus der `Cargo.toml` des Workspace
per Patch eingebunden; entfernen, sobald es upstream korrigiert ist). gpui kann
Text von rechts nach links nicht umbrechen: Es formt einen Absatz als eine
Zeile und schneidet ihn an x-Positionen der Glyphen, von denen es annimmt, dass
sie von links nach rechts wachsen. Daher geht Text, der umbrechen kann
(Hinweise, Tipps, Beschreibungen, Meldungen), durch `bidi::para`, das in RTL den
Text in Lesereihenfolge an Leerzeichen umbricht, jede mögliche Zeile misst und
jede Zeile ohne Umbruch zeichnet (`src/bidi/paragraph.rs`). In anderen Sprachen
ist es einfacher gpui-Text. gpui legt außerdem jede Zeile mit einer
Grundrichtung von links nach rechts an, daher hüllt `tr!` in einer RTL-Sprache
jeden Text in eine Einbettung von rechts nach links (U+202B…U+202C; DirectWrite
ignoriert die neueren Isolates). Ein Satz, der mit einem lateinischen Wort
beginnt oder endet („URL“, „OAuth 2.0“, „Ctrl+S“), liest sich dann trotzdem von
rechts nach links, und `i18n::display` tut dasselbe für zusammengesetzte Texte
wie die Statusleiste. Die Zeichen gelangen nie in Dateien: Gespeicherte Texte
verwenden `tr_plain!`. Fehlermeldungen aus den Engine-Crates bleiben englisch.

In einer Sprache, die von rechts nach links geschrieben wird (`Language::rtl`,
Arabisch), wird das Fenster gespiegelt: Die Seitenleiste wandert nach rechts,
der Antwortbereich links neben den Anfragebereich, die Fenstersteuerung nach
links, und Menüs, Reiterleisten, Formularzeilen und die Statusleiste laufen von
rechts, mit rechtsbündigem Text. GPUI hat keine Layoutrichtung, daher erledigt
das `src/bidi.rs`: ein Trait `Bidi` auf jedem gestylten Element (`dir_row`
kehrt eine Zeile um, `text_start` / `text_end`, die Abstände `ps` / `pe` und
`border_e_1` wählen die Seite nach Richtung, `ltr` heftet Text links fest),
`tab_bar` / `fill_tabs` für die `TabBar` von gpui-kit (Reiter in umgekehrter
Reihenfolge übergeben, Indizes zurück zugeordnet) und `bidi_title` für Dialoge
(in RTL wandert die Schließen-Schaltfläche links in die Titelzeile). Die Helfer
lesen die Sprache, während eine Ansicht rendert, sodass ein Sprachwechsel das
Fenster sofort spiegelt. Code- und Antwortbetrachter, die URL-Leiste,
Schlüssel-Wert-Tabellen (Parameter, Header, Formularfelder, Variablen) und
Antwort-Header bleiben in jeder Sprache von links nach rechts. Die Popup-Menüs
und Schaltflächen von gpui-kit behalten ihr eigenes inneres Layout von links
nach rechts (Tastenkürzel rechts, Icon vor der Beschriftung); Menüs öffnen sich
von ihrer Schaltfläche aus nach links.

Eine Sprache hinzufügen: `locales/en.yml` nach `locales/<code>.yml` kopieren,
die Werte übersetzen und eine Zeile zu `LANGUAGES` in `src/i18n.rs` hinzufügen.

Die Benutzerdokumentation liegt getrennt vom Plan, ein Ordner pro Sprache
(`Docs/<code>/`: Benutzerhandbuch, Architektur, technische Hinweise, Workflows,
Bilder), aufgelistet in `Docs/languages.toml`. Englisch ist die Quelle; jede
Übersetzung hält fest, aus welcher englischen Version sie entstanden ist, sodass
veraltete Übersetzungen auffallen.

## Thread-Modell (Desktop)

- GPUI besitzt den Haupt-Thread, der die Event-Schleife und das Rendern
  ausführt.
- Eine einzige Multi-Thread-**tokio-Runtime** wird einmal in einem
  Hintergrund-Thread gestartet. reqwest, tonic und tungstenite brauchen sie.
- Die Oberfläche schickt Arbeit an tokio und empfängt das Ergebnis in einer
  GPUI-Aufgabe (`cx.spawn_in`). Wird diese Aufgabe verworfen, bricht die Anfrage
  ab, sodass Esc und erneutes Senden sauber abbrechen.
- Nachdem eine Antwort angekommen ist, wird der Body auf dem
  Hintergrund-Executor von GPUI formatiert und indiziert, bevor die Oberfläche
  ihn sieht. Das Formatieren verwendet `apix_core::text::pretty_json`, einen
  Einrücker in einem Durchgang, der keinen Baum baut und Zahlen genau so
  belässt, wie sie gesendet wurden.

## Betrachter für große Antworten

Bodys ab 512 KB verwenden `large_view.rs` statt des Code-Editors:

- `apix_core::text::RowIndex` hält fest, wo jede Anzeigezeile beginnt, 8 Byte
  pro Zeile. Zeilen über 2 KB (minifiziertes JSON) werden auf mehrere Zeilen
  aufgeteilt.
- Die Scrollposition ist eine **Zeilennummer**, kein Pixel-Versatz. Zehn
  Millionen Zeilen zu 20 px wären etwa 200 Millionen px, weit jenseits des
  Bereichs, in dem `f32`-Pixel exakt sind. Der Betrachter layoutet nur die ~120
  Zeilen, die auf den Bildschirm passen. Er verarbeitet das Mausrad, seine eigene
  Scrollleiste und die Tastatur (↑ ↓ Bild↑ Bild↓, Strg+Pos1/Ende).
- Der Reiter Roh eines großen JSON-Bodys wird erst gebaut, wenn er geöffnet
  wird.
- Strg+F durchsucht den ganzen Text in einem Hintergrund-Thread (ein `regex`
  ohne Beachtung der Groß-/Kleinschreibung über die wörtliche Suchanfrage) und
  behält bis zu 100,000 Byte-Bereiche; `RowIndex::row_of` findet die Zeile eines
  Treffers per binärer Suche, und nur die Zeilen auf dem Bildschirm schlagen
  ihre Treffer nach, um sie hervorzuheben.

Kleinere Bodys stehen im Code-Editor, dessen eigenes Suchfeld eingeschaltet ist.
Der Filter über Formatiert (`workspace/filter_ui.rs`) führt `apix_core::filter`
außerhalb des UI-Threads aus, 250 ms nach dem letzten Tastendruck: JSONPath über
`serde_json_path` für einen Ausdruck, der mit `$` beginnt, sonst jq über `jaq`,
höchstens 10,000 jq-Ausgaben. Sein Ergebnis ist ein `Document` wie jeder Body,
sodass ein gefilterter großer Body im Editor landen kann und umgekehrt.

Gemessen unter Windows in einem Release-Build, für 100 MB minifiziertes JSON
von einem lokalen Server
(`cargo run -p apix-http --release --example big_json_server`): Die Antwort ist
etwa 0.33 s nach Enter auf dem Bildschirm. Das umfasst die Übertragung, das
Formatieren in 9.6 Millionen Zeilen, das Indizieren und den ersten Frame. Der
Speicher liegt mit geladenem Body bei etwa 407 MB, gegenüber 69 MB im Leerlauf.

## Leistungsbudget

| Metrik                              | Ziel              | Gemessen (Windows, Release) |
|-------------------------------------|-------------------|-----------------------------|
| Kaltstart bis zum ersten Frame      | < 150 ms          | 222 ms (`main` bis zum ersten Frame; etwa 150 ms davon lädt der Grafiktreiber) |
| RAM im Leerlauf                     | < 60 MB           | 88 MB Working Set, Beispielsammlung geöffnet (größtenteils der Grafiktreiber) |
| Release-Binärdatei                  | < 100 MB          | 54.4 MB |
| Latenz von Eingabe bis Zeichnen     | 1 Frame (≤ 8 ms bei 120 Hz) | nicht gemessen |
| Einen JSON-Body von 100 MB öffnen und scrollen | keine verlorenen Frames | 306 ms nach dem Senden auf dem Bildschirm (lokaler Server); Frame-Zeiten nicht gemessen |

Startzeit und Speicher im Leerlauf liegen heute über dem Budget; ihr Profiling
ist geplant. Die Zahlen stammen aus `scripts/measure.ps1` (siehe die
[technischen Hinweise](technical.md#leistung-messen)).

Das Release-Profil verwendet bereits Fat LTO, eine Codegen-Unit und entfernte
Symbole (siehe die `Cargo.toml` im Wurzelverzeichnis).
