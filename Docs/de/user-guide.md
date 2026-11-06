<!-- translated from en/user-guide.md sha256:302bcb1b0e41cc49 -->

# Apix-Benutzerhandbuch

Apix ist ein API-Client für den Desktop: Sie erstellen HTTP-Anfragen, senden sie
und lesen die Antworten. Anfragen werden als kleine Textdateien in einem Ordner
gespeichert, daher kann eine Sammlung neben Ihrem Code in git liegen. Dieses
Handbuch führt durch die App; die [Arbeitsabläufe](workflows.md) zeigen häufige
Aufgaben Schritt für Schritt, und die Kommandozeile `apix` führt dieselben
Anfragen in einem Terminal oder in CI aus.

![Das Apix-Fenster](../en/images/main.png)

## Inhalt

- [Apix installieren](#apix-installieren)
- [Eine Sammlung öffnen](#eine-sammlung-öffnen)
- [Das Fenster](#das-fenster)
- [Eine Anfrage senden](#eine-anfrage-senden)
- [Parameter und Header](#parameter-und-header)
- [Body](#body)
- [Authentifizierung](#authentifizierung)
- [Anfrage-Einstellungen](#anfrage-einstellungen)
- [Cookies](#cookies)
- [Variablen und Umgebungen](#variablen-und-umgebungen)
- [Die Antwort](#die-antwort)
- [Der Reiter Netzwerk](#der-reiter-netzwerk)
- [WebSocket und Server-Sent Events](#websocket-und-server-sent-events)
- [Socket.IO](#socketio)
- [MQTT](#mqtt)
- [SOAP](#soap)
- [gRPC](#grpc)
- [Skripte und Tests](#skripte-und-tests)
- [Runner](#runner)
- [Abläufe](#abläufe)
- [Lasttests](#lasttests)
- [Monitore](#monitore)
- [Mock-Server](#mock-server)
- [API-Dokumentation](#api-dokumentation)
- [Mitschnitt-Proxy](#mitschnitt-proxy)
- [Tabs](#tabs)
- [Anfragen ordnen](#anfragen-ordnen)
- [Projekte](#projekte)
- [Git](#git)
- [Verschlüsselte Sammlungen](#verschlüsselte-sammlungen)
- [Kopieren als und curl einfügen](#kopieren-als-und-curl-einfügen)
- [Import aus Postman](#import-aus-postman)
- [Import aus Insomnia, OpenAPI und HAR](#import-aus-insomnia-openapi-und-har)
- [Verlauf](#verlauf)
- [Antworten vergleichen](#antworten-vergleichen)
- [KI-Assistent](#ki-assistent)
- [Plugins](#plugins)
- [Befehlspalette](#befehlspalette)
- [Einrichtung](#einrichtung)
- [Einstellungen und Sprache](#einstellungen-und-sprache)
- [Tastenkürzel](#tastenkürzel)

## Apix installieren

Die Download-Seite, **https://11r5.github.io/Apix/**, bietet die neueste
Version für Ihr System an; alle Dateien liegen auch auf der
[Release-Seite](https://github.com/11R5/Apix/releases). Jede enthält die
Desktop-App und die Kommandozeile `apix`.

| System | Datei | So geht's |
|---|---|---|
| Windows 10/11 | `Apix-<version>-windows-x64-setup.exe` | Ausführen. Es installiert für Ihr Benutzerkonto ohne Administratorrechte (oder auf Wunsch für alle), fügt Apix dem Startmenü hinzu und kann `apix` in den PATH aufnehmen und `.apix`-Dateien öffnen. `…-windows-x64.zip` ist dasselbe ohne Installation. |
| macOS 11+ | `Apix-<version>-macos-universal.dmg` | Öffnen und Apix in „Programme“ ziehen; eine App für Apple silicon und Intel. Die Kommandozeile ist `/Applications/Apix.app/Contents/MacOS/apix`. Oder `brew install --cask 11r5/apix/apix`, das `apix` auch in den PATH legt. |
| Debian, Ubuntu, Mint, … | `apix_<version>_amd64.deb` | `sudo apt install ./apix_<version>_amd64.deb` |
| Arch, Manjaro, EndeavourOS, … | `apix-<version>-1-x86_64.pkg.tar.zst` | `sudo pacman -U ./apix-<version>-1-x86_64.pkg.tar.zst` oder `makepkg -si` mit dem `PKGBUILD` des Releases |
| Anderes Linux (glibc 2.35+) | `apix-<version>-linux-x64.tar.gz` | Entpacken und `./install.sh` ausführen (für Ihr Benutzerkonto; `--system` für alle, `--uninstall` zum Entfernen) |
| Jedes Linux, eine Datei | `Apix-<version>-x86_64.AppImage` | Mit `chmod +x` ausführbar machen und starten; `Apix-….AppImage cli …` startet die Kommandozeile |

Die Builds sind nicht codesigniert (Signieren kostet jedes Jahr Geld und ist
deshalb vorerst nicht geplant). Unter Windows kann SmartScreen das
Installationsprogramm aufhalten: **Weitere Informationen › Trotzdem ausführen**.
Unter macOS öffnen Sie Apix beim ersten Mal per Rechtsklick › **Öffnen** oder
führen `xattr -dr com.apple.quarantine /Applications/Apix.app` aus. Unter Linux
zeichnet Apix mit Vulkan, der Grafiktreiber muss also Vulkan unterstützen (Mesa
liefert das auf den meisten Systemen). `SHA256SUMS.txt` in jedem Release listet
die Prüfsummen der Dateien.

## Eine Sammlung öffnen

Eine **Sammlung** ist ein Ordner mit einer Datei `apix.toml` und Ihren Anfragen
(`.apix`-Dateien), auf Wunsch in Unterordnern. Öffnen Sie eine mit
**Datei › Öffnen…** (Strg+O), oder beginnen Sie eine neue mit
**Datei › Neue Sammlung…**. Wählen Sie einen Ordner, der noch keine Sammlung
ist, bietet Apix an, eine daraus zu machen.

Zuletzt geöffnete Sammlungen stehen unter **Datei › Zuletzt geöffnet**. Wird die
App mit einem Ordner gestartet (`apix-desktop path/to/collection`), öffnet sie
ihn direkt.

## Das Fenster

- **Seitenleiste (links):** die Ordner und Anfragen der Sammlung, gelesen aus
  den Dateien auf der Festplatte und aktualisiert, wenn sie sich ändern. Die
  Auswahl unter dem Namen der Sammlung bestimmt die aktive Umgebung.
- **Tabs (oben):** Jede geöffnete Anfrage bekommt einen Tab.
- **Anfragebereich:** Methode, URL und die Reiter Parameter, Header, Auth, Body
  und Einstellungen.
- **Antwortbereich (rechts):** Status, Zeit, Größe und der Inhalt der Antwort.
- **Statusleiste (unten):** die Sammlung, die Datei der geöffneten Anfrage und
  die nützlichsten Tastenkürzel.
- **Menüleiste:** Datei, Anfrage, Umgebung, Ansicht und Hilfe enthalten jeden
  Befehl.

## Eine Anfrage senden

1. Wählen Sie die Methode links in der URL-Leiste (oder drücken Sie Alt+1…7 für
   GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS).
2. Geben Sie die URL ein. `{{variables}}` sind überall erlaubt (siehe
   [Variablen](#variablen-und-umgebungen)).
3. Drücken Sie **Senden** oder Strg+Enter. Esc bricht eine noch laufende Anfrage
   ab.

Solange eine Umgebung aktiv ist, deren Name „prod“ enthält, fragt Apix vor dem
Senden von DELETE, PUT oder PATCH nach und zeigt die vollständige URL, die
aufgerufen würde. **Einstellungen › In Produktion nachfragen** schaltet das ab.

**Neue Anfrage** (Strg+N) öffnet eine GET-Anfrage; das Menü **+** der
Seitenleiste bietet jede Methode an, und POST, PUT und PATCH beginnen mit einem
JSON-Body. Speichern mit Strg+S. Ein Punkt auf dem Tab und in der Seitenleiste
markiert ungespeicherte Änderungen.

## Parameter und Header

**Parameter** sind der Query-String der URL, **Header** werden so gesendet, wie
sie sind. Entfernen Sie das Häkchen einer Zeile, um sie in der Datei zu
behalten, ohne sie zu senden. Eine Zeile ohne Schlüssel wird ignoriert.
`Content-Type` wird aus dem Body-Typ gesetzt, sofern Sie keinen eigenen
hinzufügen. `{{variables}}` in Schlüsseln und Werten erscheinen als Chips, wie
in der URL-Leiste; fahren Sie mit der Maus darüber, um den Wert zu sehen.

URL-Leiste und Parametertabelle bleiben im Gleichklang: `?page=2&sort=name` in
der URL füllt die Tabelle, und Bearbeiten, Hinzufügen, Abwählen oder Löschen
einer Zeile schreibt die URL neu. Werte erscheinen dekodiert (`%20` wird zu
einem Leerzeichen) und werden beim Senden kodiert. Die Datei speichert die URL
ohne ihre Query und die Zeilen als `[[query]]`.

**Pfadvariablen.** Ein Pfadsegment der Form `:name`, wie in
`{{base}}/users/:id/posts`, bekommt eine Zeile unter **Pfadvariablen** im Reiter
Parameter. Ihr Wert ersetzt beim Senden das Segment, kodiert als ein Segment
(`a b/c` wird zu `a%20b%2Fc`), und `{{variables}}` funktionieren darin. Eine
leer gelassene Variable wird so gesendet, wie sie geschrieben ist. Die Werte
stehen in der Datei als `[[path]]`, und Postman-Sammlungen bringen ihre mit
(und bekommen sie beim Export zurück).

![Parameter und Pfadvariablen](../en/images/path-variables.png)

**Header-Vorschläge.** Während Sie den Namen eines Headers eingeben, erscheinen
unter der Zeile die üblichen Header-Namen, die dazu passen (zuerst die, die mit
dem Getippten beginnen, und keiner, der schon in der Tabelle steht); der Wert
bekommt die üblichen Werte dieses Headers, etwa `application/json` für
`Content-Type`. Enter übernimmt den ersten, ein Klick einen beliebigen, und nach
einem Namen ist der Wert dran.

![Vorgeschlagene Header-Namen beim Tippen](../en/images/header-hints.png)

## Body

Der Reiter Body bietet:

| Typ | Was gesendet wird |
|---|---|
| Keiner | Kein Body. |
| JSON, Text, XML, HTML | Der Text, wie er geschrieben ist, mit Syntaxhervorhebung (`application/json`, `text/plain`, `application/xml`, `text/html`, sofern ein Header nichts anderes sagt). |
| Formular | Schlüssel-Wert-Zeilen als `application/x-www-form-urlencoded`. |
| Multipart | `multipart/form-data`. Jedes Feld ist **Text** oder **Datei**; die Ordner-Schaltfläche wählt eine oder mehrere Dateien. Das Typfeld daneben setzt den eigenen Content-Type des Felds (etwa `application/json` oder `image/png`); leer lässt Apix wählen. |
| Binär | Die Bytes einer Datei, als `application/octet-stream`, sofern ein Header nichts anderes sagt. Apix zeigt die Dateigröße oder „Datei nicht gefunden“. |
| GraphQL | Ein Query-Editor und ein JSON-Editor für Variablen; gesendet als JSON `{"query", "variables"}`. |

![Multipart-Body](../en/images/body.png)

Dateipfade innerhalb der Sammlung werden relativ zur Anfragedatei gespeichert,
damit die Sammlung auch auf einem anderen Rechner funktioniert.

### GraphQL-Schema und Vervollständigung

**Schema abrufen** über der Query fragt den Server nach seinem Schema (mit der
üblichen Introspection-Query), mit URL, Headern, Authentifizierung und Umgebung
der Anfrage, sodass auch ein Schema hinter einem Token funktioniert. Danach:

- Beim Tippen in der Query wird vorgeschlagen, was an die Stelle des Cursors
  passt: die Felder des gerade ausgewählten Typs (mit ihren Typen), die
  Argumente eines Felds nach `(`, Enum-Werte und `true`/`false` nach `:`, die
  Felder eines Input-Objekts, `... on Type`-Fragmente für Unions und Interfaces
  sowie Typnamen nach `on` oder in `$variable: Type`. Veraltete Felder kommen
  zuletzt. Strg+Leertaste öffnet die Vorschläge ohne Tippen.
- Fahren Sie mit der Maus über ein Feld, ein Argument oder einen Enum-Wert, um
  Typ und Beschreibung zu sehen.
- Im Variablen-Editor werden die Variablen, die die Query deklariert
  (`$role: Role`), als Schlüssel vorgeschlagen, dann die Felder von
  Input-Objekten, Enum-Werte (als JSON-Strings) und `true`/`false` als Werte.
- **Schema anzeigen** stellt das ganze Schema als SDL neben die Query.

![Vervollständigung aus dem Schema](../en/images/graphql.png)

Apix speichert das Schema jedes Endpunkts (im Konfigurationsordner, unter
`graphql/`) und lädt es wieder, wenn Sie eine Anfrage an diese URL öffnen;
**Schema aktualisieren** ist also nur nötig, wenn sich die API ändert. Ein
Server mit abgeschalteter Introspection antwortet mit einem Fehler, der an
Stelle des Schemastatus angezeigt wird.

Auf der Kommandozeile gibt `apix schema request.apix` das Schema als SDL aus
(`--json` gibt die Antwort des Servers unverändert aus).

## Authentifizierung

Der Reiter Auth legt fest, wie sich die Anfrage ausweist:

| Typ | Hinweise |
|---|---|
| Vom übergeordneten Element erben | Die Vorgabe: die Authentifizierung des nächsten Ordners, der nächsten Sammlung oder des Projekts verwenden, die eine festlegen. Der Reiter zeigt, welche das ist und woher sie kommt. |
| Keine Authentifizierung | Nichts senden, auch wenn ein übergeordnetes Element eine Authentifizierung festlegt. |
| Bearer-Token | `Authorization: Bearer <token>`. |
| Basic Auth | Benutzername und Passwort. |
| API-Schlüssel | Ein Header oder Query-Parameter Ihrer Wahl. |
| Digest Auth | Apix beantwortet die Digest-Challenge des Servers (MD5 oder SHA-256). |
| OAuth 1.0 | HMAC-SHA1, HMAC-SHA256 oder PLAINTEXT; im Header oder im Query-String; bei jedem Senden eine neue Nonce und ein neuer Zeitstempel. |
| OAuth 2.0 | Client Credentials, Passwort oder Authorization Code mit PKCE. Apix holt das Token selbst und sendet es als Bearer-Token. |
| AWS Signature | AWS Signature Version 4 mit Access Key, Secret Key, optionalem Session-Token, Region und Dienst. |

![OAuth 2.0 im Reiter Auth](../en/images/auth.png)

**OAuth-2.0-Tokens.** Das erste Senden holt ein Token; spätere Sendungen
verwenden es weiter und erneuern es mit dem Refresh-Token, wenn es abläuft.
**Neues Token holen** holt sofort eines, **Verwerfen** wirft es weg. Tokens
werden nur im Arbeitsspeicher gehalten und nie auf die Festplatte geschrieben.
Beim Authorization-Code-Grant öffnet Apix die Anmeldeseite in Ihrem Browser und
empfängt die Antwort auf `http://127.0.0.1` (ein freier Port oder die
**Redirect-URL**, die Sie für den Client registriert haben).

**Gemeinsame Authentifizierung.** Klicken Sie mit der rechten Maustaste auf
einen Ordner oder eine Sammlung und wählen Sie **Authentifizierung…**, um die
Authentifizierung einmal für alles darin festzulegen;
**Umgebung › Projekt-Authentifizierung…** tut dasselbe für ein ganzes Projekt.
Anfragen mit *Vom übergeordneten Element erben* verwenden sie.

## Anfrage-Einstellungen

Der Reiter Einstellungen steuert, wie die Anfrage übertragen wird:

| Einstellung | Vorgabe |
|---|---|
| Timeout (Sekunden) | keiner – eine Anfrage läuft, bis sie fertig ist oder Sie sie abbrechen; `0` heißt ebenfalls kein Timeout |
| Weiterleitungen folgen | an |
| Max. Weiterleitungen | 10 |
| TLS-Zertifikat prüfen | an – schalten Sie es nur für Testserver ab, denen Sie vertrauen |
| Proxy | aus der Umgebung (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`); *Aus* ignoriert diese; eine URL nennt einen ausdrücklich |
| Client-Zertifikat | keines – eine PEM-Datei, mit dem Schlüssel in derselben oder einer eigenen Datei |
| Vertrauenswürdige CA | nur die üblichen Stammzertifikate – eine PEM-Datei fügt eigene Zertifizierungsstellen hinzu (eine Firmen-CA oder eine selbstsignierte), damit der Server trotzdem geprüft wird |
| Cookies senden | an |

Jedes leer gelassene Feld (oder auf *Erben*) greift auf den nächsten Ordner, die
nächste Sammlung oder das Projekt zurück, die es setzen, und zuletzt auf die
Vorgaben oben – dieselbe Kette wie bei der Authentifizierung. Klicken Sie mit
der rechten Maustaste auf einen Ordner oder eine Sammlung und wählen Sie
**Anfrage-Einstellungen…**, um sie einmal für alles darin festzulegen;
**Umgebung › Anfrage-Einstellungen des Projekts…** tut dasselbe für ein ganzes
Projekt. Die Zeile unter dem Formular zeigt, was die Anfrage am Ende verwendet.
`{{variables}}` funktionieren in den Proxy- und Zertifikatsfeldern, und relative
Zertifikatspfade werden relativ zu der Datei aufgelöst, die sie setzt.

In einer Anfragedatei (und in `folder.toml`, `apix.toml` und `apix-project.toml`)
stehen die Einstellungen in einer Tabelle `[settings]`:

```toml
[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""          # "" = kein Proxy, auch wenn die Umgebung einen setzt
client_cert = "certs/client.pem"
ca_cert = "certs/company-ca.pem"
```

**Kopieren als** schreibt die passenden curl-Optionen (`-m`, `-L --max-redirs`,
`-k`, `-x`, `--cert`, `--key`, `--cacert`), und das Einfügen eines curl-Befehls
mit diesen Optionen füllt den Reiter. `apix send` und `apix run` haben
`--timeout`, `--insecure`, `--proxy` und `--cacert`, die die Datei übersteuern.
Diese Einstellungen gelten auch für gRPC-Anfragen. Die Optionen von Postman pro
Anfrage (`protocolProfileBehavior`) werden importiert.

## Cookies

Apix führt einen Cookie-Speicher: `Set-Cookie`-Antworten werden automatisch
gespeichert und mit späteren Anfragen an dieselbe Website gesendet, auch über
Neustarts hinweg (`cookies.json` neben den App-Einstellungen; Sitzungs-Cookies
enden mit der App). **Anfrage › Cookies…** listet sie auf, löscht einzelne
Cookies oder leert den Speicher. **Cookies senden** im Reiter Einstellungen
schaltet den Speicher für eine Anfrage, einen Ordner, eine Sammlung oder ein
Projekt ab. Die CLI beginnt bei jedem Lauf mit einem leeren Speicher.

## Variablen und Umgebungen

Schreiben Sie `{{name}}` in eine URL, einen Header, einen Body oder ein
Authentifizierungsfeld, und Apix setzt beim Senden den Wert ein. In der
URL-Leiste ist jede Variable ein farbiger Chip: Fahren Sie mit der Maus darüber,
um den Wert und seine Herkunft zu sehen; ein roter Chip ist nirgends definiert.

Werte kommen in Schichten, die spätere gewinnt:

1. **Projekt**variablen (von allen Sammlungen eines Projekts geteilt)
2. **Sammlungs**variablen (`apix.toml`)
3. Die aktive **Umgebung** (`environments/<name>.toml`)

Öffnen Sie **Umgebung › Variablen und Umgebungen…** (Strg+E), um sie zu
bearbeiten. Setzen Sie bei Tokens und Passwörtern das Häkchen **Geheim**:
geheime Werte werden in `environments/<name>.local.toml` gespeichert, das Apix
zu `.gitignore` hinzufügt, sodass sie nie committet werden.

Um sie auch von der Festplatte fernzuhalten, wählen Sie
**Einstellungen › Geheime Umgebungswerte › Schlüsselbund des Systems**. Die
Werte gehen dann in den Anmeldeinformationsspeicher des Systems
(Anmeldeinformationsverwaltung unter Windows, Schlüsselbund unter macOS, Secret
Service unter Linux), und `<name>.local.toml` behält nur die ID dieses Eintrags,
sodass Umbenennen der Umgebung oder Verschieben des Ordners weiter funktioniert.
Die Kommandozeile `apix` liest sie auf dieselbe Weise. Ein Wechsel der
Einstellung verschiebt die geheimen Werte der geöffneten Sammlung oder des
geöffneten Projekts sofort; andere werden beim nächsten Speichern verschoben.
Geheime Werte einer verschlüsselten Sammlung bleiben immer in ihr.

![Der Dialog Variablen](../en/images/variables.png)

**Dynamische Variablen** bekommen bei jeder Verwendung einen neuen Wert:
`{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`,
`{{$randomEmail}}` und weitere; die Seite **Dynamische Variablen** im Dialog
Variablen listet sie alle mit einem Beispielwert auf.

## Die Antwort

Der Antwortbereich zeigt den Status, die Zeit bis zum ersten Byte und insgesamt
sowie die Größe. **Formatiert** formatiert JSON (Blöcke lassen sich mit den
Pfeilen einklappen) und rückt XML und HTML ein und färbt sie; **Roh** zeigt den
Inhalt so, wie er ankam, **Header** listet die Antwort-Header und **Hex** zeigt
die Bytes. Sehr große Inhalte (zig Megabyte) öffnen sich in einer schnellen
Ansicht, die nur die sichtbaren Zeilen zeichnet.

Apix erkennt die Art des Inhalts an seinem `Content-Type` oder an den Bytes,
wenn der Typ nichts Brauchbares sagt. Bilder (PNG, JPEG, GIF, WebP, BMP, SVG)
werden in **Formatiert** angezeigt; anderer binärer Inhalt (ein PDF, ein Zip…)
bekommt eine kurze Zusammenfassung statt unlesbarem Text. HTML, das optionale
schließende Tags weglässt, wird so gezeigt, wie es kam, statt neu formatiert.

Die Schaltfläche **↓** am Ende der Statuszeile speichert den Inhalt Byte für
Byte in eine Datei. Der vorgeschlagene Name stammt aus dem
`Content-Disposition` des Servers oder aus der URL, mit einer Endung, die zum
Inhalt passt.

**Suche.** Strg+F im Inhalt oder die Lupe in der Statuszeile durchsucht
**Formatiert**, **Roh** und **Hex**, ohne auf Groß- und Kleinschreibung zu
achten. Enter (oder F3) springt zum nächsten Treffer und Shift+Enter
(Shift+F3) zurück; der Zähler zeigt, wo Sie sind. Die schnelle Ansicht für sehr
große Inhalte durchsucht alles im Hintergrund und hört bei 100.000 Treffern auf
zu zählen.

**Filter.** Eine JSON-Antwort hat über **Formatiert** ein Filterfeld. Ein
Ausdruck, der mit `$` beginnt, ist JSONPath (RFC 9535), alles andere ist jq,
daher funktionieren beide:

```text
$.users[?@.active].email
.users[] | select(.active) | {name, email}
```

![Eine mit JSONPath gefilterte JSON-Antwort](../en/images/response-filter.png)

Das Ergebnis ersetzt den Inhalt in **Formatiert**, solange das Feld einen
Ausdruck enthält (der Zähler daneben nennt die Anzahl der Werte); **Roh** und
**Hex** zeigen weiterhin den ganzen Inhalt, und ✕ holt ihn zurück. JSONPath
liefert eine Liste seiner Treffer; jq liefert seine Ausgaben nacheinander. Der
Filter bleibt für die nächste Antwort derselben Anfrage bestehen, sie lässt sich
also immer wieder senden, während man einen Wert beobachtet.
`apix send --filter '<expression>'` gibt nur aus, was der Filter herausgreift.

**Tabelle und Diagramm.** **Text**, **Tabelle** und **Diagramm** am Ende der
Filterzeile zeigen eine JSON-Antwort (oder das, was der Filter herausgegriffen
hat) als Text, als Tabelle ihrer Datensätze oder als Balkendiagramm. Die
Datensätze sind das Array im Body oder das längste Array von Objekten darin
(`$.items`, `$.data`); jeder Schlüssel ist eine Spalte, verschachtelte Objekte
erscheinen als kompaktes JSON. Das Diagramm zeichnet eine Zahlenspalte
(**Wert**) gegen eine Spalte, die die Zeilen benennt (**Beschriftung**); beide
sind wählbar. Bodies über 8 MB werden nicht zur Tabelle: Filtern Sie sie zuerst.
`apix send --table` gibt dieselbe Tabelle in einem Terminal aus (mit
`--filter` von dem, was er herausgreift). Vorlagen von Postmans
`pm.visualizer.set` werden nicht gezeichnet, da Apix keine Web-Ansicht hat;
Skripte, die es aufrufen, bekommen einen Hinweis in der Konsole.

![Die Tabellenansicht einer gefilterten Antwort](../en/images/visual-table.png)

![Die Diagrammansicht](../en/images/visual-chart.png)

**Beispiele.** Die Ablage-Schaltfläche in der Statuszeile behält die Antwort
als Beispiel der Anfrage: ihren Status, ihre Header und ihren Body sowie die
Anfrage, von der sie stammt, so wie sie geschrieben ist (`{{variables}}`
bleiben, damit kein Token in der Datei landet). Cookies, die der Server setzt,
werden weggelassen, und Bodies über 2 MB werden abgelehnt. Der Reiter
**Beispiele** der Anfrage listet sie auf: Benennen Sie eines direkt dort um, ▷
zeigt es im Antwortbereich (markiert mit „Beispiel: …“), und der Papierkorb
entfernt es. Sie gehören zur Anfrage, daher schreibt Strg+S sie als
`[[example]]` in ihre Datei, und die gespeicherten Antworten von Postman kommen
beim Import als Beispiele herein und gehen beim Export wieder hinaus.
`apix send --save-example '<name>'` behält die Antwort von der Kommandozeile
aus.

![Ein Beispiel im Antwortbereich](../en/images/examples.png)

Ist der Bereich zu schmal für alle Reiter der Anfrage, listet der Pfeil am Ende
der Reiterleiste sie alle auf.

## Der Reiter Netzwerk

Der Reiter **Netzwerk** erklärt, wohin die Zeit ging und wie die Verbindung
aussah:

- ein Zeitbalken, aufgeteilt in DNS, TCP-Verbindung, TLS-Handshake, Warten und
  Herunterladen
- die DNS-Antworten, die Server- und die lokale Adresse, die
  Netzwerkschnittstelle (mit einem Hinweis, wenn ein VPN im Spiel ist), das
  Gateway, die DNS-Server und der Proxy
- jede Weiterleitung
- ein komprimierter Body: seine Kodierung, die Größe bei der Übertragung und
  entpackt
- die Verschlüsselung: TLS-Version, Cipher-Suite, Schlüsselaustausch, ALPN
  (`h2`, wenn der Server HTTP/2 spricht), der gesendete Servername (SNI) und
  wie lange der Handshake dauerte
- die Zertifikatskette, wie der Server sie gesendet hat: für jedes Zertifikat,
  für wen es ausgestellt ist und von wem, Gültigkeit und verbleibende Tage, der
  Schlüssel; für das Zertifikat des Servers außerdem seine Namen und sein
  SHA-256-Fingerabdruck

![Der Reiter Netzwerk](../en/images/network.png)

![Verschlüsselung und Zertifikatskette](../en/images/network-tls.png)

All das stammt aus der eigenen Verbindung der Anfrage, nicht aus einer zweiten,
die nur zum Nachsehen aufgebaut wird: Jedes Senden öffnet eine neue Verbindung,
und Apix zeichnet auf, worauf sich dieser Handshake geeinigt hat. Apix bittet um
komprimierte Antworten (`gzip`, `deflate`, `br`), sofern die Anfrage kein
eigenes `Accept-Encoding` setzt, und zeigt die Header `Content-Encoding` und
`Content-Length` so, wie der Server sie gesendet hat.

### Route und Karte

**Route verfolgen und auf einer Karte zeigen…** unter der Adresse des Servers
(oder **Ansicht › Route und Karte…**) listet die Router zwischen diesem Computer
und dem Server mit der Zeit bis zu jedem auf und zeichnet sie auf eine
Weltkarte. Die Hops stammen aus dem Werkzeug des Systems (`tracert` unter
Windows, `traceroute` oder `tracepath` anderswo), daher sind keine
Administratorrechte nötig; ein Router, der nicht antwortet, erscheint als `*`.
Adressen in Ihrem eigenen Netz werden als solche markiert.

Wo jeder Router steht, kommt aus einer IP-Standortdatenbank auf diesem
Computer; nichts wird online abgefragt. Ohne eine funktioniert die Liste
trotzdem, und die Karte bleibt leer: **DB-IP Lite herunterladen** holt die
kostenlose Datenbank von DB-IP (etwa 130 MB, CC BY 4.0) nach `<config>/Apix/geo`
(die Einrichtung und `apix geo download` tun dasselbe), oder legen Sie dort eine
beliebige `.mmdb`-Städtedatenbank ab (DB-IP Lite, MaxMind GeoLite2 City). Solche
Datenbanken verorten eine Adresse bestenfalls in einer Stadt und liegen bei
Backbone-Routern manchmal falsch. In einem Terminal gibt
`apix trace <url or host>` dieselbe Liste aus.

![Die Route auf einer Karte](../en/images/route.png)

Der Screenshot verwendet eine kleine Demonstrationsdatenbank
(`cargo run -p apix-route --example demo_geo`), keine echten Abfragen.

## WebSocket und Server-Sent Events

**WebSocket.** Eine Anfrage, deren URL mit `ws://` oder `wss://` beginnt, ist
ein WebSocket: Aus **Senden** wird **Verbinden**, und aus dem Reiter Body wird
**Nachrichten**. Der Handshake verwendet Parameter, Header und
Authentifizierung der Anfrage (Bearer, Basic, API-Schlüssel, OAuth 2.0 …), wie
jede Anfrage.

![Eine WebSocket-Verbindung und ihr Reiter Nachrichten](../en/images/websocket.png)

- Geben Sie eine Nachricht ein und drücken Sie **Senden** (oder Strg+Enter).
  `{{variables}}` werden beim Senden eingesetzt. Ohne Verbindung verbindet Apix
  zuerst und sendet sie, sobald die Verbindung steht.
- **Nachricht speichern** behält sie als `[[message]]` in der Anfragedatei;
  Strg+S speichert die Datei. Klicken Sie auf eine gespeicherte Nachricht, um
  sie zu bearbeiten, oder auf ▷, um sie unverändert zu senden.
- Der Antwortbereich ist ein Protokoll: ↑ gesendet, ↓ empfangen, mit der Zeit
  seit dem Verbinden, dazu wann die Verbindung aufgebaut und wie sie
  geschlossen wurde (Code und Grund). Es folgt neuen Nachrichten, außer Sie
  scrollen nach oben.
- **Trennen** (oder Esc) schließt die Verbindung; das Protokoll bleibt, bis Sie
  erneut verbinden. Eine Verbindung läuft weiter, während ihr Tab im
  Hintergrund ist.
- **Binäre Nachrichten:** Wählen Sie **Base64** oder **Hex** neben den
  Schaltflächen und schreiben Sie die Bytes in dieser Form (`01 02 ff`,
  Leerzeichen und `0x` erlaubt); eine Nachricht, die sich nicht als Bytes lesen
  lässt, kann nicht gesendet werden. Gespeicherte Nachrichten behalten ihr
  Format. Binäre Nachrichten erscheinen im Protokoll mit ihrer Größe und ihren
  ersten Bytes.
- **Subprotokolle** (oben im Reiter Nachrichten, durch Kommas getrennt) werden
  im Handshake angeboten; das Protokoll nennt das, welches der Server gewählt
  hat. Ein Server, der keines davon wählt, wird abgelehnt, wie es Browser tun.
- **Ping alle (s)** darunter sendet in diesem Abstand einen Ping, solange die
  Verbindung steht (`[websocket] ping_interval` in der Datei). Jede Antwort
  erscheint im Protokoll als „Pong nach 2 ms“; ein Ping, der noch unbeantwortet
  ist, wenn der nächste fällig wird, schließt die Verbindung mit einem Fehler,
  sodass eine still gestorbene Verbindung als tot erscheint. Pings des Servers
  werden immer beantwortet. `apix send --ping 30` tut dasselbe (`--ping 0`
  schaltet ein gespeichertes Intervall ab) und endet bei einer toten Verbindung
  mit einem Exit-Code ungleich null.
- `wss://` folgt den Anfrage-Einstellungen: eine vertrauenswürdige CA, ein
  Client-Zertifikat oder abgeschaltetes „TLS-Zertifikat prüfen“.

**Server-Sent Events.** Ist eine Antwort `text/event-stream`, erscheinen ihre
Ereignisse, sobald sie eintreffen, mit Zeit, Ereignistyp und ID sowie den
Daten. **Stoppen** (oder Esc) beendet den Stream und behält, was gekommen ist;
ein Stream, den der Server beendet, zeigt dieselbe Liste im Reiter Formatiert,
mit Roh, Header, Netzwerk und Hex wie bei jeder Antwort. Senden Sie
`Accept: text/event-stream`, wenn der Server es braucht. Ein Timeout in den
Einstellungen der Anfrage beendet auch einen Stream; lassen Sie ihn also für
Streams, die offen bleiben, leer.

- Bei mehr als einem Ereignistyp erscheinen die Typen über der Liste, mit der
  Anzahl je Typ; klicken Sie auf einen, um nur diese zu zeigen, auf **Alle** für
  jedes Ereignis.
- **Neu verbinden** (in der Live-Ansicht) nimmt den Stream wieder auf, wenn er
  endet oder die Verbindung abbricht: nach der `retry:`-Zeit des Servers (3 s,
  wenn er keine angegeben hat), mit `Last-Event-ID` auf die zuletzt gesehene ID
  gesetzt, wie ein Browser es tut. Die Liste vermerkt jede neue Verbindung. Eine
  Antwort, die kein Ereignisstrom ist, beendet das, ebenso Esc.

Auf der Kommandozeile gibt `apix send` Ereignisse aus, sobald sie kommen
(`--event TYPE`, wiederholbar, gibt nur diese aus; `--reconnect` verbindet immer
wieder neu, bis Strg+C). Bei einem WebSocket verbindet es sich, sendet die
gespeicherten Nachrichten (oder `--message`-Texte, gelesen als
`--format text|base64|hex`), gibt beide Richtungen aus und schließt nach
`--listen` Sekunden (standardmäßig 3, `0` wartet auf den Server). `apix run`
überspringt WebSocket-Anfragen.

## Socket.IO

Eine Socket.IO-Anfrage spricht über WebSocket mit Servern von Socket.IO 3 und 4.
Öffnen Sie den Reiter **Nachrichten** einer Anfrage (geben Sie ihr eine
`ws://`-URL, damit der Reiter erscheint) und wählen Sie oben **Socket.IO**; die
URL darf dann `http://`, `https://`, `ws://` oder `wss://` sein, wie bei einem
Socket.IO-Client. Die Anfragedatei bekommt eine Tabelle `[socketio]`, und eine
Socket.IO-Anfrage öffnet sich mit ihrem Reiter Nachrichten.

![Eine Socket.IO-Verbindung: Ereignisse, Acks und ein binärer Anhang](../en/images/socketio.png)

- **Namespace:** der, dem beigetreten wird, etwa `/chat`. Leer gelassen ist es
  der Pfad der URL (`http://localhost:3000/chat` tritt `/chat` bei) oder `/`.
  **Pfad** ist, wo der Server lauscht, `/socket.io/`, sofern der Server ihn
  nicht verlegt hat. Eine aus dem Netzwerk-Reiter des Browsers kopierte Adresse
  (`…/socket.io/?EIO=4&transport=websocket`) funktioniert unverändert.
- **Auth:** ein JSON-Objekt, das beim Beitreten gesendet wird (die Option
  `auth` des Clients), mit eingesetzten `{{variables}}`. Lehnt der Server ab,
  wird der angegebene Grund als Fehler angezeigt.
- **Ereignis** nennt, als was eine Nachricht gesendet wird (`message`, wenn
  leer). Der Text ist ein Argument: JSON so, wie es ist, anderer Text als
  String, und gar nichts, wenn er leer ist. **Base64** und **Hex** senden die
  Bytes als binären Anhang. **Bestätigung (Ack) anfordern** bittet den Server um
  eine Bestätigung. Gespeicherte Nachrichten behalten Ereignis und Ack.
- Das Protokoll zeigt jedes Ereignis mit seinem Namen: ↑ gesendet (mit `ack 0`,
  `ack 1` …, wenn ein Ack angefordert wurde), ↓ empfangen (mit
  „wartet auf ack 9“, wenn der Server eines anfordert) und ✓ für die Acks des
  Servers mit seiner Antwort. Binäre Anhänge erscheinen mit ihrer Größe und
  ihren ersten Bytes. Header, Parameter und Authentifizierung des Handshakes
  sowie die TLS-Einstellungen funktionieren wie bei einem WebSocket.
- Apix beantwortet die Pings des Servers. Lässt der Server den Client den
  Namespace verlassen, vermerkt das Protokoll das und die Verbindung schließt
  sich; **Trennen** (oder Esc) verlässt den Namespace und schließt.

`apix send` tritt bei, sendet die gespeicherten Nachrichten und gibt Ereignisse,
Acks und binäre Anhänge aus, sobald sie kommen; `--message TEXT` (wiederholbar)
sendet stattdessen Texte, als das Ereignis, das `--event NAME` angibt
(standardmäßig `message`). `apix run` überspringt Socket.IO-Anfragen. Empfangene
Anhänge werden als `{"$binary": "<base64>"}` geschrieben. Noch nicht: Server
mit Socket.IO 2 (Engine.IO 3), der Long-Polling-Transport und das Beantworten
eines Acks, das der Server anfordert.

## MQTT

Eine Anfrage, deren URL mit `mqtt://` (oder `mqtts://` für TLS) beginnt,
verbindet sich mit einem MQTT-3.1.1-Broker: URL-Leiste und Seitenleiste zeigen
**MQTT**, aus **Senden** wird **Verbinden**, und aus dem Reiter Body wird
**Nachrichten**. Der Port ist 1883, mit TLS 8883, sofern die URL keinen nennt.

![Eine MQTT-Verbindung: Abonnements, veröffentlichte und empfangene Nachrichten](../en/images/mqtt.png)

- **Anmeldung:** Benutzername und Passwort aus dem Typ Basic des Reiters Auth
  oder aus der URL (`mqtt://user:password@host`). Andere
  Authentifizierungstypen gelten für MQTT nicht. Lehnt der Broker ab, sagt er,
  warum. Ein Passwort in der URL lässt Apix bei seinen Ausgaben weg.
- **Client-ID** ist bei jeder Verbindung neu, wenn sie leer bleibt. **Sitzung
  behalten** bittet den Broker, die Sitzung zwischen Verbindungen zu behalten,
  was eine Client-ID braucht; das Protokoll sagt, wenn der Broker noch eine
  hatte. **Keep-Alive** ist die Zahl der Sekunden zwischen Pings im Leerlauf
  (standardmäßig 60, 0 für keine).
- **Abonnements** werden beim Verbinden angelegt, jedes mit seiner
  Dienstgüte (QoS); `+` und `#` funktionieren in Topics. Solange die Verbindung
  steht, abonniert ▷ eine Zeile sofort, und ✕ beendet das Abonnement. Das
  Protokoll vermerkt, was der Broker angenommen oder abgelehnt hat.
- Zum Veröffentlichen füllen Sie das **Topic** aus, wählen die Dienstgüte,
  setzen das Häkchen bei **Retain**, wenn der Broker die Nachricht für spätere
  Abonnenten behalten soll, und drücken **Senden** (oder Strg+Enter).
  `{{variables}}` werden eingesetzt. **Base64** und **Hex** senden Bytes. Eine
  leere Nachricht ist erlaubt: Mit Retain löscht sie, was der Broker behalten
  hat. **Nachricht speichern** behält sie in der Anfragedatei.
- Das Protokoll zeigt jede Nachricht unter ihrem Topic: ↑ veröffentlicht, ↓
  empfangen, mit Dienstgüte und Retain-Flag, und ✓, wenn der Broker eine
  Nachricht mit QoS 1 oder 2 bestätigt hat. Nutzdaten, die kein Text sind,
  erscheinen mit ihrer Größe und ihren ersten Bytes.
- `mqtts://` folgt den Anfrage-Einstellungen: eine vertrauenswürdige CA, ein
  Client-Zertifikat oder abgeschaltetes „TLS-Zertifikat prüfen“. Der Timeout
  der Anfrage begrenzt den Verbindungsaufbau (10 s ohne einen).

`apix send` verbindet sich, abonniert, veröffentlicht die gespeicherten
Nachrichten und gibt aus, was in beide Richtungen geht, und schließt nach
`--listen` Sekunden (standardmäßig 3, `0` wartet auf den Broker).
`--subscribe TOPIC` (wiederholbar) fügt Abonnements hinzu; `--message TEXT` mit
`--topic TOPIC` veröffentlicht statt der gespeicherten Nachrichten; `--qos` und
`--retain` gelten für diese. Der Exit-Code ist 1, wenn der Broker ein
Abonnement abgelehnt oder aufgelegt hat, ohne eines zu beantworten oder eine
Nachricht zu bestätigen. `apix run` überspringt MQTT-Anfragen. Noch nicht:
MQTT 5, ein Last Will und MQTT über WebSocket.

## SOAP

Ein SOAP-Aufruf ist ein XML-Umschlag, der mit POST gesendet wird; eine
SOAP-Anfrage ist also eine gewöhnliche Anfrage: Skripte, Tests, Läufe und der
Verlauf funktionieren mit ihr wie mit jeder anderen.

**Aus einer WSDL.** Importieren Sie eine WSDL-1.1-Datei mit
**Datei › Importieren…** oder ihre Adresse mit **Datei › Von URL importieren…**
(`http://host/service?wsdl`). Sie bekommen eine Sammlung mit einer Anfrage pro
Operation und einem Ordner pro Port, wenn es mehrere gibt, typischerweise einen
für SOAP 1.1 und einen für 1.2. Die Adresse des Ports ist die Variable `baseUrl`
(eine Variable pro Port, wenn sie sich unterscheiden), und Dokumente, die die
WSDL importiert, werden ebenfalls gelesen. Was nicht gelesen werden konnte und
Ports, die kein SOAP sind, werden am Ende des Imports aufgelistet.

![Eine SOAP-Anfrage aus einer WSDL, mit dem Fault, den der Server geantwortet hat](../en/images/soap.png)

- **Header** folgen der Version: SOAP 1.1 sendet `Content-Type: text/xml` und
  `SOAPAction`; SOAP 1.2 sendet `application/soap+xml` mit der Action darin.
- **Der Body** ist ein Umschlag mit Beispielwerten, die aus dem Schema gebaut
  werden: `1` für Zahlen, `string` für Text, der erste Wert einer Aufzählung,
  die erste Option einer Auswahl. Kommentare sagen, was optional ist oder sich
  wiederholt und welche anderen Werte es gibt. Optionale Attribute werden
  weggelassen. Ersetzen Sie die Werte oder setzen Sie `{{variables}}` hinein,
  und löschen Sie, was Sie nicht brauchen.
- Ein Typ, der sich selbst enthält, endet auf der zweiten Ebene mit einem
  Kommentar.

**Ohne WSDL.** Wählen Sie **XML** als Body; solange er leer ist, beginnen
**SOAP 1.1** und **SOAP 1.2** daneben einen Umschlag und setzen die Header.
Tragen Sie die Operation und die Action ein (`SOAPAction` oder `action=` im
`Content-Type`).

**Faults.** Enthält eine Antwort einen SOAP-Fault, zeigt eine rote Zeile über der
Antwort seinen Code und seine Meldung, und `apix send` gibt ihn aus. Eine Anfrage
ohne Tests schlägt bei einem Fault fehl, auch wenn der Status 200 ist, wie
manche Server ihn senden – in `apix send`, `apix run` und im Runner.
Testskripte entscheiden selbst.

Noch nicht: WSDL 2.0, das mit diesem Grund abgelehnt wird, WS-Security,
MTOM-Anhänge und `xsi:type` in Beispielen für die SOAP-Kodierung, vor dem der
Import warnt.

## gRPC

Eine Anfrage, deren URL mit `grpc://` (oder `grpcs://` für TLS) beginnt, ruft
eine gRPC-Methode auf: Die URL-Leiste zeigt **gRPC**, und aus dem Reiter Body
wird **Nachricht**.

![Ein gRPC-Aufruf](../en/images/grpc.png)

1. Schreiben Sie die Adresse, etwa `grpc://localhost:50051`.
2. **Methoden laden** fragt den Server, was er anbietet (gRPC Server
   Reflection). Hat der Server keine Reflection, wählt **Datei hinzufügen** die
   `.proto`-Dateien oder Descriptor-Sets des Dienstes
   (`protoc --descriptor_set_out`, `buf build`, binär oder JSON); Importe werden
   daneben gesucht, und die Well-Known Types von Google sind eingebaut. Nichts
   davon braucht `protoc`. Ohne eine solche Datei siehe
   [Ohne .proto-Datei](#ohne-proto-datei).
3. Wählen Sie die Methode. Ihre Art steht daneben: unär, Server-Streaming,
   Client-Streaming oder bidirektional.
4. Schreiben Sie die Nachricht als JSON, oder drücken Sie **Beispielnachricht**
   für eine mit jedem Feld. Bei einer Methode, die einen Stream annimmt, sendet
   eine JSON-Liste eine Nachricht pro Element.
5. **Senden** (Strg+Enter). Der Antwortbereich listet, was hinausging (↑) und
   was zurückkam (↓), mit Zeiten, dann den Status (`0 OK` oder einen Code wie
   `5 NOT_FOUND` mit seiner Meldung), die Antwort-Metadaten und die Trailer.
   Ein Stream zeigt seine Nachrichten, sobald sie eintreffen; Esc stoppt ihn.

Header werden als Metadaten gesendet, und die Authentifizierung funktioniert wie
bei HTTP mit Bearer, Basic, einem API-Schlüssel im Header und OAuth 2.0. Timeout,
Client-Zertifikat, vertrauenswürdige CA und „TLS-Zertifikat prüfen“ aus den
Anfrage-Einstellungen gelten (abgeschaltet wird das Zertifikat eines
`grpcs://`-Servers nicht geprüft). Ein `-bin`-Header wird als Base64 gelesen.

Auf der Kommandozeile ruft `apix send` die Methode auf (Streams werden eine
Nachricht pro Zeile ausgegeben, sobald sie kommen; der Exit-Code ist ungleich
null, außer der Status ist OK), und `apix schema` gibt den Dienst und seine
Nachrichten aus.

**In Läufen und Tests.** `apix run` und der Runner rufen auch gRPC-Anfragen auf,
und ihre Testskripte und Prüfungen sehen den Aufruf als HTTP-Antwort: Der Status
ist der HTTP-Status, für den der gRPC-Code steht (`OK` ist 200,
`INVALID_ARGUMENT` 400, `NOT_FOUND` 404, `UNAUTHENTICATED` 401, …), gRPC-Code
und -Meldung stehen in den Headern `grpc-status` und `grpc-message` neben den
Metadaten und Trailern, und der Body ist die Antwort oder, bei einer Methode,
die mit einem Stream antwortet, eine JSON-Liste der Nachrichten. So
funktionieren `pm.response.json().message` oder eine Prüfung auf `body.message`
wie bei jeder Anfrage. Ein einzelnes **Senden** zeigt den Aufruf, ohne seine
Skripte auszuführen.

Jeder Aufruf wird im **Verlauf** (Strg+H) mit Protokoll, Status und Metadaten
behalten; wählt man ihn aus, wird der Aufruf wieder so gezeigt, wie er endete.

### Ohne .proto-Datei

Auf der Leitung sendet gRPC Feldnummern, keine Namen, und ruft
`package.Service/Method` auf. Womit Sie auch beginnen, diese müssen am Ende
stimmen. Der Reiter **Definition** neben **Nachricht** macht aus dem, was Sie
haben, eine Definition. Fügen Sie es ein (oder **Datei lesen** oder **Nachricht
verwenden**), geben Sie die **Aufzurufende Methode** an, wenn die Quelle sie
nicht nennt, und drücken Sie **Umwandeln**. Das Ergebnis ist bearbeitbarer
`.proto`-Text, der in der Anfrage gespeichert wird; **Als Datei speichern**
schreibt ihn in eine Datei.

![Eine aus einem JSON-Beispiel erzeugte Definition und die Warnung, wenn sie nicht passt](../en/images/grpc-definition.png)

| Was Sie haben | Was daraus wird |
|---|---|
| `.proto`-Text, ein Descriptor-Set (binär oder JSON) | Exakt. |
| Generierter Code, der die Beschreibung enthält: `_pb2.py`, die äußere Klasse von Java, Kotlin darüber, `.cs`, `.pb.cc`, `.pb.go`, `_pb.rb`, `GPBMetadata` von PHP, protobuf-es `_pb.ts` | Exakt: Die enthaltene Beschreibung wird gelesen, egal in welcher Sprache. |
| Code mit Nummern, aber ohne Beschreibung: Go-Tags `protobuf:"…"`, Rust `#[prost(…)]`, ts-proto, betterproto, `[ProtoMember(n)]`, `@ProtoNumber(n)` | Nummern und Typen stimmen; prüfen Sie die Aufrufe, wenn die Quelle sie nicht auflistet. |
| Von Hand geschriebene Typen: TypeScript, Go, Rust, C#, Java, Kotlin, Python, Swift, Dart, C++ | **Eine Schätzung.** Felder werden in der Reihenfolge ihres Auftretens nummeriert. |
| Eine Beispielnachricht in JSON (das optionale zweite Feld nimmt die Antwort auf) | **Eine Schätzung.** Typen ergeben sich aus den Werten; eine JSON-Liste ergibt einen Stream. |

Eine Schätzung ist nur so gut wie ihre Nummern, und **falsche Nummern schlagen
nicht fehl: Sie füllen die falschen Felder.** Darum verschweigt Apix das nie:

- Bietet der Server Reflection an, wird seine Beschreibung statt einer Schätzung
  verwendet, und jeder Unterschied wird aufgelistet
  (`number 1 here, 7 on the server`).
- Wenn nicht, sagt jeder Aufruf, dass die Nummern ungeprüft sind. Felder der
  Antwort, die die Definition nicht kennt, werden mit Nummer und Wert gezeigt
  (`1:"o-1"`); so lässt sich auch eine Antwort entschlüsseln, für die Sie keinen
  Typ haben. Eine Antwort, die gar nicht passt, schlägt mit einer Zeile fehl, die
  sagt, dass wahrscheinlich die Definition die Ursache ist.

Andere Dateien, die `protoc` erzeugt, verwenden die Beschreibung nur (`.pb.h`,
die Builder von Kotlin, `.pyi`), und gRPC-Stubs enthalten nur die Aufrufe; Apix
sagt, welche Datei stattdessen zu verwenden ist. Nicht gelesen werden:
Protobuf-Editions als `.proto`-Text (sie bleiben ein Descriptor-Set), Oneofs aus
Tags oder Typen (ihre Felder werden zu einfachen Feldern, die auf der Leitung
gleich aussehen) und Generics.

Auf der Kommandozeile:

```bash
apix proto service_pb2.py
```

```bash
apix proto order.json --method shop.v1.Orders/Create --response answer.json --out orders.proto
```

```bash
apix proto types.ts --method shop.v1.Orders/Create --into create.apix
```

Die `.proto` geht in die Standardausgabe oder nach `--out`, `--into` behält sie
in einer Anfrage, und was gewählt werden musste, geht in die
Standardfehlerausgabe. `apix send`, `apix run` und `apix schema` verwenden eine
Definition auf dieselbe Weise, und `apix send` gibt dieselben Warnungen aus; der
Bericht eines Laufs enthält sie nicht.

## Skripte und Tests

Der Reiter **Skripte** enthält zwei JavaScript-Skripte für die Anfrage:

- **Vor der Anfrage** läuft, bevor die Anfrage gesendet wird. Es kann die
  Anfrage ändern (`pm.request.headers.upsert(…)`,
  `pm.request.url.query.add(…)`, `pm.request.body.raw = …`) und Variablen setzen
  (`pm.environment.set("token", …)`).
- **Tests** läuft, wenn die Antwort da ist, und prüft sie:

```js
pm.test("status is 200", () => pm.response.to.have.status(200));
pm.test("has users", () => {
  const body = pm.response.json();
  pm.expect(body.users).to.be.an("array").that.is.not.empty;
});
```

Die API ist Postmans `pm.*` (auch als `apix.*` verfügbar), daher laufen aus
Postman importierte Skripte unverändert: `pm.environment`,
`pm.collectionVariables`, `pm.globals` (die Variablen des Projekts),
`pm.variables`, `pm.request`, `pm.response`, `pm.test`, `pm.expect` mit
Assertions im Chai-Stil, `pm.info`, `console.log` sowie die älteren
`tests["…"] = …` und `postman.setEnvironmentVariable(…)`.

`pm.sendRequest` sendet aus einem Skript eine weitere Anfrage, etwa um zuerst
ein Token zu holen; sowohl `pm.sendRequest(req, (err, res) => …)` als auch
`const res = await pm.sendRequest(req)` funktionieren. `require(…)` bietet die
Teile von `crypto-js` (MD5, SHA-1, SHA-2 und ihre HMACs,
Base64-/Hex-/UTF-8-Kodierer), `uuid`, `lodash` (auch `_`), `moment`, `ajv`,
`tv4` und `chai`, die API-Skripte verwenden; sie sind eingebaut, es wird also
nichts heruntergeladen. Eine Funktion, die eine Bibliothek nicht hat, schlägt
mit einer klaren Meldung fehl.

Der dritte Teil des Reiters Skripte, **Prüfungen**, testet die Antwort ohne
Code: Jede Zeile vergleicht ein Ziel mit einem Wert. Ziele sind `status`,
`time` (ms), `size` (Bytes), `body`, ein Pfad in einem JSON-Body wie
`body.users[0].id` oder `header.Content-Type`; Vergleiche sind `==`, `!=`,
`>`, `>=`, `<`, `<=`, `contains`, `not contains`, `exists`, `not exists`,
`matches` (ein regulärer Ausdruck wie `/^ok/i`) und `is` (ein JSON-Typ). Werte
dürfen `{{variables}}` enthalten. Prüfungen werden in der Anfragedatei als
`[[assert]]` gespeichert und erscheinen als Tests, in der App und in `apix run`.

Um einen Wert aus einer Antwort zu behalten, verwenden Sie die Schaltfläche
**{x}** neben dem Status: Geben Sie den Wert (dieselben Ziele wie bei den
Prüfungen) und einen Variablennamen an, und er wird in der aktiven Umgebung
gespeichert (oder, wenn keine aktiv ist, in den Variablen der Sammlung). Setzen
Sie das Häkchen bei **Bei jedem Senden aktualisieren**, um zusätzlich die
passende Zeile `pm.environment.set(…)` zum Tests-Skript hinzuzufügen.

Skripte sind Dateien neben dem, wozu sie gehören: `<request>.prerequest.js` und
`<request>.test.js` neben der Anfrage, `_folder.prerequest.js` /
`_folder.test.js` in einem Ordner, `_collection.prerequest.js` /
`_collection.test.js` im Stammordner der Sammlung. Sie laufen von außen nach
innen: Sammlung, Ordner, dann die Anfrage. Strg+S speichert die Skripte der
Anfrage mit ihr; der Text im Editor wird verwendet, auch bevor Sie speichern.

Nach dem Senden bekommt der Antwortbereich einen Reiter **Tests** (✓ bestanden,
✗ fehlgeschlagen mit Grund) und einen Reiter **Konsole** mit den Ausgaben von
`console.*`. Wirft ein Skript vor der Anfrage einen Fehler, wird die Anfrage
nicht gesendet, und der Fehler nennt die Datei, die fehlgeschlagen ist. Skripte
werden nach 10 Sekunden angehalten.

Werte, die ein Skript setzt, werden in keine Datei geschrieben. Sie bleiben,
solange Apix geöffnet ist (ein Umgebungswechsel verwirft, was
`pm.environment.set` gesetzt hat), und erscheinen in den Variablenkarten als
„Von einem Skript gesetzt“. `pm.variables.set` gilt für ein Senden.

## Runner

Der **Runner** sendet viele Anfragen auf einmal und prüft sie alle. Öffnen Sie
ihn mit **Ansicht › Runner…** oder Strg+Shift+R für die Sammlung, in der Sie
sind, oder mit **Ausführen…** auf einer Sammlung oder einem Ordner in der
Seitenleiste nur für diesen Teil.

![Der Runner nach einem datengetriebenen Lauf](../en/images/runner.png)

- **Anfragen**: jede Anfrage in der Reihenfolge der Seitenleiste, mit Häkchen.
  Entfernen Sie das Häkchen bei denen, die wegfallen sollen, und verschieben Sie
  eine mit ↑ ↓, um die Reihenfolge für diesen Lauf zu ändern.
- **Umgebung**: welche Umgebung ihre Variablen liefert.
- **Durchläufe**: wie oft die Anfragen durchlaufen werden. Leer gelassen ist es
  einmal, oder einmal pro Zeile, wenn es eine Datendatei gibt.
- **Pause (ms)**: eine Pause zwischen zwei Anfragen, für APIs mit Ratenbegrenzung.
- **Gleichzeitig**: wie viele Durchläufe nebeneinander laufen, wie mehrere
  Nutzer der API zur selben Zeit. Jeder sendet seine Anfragen der Reihe nach
  (so erreicht das Token einer Anmeldung trotzdem die nächste Anfrage) und
  behält, was seine eigenen Skripte setzen; die Ergebnisse werden nach Durchlauf
  aufgelistet. Leer gelassen läuft einer nach dem anderen.
- **Datendatei**: eine CSV- oder JSON-Datei für einen datengetriebenen Lauf.
  Jede Zeile ist ein Durchlauf, und ihre Spalten sind Variablen: `{{email}}` in
  der Anfrage und `pm.iterationData.get("email")` in Skripten. CSV braucht eine
  Kopfzeile; Anführungszeichen und durch Semikolon getrennte Dateien (Excel in
  vielen Sprachen) werden verstanden. JSON ist ein Array von Objekten, etwa
  `[{"id": 1, "email": "a@x.test"}, …]`.
- **Beim ersten Fehler anhalten**: den Lauf bei der ersten fehlschlagenden
  Anfrage beenden.

Eine Anfrage besteht, wenn ihre Tests (Skripte und Prüfungen) bestehen; eine
Anfrage ohne Tests besteht bei einem 2xx- oder 3xx-Status. Während des Laufs
können Sie **Pausieren**, **Fortsetzen** und **Stoppen** (nach der gerade
gesendeten Anfrage). Das Schließen des Dialogs stoppt den Lauf nicht: Öffnen Sie
den Runner erneut, um ihn zu sehen. Klicken Sie auf ein Ergebnis, um seine Tests
zu sehen, warum es nicht gesendet wurde, und seine Konsolenzeilen.
`pm.execution.setNextRequest("name")` springt zu einer anderen Anfrage des Laufs,
und `setNextRequest(null)` beendet den Durchlauf, wie in Postman.

Der Runner sendet die gespeicherte Fassung jeder Anfrage. Werte, die Skripte vor
dem Lauf gesetzt haben, werden verwendet; was die Skripte des Laufs setzen,
bleibt innerhalb des Laufs.

Ist der Lauf vorbei, schreibt **Bericht speichern** ihn als **JUnit XML** (für
Testansichten in CI), **JSON** (alles, was geschehen ist) oder **HTML** (eine
Seite, die sich in jedem Browser öffnet, hell oder dunkel). `apix run` tut
dasselbe in einem Terminal; siehe
[Eine Sammlung in CI ausführen](workflows.md#eine-sammlung-in-ci-ausführen).
Ein in einer URL geschriebenes Passwort (`https://user:password@host`) wird aus
Berichten, Kommandozeilenausgaben, Protokollen und Fehlermeldungen weggelassen.
Die Anfragedatei und die URL-Leiste enthalten es weiterhin; nehmen Sie für echte
Passwörter also lieber eine Variable oder den Reiter Auth.

## Abläufe

Ein **Ablauf** verbindet Anfragen zu einem Graphen: anmelden, das Token
behalten, abfragen, bis ein Job fertig ist, je nach Antwort verzweigen. Jeder
Ablauf ist eine `.flow`-Datei in der Sammlung, neben den Anfragen, und kommt so
mit ihnen in git. Öffnen Sie **Ansicht › Abläufe…**; geben Sie einen Namen ein
und drücken Sie **Anlegen** für einen neuen.

![Das Fenster Abläufe](../en/images/flows.png)

Es gibt vier Arten von Knoten:

- **Anfrage** sendet eine Anfrage der Sammlung, mit ihren Skripten und Tests.
  Ihr Ergebnis ist **ok**, wenn sie bestanden hat (ein erfolgreicher Status oder
  alle Tests bestanden), sonst **fail**. **Aus der Antwort behalten** macht
  Werte der Antwort zu Variablen für die nachfolgenden Knoten, einen pro Zeile:
  `token = $.token`.
- **Prüfung** vergleicht einen Gegenstand mit einem Wert: **yes** oder **no**.
  Der Gegenstand ist `status`, `time` (ms), `body`, `header:Name`, ein JSONPath
  (`$.items[0].id`) oder jq-Filter (`.items | length`) auf die letzte Antwort
  oder ein Text mit `{{variables}}`. Vergleiche: gleich, ungleich, enthält,
  größer als, kleiner als (als Zahlen, wenn beide Seiten Zahlen sind),
  vorhanden, fehlt.
- **Variablen setzen** setzt Variablen (Zeilen `name = value`; `{{variables}}`
  funktionieren).
- **Warten** pausiert, für Abfrageschleifen.

Ziehen Sie einen Knoten, um ihn zu verschieben. Zum Verbinden klicken Sie auf
den Punkt eines Ergebnisses rechts an einem Knoten, dann auf den Knoten, zu dem
es führt; ein erneuter Klick auf den Punkt leitet es um, und **×** im Bereich
entfernt es. Der Bereich rechts bearbeitet den ausgewählten Knoten: seine Id,
seine Einstellungen, wohin jedes Ergebnis führt, **Hier starten**, **Knoten
löschen**. Ein Lauf beginnt beim Knoten **Start** (ohne einen beim ersten
Knoten, zu dem nichts führt), folgt jedem Ergebnis und endet bei einem Ergebnis,
das nirgendwohin führt. Er schlägt fehl, wenn eine Anfrage ohne eine
**fail**-Kante fehlschlägt oder wenn ein zu behaltender Wert nicht vorhanden
ist. Schleifen sind erlaubt; ein Lauf stoppt nach 1000 Schritten.

**Ausführen** sendet den Ablauf mit den Variablen der Sammlung und der gewählten
Umgebung; jeder Knoten wird je nach Ergebnis grün, gelb oder rot, und die Liste
unter der Zeichenfläche zeigt jeden Schritt mit dem, was behalten oder
verglichen wurde. Änderungen werden mit **Speichern** gespeichert, und wenn sich
das Fenster schließt.

In einem Terminal läuft derselbe Ablauf in CI:

```sh
apix flow list my-api
apix flow run my-api/new-user.flow --env staging --junit flow.xml
```

`--junit`, `--html` und `--markdown` berichten über die Anfragen des Ablaufs wie
`apix run`; `--json` schreibt jeden Schritt. Die Beispielsammlung enthält
`new-user.flow`.

Eine Ablaufdatei:

```toml
name = "New user"

[[node]]
id = "first-user"
kind = "request"
request = "users/list-users.apix"
x = 40.0
y = 40.0
extract = { user_id = "$[0].id" }

[[node]]
id = "found"
kind = "check"
subject = "{{user_id}}"
op = "exists"
x = 300.0
y = 40.0

[[edge]]
from = "first-user"
port = "ok"
to = "found"
```

## Lasttests

**Anfrage › Lasttest…** (oder die Befehlspalette) sendet die geöffnete Anfrage
von vielen Workern gleichzeitig und zeigt, wie der Server standhält: Anfragen
und Anfragen pro Sekunde, Fehlschläge, das 50., 95. und 99. Perzentil und die
langsamste Zeit sowie ein sekundengenaues Diagramm der Anfragen (fehlgeschlagene
in Rot) mit der mittleren Latenz als gelbem Strich.

![Ein Lasttest](../en/images/load-test.png)

- **Gleichzeitig**: gleichzeitig laufende Anfragen (1 bis 10 000).
- **Anfragen**: nach so vielen aufhören; leer für keine Grenze.
- **Dauer**: nach dieser Zeit aufhören (`30s`, `2m`, `1500ms`); ohne beides
  10 s.
- **Pro Sekunde**: höchstens so viele Anfragen pro Sekunde, gleichmäßig
  verteilt; leer für so schnell, wie der Server antwortet.

Verbindungen werden offen gehalten und wiederverwendet, wie es echte Clients
unter Last tun (ein einzelnes Senden öffnet eine neue, damit sein Reiter
Netzwerk den Handshake zeigt). Variablen werden einmal eingesetzt; Skripte
laufen nicht, OAuth 2.0 holt sein Token einmal, und Digest Auth wird nicht
unterstützt. Eine Anfrage gilt als fehlgeschlagen, wenn keine Antwort kam oder
der Status 400 oder höher ist. **Stoppen** beendet den Test, und das Schließen
des Fensters stoppt ihn ebenfalls. Testen Sie nur Server, für die Sie die
Erlaubnis haben.

In einem Terminal tut `apix bench` dasselbe und kann einen CI-Job fehlschlagen
lassen:

```bash
apix bench my-api/users/list.apix -c 50 -d 30s --max-p95 300 --max-failed 1 --json bench.json
```

Es endet mit 1, wenn das 95. Perzentil über `--max-p95` Millisekunden liegt,
mehr als `--max-failed` Prozent der Anfragen fehlgeschlagen sind oder keine
erfolgreich war.

## Monitore

Ein **Monitor** führt die Sammlung oder einen Ordner oder eine Anfrage daraus
nach Zeitplan aus und meldet, wenn er fehlschlägt oder wieder besteht. Öffnen
Sie **Ansicht › Monitore…**.

![Monitore](../en/images/monitors.png)

- **Monitor hinzufügen** fragt nach einem Namen, wie oft (`30s`, `5m`, `1h`,
  `1d` oder zusammen wie in `1h30m`; mindestens `10s`), einem Ordner oder einer
  Anfrage (leer für die ganze Sammlung), einer Umgebung und einem Webhook. Die
  Schaltfläche ⚙ bearbeitet einen Monitor, die Schaltfläche ▷ führt ihn sofort
  aus.
- Jeder Monitor zeigt sein letztes Ergebnis (bestanden oder wie viele
  fehlgeschlagen sind und die ersten Fehlschläge) und, solange die Zeitplanung
  an ist, wann er als Nächstes läuft.
- **Monitore nach Zeitplan ausführen, solange Apix geöffnet ist** schaltet den
  Zeitplan ein (er ist aus, bis Sie ihn wählen). Apix führt dann die fälligen
  Monitore im Hintergrund aus, einen nach dem anderen, und zeigt eine
  Benachrichtigung, wenn einer fehlschlägt oder wieder besteht.
- Ein **Webhook** erfährt dasselbe: Apix sendet per POST JSON mit einer Zeile
  `text` (und `content`), die Slack, Discord, Mattermost und Google Chat als
  Nachricht anzeigen, dazu die Details (`monitor`, `status`, `passed`,
  `failed`, `failures`). `{{variables}}` werden eingesetzt, die Adresse kann
  also in den geheimen Werten einer Umgebung bleiben.

Monitore stehen in der `apix.toml` der Sammlung und werden so über git geteilt:

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "users"            # ein Ordner oder eine Anfrage; weglassen für die ganze Sammlung
environment = "staging"
webhook = "{{alerts_url}}"
# enabled = false        # aufgelistet, aber nicht nach Zeitplan ausgeführt
```

Um eine API zu überwachen, während die App geschlossen ist, führen Sie
`apix monitor` in einem Terminal aus, per cron oder Aufgabenplanung oder nach
einem CI-Zeitplan; siehe
[Eine API mit Monitoren überwachen](workflows.md#eine-api-mit-monitoren-überwachen).
Die App und `apix monitor` führen dasselbe Ergebnisprotokoll, daher zeigt jede
Seite, was die andere gefunden hat.

## Mock-Server

Der **Mock-Server** antwortet mit den Beispielen, die bei den Anfragen der
Sammlung gespeichert sind (siehe Beispiele), sodass ein Frontend gebaut oder ein
Test ausgeführt werden kann, bevor die echte API existiert. Öffnen Sie
**Ansicht › Mock-Server…**, wählen Sie einen Port und **Starten**.

![Der Mock-Server](../en/images/mock.png)

- Eine Anfrage wird nach Methode und Pfad zugeordnet: die URL der Anfrage nach
  ihrem Host oder nach einer führenden `{{variable}}`, sodass
  `{{base_url}}/users/:id` die Adresse `/users/7` bedient. Segmente `:name` und
  `{{name}}` passen auf jedes einzelne Segment, und der Pfad mit den meisten
  festen Teilen gewinnt. Ein Beispiel, dessen eigene gespeicherte Anfrage eine
  andere URL hat, bedient diese URL.
- Bei mehreren Beispielen antwortet das erste mit einem 2xx-Status. Eine Anfrage
  kann mit dem Header `x-mock-response-name: <example>` oder
  `x-mock-response-code: 404` ein anderes wählen (die, die auch die Mock-Server
  von Postman lesen).
- Jede Antwort erlaubt jeden Ursprung (CORS) und nennt in `X-Apix-Mock`, aus
  welchem Beispiel sie stammt; ein `OPTIONS`-Preflight wird ebenfalls
  beantwortet.
- Ohne Treffer gibt es `404` mit einer JSON-Liste dessen, was bereitgestellt
  wird.
- Das Fenster listet, was bereitgestellt wird, und die letzten Anfragen.
  Hinzugefügte oder bearbeitete Beispiele werden sofort bereitgestellt; der
  Server läuft weiter, wenn das Fenster geschlossen wird, bis **Stoppen** oder
  bis die App beendet wird.

Um ihn zu verwenden, fügen Sie eine Umgebung hinzu (etwa `mock`), deren
`base_url` `http://127.0.0.1:4010` ist. In einem Terminal:
`apix mock my-api --port 4010` (`--host 0.0.0.0`, um von anderen Computern
erreichbar zu sein, `--delay 300` für einen langsamen Server).

## API-Dokumentation

**Datei › Dokumentation exportieren…** (oder **Dokumentation exportieren…** auf
einer Sammlung in der Seitenleiste) schreibt die Sammlung als API-Dokumentation:
eine HTML-Seite, die sich in jedem Browser öffnet, hell oder dunkel, mit einem
Inhaltsverzeichnis; oder Markdown für ein Repository, wenn der Dateiname auf
`.md` endet.

![Erzeugte Dokumentation](../en/images/api-docs.jpg)

Sie zeigt die Beschreibung der Sammlung (Markdown), ihre Variablen und die in
jeder Umgebung committeten Variablen, dann jede Anfrage in der Reihenfolge der
Seitenleiste unter ihren Ordnern: Methode, URL, Beschreibung, Pfadvariablen,
Query, Header, Authentifizierungstyp, Body, Prüfungen und gespeicherte
Beispiele. Nichts Geheimes wird ausgegeben: `.local.toml`-Dateien und der
Schlüsselbund werden nicht gelesen, die Authentifizierung zeigt nur ihre Art,
und `Authorization`-, `Cookie`- und API-Schlüssel-Header zeigen ihren Wert nur,
wenn er eine `{{variable}}` ist. Rohes HTML in Beschreibungen wird weggelassen.

In einem Terminal: `apix docs my-api --out api.html` (oder `--out API.md`; ohne
`--out` Markdown auf der Standardausgabe).

## Mitschnitt-Proxy

Der **Mitschnitt-Proxy** macht aus echtem Verkehr Anfragen. Öffnen Sie
**Ansicht › Mitschnitt-Proxy…**, wählen Sie einen Port (8888) und **Starten**,
und stellen Sie dann `http://127.0.0.1:8888` als Proxy eines Browsers, einer App
oder eines Skripts ein (`HTTP_PROXY` und `HTTPS_PROXY`). Jede durchgehende
Anfrage wird als Anfragedatei im Ordner `captured/` der Sammlung gespeichert,
mit ihrer Antwort als Beispiel `Captured`, und im Fenster aufgelistet.

![Der Mitschnitt-Proxy](../en/images/capture.png)

- **Nur** beschränkt das Aufzeichnen auf bestimmte Hosts (`example.com` umfasst
  auch `api.example.com`); der Rest geht unaufgezeichnet durch.
- Nichts Geheimes wird geschrieben: `Cookie` wird verworfen, und die Werte von
  `Authorization`- und API-Schlüssel-Headern werden zu Variablen
  (`{{authorization}}`).
- HTTPS läuft in einem Tunnel, den der Proxy nicht lesen kann, daher wird es
  standardmäßig weitergereicht und nur sein Host aufgelistet. Mit Häkchen bei
  **HTTPS mit der lokalen Zertifizierungsstelle von Apix öffnen, um es auch
  aufzuzeichnen** beantwortet Apix den TLS-Handshake mit einem Zertifikat für den
  Host, das von einer lokalen Stelle signiert ist
  (`<config>/Apix/capture-ca.pem`), und zeichnet die Anfragen ebenfalls auf. Das
  funktioniert nur, wo dieser Stelle vertraut wird: Das Fenster zeigt den Befehl,
  der ihr vertraut (unter Windows `certutil -user -addstore Root …`); Apix
  installiert sie nie, und Firefox führt eine eigene Liste.

In einem Terminal: `apix capture my-api/captured --port 8888 --only example.com`
(`--https`, um HTTPS zu öffnen, `--no-save`, um nur zuzusehen).

## Tabs

Jeder Tab behält seine ungespeicherten Änderungen, seine letzte Antwort und
sogar eine noch laufende Anfrage. Strg+Tab und Strg+Shift+Tab wechseln zwischen
Tabs, Strg+W oder ein Mittelklick schließt einen; Apix fragt nach, bevor
ungespeicherte Änderungen geschlossen werden. Die geöffneten Tabs kommen zurück,
wenn Sie dieselbe Sammlung oder dasselbe Projekt das nächste Mal öffnen (Tabs
verschlüsselter Sammlungen werden nicht gemerkt, da auch ihre Dateinamen geheim
sind).

## Anfragen ordnen

Die **Suche** oben in der Seitenleiste grenzt sie auf die Anfragen ein, deren
Name, Methode oder URL jedes eingegebene Wort enthält (`get users` findet eine
GET-Anfrage an `/users`), wobei die Ordner auf dem Weg dorthin geöffnet werden.
Ein Ordner, dessen Name passt, zeigt alles, was er enthält. Esc oder ✕ leert
sie, und die Ordner sind wieder wie vorher.

Klicken Sie mit der rechten Maustaste in die Seitenleiste, um Anfragen und
Ordner zu erstellen, umzubenennen, zu duplizieren oder zu löschen; gelöschte
Dateien gehen in den Papierkorb des Systems. Ziehen Sie eine Anfrage oder einen
Ordner auf die Mitte eines anderen Ordners, um sie dorthin zu verschieben.
Lassen Sie sie stattdessen am oberen oder unteren Rand einer Zeile fallen, um
sie vor oder nach diese Zeile zu setzen; eine Linie zeigt, wo sie landen wird.
Die Reihenfolge wird in `apix.toml` (für die oberste Ebene der Sammlung) und
`folder.toml` (für einen Ordner) als Liste `order` gespeichert und so über git
geteilt; Einträge, die sie nicht auflistet, folgen nach Namen. Postman-Importe
behalten die Reihenfolge von Postman, und Exporte nach Postman folgen Ihrer.

## Projekte

Ein **Projekt** fasst mehrere zusammengehörige Sammlungen zusammen und teilt
Umgebungen und Authentifizierung zwischen ihnen. Erstellen Sie eines mit
**Datei › Neues Projekt…**. In einem Projekt kann
**Umgebung › Seitenleiste gruppieren nach** die Seitenleiste als
Projekt › Umgebung › Sammlungen zeigen.

- Ziehen Sie den Kopf einer Sammlung über oder unter eine andere, um ihre
  Reihenfolge zu ändern.
- Rechtsklick auf eine Sammlung › **Farbe** gibt ihr eine Farbe; ein Punkt
  zeigt sie in der Seitenleiste und auf den Tabs ihrer Anfragen. Farben werden in
  `apix-project.toml` gespeichert.
- **Datei › In ein Projekt verschieben…** (bei einer einzelnen geöffneten
  Sammlung) verschiebt den Ordner dieser Sammlung in ein Projekt Ihrer Wahl. Ist
  ein Projekt geöffnet, bietet das Ziehen des Ordners einer Sammlung aus dem
  Dateimanager ins Fenster dasselbe an.

## Git

Eine Sammlung besteht aus einfachen Dateien und lebt daher wie Code in git.
Liegt die geöffnete Sammlung (oder das Projekt) in einem Git-Repository, zeigt
die Statusleiste den Branch und wie viele seiner Dateien sich seit dem letzten
Commit geändert haben, und die Seitenleiste markiert jede geänderte Anfrage:
**M** geändert (ihre Datei oder ihre eigenen Skripte), **A** neu. Die
Markierungen folgen jedem Speichern und jeder Änderung auf der Festplatte und
werden neu gelesen, wenn das Fenster wieder in den Vordergrund kommt; ein in
einem Terminal gemachter Commit löscht sie also.

![Branch und geänderte Anfragen](../en/images/git-status.png)

Klicken Sie auf den Branch in der Statusleiste (oder **Ansicht › Git…**), um das
Fenster Git zu öffnen. Links stehen die seit dem letzten Commit geänderten
Dateien und die letzten Commits; wählen Sie eine Datei, um zu sehen, was sich
darin geändert hat, so wie es `apix diff` zeigt: eine Anfrage Feld für Feld
(`query._limit: 5 → 10`, ein hinzugefügter Header), Skripte und anderer Text
Zeile für Zeile und gar nichts, wenn sich nur die Formatierung geändert hat.
Darunter schreiben Sie eine Nachricht und klicken **Commit**: Er übernimmt jede
Änderung im geöffneten Ordner und nichts außerhalb davon. Das Branch-Menü
wechselt Branches, **Neuer Branch** legt einen mit dem daneben eingegebenen
Namen an und wechselt zu ihm, und **Pull** (nur Fast-Forward) und **Push**
(setzt beim ersten Mal den Upstream) führen git mit Ihren eigenen
Einstellungen, Hooks und Zugangsdaten aus. Die Seitenleiste folgt einem
Branch-Wechsel sofort.

![Das Fenster Git](../en/images/git-window.png)

Auf der Kommandozeile sendet `apix run --changed` nur die Anfragen, die eine
Änderung berührt, und `apix diff` zeigt Feld für Feld, was sich darin geändert
hat; bei einem Pull Request kann die GitHub Action das Ergebnis kommentieren.
Siehe
[Nur was sich geändert hat](workflows.md#nur-was-sich-geändert-hat-git-bewusste-läufe).

## Verschlüsselte Sammlungen

Eine verschlüsselte Sammlung hält jede Anfrage, jede Umgebung und sogar jeden
Dateinamen auf der Festplatte versiegelt (XChaCha20-Poly1305 mit einem
Passwortschlüssel aus Argon2id). Git sieht nur, welcher verschlüsselte Blob sich
geändert hat.

- **Datei › Diese Sammlung verschlüsseln…** oder Rechtsklick auf eine Sammlung ›
  **Verschlüsseln…** verschlüsselt eine bestehende Sammlung an Ort und Stelle.
- **Datei › Neue verschlüsselte Sammlung…** erstellt eine in einem Projekt.
- Sie erhalten einmalig einen **Wiederherstellungsschlüssel**. Bewahren Sie ihn
  sicher auf: Er öffnet die Sammlung, wenn das Passwort verloren geht.
- Beim Öffnen der Sammlung wird nach dem Passwort gefragt. Sie wird wieder
  gesperrt, wenn Sie **Datei › Verschlüsselte Sammlungen sperren** wählen, Apix
  beenden oder sie so lange unbenutzt lassen, wie in den Einstellungen
  festgelegt (standardmäßig 15 Minuten).
- Setzen Sie im Entsperrdialog das Häkchen bei **Auf diesem Computer merken**,
  um das Passwort im Anmeldeinformationsspeicher des Systems zu behalten
  (Anmeldeinformationsverwaltung unter Windows, Schlüsselbund unter macOS,
  Secret Service unter Linux). Die Sammlung öffnet sich dann ohne Nachfrage,
  solange Sie angemeldet sind, und auch die Kommandozeile `apix` verwendet das
  gespeicherte Passwort. **Datei › Gespeichertes Passwort vergessen** (oder
  Rechtsklick auf die Sammlung) entfernt es. Wurde das Passwort anderswo
  geändert, wird das gespeicherte verworfen, und Apix fragt erneut.

## Kopieren als und curl einfügen

Die Kopier-Schaltfläche neben **Senden** (oder **Anfrage › Kopieren als**)
kopiert die Anfrage als `curl` für bash oder fish, als `curl.exe` für die
Windows-Eingabeaufforderung, als PowerShell-Aufruf `Invoke-RestMethod`, als
HTTPie-Befehl `http` (HTTPie 3.0 oder neuer) oder als GNU-Befehl `wget`, mit
eingesetzten Variablen. wget kann keine Multipart-Formulare senden; der kopierte
Befehl sagt das. Anfragen, die bei jedem Senden signiert werden (OAuth 1.0, AWS
für PowerShell und Code), werden im Moment des Kopierens signiert.

Dasselbe Menü kopiert die Anfrage als Programm: JavaScript `fetch`, Node.js
axios, Python requests, Go `net/http`, Rust reqwest, C# `HttpClient`, Java
`HttpClient`, PHP cURL, Swift `URLSession` oder Kotlin OkHttp. Jedes sendet, was
Apix senden würde – URL, Header, Authentifizierung und jeden Body-Typ – und die
Anfrage-Einstellungen, die die Sprache ausdrücken kann (Timeout,
Weiterleitungen, TLS-Prüfung, Proxy, Client-Zertifikat). Eine Einstellung, für
die eine Sprache keinen einfachen Schalter hat, wird in einem Kommentar
`Not in this snippet` oben aufgeführt, statt stillschweigend wegzufallen. In der
Seitenleiste klicken Sie mit der rechten Maustaste auf eine Anfrage und
verwenden **Als Code kopieren**.

Die CLI schreibt dasselbe: `apix code request.apix --as python` (`apix shell`
ist derselbe Befehl; `--as` nimmt `bash`, `cmd`, `powershell`, `fetch`,
`axios`, `python`, `go`, `rust`, `csharp`, `java`, `php`, `swift` oder
`kotlin`).

Fügen Sie einen `curl`-Befehl (zum Beispiel aus „Copy as cURL“ Ihres Browsers)
in das URL-Feld ein, und Apix füllt die ganze Anfrage aus: Methode, URL, Query,
Header, Authentifizierung und Body.

## Import aus Postman

**Datei › Importieren…**, oder ziehen Sie einen Postman-Export ins Fenster.
Sammlungen (v2.0 und v2.1) und Umgebungen werden unterstützt. Ordner, Anfragen,
Bodies, Authentifizierung und Variablen werden übernommen; was Apix noch nicht
übernehmen kann, wird in einem Bericht aufgelistet. Skripte vor der Anfrage und
Testskripte bleiben als `.js`-Dateien neben ihren Anfragen erhalten, und geheime
Umgebungswerte gehen in die von git ignorierte `.local.toml`.

**Viele Dateien auf einmal.** Öffnen Sie einen Ordner mit Postman-Exporten –
oder das Zip, das *Export data* von Postman erzeugt – oder ziehen Sie ihn ins
Fenster, und Apix erkennt die Dateien an ihrem Inhalt, nicht an ihren Namen, und
bietet an, alles als ein Projekt zu importieren: eine Sammlung pro
Postman-Sammlung, die Umgebungen von allen gemeinsam genutzt und Globals als
Projektvariablen (geheime Globals werden weggelassen, da Projektvariablen
committet werden). Zwei Umgebungen mit demselben Namen werden als `name` und
`name-2` behalten.

**Nach Postman exportieren.** Klicken Sie mit der rechten Maustaste auf eine
Sammlung und wählen Sie **Nach Postman exportieren…** (oder
**Datei › Nach Postman exportieren…**), um eine Postman-v2.1-Sammlungsdatei und
eine Umgebungsdatei pro Umgebung zu schreiben, damit Teammitglieder mit Postman
weiter damit arbeiten können. Ordner, Authentifizierung, Bodies, Skripte,
Variablen und die Einstellungen für Weiterleitungen, TLS und Cookies werden
übernommen. Postman hat keinen Timeout, Proxy oder kein Client-Zertifikat pro
Anfrage, und geheime Umgebungswerte werden nur mit ihren Namen exportiert; die
Benachrichtigung listet, was weggelassen wurde.

Auf der Kommandozeile: `apix import <file, folder or zip>` und
`apix export <collection> --out <folder>`.

## Import aus Insomnia, OpenAPI und HAR

**Datei › Importieren…** (oder das Ziehen der Datei ins Fenster) liest auch
diese; das Format wird am Inhalt erkannt, und ein Ordner oder Zip darf sie
mischen:

| Format | Was Sie bekommen |
|---|---|
| Insomnia v4 (JSON-Export) | Eine Sammlung pro Workspace, Ordner mit ihrer Authentifizierung, die Basisumgebung als Sammlungsvariablen und jede Unterumgebung als Umgebung. `{{ _.name }}` wird zu `{{name}}`. |
| OpenAPI 3 / Swagger 2 (JSON oder YAML) | Eine Anfrage pro Operation, ein Ordner pro Tag. Der Server wird zur Variable `baseUrl` (mehrere Server: je eine Umgebung), Pfadparameter werden zu Variablen mit ihren Beispielwerten, erforderliche Query- und Header-Parameter werden eingeschaltet, Bodies kommen aus Beispielen oder werden aus dem Schema gebaut, und Security Schemes werden zu Authentifizierung mit leeren Variablen für die Zugangsdaten (`bearerToken`, `apiKey`, …), die in einer Umgebung auszufüllen sind. |
| WSDL 1.1 | Eine Anfrage pro Operation mit einem Beispielumschlag; siehe [SOAP](#soap). |
| HAR („Save all as HAR“ in den DevTools des Browsers) | Nur API-Aufrufe, ein Ordner pro Host. Skripte, Styles, Bilder und Schriften, `data:`-URLs und Wiederholungen werden weggelassen, ebenso `Cookie`-Header: Sie tragen die Sitzung des Browsers. |

Eine Sammlung aus einer Datei wird als Sammlung importiert; mehrere werden zu
einem Projekt. Der Bericht listet alles auf, was nicht übernommen werden konnte.

## Verlauf

Jede gesendete Anfrage wird mit ihrer Antwort behalten. **Ansicht › Verlauf…**
(Strg+H) listet sie auf, die neueste zuerst, mit Methode, Name, URL, Status und
Sendezeitpunkt; tippen Sie, um nach einem davon zu suchen. Wählen Sie einen
Eintrag, und die Anfrage kommt in dem Tab zurück, aus dem sie gesendet wurde,
als ungespeicherte Änderungen, zusammen mit der Antwort, die sie bekam. Drücken
Sie Strg+Enter, um sie erneut zu senden, Strg+S, um diese Fassung zu behalten.
Ist die Anfragedatei nicht mehr da, öffnet sich der Eintrag im Editor ohne Tab.

Der Verlauf bleibt auf diesem Computer (der Ordner `history` neben den
Einstellungen), nie in einer Sammlung. Anfragen werden so gespeichert, wie sie
geschrieben sind, mit unaufgelösten `{{variables}}`, daher werden geheime
Umgebungswerte nicht hineinkopiert. Die letzten 300 Anfragen werden behalten,
Antwortinhalte bis je 256 KB. Anfragen aus verschlüsselten Sammlungen werden nie
in den Verlauf geschrieben. **Verlauf löschen** im Fenster leert ihn;
**Datei › Einstellungen… › Verlauf** schaltet ihn ab.

## Antworten vergleichen

**⇄** neben der Antwort (oder **Anfrage › Antworten vergleichen…**) stellt die
aktuelle Antwort neben eine andere derselben Anfrage:

- **Frühere Antworten**: die Antworten der Anfrage aus dem Verlauf, die neueste
  zuerst. Wählen Sie eine, um zu sehen, was sich seitdem geändert hat; so reisen
  Sie in der Zeit zurück zur Antwort von gestern.
- **Andere Umgebungen**: die Anfrage, erneut in einer anderen Umgebung gesendet
  (Staging gegen Produktion, ein Mock gegen die echte API).

Es zeigt, wie viele Dinge sich unterscheiden, dann den Status, die Header nach
Namen und den Body: JSON Feld für Feld (Datensätze in Listen werden über ihr
`id`, `_id`, `uuid` oder `key` zugeordnet, sodass eine Liste in anderer
Reihenfolge keine Änderung ist, und `[id=1].email` benennt das Feld), anderen
Text Zeile für Zeile. Header, die sich bei jeder Antwort ändern (Date,
Anfrage-IDs, Caching, Übertragungsart des Bodys), werden ausgelassen und am Ende
aufgelistet. **Als Text kopieren** kopiert das Ergebnis.

![Vergleich mit einer früheren Antwort](../en/images/compare.png)

In einem Terminal sendet `apix compare <request> --env staging --against production`
die Anfrage in beiden Umgebungen und gibt die Unterschiede aus; es endet mit 1,
wenn sie sich unterscheiden, sodass eine Pipeline prüfen kann, ob zwei
Umgebungen gleich antworten. Der Verlauf behält Bodies bis 256 KB, längere
werden also nur so weit verglichen, wie sie behalten wurden.

## KI-Assistent

Der KI-Assistent ist aus, bis Sie ihn unter
**Datei › Einstellungen… › KI-Assistent** einschalten. Apix ist Open Source und
hat kein eigenes KI-Konto: Sie wählen einen Anbieter und geben Ihren eigenen
API-Schlüssel an.

- **KI-Anbieter**: Claude (Anthropic), ChatGPT (OpenAI) oder Gemini (Google).
- **Modell**: das Standardmodell des Anbieters (Claude Opus 5.5, `gpt-5.5`,
  `gemini-2.5-flash`), oder **↻** lädt die Modelle, die Ihr Schlüssel nutzen
  kann, und Sie wählen eines.
- **API-Schlüssel**: für den Anbieter oben, im Schlüsselbund des Systems
  gespeichert, nie in einer Datei; ein leeres Feld zu speichern entfernt ihn.
  Ohne einen wird `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` oder `GEMINI_API_KEY`
  verwendet.

Dann bekommt die Antwort eine weitere Schaltfläche:

- **KI-Tests** bei einer erfolgreichen Antwort: Die KI schreibt ein
  Tests-Skript, das Status, Inhaltstyp und Aufbau des Bodys prüft, und Apix fügt
  es den Tests der Anfrage hinzu (als von ihr geschrieben markiert,
  ungespeichert), sodass Sie es lesen, bevor Sie es behalten.
- **Erklären** bei einer fehlgeschlagenen Antwort (ab 400): was die Antwort
  bedeutet, die wahrscheinlichen Ursachen und was zu prüfen ist, in der Sprache
  der App.

![Erklärung einer fehlgeschlagenen Antwort](../en/images/ai-explain.png)

Was an den Anbieter gesendet wird: Methode, URL, Header und Body der Anfrage
sowie Status, Header und Body der Antwort (abgeschnitten bei 12 KB).
Authorization, Cookies und Header mit key, token, secret oder signature im Namen
werden als ••• maskiert. Die KI kann sich irren; nichts wird ohne Sie geändert
oder irgendwohin sonst gesendet. Der Screenshot wurde mit einem lokalen
Testserver aufgenommen, nicht mit einer echten Antwort.

## Plugins

Plugins fügen hinzu, was Apix nicht eingebaut hat: einen Authentifizierungstyp,
einen Importer für ein anderes Dateiformat oder eine Ansicht einer Antwort. Ein
Plugin ist eine WebAssembly-Datei (`.wasm`) und läuft abgeschottet: Es bekommt
JSON und antwortet mit JSON, ohne Netzwerk, ohne Dateien und ohne Uhr, und es
wird angehalten, wenn es zu lange läuft oder zu viel Speicher braucht. Ein
Plugin von jemand anderem kann die Anfrage ändern, die es signiert, oder die
Sammlung, die es importiert, und sonst nichts.

**Ansicht › Plugins…** listet auf, was installiert ist und was jedes Plugin
bietet. **Aus Datei installieren…** prüft eine `.wasm`-Datei und kopiert sie in
den Plugin-Ordner (`<config>/Apix/plugins`), **Entfernen** löscht sie, **Ordner
öffnen** zeigt den Ordner, und **Neu laden** liest ihn erneut, nachdem Sie
selbst Dateien hineinkopiert haben.

![Das Fenster Plugins](../en/images/plugins.png)

- **Authentifizierungstypen** erscheinen in der Liste des Reiters Auth als
  „… (Plugin)“, mit den Feldern, nach denen das Plugin fragt (`{{variables}}`
  funktionieren darin). Beim Senden der Anfrage bekommt das Plugin die fertige
  Anfrage (Methode, URL, Header, Body) und fügt Header oder Query-Parameter
  hinzu. Eine Anfrage, deren Plugin nicht installiert ist, sagt das und wird
  nicht gesendet.

  ![Eine von einem Plugin signierte Anfrage](../en/images/plugin-auth.png)

- **Ansichten** erscheinen neben Text, Tabelle und Diagramm über einer
  JSON-Antwort; das Plugin liest Status, Header und Body und antwortet mit einer
  Tabelle oder einem Text.

  ![Eine Ansicht aus einem Plugin](../en/images/plugin-view.png)

- **Importer** haben im Fenster Plugins eine Schaltfläche **Datei importieren…**:
  Die Datei wird zu einer neuen Sammlung daneben, die sich dann öffnet.

In einer Anfragedatei sieht eine Plugin-Authentifizierung so aus:

```toml
[auth.plugin]
plugin = "hmac-signature"

[auth.plugin.config]
key_id = "k1"
secret = "{{signing_secret}}"
```

In einem Terminal:

```sh
apix plugins list                     # installierte Plugins und was sie bieten
apix plugins install my-plugin.wasm
apix plugins remove my-plugin
apix import endpoints.csv --plugin csv-endpoints --out shop
apix send posts/list-posts.apix --view json-stats
```

Der Ordner `plugins/` des Repositorys enthält drei Beispiele, gebaut aus
`crates/apix-plugin-sdk/examples`: **hmac-signature** (signiert Anfragen mit
HMAC-SHA256 über Methode, Pfad und Body-Hash), **csv-endpoints** (eine CSV-Liste
aus `method,url,name,folder` wird zu Anfragen) und **json-stats**
(Feldstatistik einer JSON-Liste: Typen, Vorkommen, verschiedene Werte,
Wertebereiche).

### Ein Plugin schreiben

In Rust hängen Sie von `apix-plugin-sdk` ab, implementieren seinen Trait
`Plugin` und rufen `export!` auf:

```rust
use apix_plugin_sdk::{Plugin, export, serde_json::{Value, json}};

struct Hello;

impl Plugin for Hello {
    fn manifest() -> Value {
        json!({ "name": "hello", "version": "1.0.0",
                "visualize": { "label": "Hello" } })
    }
    fn call(_hook: &str, input: Value) -> Result<Value, String> {
        let size = input["body"].as_str().unwrap_or("").len();
        Ok(json!({ "text": format!("{size} bytes") }))
    }
}

export!(Hello);
```

Bauen Sie das Crate als `cdylib` mit
`cargo build --release --target wasm32-unknown-unknown` und installieren Sie die
`.wasm`. Über den `name` des Manifests (Kleinbuchstaben, Ziffern und `-`)
verweisen Anfragen auf das Plugin; es bietet beliebige der folgenden an:

| Manifest-Schlüssel | Der Hook bekommt | Er antwortet |
|---|---|---|
| `auth: {label, fields: [{name, label, secret}]}` | `{method, url, headers, body, config}` | `{headers: [[name, value]], query: [[name, value]]}` |
| `import: {label, extensions}` | `{file_name, content}` | `{name, requests: [{folder, name, method, url, headers, body}]}` |
| `visualize: {label}` | `{status, headers, body}` | `{title, text}` oder `{title, columns, rows}` |

Jede Sprache, die nach WebAssembly kompiliert, funktioniert. Das Modul
exportiert `memory`, `apix_alloc(len) -> ptr`, `apix_manifest() -> i64` und
`apix_call(hook_ptr, hook_len, input_ptr, input_len) -> i64`, wobei das `i64`
`ptr << 32 | len` einer UTF-8-JSON-Antwort ist, `{"ok": …}` oder
`{"error": "…"}`. Es darf nichts importieren, und jeder Aufruf läuft in einer
neuen Instanz.

## Befehlspalette

Strg+K (oder Strg+P) öffnet die Befehlspalette: Tippen Sie ein paar Buchstaben
eines Anfragenamens oder eines Befehls und drücken Sie Enter. Befehle lassen
sich in jeder Sprache auch über ihre englischen Namen finden.

![Die Befehlspalette](../en/images/palette.png)

## Einrichtung

Beim ersten Start öffnet Apix die **Einrichtung**, die optionalen Extras an
einem Ort; **Hilfe › Einrichtung…** öffnet sie erneut. Apix funktioniert ohne
jedes davon.

- **IP-Standortdatenbank**: verortet jeden Router auf der Routenkarte und in
  `apix trace`. **DB-IP Lite herunterladen** holt die kostenlose
  Städtedatenbank von DB-IP (etwa 130 MB, CC BY 4.0) nach `<config>/Apix/geo`;
  **Aktualisieren** holt den neuesten Monat. Nichts wird online abgefragt. In
  einem Terminal: `apix geo download`, `apix geo status`, `apix geo remove`.
- **KI-Assistent**: zeigt, ob er an ist und mit welchem Anbieter;
  **Einstellungen öffnen** springt zu seinen Zeilen.
- **Claude (MCP)**: **Mit Claude verbinden** (auch
  **Hilfe › Mit Claude verbinden (MCP)…**) für die geöffnete Sammlung, siehe
  unten.

![Einrichtung](../en/images/setup.png)

### Claude verbinden

**Mit Claude verbinden** macht die geöffnete Sammlung zu einem MCP-Server für
Claude, sodass Claude ihre Anfragen auflisten, senden und mit ihren Tests
ausführen kann, ohne Variablenwerte zu sehen (siehe
[MCP](workflows.md#einen-ki-agenten-ihre-sammlung-nutzen-lassen-mcp)).

- **Zu Claude Desktop hinzufügen** schreibt den Eintrag `apix-<collection>` in
  die `claude_desktop_config.json` von Claude Desktop. Seine anderen Server und
  Einstellungen bleiben erhalten, und die ursprüngliche Datei wird beim ersten
  Mal nach `.bak` kopiert; starten Sie Claude Desktop neu, um ihn zu verwenden.
  **Entfernen** nimmt den Eintrag wieder heraus.
- **Zu Claude Code hinzufügen** führt `claude mcp add --scope user …` aus,
  sodass die Sammlung in jedem Projekt verfügbar ist.
- **Nur auflisten (Claude kann nichts senden)** gibt Claude nur die Werkzeuge
  zum Auflisten (`--list-only`).
- Eintrag und Befehl werden beide mit **Kopieren** angezeigt, um sie von Hand
  hinzuzufügen.

![Mit Claude verbinden](../en/images/claude-connect.png)

Dasselbe in einem Terminal: `apix mcp my-api --add-to claude-desktop`,
`apix mcp my-api --add-to claude-code`,
`apix mcp my-api --remove-from-claude-desktop`.

## Einstellungen und Sprache

**Datei › Einstellungen…** (Strg+,):

- **Sprache:** English, Türkçe, العربية, Deutsch, Français, Русский, 日本語,
  简体中文 oder Español. Die Änderung gilt sofort. Beim ersten Start folgt Apix
  der Systemsprache. Auf Arabisch wird das Fenster gespiegelt: Die Seitenleiste
  ist rechts, und Menüs, Tabs und Formulare laufen von rechts, während URLs,
  Code und Schlüssel-Wert-Tabellen von links nach rechts bleiben.
- **Verschlüsselte Sammlungen sperren:** nach 5, 15 oder 30 Minuten, nach
  1 Stunde oder nie.
- **Geheime Umgebungswerte:** in einer `.local.toml`-Datei oder im Schlüsselbund
  des Systems (siehe [Variablen und Umgebungen](#variablen-und-umgebungen)).
- **In Produktion nachfragen:** vor DELETE, PUT oder PATCH nachfragen, solange
  eine „prod“-Umgebung aktiv ist (standardmäßig an).
- **Nach Updates suchen:** beim Start GitHub fragen, ob eine neuere Apix-Version
  erschienen ist (standardmäßig an). Nur diese eine Anfrage wird gesendet, ohne
  etwas, das Sie identifiziert, und nichts wird heruntergeladen: Eine neuere
  Version zeigt eine Benachrichtigung mit einer Schaltfläche **Herunterladen**,
  die ihre Seite öffnet. **Hilfe › Nach Updates suchen…** fragt jederzeit nach,
  und `apix update` tut dasselbe in einem Terminal.

![Eine neuere Version wird angekündigt](../en/images/update.png)

- **Verlauf:** an oder aus (siehe [Verlauf](#verlauf)).

Der Teil **Theme** der Einstellungen ändert das Aussehen von Apix sofort, ohne
Neustart:

- **Erscheinungsbild:** dunkel, hell oder **System folgen**, das umschaltet,
  wenn Windows oder macOS zwischen hellem und dunklem Modus wechselt.
- **Dunkles Theme:** Obsidian (die Vorgabe), Cyber Glass, Dracula, Nord oder
  Solarized Dark. **Helles Theme:** Tageslicht oder Solarized Light. Die Wahl
  eines Themes der jeweils anderen Art schaltet das Erscheinungsbild darauf um.
- **Akzentfarbe:** die des Themes oder Cyan, Blau, Lila, Pink, Rot, Orange, Gelb
  oder Grün, für Schaltflächen, Auswahl, Fokusrahmen und die Variablen-Chips.
  Helle Themes bekommen einen tieferen Farbton, und der Text auf dem Akzent ist
  dunkel oder weiß, je nachdem, was sich besser liest.
- **Transparenz:** ein Schieberegler von 0 (ein deckendes Fenster) bis 90 %:
  wie viel vom Desktop durchscheint, verschwommen wie Milchglas (Acrylic unter
  Windows, Vibrancy unter macOS). Der Fensterhintergrund wird damit
  durchsichtiger, Bereiche langsamer, damit Text einen Hintergrund behält, und
  Rahmen hellen sich auf wie die Kante von Glas; Menüs und Dialoge bleiben
  deckend. Helle Themes werden sanfter durchsichtig, da dunkler Text einen
  hellen Hintergrund braucht. Die Vorgabe ist 20 %.
- **Textgröße:** 90 %, 100 %, 110 %, 125 % oder 150 % der normalen Größe, für
  jeden Text im Fenster, sofort.
- **Schriftgröße für Code:** 11 bis 18 px für die Code-Editoren (Bodies,
  Skripte, die Ansicht Formatiert der Antwort); die Ansicht für sehr große
  Inhalte behält 13 px.
- **Dichte:** kompakte, normale oder großzügige Zeilen in der Seitenleiste.
- Die Code-Editoren nehmen ihre Farben aus dem Theme: Strings, Zahlen und
  Schlüsselwörter so, wie das Theme sie vorgibt, Typen und Funktionen in der
  Akzentfarbe, Kommentare und Satzzeichen gedämpft.

![Textgröße, Schriftgröße für Code und Dichte](../en/images/look.png)

- **Eigene Themes:** **Ordner öffnen** öffnet `<config>/Apix/themes` und
  schreibt beim ersten Mal `my-theme.toml` als Ausgangspunkt dorthin. Jede
  `.toml`-Datei in diesem Ordner ist ein Theme in den Listen; eine Datei mit dem
  Namen eines eingebauten Themes (`nord.toml`) ersetzt es. Eine Datei, die nicht
  gelesen werden kann, wird rot mit dem Grund genannt.

![Einstellungen](../en/images/settings.png)

Eine Theme-Datei legt einen Namen fest, ob es hell ist, und beliebige dieser
Farben (`#RRGGBB` oder `#RRGGBBAA` mit Deckkraft); die, die sie weglässt, kommen
aus Obsidian oder, bei einem hellen Theme, aus Tageslicht:

```toml
name = "Deep sea"
light = false

[colors]
background = "#03141FEE"      # das Fenster, ein Verlauf zu background_end
background_end = "#062A3AEE"
surface = "#05202DB3"         # Bereiche
raised = "#0A2B3BF7"          # Menüs und Dialoge
border = "#FFFFFF14"
accent = "#FF9F43"
text = "#E8F6FF"
muted = "#7FA3B8"
ok = "#34D399"                # 2xx, bestandene Tests
warning = "#FBBF24"
error = "#F87171"
string = "#86EFAC"            # JSON in der Ansicht für große Inhalte
number = "#FDBA74"
keyword = "#C4B5FD"
```

Theme-Dateien sind einfacher Text, ein Team kann sie also teilen; Apix liest sie
beim Start und beim Öffnen der Einstellungen.

Die Einstellungen werden in `settings.toml` im Apix-Ordner des
Konfigurationsverzeichnisses Ihres Systems gespeichert (`%APPDATA%\Apix` unter
Windows, `~/Library/Application Support/Apix` unter macOS, `~/.config/Apix`
unter Linux).

## Tastenkürzel

| Tasten | Aktion |
|---|---|
| Strg+Enter | Senden |
| Esc | Die Anfrage abbrechen |
| Strg+S | Speichern |
| Strg+N | Neue GET-Anfrage |
| Strg+O | Einen Ordner, ein Projekt oder eine Sammlung öffnen |
| Strg+K oder Strg+P | Befehlspalette |
| Strg+E | Variablen und Umgebungen |
| Strg+, | Einstellungen |
| Strg+H | Verlauf |
| Strg+Shift+R | Runner |
| Alt+1…7 | Die Methode wechseln |
| Strg+W | Den Tab schließen |
| Strg+Tab / Strg+Shift+Tab | Nächster / vorheriger Tab |
| Strg+Leertaste | Vorschläge in der GraphQL-Query (nach dem Abrufen des Schemas) |

Unter macOS verwenden Sie ⌘ statt Strg (Strg+Tab und Strg+Leertaste bleiben
Strg).
