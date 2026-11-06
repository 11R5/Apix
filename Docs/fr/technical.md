<!-- translated from en/technical.md sha256:20fbf0a60187a9e2 -->

# Notes techniques

Comment Apix est construit, de quoi il dépend et comment le mesurer.
L’[architecture](architecture.md) explique la conception ; cette page en est le
côté pratique.

## Organisation du dépôt

```
crates/
├── apix-core       modèle de requête, variables, format sur disque, projets, signature (sign.rs), schémas GraphQL, git (git.rs, diff.rs)
├── apix-http       le moteur HTTP (reqwest), trace réseau, jetons OAuth 2.0, SSE, WebSocket, Socket.IO, récupération du schéma GraphQL
├── apix-import     import de Postman, Insomnia, OpenAPI, HAR et WSDL
├── apix-codegen    documentation d’API (HTML, Markdown), « Copier en » (curl, curl.exe, PowerShell, code en 10 langages) et collage de curl
├── apix-vault      collections chiffrées
├── apix-script     scripts de pré-requête et de test (QuickJS)
├── apix-runner     exécutions de collection, fichiers de données, rapports JUnit/JSON/HTML/Markdown
├── apix-grpc       appels gRPC : fichiers .proto, réflexion du serveur ou définitions tirées du code et du JSON, messages en JSON
├── apix-proto      autres protocoles : MQTT
├── apix-mock       un serveur fictif local qui répond à partir des exemples enregistrés
├── apix-capture    un proxy d’enregistrement qui enregistre les requêtes qui le traversent
├── apix-route      la route vers un serveur et l’emplacement de ses sauts
├── apix-plugins    extensions WebAssembly (wasmi) : chargement, bac à sable, hooks d’auth, d’import et de vue
├── apix-plugin-sdk écrire des extensions en Rust ; ses examples/ sont les extensions de plugins/
├── apix-cli        la commande `apix`
└── apix-desktop    l’application de bureau (GPUI) ; textes de l’interface dans locales/
tools/
├── icongen         construit les icônes de l’application à partir de Logo/
└── docs-check      liste les traductions de la documentation périmées
vendor/
└── gpui-pre-windows  le backend Windows de gpui avec une correction du texte de droite à gauche
scripts/
├── package-*.sh/.ps1  paquets de version : installateur et zip Windows, .dmg macOS, .deb, tarball, paquet Arch
└── measure.ps1     mesures de performance pour le README
examples/
├── jsonplaceholder une petite collection que l’application ouvre quand elle est lancée depuis le dépôt
└── ci/             exemples GitHub Actions (aussi des moniteurs planifiés), GitLab CI et Azure Pipelines
plugins/            extensions d’exemple (.wasm), construites à partir des exemples d’apix-plugin-sdk
site/               la page de téléchargement (GitHub Pages), qui lit la dernière version sur GitHub
action.yml          la GitHub Action (`uses: 11R5/Apix@…`)
Dockerfile          la ligne de commande `apix` en image de conteneur
.github/workflows/  CI sur trois systèmes, et l’image publiée sur les tags de version
Docs/
├── languages.toml  langues de la documentation
├── en/ tr/         guide de l’utilisateur, architecture, notes techniques, workflows
└── IS-PLANI.md, ROADMAP.md   le plan (détail en turc, résumé en anglais)
```

## Compilation

Prérequis : [Rust](https://rustup.rs) stable (le dépôt est testé avec la 1.99).

- **Windows :** la chaîne d’outils MSVC (Visual Studio ou Build Tools avec
  « Développement Desktop en C++ » et un SDK Windows 10/11).
- **macOS :** les outils en ligne de commande de Xcode.
- **Linux :** les paquets de développement X11/Wayland, Vulkan et fontconfig
  dont GPUI a besoin (le README de gpui-kit les liste par distribution) ; pas
  encore testé.

```sh
cargo test                       # moteur, CLI, import, codegen, vault, vérification de la doc
cargo run -p apix-desktop        # l’application de bureau (debug)
cargo test -p apix-desktop       # tests unitaires du bureau (traductions, chemins, …)
cargo build --release            # CLI optimisée
cargo build --release -p apix-desktop
```

`cargo build` et `cargo test` sans `-p` laissent de côté l’application de
bureau, qui entraîne toute la pile GPU ; compilez-la explicitement. Les
dépendances sont optimisées même en debug, pour que l’interface reste fluide
pendant le développement.

Les extensions d’exemple de `plugins/` ont besoin une fois de la cible
WebAssembly (`rustup target add wasm32-unknown-unknown`), puis de
`cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown` ;
copiez `target/wasm32-unknown-unknown/release/examples/*.wasm` dans `plugins/`
(avec `-` à la place de `_` dans les noms). Les tests d’`apix-plugins` chargent
ces fichiers. `wasmi` est optimisé et compilé sans assertions de debug, même en
debug (`[profile.dev.package.wasmi]`) : sa boucle d’interprétation repose sur
des appels terminaux qui, sinon, débordent la pile.

Le profil release utilise le LTO complet (fat), une seule unité de génération de
code et des symboles retirés. Une compilation release complète de l’application
de bureau prend quelques minutes.

## Dépendances principales

| Domaine | Crates |
|---|---|
| Interface | `gpui-kit` 0.7 (GPUI et sa bibliothèque de composants), grammaires tree-sitter pour JSON et GraphQL |
| HTTP | `reqwest` 0.12 avec rustls, `tokio`, `hyper-util`, couches `tower` pour les temps |
| gRPC | `tonic` 0.14 (client, sur les mêmes hyper et rustls), `prost-reflect` (messages connus à l’exécution, JSON), `protox` (lit les fichiers .proto sans `protoc`), `regex` (lecture du code généré et des types), `tonic-reflection` (le client de réflexion), `rustls` et `tokio-rustls` pour le gRPC sans vérification des certificats ; `rcgen` crée des certificats de test (tests uniquement) |
| Détails réseau | `x509-parser` (certificats), `sha2` (empreintes), `netdev` (interfaces, passerelle, serveurs DNS) ; détails TLS par les propres hooks de rustls (aucune crate de plus) ; `flate2` et `brotli-decompressor` décodent les corps compressés (déjà compilés pour reqwest) ; la route à partir de `tracert`, `traceroute` ou `tracepath` du système, les lieux à partir de `maxminddb` |
| Signature | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Chiffrement | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize`, `keyring` (magasin d’identifiants du système) |
| Scripts | `rquickjs` (QuickJS, compilé depuis le C avec le compilateur du système) ; les bibliothèques de `require` sont du JavaScript propre à Apix, le hachage passe par `sha1`, `sha2`, `md-5`, `hmac` |
| Fichiers | `serde`, `toml`, `serde_json`, `notify` (surveillance des fichiers), `trash` ; git par le programme `git` installé (aucune bibliothèque) |
| Réponses | `serde_json_path` (JSONPath, RFC 9535) et `jaq-core`, `jaq-std`, `jaq-json` (jq) pour le filtre ; `regex` pour chercher dans les grands corps |
| Extensions | `wasmi` 2 (un interpréteur WebAssembly avec limites de carburant et de mémoire) |
| Langues | `rust-i18n` (partagé avec gpui-kit), `sys-locale` |

## Fichiers qu’Apix écrit hors des collections

| Fichier | Contenu |
|---|---|
| `<config>/Apix/settings.toml` | langue, apparence, thème sombre et clair, couleur d’accent, transparence, délai de verrouillage automatique, historique activé ou non, où vont les secrets d’environnement, si les moniteurs tournent pendant que l’application est ouverte |
| `<config>/Apix/capture-ca.pem`, `capture-ca.key` | l’autorité de certification locale du proxy de capture, créée la première fois qu’une connexion HTTPS est ouverte ; approuvée seulement si vous l’ajoutez vous-même |
| `<config>/Apix/geo/*.mmdb` | la base de localisation des adresses IP pour la carte de la route (téléchargée depuis la fenêtre Route et carte, ou déposée là) |
| `<config>/Claude/claude_desktop_config.json` | écrit seulement par Connecter à Claude : l’entrée `apix-<collection>` sous `mcpServers` (l’original conservé en `.json.bak`) |
| `<config>/Apix/monitors/` | les 500 derniers résultats de chaque moniteur, une ligne JSON par exécution (`<hash of the collection folder>-<monitor name>.jsonl`), écrits par `apix monitor` et par l’application |
| `<config>/Apix/state.toml` | collections ouvertes récemment, l’environnement choisi et les onglets ouverts par collection ou projet (pas pour les collections chiffrées) |
| `<config>/Apix/cookies.json` | la boîte à cookies (les cookies de session ne sont pas conservés) |
| `<config>/Apix/graphql/*.json` | le dernier schéma GraphQL récupéré pour chaque point de terminaison (la réponse d’introspection du serveur), nommé d’après un hash de l’URL |
| `<config>/Apix/themes/*.toml` | vos propres thèmes (écrit seulement quand vous appuyez sur **Ouvrir le dossier** dans Paramètres et que le dossier est vide : un exemple) |
| Magasin d’identifiants du système, service `Apix` | seulement sur demande : mots de passe mémorisés des collections chiffrées (`vault:<folder>`) et secrets d’environnement (`env:<id>`, plus `env:<id>#1`, … pour les valeurs longues), clés de l’assistant IA (`ai:anthropic-api-key`, `ai:openai-api-key`, `ai:google-api-key`) |
| `<config>/Apix/plugins/*.wasm` | extensions installées (la fenêtre Extensions, `apix plugins install`) |
| `<config>/Apix/history/` | requêtes envoyées (`{{variables}}` non résolues) et leurs réponses : `index.jsonl`, un `<id>.json` et un `<id>.body` par entrée ; les 300 dernières, corps coupés à 256 KB ; un appel gRPC garde son journal comme corps |

`<config>` est `%APPDATA%` sous Windows, `~/Library/Application Support` sous
macOS et `~/.config` sous Linux. Les jetons OAuth 2.0 et les mots de passe des
collections chiffrées ne sont jamais écrits sur le disque.

## Variables d’environnement

| Variable | Effet |
|---|---|
| `APIX_PASSWORD` | Mot de passe d’une collection chiffrée pour la CLI (`--password-env` choisit une autre variable). |
| `APIX_VAR_<NAME>` | Une variable pour la CLI : `APIX_VAR_TOKEN` remplit `{{token}}` (rapprochée des noms de la collection quelle que soit la casse) ; l’emporte sur les environnements, `--var` l’emporte sur elle. |
| `APIX_THEME` | un identifiant de thème (`obsidian`, `cyber`, `dracula`, `nord`, `solarized-dark`, `daylight`, `solarized-light` ou le nom d’un fichier de thème) ; affiche ce thème quoi que disent les paramètres. |
| `APIX_BENCH` | `startup` ou `json:<url>` : mesurer puis quitter (voir plus bas). |
| `APIX_BENCH_OUT` | Fichier auquel ajouter les mesures. |
| `APIX_BENCH_PHASES` | Avec `APIX_BENCH=startup` : indique aussi quand chaque étape du démarrage a été atteinte (lignes `phase_…`), pour voir où passe le temps. |
| `APIX_UPDATE_URL` | Où la vérification des mises à jour interroge, au lieu de la dernière version sur GitHub (un fork, un miroir, un serveur de test). |
| `OPENAI_API_KEY`, `GEMINI_API_KEY` (ou `GOOGLE_API_KEY`), `ANTHROPIC_API_KEY` | La clé de l’assistant IA quand aucune n’est gardée dans le trousseau pour ce fournisseur. |
| `APIX_CLAUDE_CONFIG` | Le fichier de configuration de Claude Desktop qu’écrit Connecter à Claude, à la place du vrai (pour les tests). |
| `APIX_AI_URL` | Où l’assistant IA envoie ses requêtes Messages API au lieu d’Anthropic (un proxy, un serveur de test). |
| `APIX_GEO_URL` | D’où **Télécharger DB-IP Lite** télécharge au lieu de DB-IP. |
| `APIX_PLUGINS_DIR` | Le dossier des extensions, au lieu de `<config>/Apix/plugins`. |
| `GITHUB_BASE_REF` | Définie par GitHub Actions sur les pull requests : `apix run --changed` sans valeur compare avec `origin/<it>`. |

## Mesurer les performances

`scripts/measure.ps1` compile l’application en release et mesure :

- la taille du **binaire release** ;
- **Du démarrage à la première image :** de `main` jusqu’à ce que la première
  image soit dessinée (`APIX_BENCH=startup`), médiane de plusieurs exécutions ;
- **Mémoire au repos :** l’ensemble de travail cinq secondes après le
  démarrage, avec la collection d’exemple ouverte ;
- **Gros JSON à l’écran :** un GET d’un corps JSON de 100 MB depuis l’exemple
  local `big_json_server`, de l’envoi jusqu’à l’affichage de la réponse
  (`APIX_BENCH=json:<url>`), pour que le réseau n’entre pas dans le chiffre.

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7, aussi sous macOS/Linux
```

Le script affiche un tableau Markdown avec la machine sur laquelle il a tourné ;
le README montre le plus récent. Les temps viennent de
`crates/apix-desktop/src/bench.rs`, qui ne fait rien tant que `APIX_BENCH`
n’est pas définie.

Autres outils :

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # 100 MB de JSON sur :8765
cargo run -p apix-http --example oauth_server                       # serveur de test OAuth 2.0 sur :8766
cargo run -p apix-http --example sse_server                         # server-sent events sur :8767 (reprend après Last-Event-ID)
cargo run -p apix-http --example ws_server                          # écho WebSocket (binaire aussi) et ticker sur :8768
cargo run -p apix-http --example graphql_server                     # un point de terminaison GraphQL (schéma et quelques utilisateurs) sur :8769
cargo run -p apix-http --example socketio_server                    # Socket.IO 4 : espaces de noms /, /chat, /ticker, /private (auth) sur :8771
cargo run -p apix-grpc --example grpc_server                        # un Greeter gRPC avec réflexion sur :8770
cargo run -p apix-http --example soap_server                        # une calculatrice SOAP sur :8773 (WSDL sur ?wsdl ; fautes avec 500 et 200)
cargo run -p apix-proto --example mqtt_broker                       # un broker MQTT sur :8772 (`clock` bat chaque seconde)
```

## Langues de l’interface

Les textes de l’interface sont dans `crates/apix-desktop/locales/<code>.yml`,
une paire `key: value` à plat par ligne, lue par rust-i18n via la macro `tr!`.
Un test unitaire échoue quand une langue n’a pas une clé ou utilise d’autres
`%{placeholders}` que l’anglais. Pour ajouter une langue, copiez `en.yml`,
traduisez-le et ajoutez une ligne à `LANGUAGES` dans `src/i18n.rs`.

## Langues de la documentation

`Docs/languages.toml` liste les langues de la documentation. Les traductions
commencent par un tampon qui nomme le texte anglais dont elles viennent :

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check                      # liste les traductions manquantes ou périmées et les liens cassés
cargo run -p docs-check -- stamps            # affiche le tampon actuel de chaque document anglais
cargo run -p docs-check -- site site/docs    # la documentation en pages web (GitHub Pages)
```

Les images du guide d’utilisation sont prises par `scripts/docs-tour.ps1`. Il démarre les serveurs d’exemple et lance l’application de bureau une fois par langue avec `APIX_TOUR=Docs/tour/scenes.toml` : l’application prépare elle-même chaque scène (ouvre des requêtes de la collection de démonstration dans `Docs/tour/demo`, les envoie, ouvre une fenêtre) et le script enregistre la fenêtre sous `Docs/<langue>/images/<scène>.png`. `APIX_CONFIG_DIR` garde les réglages, l’historique et les cookies de l’application dans un dossier à part pendant la visite, sans toucher aux vôtres.

## Le backend gpui embarqué

Le moteur de rendu de texte Windows de GPUI (crate `gpui-pre-windows` 0.3.9)
plaçait chaque suite de glyphes de gauche à droite, si bien que les mots arabes
apparaissaient en miroir. `vendor/gpui-pre-windows` est cette crate avec
`DrawGlyphRun` corrigé (la modification est marquée « Apix patch »), et le
`Cargo.toml` de l’espace de travail la substitue. On pourra la retirer quand la
crate d’origine sera corrigée.

## Intégration continue

`.github/workflows/ci.yml` tourne à chaque push et à chaque pull request :

| Tâche | Ce qu’elle vérifie |
|---|---|
| Format et docs | `cargo fmt --check` ; `docs-check` (les traductions suivent la doc anglaise) |
| Moteur et CLI | `cargo clippy -D warnings` et `cargo test` pour tout sauf l’application de bureau, sous Windows, macOS et Linux |
| Application de bureau | la même chose pour `apix-desktop` sur les trois systèmes (Linux installe d’abord les bibliothèques de fenêtres, de polices et de trousseau) |
| GitHub Action | l’action du dépôt lui-même (`uses: ./`) sur `examples/jsonplaceholder`, sous Linux et Windows, et le fait qu’elle a écrit ses rapports |

`.github/workflows/release.yml` construit les paquets de version sur un tag de
version (`v1.2.3`, qui doit correspondre à `Cargo.toml`) et les publie comme
version GitHub avec `SHA256SUMS.txt` ; lancé à la main, il ne fait que les
construire, comme artefacts de l’exécution. Windows : `scripts/package-windows.ps1`
(Inno Setup 6, `packaging/windows/apix.iss` : installation par utilisateur ou
par machine, PATH et association `.apix` en option, programme de
désinstallation). macOS : `scripts/package-macos.sh` compile les deux
architectures, les réunit avec `lipo` dans `Apix.app` (signé ad hoc) et crée un
`.dmg`. Linux (sous Ubuntu 22.04, donc glibc 2.35 suffit) :
`scripts/package-linux.sh` crée le tarball avec `install.sh`, le `.deb`
(dépendances tirées de `dpkg-shlibdeps`, plus `libvulkan1`) et un `PKGBUILD`
rempli ; `scripts/package-arch.sh` construit à partir d’eux le paquet Arch dans
un conteneur `archlinux`. Le `.deb` et le paquet Arch sont installés dans le
workflow et vérifiés pour les bibliothèques manquantes. `.github/workflows/pages.yml`
publie `site/` sur https://11r5.github.io/Apix/ .
Chaque version contient aussi un cask Homebrew rempli (`apix.rb`, à partir de
`packaging/homebrew`) et des manifestes winget (`AhmetDemir.Apix*.yaml`, à partir
de `packaging/winget`), créés par `scripts/channel-manifests.sh`, à copier dans
le dépôt du tap ([11R5/homebrew-apix](https://github.com/11R5/homebrew-apix))
et dans une pull request à `microsoft/winget-pkgs`.
`.github/workflows/docker.yml` publie l’image de conteneur sur les tags
de version. L’action met en cache l’`apix` qu’elle compile sous un hash des
sources du moteur : un nouveau commit le recompile, un commit inchangé le
réutilise.

## Tests

| Crate | Ce qui est couvert |
|---|---|
| apix-core | allers-retours du modèle et du format de fichier, variables, secrets d’environnement dans le trousseau (sautés sans magasin d’identifiants), signature contre la suite de tests AWS, RFC 7616 (Digest), RFC 7636 (PKCE) et l’exemple OAuth 1.0 ; schémas GraphQL (lecture de l’introspection, SDL) et ce qui est suggéré et décrit à chaque endroit d’une requête (champs, alias, arguments, valeurs d’enum, objets d’entrée, listes, fragments, types de variables, commentaires et chaînes) ; la table `[grpc]` ; entrées d’historique, appels gRPC compris ; déclarations de variables GraphQL et complétion dans les variables (JSON) ; le paramètre d’autorité de certification approuvée ; filtrage du JSON avec JSONPath et jq (correspondances, sorties dans l’ordre, erreurs en mots, filtres sans fin arrêtés) ; la query de l’URL et ses lignes dans les deux sens, variables de chemin `:name` trouvées et remplies (seulement dans le chemin, encodées, un second remplissage ne changeant rien), mise en ordre de l’URL d’une requête ; noms et valeurs d’en-têtes suggérés (début avant contenu, aucun une fois tapé en entier) ; exemples (allers-retours texte et octets, la requête telle qu’écrite, pas de Set-Cookie) ; git sur un vrai dépôt (modifications depuis un point de branchement, non commitées, entre commits, un fichier à un commit, une ref inconnue), ce que touche un fichier modifié (scripts, dossiers, environnements, le projet, fichiers nommés par une requête), diffs de requêtes (lignes par nom, mise en forme ignorée, corps ligne par ligne, déplacements) ; intervalles des moniteurs et allers-retours de `[[monitor]]` ; requêtes de l’assistant IA (identifiants masqués, corps coupés, la langue, solutions de repli seulement pour le modèle par défaut) et réponses (blocs de texte, refus, types d’erreur, le script dans un bloc de code) ; comparaison de réponses (en-têtes par nom, les volatils laissés de côté, JSON champ par champ avec les enregistrements rapprochés par id, mise en forme ignorée, corps texte et binaires) ; enregistrements trouvés dans le JSON pour les tableaux (enveloppes, valeurs simples, colonnes de nombres et de libellés, texte aligné) |
| apix-http | construction des requêtes, chaque type de corps, un serveur local pour OAuth 2.0, Digest, AWS et OAuth 1.0, la trace réseau ; le parseur SSE (morceaux coupés n’importe où), streaming contre un serveur qui envoie lentement, WebSocket (écho, auth et paramètres dans la poignée de main, fermeture de chaque côté, poignée de main refusée), récupération d’un schéma GraphQL avec l’auth de la requête et les erreurs quand il n’y en a pas, HTTPS avec sa propre autorité de certification ; sous-protocoles WebSocket et messages binaires, `wss://` avec sa propre CA ou sans vérification ; ce qu’un flux d’événements transmet à une reconnexion ; paquets Socket.IO, adresses de poignée de main et pièces jointes binaires, et contre un serveur Socket.IO 4 local : événements, acks dans les deux sens, pings, espaces de noms avec auth et refus, pièces jointes binaires, le serveur qui fait partir le client, un simple serveur WebSocket reconnu comme tel ; pings WebSocket à intervalle avec leurs pongs chronométrés, un ping sans réponse qui met fin à la connexion, `ping_interval` dans le fichier ; détails TLS de la poignée de main de la requête elle-même contre un serveur HTTPS local (version, suite de chiffrement, groupe, la chaîne envoyée avec clé et empreinte, temps de poignée de main cohérent avec la répartition TCP/TLS) ; corps gzip, zlib, deflate brut et brotli décodés par morceaux, corps tronqués refusés, `Accept-Encoding` demandé sauf si la requête en définit un, `Content-Encoding` conservé |
| apix-grpc | adresses, codes de statut, en-têtes et auth en métadonnées ; contre un Greeter local (construit à partir de messages dynamiques, avec et sans réflexion) : liste et description du service, les quatre sortes de méthodes, auth qui atteint le serveur, un statut d’échec, fichiers .proto avec imports, et les erreurs en l’absence de description, pour une méthode inconnue, un message qui ne correspond pas, une liste pour une méthode unaire, un fichier manquant et pas de serveur ; messages d’exemple ; un appel vu comme une réponse HTTP pour les exécutions (correspondance des statuts, `grpc-status`, un flux vu comme une liste) ; TLS contre un serveur dont le certificat vient de sa propre autorité (refusé, approuvé avec la CA, accepté sans vérification) ; définitions : la vraie sortie des générateurs gardée dans `tests/fixtures/generated` (protoc 3.19 pour Python, Java, C#, C++, PHP ; protoc-gen-es ; ts-proto) doit redonner le service d’origine, Go, Rust, betterproto et les annotations donnent numéros et types, le même type lu depuis 10 langages, exemples JSON, comparaison avec un serveur, une supposition qui cède la place à la réflexion, champs inconnus signalés, un mauvais type de réponse expliqué |
| apix-proto | adresses MQTT, identifiants et options ; contre un broker local : chaque qualité de service, jokers, abonnement et désabonnement pendant la connexion, abonnements refusés et mauvais topics, messages retenus, identifiants et sessions conservées, le broker qui raccroche, `mqtts://` avec sa propre CA ou sans vérification |
| apix-codegen | documentation d’API (dossiers, identifiants masqués, pas de valeurs de `.local.toml`, descriptions Markdown sans HTML brut, ancres) ; échappement shell pour bash, cmd et PowerShell ; collage de curl ; allers-retours ; l’URL, les en-têtes, les corps, l’échappement des chaînes et les paramètres de chaque langage |
| apix-import | fixtures Postman v2.0/v2.1 ; WSDL : les deux versions de SOAP, ports et adresses, styles document et rpc, en-têtes, exemples tirés du schéma (extension, enums, choix, `form`, récursion), imports et ce qui n’a pas pu être lu, emplacements, WSDL 2.0 refusé |
| apix-script | variables et modifications de la requête par les scripts de pré-requête, résultats et messages de `pm.test` / `pm.expect`, l’ancienne API `tests[...]`, erreurs qui nomment le fichier, scripts sans fin arrêtés, `setNextRequest`, `atob`/`btoa`, `pm.sendRequest` (callback et `await`, l’attente non comptée), les bibliothèques intégrées contre des empreintes connues, vérifications sans code, valeurs et expressions pour les cibles de réponse |
| apix-runner | fichiers de données CSV (guillemets, sauts de ligne, `;`, BOM) et JSON, fichiers dotenv et noms `APIX_VAR_*`, nombres d’itérations et lignes de données, temps de pause/arrêt, sortie JUnit/JSON/HTML/Markdown et échappement (Markdown pour les pull requests : marqueur, échappement, ce qui a échoué, une sélection `--changed`), dates ; exécutions entières contre un serveur local (lignes de données, ordre de la barre latérale, `bail`, `setNextRequest`), itérations en parallèle contre un serveur lent (plus rapides que l’une après l’autre, listées par itération) ; moniteurs : quand l’état bascule, journaux limités à leurs derniers résultats et séparés par collection, plannings qui suivent le journal, messages de webhook |
| apix-runner (tests de charge) | percentiles par rang le plus proche, lecture des durées ; contre un serveur keep-alive local : un nombre de requêtes sur des connexions conservées (environ une par worker pour 400 requêtes), un débit régulier, une limite de temps, un arrêt depuis ailleurs, les 500 et les connexions refusées comptés par sorte |
| apix-runner (flux) | fichiers de flux lus et écrits, nœud de départ, arêtes remplacées et retirées, problèmes nommés, sujets d’une réponse (statut, temps, en-têtes, JSONPath, jq, rien), comparaisons ; contre un serveur local : une valeur gardée par une requête utilisée dans l’URL de la suivante, une boucle d’interrogation avec une vérification et une attente, une requête en échec avec et sans arête d’échec, une valeur absente, un fichier de requête inconnu, une boucle sans fin arrêtée à 1000 étapes, un flux qui ne peut pas tourner |
| apix-cli (MCP) | le protocole (versions, notifications, méthodes inconnues, la liste des outils avec et sans `--list-only`) ; contre un serveur local : liste des requêtes et des environnements (noms, pas de valeurs), envoi avec un environnement et avec les variables de l’agent, tests des scripts dans le résultat, exécution de toutes les requêtes, chemins hors de la collection et fichiers manquants refusés |
| apix-mock | motifs de chemin (`{{base_url}}/users/:id`, hôtes, queries), la correspondance la plus littérale, choix par `x-mock-response-name` et `-code`, 2xx d’abord, corps décodés sans `Content-Encoding`, la liste du 404, requêtes preflight ; un vrai serveur sur un port qui répond via le client HTTP, avec CORS, et sert un exemple ajouté pendant qu’il tourne |
| apix-capture | les requêtes enregistrées ne gardent ni cookies ni identifiants, filtres d’hôtes ; à travers de vrais serveurs locaux : HTTP simple enregistré et sauvegardé, HTTPS ouvert avec l’autorité locale (le client ne faisant confiance qu’à elle), tunnels HTTPS transmis sans être lus |
| apix-route | lignes de tracert (une traduite aussi), traceroute et tracepath ; adresses locales ; la projection de la carte et les distances ; une base écrite au format MaxMind DB : lieux, adresses qu’elle ne connaît pas, adresses privées, choix du fichier ; installation d’un téléchargement (gzip, refusé quand ce n’est pas une base, l’ancienne remplacée seulement après) ; l’outil du système qui trace 127.0.0.1 |
| apix-plugins | les extensions d’exemple chargées avec leurs manifestes ; la signature HMAC contre une signature calculée indépendamment ; import CSV et statistiques de champs ; modules écrits en texte : une boucle sans fin arrêtée, imports refusés, un mauvais nom, un manifeste manquant, une réponse hors de la mémoire, un fichier qui n’est pas du WebAssembly ; aussi les allers-retours de `[auth.plugin]` (apix-core) et la réponse d’un importateur en fichiers de requêtes (apix-import) |
| apix-vault | format de chiffrement, falsification, changement de mot de passe, clé de récupération, entrées du trousseau (contre le vrai magasin d’identifiants sous un nom de service de test ; sautés là où il n’y en a pas) |
| apix-desktop | traductions complètes, utilitaires de langue, chemins de fichiers, requête de coloration, fichiers de thème (lecture, erreurs, aller-retour), couleurs d’accent et de transparence, noms de fichiers des schémas GraphQL, le journal d’un appel gRPC dans l’historique |
| docs-check | structure et tampons de la documentation |
