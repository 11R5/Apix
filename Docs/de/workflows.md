<!-- translated from en/workflows.md sha256:ced46a4d6c27e0a9 -->

# Workflows

Schritt-für-Schritt-Anleitungen für häufige Aufgaben. Das
[Benutzerhandbuch](user-guide.md) beschreibt jeden Teil der App; die
[technischen Hinweise](technical.md) behandeln das Bauen und Messen von Apix.

- [Die erste Anfrage senden und speichern](#die-erste-anfrage-senden-und-speichern)
- [Eine Sammlung in git verwalten](#eine-sammlung-in-git-verwalten)
- [Mit dev, staging und production arbeiten](#mit-dev-staging-und-production-arbeiten)
- [Eine Sammlung mit Geheimnissen teilen](#eine-sammlung-mit-geheimnissen-teilen)
- [Von Postman umsteigen](#von-postman-umsteigen)
- [Eine Sammlung in CI ausführen](#eine-sammlung-in-ci-ausführen)
- [Eine API mit Monitoren überwachen](#eine-api-mit-monitoren-überwachen)
- [Mit OAuth 2.0 anmelden](#mit-oauth-20-anmelden)
- [Eine AWS-API aufrufen](#eine-aws-api-aufrufen)
- [Eine Datei hochladen, GraphQL senden](#eine-datei-hochladen-graphql-senden)
- [Einen gRPC-Dienst aufrufen](#einen-grpc-dienst-aufrufen)
- [Eine Anfrage aus dem Browser nachstellen](#eine-anfrage-aus-dem-browser-nachstellen)
- [Herausfinden, warum eine Anfrage langsam ist oder fehlschlägt](#herausfinden-warum-eine-anfrage-langsam-ist-oder-fehlschlägt)
- [Einen KI-Agenten Ihre Sammlung nutzen lassen (MCP)](#einen-ki-agenten-ihre-sammlung-nutzen-lassen-mcp)

## Die erste Anfrage senden und speichern

1. **Datei › Neue Sammlung…**, wählen Sie einen leeren Ordner und geben Sie
   einen Namen ein.
2. Strg+N öffnet eine neue GET-Anfrage. Tippen Sie
   `https://jsonplaceholder.typicode.com/users`.
3. Strg+Enter sendet sie; die Antwort erscheint rechts.
4. Strg+S speichert sie als neue Datei im Sammlungsordner. Klicken Sie sie in
   der Seitenleiste mit der rechten Maustaste an › **Umbenennen**, um ihr einen
   Namen wie „List users“ zu geben.

Die Datei ist einfaches TOML, das Sie lesen und bearbeiten können:

```toml
name = "List users"
url = "https://jsonplaceholder.typicode.com/users"
```

## Eine Sammlung in git verwalten

Eine Sammlung ist ein Ordner:

```
my-api/
├── apix.toml                Name der Sammlung, Variablen, gemeinsame Auth
├── environments/
│   ├── staging.toml         gemeinsame Werte
│   └── staging.local.toml   geheime Werte (von git ignoriert)
├── users/
│   ├── folder.toml          optional: Auth für die Anfragen in diesem Ordner
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

Committen Sie alles außer `*.local.toml`; Apix trägt dieses Muster selbst in
`.gitignore` ein, wenn es zum ersten Mal ein Geheimnis schreibt. Jede Anfrage
ist eine eigene kleine Datei, sodass Reviews und Merges lesbar bleiben.

## Mit dev, staging und production arbeiten

1. Öffnen Sie **Umgebung › Variablen und Umgebungen…** (Strg+E).
2. Legen Sie Umgebungen namens `dev`, `staging` und `prod` an, jede mit einer
   `base_url`.
3. Verwenden Sie `{{base_url}}/users` in Ihren Anfragen.
4. Wählen Sie die Umgebung in der Seitenleiste unter dem Namen der Sammlung.

Werte, die nicht committet werden sollen (Tokens, Passwörter), bekommen das
Häkchen **Geheim**. Jedes Teammitglied behält seine eigene `*.local.toml`. Eine
auf Ebene der Sammlung definierte Variable ist ein Standardwert, den Umgebungen
überschreiben können.

## Eine Sammlung mit Geheimnissen teilen

Wenn die ganze Sammlung sensibel ist (interne URLs, Beispieldaten von Kunden,
langlebige Schlüssel), verschlüsseln Sie sie:

1. **Datei › Diese Sammlung verschlüsseln…** und wählen Sie ein Passwort.
2. Bewahren Sie den angezeigten **Wiederherstellungsschlüssel** in Ihrem
   Passwortmanager auf.
3. Committen Sie den Ordner. Auf der Festplatte ist er eine Menge
   verschlüsselter Objekte; sogar die Dateinamen sind verborgen.

Teammitglieder öffnen den Ordner in Apix und geben das Passwort ein. In einem
Terminal oder in CI:

```sh
APIX_PASSWORD='…' apix run my-api --env staging
```

`apix decrypt my-api --out my-api-plain` schreibt eine unverschlüsselte Kopie,
mit `--recovery` wird statt des Passworts der Wiederherstellungsschlüssel
verwendet.

## Von Postman umsteigen

1. Exportieren Sie in Postman die Sammlung (Collection v2.1) und jede Umgebung.
2. In Apix **Datei › Importieren…** oder ziehen Sie die Dateien auf das
   Fenster. Oder in einem Terminal:

   ```sh
   apix import "My API.postman_collection.json"            # erstellt ./my-api
   apix import "Staging.postman_environment.json" --into my-api
   ```

3. Lesen Sie den Importbericht: Alles, was nicht übernommen werden konnte, ist
   dort aufgeführt (zum Beispiel NTLM-Auth oder gespeicherte Beispielantworten).
4. Pre-Request- und Testskripte werden als `.js`-Dateien neben ihren Anfragen
   behalten; Apix führt sie noch nicht aus.
5. Für OAuth 2.0 mit dem Authorization-Code-Grant registrieren Sie bei Ihrem
   Identitätsanbieter eine Loopback-Redirect-URL wie
   `http://127.0.0.1:8765/callback` und tragen sie im Reiter Auth ein.

Sie ziehen einen ganzen Workspace um? Verwenden Sie in Postman **Settings ›
Data › Export data** und öffnen Sie dann den Ordner (oder die Zip-Datei) in
Apix oder ziehen Sie ihn hinein: Jede Sammlung, Umgebung und globale Variable
kommt als ein Projekt herüber (`apix import postman-export.zip` tut dasselbe).
Teammitglieder, die bei Postman bleiben, bekommen Ihre Sammlung mit **Nach
Postman exportieren…** zurück.

## Eine Sammlung in CI ausführen

`apix run` sendet jede Anfrage einer Sammlung in der Reihenfolge der
Seitenleiste und endet mit einem Exit-Code ungleich null, wenn eine
fehlschlägt; so funktioniert es als CI-Schritt:

```sh
apix run my-api --env staging --var token="$API_TOKEN"
```

Anfragen mit Tests (Skripte oder Prüfungen) bestehen oder scheitern an ihren
Tests; die anderen an ihrem Statuscode. Die Ausgabe listet jeden Test unter
seiner Anfrage auf:

```text
create.apix                              201 Created  466.7 ms · 42 B
  │ new id 101
  ✓ created
get.apix                                 200 OK  174.7 ms · 292 B
  ✓ status 200
  ✗ has the right id expected 1 to equal 2

1 passed, 1 failed (1 failed tests)
```

Was ein Skript setzt (`pm.environment.set`, `pm.variables.set`), gilt für die
nächsten Anfragen des Laufs weiter, und `pm.execution.setNextRequest("name")`
springt zu einer anderen Anfrage (`null` beendet die Iteration).

Optionen:

| Option | Was sie tut |
|---|---|
| `apix run my-api/users` | Nur die Anfragen in einem Ordner (oder eine Anfragedatei) |
| `-d, --data users.csv` | Ein datengetriebener Lauf: Jede CSV- oder JSON-Zeile ist eine Iteration, ihre Spalten sind Variablen (`{{email}}`, `pm.iterationData.get("email")`) |
| `-n, --iterations 5` | Die Anfragen 5-mal durchlaufen (Standard: einmal pro Datenzeile, sonst einmal) |
| `--delay 200` | 200 ms zwischen Anfragen warten |
| `--bail` | Bei der ersten fehlschlagenden Anfrage anhalten |
| `-p, --parallel 4` | 4 Iterationen gleichzeitig ausführen, wie 4 Benutzer; jede sendet ihre Anfragen der Reihe nach und behält für sich, was ihre Skripte setzen |
| `--junit report.xml` | Ein JUnit-XML-Bericht, den CI-Dienste als Testliste anzeigen |
| `--json report.json` / `--html report.html` | Alles als JSON oder eine Seite, die sich in jedem Browser öffnet |
| `--markdown report.md` | Eine Zusammenfassung für einen Pull-Request-Kommentar oder eine CI-Job-Zusammenfassung (siehe unten) |
| `--changed` / `--changed=origin/main` | Nur die Anfragen, die eine Änderung betrifft (siehe unten) |
| `--timeout 10`, `-k`, `--proxy URL` | Die Transporteinstellungen der Anfragen überschreiben |
| `--env-file .env` | Variablen aus einer dotenv-Datei (siehe unten) |

Eine Datendatei sieht so aus; Werte von außerhalb der Sammlung (`--var` und die
unten genannten) haben Vorrang vor ihren Spalten:

```csv
id,name
1,Leanne Graham
2,Ervin Howell
```

### Geheimnisse in CI

Geheimnisse bleiben außerhalb von git (`.local.toml`-Dateien werden ignoriert,
der Schlüsselbund liegt auf Ihrem Rechner), daher übergibt eine Pipeline sie
auf eine von drei Arten, wobei jede Vorrang vor der vorherigen hat:

1. `--env-file .env`: eine dotenv-Datei mit Zeilen `NAME=value` (`#`-Kommentare,
   Anführungszeichen und `export` erlaubt).
2. Umgebungsvariablen `APIX_VAR_<NAME>`: `APIX_VAR_TOKEN` füllt `{{token}}`.
   CI-Geheimnisse werden meist in Großbuchstaben geschrieben, daher passt ein
   Name zu einer Variablen, die die Sammlung definiert oder ihre Anfragen
   verwenden, egal in welcher Schreibweise.
3. `--var name=value`.

Alle drei haben Vorrang vor den Werten der Umgebung (und vor den Spalten einer
Datendatei). `apix` gibt auf stderr aus, welche Namen es woher genommen hat,
nie die Werte.

```sh
APIX_VAR_TOKEN="$API_TOKEN" apix run api --env staging
```

### GitHub Actions

Das Repository ist auch eine GitHub Action. Sie führt die Sammlung aus und
schreibt JUnit- und HTML-Berichte. An einem Release-Tag verwendet, lädt sie das
`apix` dieses Releases herunter (Sekunden); an einem Branch oder Commit baut sie
`apix` beim ersten Mal und speichert es im Cache, und `apix-version: latest`
(oder `0.1.1`) lädt stattdessen ein Release herunter:

```yaml
- uses: 11R5/Apix@v0.1.1        # ein Release: apix wird heruntergeladen, nicht gebaut
  with:
    path: api
    environment: staging        # außerdem: data, iterations, delay, bail, env-file, junit, html, json, markdown, changed, diff, comment, args, apix-version
  env:
    APIX_VAR_TOKEN: ${{ secrets.API_TOKEN }}
```

[examples/ci/github-actions.yml](../../examples/ci/github-actions.yml) ist ein
vollständiger Workflow: Er führt die Sammlung einmal pro Umgebung aus (eine
Matrix aus `test` und `staging`), zeigt die JUnit-Ergebnisse auf der Seite des
Laufs und bewahrt die HTML-Berichte auf. Die Action fügt außerdem ihre
Markdown-Zusammenfassung zur Seite des Jobs hinzu.

## Nur was sich geändert hat: git-bewusste Läufe

Eine Sammlung ist ein Ordner in git, daher kann `apix` git fragen, was eine
Änderung betroffen hat. `--changed` sendet nur diese Anfragen:

```sh
apix run api --changed                 # was noch nicht committet ist
apix run api --changed=origin/main     # was dieser Branch geändert hat, seit er von main abzweigte
```

Eine Anfrage läuft erneut, wenn sich ihre eigene Datei oder ihre Skripte
geändert haben, wenn sich eine `folder.toml` oder ein Ordnerskript über ihr
geändert hat (jede Anfrage in diesem Ordner), wenn sich `apix.toml`, ein
Sammlungsskript, eine Umgebung oder die Projektdatei geändert hat (jede
Anfrage), oder wenn sie eine andere geänderte Datei nennt (eine Body-Datei,
eine `.proto`). Neue Dateien zählen schon, bevor sie committet sind; gelöschte
Anfragen werden aufgelistet. Ohne Wert vergleicht `--changed` unter GitHub
Actions mit dem Basis-Branch des Pull Requests (`origin/$GITHUB_BASE_REF`) und
sonst mit `HEAD`. Schreiben Sie den Wert mit `=`, damit er nicht mit dem
auszuführenden Ordner verwechselt werden kann.

```text
2 of 3 requests changed since origin/main
  users/create-user.apix  ← users/create-user.test.js
  users/list-users.apix
```

Wie bei einem Pull Request beginnt der Vergleich dort, wo der Branch von der
Basis abzweigte, sodass das, was inzwischen auf main gelandet ist, nicht zählt.
Dafür braucht es die Historie: In einem flachen Klon (der Standard von
`actions/checkout`) warnt `apix` und vergleicht mit der Basis selbst; checken
Sie mit `fetch-depth: 0` aus. Verschlüsselte Sammlungen können nicht verglichen
werden, weil git nur ihre verschlüsselten Dateien sieht.

### Was sich geändert hat: `apix diff`

`apix diff` zeigt eine Änderung so, wie ein Reviewer sie liest: die Felder
einer Anfrage statt TOML-Zeilen, Header und Query-Parameter nach Name, Body und
Skripte Zeile für Zeile, Umgebungen nach Variable. Eine neu formatierte Datei
ist keine Änderung, und eine Anfrage, die ohne weitere Bearbeitung verschoben
oder umbenannt wurde, ist eine einzige Verschiebung.

```sh
apix diff                       # HEAD gegen die Dateien im jetzigen Zustand
apix diff main                  # main gegen jetzt
apix diff v1.2 v1.3 -C api      # zwischen zwei Tags, für die Sammlung in api/
```

```text
Between main and the working tree: 1 script, 1 request

A  script  users/create-user.test.js
     + pm.test("ok", () => pm.response.to.have.status(200));
M  GET List users  users/list-users.apix
     query._limit: 3 → 5
     headers.Accept: (none) → application/json
```

`--markdown` gibt es für einen Pull Request aus, `--json` für Werkzeuge, und
`--exit-code` endet mit 1, wenn sich etwas geändert hat.

### Eine Zusammenfassung im Pull Request

`--markdown report.md` schreibt den Lauf als GitHub-Markdown: die Summen, eine
Zeile pro Anfrage und, darunter aufgeklappt, was fehlschlug und warum. Seine
erste Zeile ist eine versteckte Markierung mit dem Namen der Sammlung, sodass
ein CI-Schritt den Kommentar finden kann, den er beim letzten Mal geschrieben
hat, und ihn bearbeitet, statt einen weiteren hinzuzufügen.

Die Action erledigt all das. Bei einem Pull Request sendet sie nur, was der
Pull Request betrifft, fügt `apix diff` unter der Zusammenfassung hinzu und
hält einen Kommentar im Pull Request aktuell:

```yaml
on: pull_request
permissions:
  contents: read
  pull-requests: write          # für den Kommentar
jobs:
  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0        # Historie, um zu finden, wo der Branch begann
      - uses: 11R5/Apix@main
        with:
          path: api
          changed: "true"       # oder ein Commit, Branch oder Tag
          diff: "true"
          comment: "true"
```

Bei einem Push vergleicht `changed: "true"` mit dem Commit vor dem Push. Die
Zusammenfassung wird außerdem in `apix-report.md` geschrieben (Eingabe
`markdown`) und auf der Seite des Jobs angezeigt.

In der Desktop-App zeigt die Statusleiste den Branch und wie viele Dateien sich
seit dem letzten Commit geändert haben, und die Seitenleiste markiert geänderte
Anfragen mit **M** (geändert) oder **A** (neu); siehe das
[Benutzerhandbuch](user-guide.md#git).

### Docker

Für andere CI-Systeme gibt es ein Container-Image, das nur den Befehl `apix`
enthält (veröffentlicht unter `ghcr.io/11r5/apix`, wenn eine Version getaggt
wird; selbst bauen mit `docker build -t apix .`):

```sh
docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging --junit reports/apix.xml
```

[examples/ci/gitlab-ci.yml](../../examples/ci/gitlab-ci.yml) und
[examples/ci/azure-pipelines.yml](../../examples/ci/azure-pipelines.yml) tun
dasselbe in GitLab CI (mit dem Image) und Azure Pipelines (bauen `apix` einmal
und speichern es im Cache); beide veröffentlichen die JUnit-Ergebnisse.

Der **Runner** der Desktop-App (Strg+Shift+R) führt dieselben Läufe mit
einem Fenster aus; siehe das [Benutzerhandbuch](user-guide.md#runner).

`apix send <file>` sendet eine Anfrage und gibt die Antwort aus; fügen Sie `-i`
für die Header und `--network` für die Verbindungsdetails hinzu.

## Eine API mit Monitoren überwachen

Ein Monitor führt eine Sammlung (oder einen Ordner oder eine Anfrage daraus)
nach Zeitplan aus und benachrichtigt einen Webhook, wenn sie anfängt
fehlzuschlagen und wenn sie wieder besteht. Monitore stehen in der `apix.toml`
der Sammlung, von Hand geschrieben oder über **Ansicht › Monitore…** in der App
(siehe das [Benutzerhandbuch](user-guide.md#monitore)):

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "health.apix"
environment = "production"
webhook = "{{alerts_url}}"
```

`apix monitor` führt sie aus, solange es läuft, jeden nach seinem eigenen
Zeitplan, und liest `apix.toml` dabei erneut ein, sodass Änderungen wirksam
werden:

```sh
APIX_VAR_ALERTS_URL=https://hooks.slack.com/… apix monitor api
```

```text
Watching 2 monitor(s) of Shop API; logs in ~/.config/Apix/monitors (Ctrl+C stops)
09:30:05Z ✓ Health                 1 passed · 166 ms
                         next at 09:35:05Z
09:30:05Z ✗ Orders                 1 of 3 failed · 412 ms
    orders/list.apix: got 500 Internal Server Error
    webhook told: failing
```

| Option | Was sie tut |
|---|---|
| `--once` | Jeden Monitor einmal ausführen und beenden, mit 1, wenn einer fehlschlägt: für cron, die Aufgabenplanung oder einen CI-Zeitplan |
| `--list` | Die Monitore und ihre letzten Ergebnisse |
| `--only NAME` | Nur dieser Monitor (wiederholbar); mit `--once` läuft er auch, wenn er ausgeschaltet ist |
| `--var`, `--env-file` | Variablen über den Umgebungen der Monitore, wie bei `apix run` (`APIX_VAR_*` werden ebenfalls gelesen) |
| `--log-dir DIR` | Das Protokoll woanders aufbewahren (in CI ein Ordner im Cache) |
| `--timeout`, `-k`, `--proxy`, `--cacert` | Transporteinstellungen, wie bei `apix run` |

Das Protokoll bewahrt die letzten 500 Ergebnisse jedes Monitors in
`<config>/Apix/monitors` auf; die Desktop-App liest dieselben Dateien und zeigt
daher, was `apix monitor` auf demselben Rechner gefunden hat. Ein Webhook wird
nur benachrichtigt, wenn der Zustand eines Monitors wechselt (fehlschlagend nach
bestanden oder beim ersten Lauf; bestanden nach fehlschlagend), mit einem
JSON-Body, den Slack, Discord, Mattermost und Google Chat als Nachricht
anzeigen.

Auf einem Server starten Sie `apix monitor api` als Dienst oder tragen
`apix monitor api --once` in cron ein (`*/5 * * * *`). Auf GitHub erledigt das
ein geplanter Workflow:
[examples/ci/monitors.yml](../../examples/ci/monitors.yml) führt die Monitore
alle 15 Minuten über die Action aus (`monitor: "true"`) und speichert das
Protokoll im Cache, damit Erholungen bemerkt werden.

## Mit OAuth 2.0 anmelden

**Client Credentials** (Dienst zu Dienst):

1. Reiter Auth › **OAuth 2.0**, Grant-Typ **Client Credentials**.
2. Füllen Sie **Token-URL**, **Client-ID**, **Client Secret** und **Scope**
   aus. Verwenden Sie Variablen (`{{client_secret}}`, als geheim markiert)
   statt wörtlicher Geheimnisse.
3. Senden. Apix holt ein Token, verwendet es danach wieder und erneuert es.

**Authorization Code mit PKCE** (ein Benutzer meldet sich an):

1. Registrieren Sie `http://127.0.0.1:<port>/callback` als Redirect-URL für
   Ihren Client (oder lassen Sie **Redirect-URL** leer, wenn der Anbieter jeden
   Loopback-Port akzeptiert).
2. Grant-Typ **Authorization Code**, füllen Sie **Auth-URL**, **Token-URL**,
   **Client-ID** und **Scope** aus; lassen Sie **PKCE verwenden** angehakt.
3. Drücken Sie **Neues Token holen** oder senden Sie. Ihr Browser öffnet die
   Anmeldeseite; nachdem Sie sich angemeldet haben, erscheint das Token in Apix.

Legen Sie OAuth 2.0 einmal für die Sammlung fest (Rechtsklick ›
**Authentifizierung…**), und jede Anfrage, die ihre Auth erbt, verwendet
dasselbe Token. Tokens bleiben im Speicher; nach einem Neustart von Apix melden
Sie sich erneut an.

## Eine AWS-API aufrufen

1. Reiter Auth › **AWS Signature**.
2. **Access Key** und **Secret Key** als Variablen (`{{aws_access_key}}`,
   `{{aws_secret_key}}` als geheim markiert), **Session-Token** für temporäre
   Anmeldedaten, **Region** (zum Beispiel `eu-central-1`) und **Dienst** (zum
   Beispiel `execute-api`, `s3`, `lambda`).
3. Senden. Jede Anfrage wird kurz vor dem Absenden mit Signature Version 4
   signiert.

**Kopieren als › cURL** schreibt `curl --aws-sigv4`, sodass curl sie beim
Ausführen erneut signiert; die PowerShell-Kopie enthält eine beim Kopieren
erstellte Signatur, die einige Minuten gültig ist.

## Eine Datei hochladen, GraphQL senden

**Multipart:** Body › **Multipart**, fügen Sie ein Feld hinzu, stellen Sie es
auf **Datei** um und wählen Sie die Datei. Textfelder bleiben **Text**.
Bewahren Sie Testdateien innerhalb der Sammlung auf: Der Pfad wird relativ zur
Anfrage gespeichert, sodass er auf jedem Rechner funktioniert.

**Binär:** Body › **Binär** sendet eine Datei als gesamten Body.

**GraphQL:** Body › **GraphQL**, schreiben Sie die Abfrage, die Variablen als
JSON darunter. Apix sendet `{"query": …, "variables": …}` mit
`Content-Type: application/json`. Legen Sie zuerst die Auth fest und drücken
Sie dann **Schema abrufen**: Der Abfrage-Editor schlägt von da an Felder,
Argumente und Enum-Werte vor, und beim Darüberfahren erscheint ihre
Dokumentation. Das Schema wird pro URL aufbewahrt, sodass es beim nächsten
Öffnen der Anfrage schon da ist. Um eine Schemaänderung in einem Pull Request
zu prüfen, speichern Sie `apix schema api/graphql.apix > schema.graphql` im
Repository und lassen den Diff zeigen, was sich geändert hat.

## Einen gRPC-Dienst aufrufen

1. **Anfrage › Neue Anfrage**, dann tippen Sie die Adresse mit `grpc://` (oder
   `grpcs://` hinter TLS), zum Beispiel `grpc://localhost:50051`.
2. Drücken Sie im Reiter **Nachricht** auf **Methoden laden**. Ist die
   Server-Reflection ausgeschaltet, fügen Sie zuerst die `.proto`-Dateien hinzu
   (**Datei hinzufügen**); bewahren Sie sie in der Sammlung auf, damit die Pfade
   auf jedem Rechner funktionieren.
3. Wählen Sie die Methode, drücken Sie **Beispielnachricht** und füllen Sie die
   Werte aus.
4. Legen Sie das Token unter **Auth** (Bearer) fest oder fügen Sie Metadaten
   unter **Header** hinzu, dann **Senden**.

In einer Pipeline funktioniert dieselbe Datei mit der Kommandozeile:

```sh
apix send api/say-hello.apix --env staging   # Exit-Code 1, außer der Status ist OK
apix schema api/say-hello.apix > greeter.txt   # was der Dienst anbietet, zum Vergleichen in Reviews
```

## Eine Anfrage aus dem Browser nachstellen

1. Klicken Sie in den Entwicklerwerkzeugen des Browsers mit der rechten
   Maustaste auf eine Anfrage › **Copy as cURL** (bash oder cmd, beides
   funktioniert).
2. Klicken Sie in Apix in das URL-Feld und fügen Sie ein. Methode, URL, Query,
   Header, Cookies, Auth und Body werden ausgefüllt.
3. Senden, ändern, speichern.

Umgekehrt liefert Ihnen die Kopier-Schaltfläche neben **Senden** die Anfrage
als `curl`, `curl.exe` oder PowerShell, um sie in ein Terminal oder einen
Fehlerbericht einzufügen.

## Herausfinden, warum eine Anfrage langsam ist oder fehlschlägt

1. Senden Sie die Anfrage und öffnen Sie den Reiter **Netzwerk** der Antwort.
2. Der Zeitbalken zeigt, ob die Zeit in DNS, den Aufbau der TCP-Verbindung, den
   TLS-Handshake, das Warten auf den Server oder das Herunterladen geht. Eine
   langsame TCP-Verbindung deutet auf das Netzwerk hin; ein langsamer Handshake
   (verglichen mit TCP, das einen Roundtrip braucht) auf den Server oder den
   zusätzlichen Roundtrip von TLS 1.2.
3. Prüfen Sie die entfernte Adresse und die lokale Schnittstelle: Ein VPN oder
   Proxy zeigt sich dort.
4. Weiterleitungen sind der Reihe nach aufgelistet. Unter Verschlüsselung: die
   TLS-Version, die Cipher-Suite und ob HTTP/2 vereinbart wurde; die
   Zertifikatskette zeigt für jedes Zertifikat den Aussteller und die
   verbleibenden Tage, sodass ein fehlendes Zwischenzertifikat oder eines kurz
   vor dem Ablauf auffällt. Ein komprimierter Body zeigt, wie viel er gespart
   hat.

Derselbe Bericht ist in einem Terminal verfügbar:

```sh
apix send my-api/health.apix --network
```

## Einen KI-Agenten Ihre Sammlung nutzen lassen (MCP)

`apix mcp` stellt eine Sammlung über das Model Context Protocol bereit, sodass
ein KI-Agent (Claude, der Assistent eines Editors, jeder MCP-Client) mit Ihrer
API so arbeiten kann, wie Sie es in Apix tun. Es spricht MCP über
Standardeingabe und -ausgabe; der Agent startet es. Mit Claude Code:

```bash
claude mcp add my-api -- apix mcp /path/to/my-api
```

Andere Clients nehmen denselben Befehl in ihre Konfiguration auf, zum Beispiel:

```json
{ "mcpServers": { "my-api": { "command": "apix", "args": ["mcp", "/path/to/my-api"] } } }
```

Der Agent bekommt vier Tools:

| Tool | Was es tut |
|---|---|
| `list_requests` | die Anfragen (Pfad, Methode, Name, URL mit `{{variables}}`), der ganzen Sammlung oder eines Ordners |
| `list_environments` | die Umgebungen und die Namen ihrer Variablen, nie die Werte |
| `send_request` | sendet eine Anfrage mit ihren Skripten und Tests, in einer Umgebung, mit Variablen, die der Agent setzt; liefert Status, Zeit, Header, Body (bis zu 64 KB) und Testergebnisse |
| `run_requests` | führt einen Ordner oder die ganze Sammlung mit ihren Tests aus und meldet, was bestanden hat |

Nur Anfragen innerhalb dieser Sammlung sind erreichbar. `--list-only` behält
die beiden Auflisten-Tools und lässt die sendenden weg. Geheimnisse bleiben, wo
sie sind: Ein Agent kann verlangen, dass eine Anfrage mit einem Token gesendet
wird, bekommt aber nie den Wert des Tokens zu sehen (eine Antwort, die ihn
zurückgibt, ist etwas anderes, also achten Sie darauf, was Sie einen Agenten
senden lassen). Für eine verschlüsselte Sammlung geben Sie das Passwort in
`APIX_PASSWORD` an.
