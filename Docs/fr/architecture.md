<!-- translated from en/architecture.md sha256:75b4d247b3b61d0d -->

# Architecture d’Apix

Apix est un client d’API natif écrit en Rust. L’interface est dessinée sur le
GPU avec [GPUI](https://github.com/zed-industries/zed) (le moteur d’interface
de Zed) et la bibliothèque de composants
[gpui-kit](https://github.com/longbridge/gpui-kit). Il n’y a ni WebView ni
Electron.

## Principes

1. **Local d’abord, ami de git.** Une collection est un dossier de petits
   fichiers TOML. Pas de compte, pas de cloud obligatoire. Les collections se
   commitent, se comparent et se relisent comme du code.
2. **L’interface ne bloque jamais.** Réseau, disque, analyse et scripts
   tournent hors du thread de l’interface. Le thread de l’interface ne fait que
   la mise en page et le dessin.
3. **Un moteur, plusieurs interfaces.** L’application de bureau, la CLI `apix`
   et les interfaces futures (une TUI, un serveur MCP) partagent les mêmes
   crates ; une requête se comporte donc de la même façon partout, CI comprise.
4. **Tenir les réponses énormes.** Une réponse JSON de 100 MB doit défiler sans
   à-coups. Seules les lignes visibles sont mises en page (listes virtuelles),
   et le corps est analysé à la demande.
5. **Léger et rapide au démarrage.** La vitesse et les fonctionnalités passent
   avant la taille. Le binaire release reste sous 100 MB (54.4 MB aujourd’hui ;
   les applications Electron font 300 MB et plus) et ouvre une fenêtre en moins
   de 150 ms.

## Crates

```
crates/
├── apix-core     modèle de requête, valeurs {{variables}} et {{$dynamic}}, format sur disque,
│                 projets, environnements et secrets, opérations sur l’arbre et les fichiers,
│                 schémas GraphQL (introspection, SDL) et complétion des requêtes,
│                 git : branche, fichiers modifiés, ce que touche une modification, diffs au niveau des requêtes
├── apix-http     moteur HTTP : reqwest + rustls, temps, capture de la réponse, trace réseau,
│                 server-sent events, WebSocket (tokio-tungstenite), Socket.IO, récupération du schéma GraphQL
│                 (DNS, établissement de la connexion, redirections, certificat, interface locale)
├── apix-import   Postman, Insomnia, OpenAPI, HAR et WSDL → collections Apix
├── apix-codegen  requêtes ↔ commandes shell : curl (bash), curl.exe (cmd), PowerShell ; analyse le curl collé ; code en 10 langages
├── apix-vault    collections chiffrées : XChaCha20-Poly1305 par fichier, Argon2id, clé de récupération
├── apix-script   scripts de pré-requête et de test : QuickJS avec une API compatible pm.*
├── apix-runner   exécute les collections : ordre, itérations (aussi en parallèle), fichiers de données, délai, bail,
│                 rapports (aussi en Markdown pour les pull requests), moniteurs : planning, journal, webhooks
├── apix-grpc     appels gRPC : tonic, messages en JSON (prost-reflect), fichiers .proto (protox), définitions tirées du code ou du JSON
│                 ou réflexion du serveur
├── apix-proto    autres protocoles : MQTT (rumqttc)
├── apix-mock     un serveur fictif local qui répond à partir des exemples enregistrés (hyper)
├── apix-capture  un proxy d’enregistrement : HTTP simple, tunnels CONNECT, HTTPS ouvert avec une CA locale (rcgen, rustls)
├── apix-route    la route vers un serveur (le traceroute du système) et l’emplacement de ses sauts (une base .mmdb hors ligne)
├── apix-plugins  extensions WebAssembly (wasmi) : types d’auth, importateurs, vues de réponse ; apix-plugin-sdk les écrit en Rust
├── apix-cli      binaire `apix` : send / run / diff / schema / import / shell depuis le terminal et la CI
└── apix-desktop  application de bureau GPUI (Windows, macOS et Linux depuis une seule crate)
tools/
└── icongen       construit toutes les icônes de l’application (ico, icns, PNG hicolor) à partir de Logo/LogoFirst.jpg
```

Dans `apix-desktop/src` : `workspace.rs` (éditeur de requête, panneau de
réponse, actions et raccourcis clavier), `workspace/collections.rs` (barre
latérale, projets, opérations sur les fichiers, import, surveillance des
fichiers), `workspace/env_editor.rs` (la boîte de dialogue des variables),
`workspace/auth_editor.rs` (l’onglet Auth et la boîte de dialogue
Authentification des dossiers, collections et projets), `workspace/palette.rs`
(la palette de commandes et sa recherche approximative), `workspace/network.rs`
(l’onglet Réseau de la réponse), `workspace/runner_ui.rs` (le Runner),
`workspace/vault_ui.rs` (déverrouillage, verrouillage et verrouillage
automatique, chiffrement, déchiffrement, changement de mot de passe, clé de
récupération), `assets.rs` (les icônes de gpui-kit plus les deux icônes de
cadenas), `workspace/menu_bar.rs` (Fichier/Requête/Environnement/Affichage/Aide
dans la barre de titre ; les boutons appellent `occlude()` sur la zone de
déplacement de la barre de titre pour que Windows leur transmette le clic),
`workspace/tabs.rs` (requêtes ouvertes ; l’onglet au premier plan vit dans les
widgets de l’éditeur, les autres gardent leur brouillon, leur réponse et leur
tâche en cours), `workspace/drag.rs` (glisser des requêtes et des dossiers pour
les déplacer), `workspace/shell_copy.rs` (copier en / coller du curl),
`workspace/var_chips.rs` (les pastilles `{{variable}}` dans la barre d’URL et
leur carte au survol), `workspace/graphql_ui.rs` (récupération du schéma
GraphQL, cache, complétion et survol), `workspace/grpc_editor.rs` et
`workspace/grpc_ui.rs` (l’onglet Message d’une requête gRPC, et l’appel dans
le panneau de réponse), `large_view.rs` (visionneuse par lignes pour les corps
énormes), `net.rs` (pont vers tokio), `bidi.rs` (utilitaires de mise en page de
droite à gauche, voir Langues), `state.rs` (éléments récents et environnement
par dossier dans le dossier de configuration de l’utilisateur), `theme.rs` et
`fonts.rs`.

### Une seule crate de bureau pour trois systèmes d’exploitation

GPUI et gpui-kit tournent sous Windows, macOS et Linux avec le même code ; c’est
ainsi que Zed est distribué. Il n’y a donc pas de dossiers d’interface par
système. Le code qui diffère vraiment selon le système se trouve derrière `cfg`
dans `crates/apix-desktop/src/platform/{windows,macos,linux}.rs` : barre de
titre et boutons de la fenêtre, flou/vibrancy, la barre de menus de macOS,
associations de fichiers pour `.apix` et accès au trousseau. Les installateurs
et les icônes se trouvent dans `packaging/{windows,macos,linux}/`. Une interface
séparée ne serait détachée que si un système avait un jour besoin d’une
interface vraiment différente.

Sens des dépendances : `core` ← `http`/`script`/`proto` ← `cli`/`desktop`.
Rien ne dépend de `desktop`.

## Parcours d’une requête

```
fichier .apix ──load──▶ Request ──resolve(Variables)──▶ Request (résolue)
                                                        │
                    script de pré-requête (plus tard) ◀─┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (runtime tokio, thread d’arrière-plan)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                        script de test (plus tard) ◀────┤
                                                        ▼
                    Interface : ligne de statut, visionneuse de corps virtualisée
```

Les `{{variables}}` se résolvent par couches. Les couches suivantes l’emportent :
**globales → collection (`apix.toml`) → environnement → remplacements (CLI `--env-file`, `APIX_VAR_*` et `--var`, rapprochés des noms de la collection quelle que soit la casse ; script `pm.variables.set`)**.
Les noms non définis restent en place et sont signalés : l’interface peut les
souligner et la CLI peut refuser d’envoyer.

### La trace réseau

Chaque envoi construit son propre client reqwest de courte durée ; tout ce
qu’il enregistre appartient donc à cette requête (`apix_http::trace`). Un
résolveur chronomètre le DNS, une couche Tower autour du connecteur de reqwest
chronomètre chaque connexion, et la politique de redirection note chaque saut.
reqwest ne livre pas sa connexion TLS et ne laisse pas un connecteur à nous en
renvoyer une ; les détails TLS viennent donc de l’intérieur de rustls, via une
configuration client qu’Apix construit lui-même (`use_preconfigured_tls`,
`tls::traced_config`, hooks dans `tls_trace.rs`) :

- un vérificateur de certificats enveloppant enregistre la chaîne envoyée par
  le serveur et la version dont il vérifie la signature (TLS 1.2 ou 1.3), puis
  laisse décider le vérificateur habituel (ou « tout accepter » quand les
  vérifications sont désactivées) ;
- un magasin de sessions qui ne garde rien est interrogé pour un indice
  d’échange de clés pendant l’écriture du client hello, ce qui marque le début
  de la poignée de main TLS : le temps de la connexion avant ce point (après le
  DNS) est du TCP, après, du TLS ; on lui indique le groupe choisi par le
  serveur (TLS 1.3) ;
- les suites de chiffrement sont celles de rustls, créées une fois avec une
  fine enveloppe autour de leur chiffrement qui note quelle suite a été mise en
  place. Les suites sont `'static` et partagées ; l’enveloppe retrouve donc la
  requête par une valeur locale au thread définie pendant que le future de la
  requête est interrogé (`Scoped`) ; la poignée de main se déroule pendant cet
  appel.

L’ALPN se déduit de la version de la réponse (HTTP/2 sur TLS seulement après
accord sur `h2`). Les corps compressés sont décodés par Apix (`decode.rs`, gzip,
zlib ou deflate brut, brotli, au fil de l’arrivée) plutôt que par reqwest, qui
supprimerait `Content-Encoding` et la taille sur le réseau.

## Format sur disque

```
my-api/
├── apix.toml               nom + [vars] et [auth] au niveau de la collection
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         facultatif : [auth] pour les requêtes de ce dossier
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Héritage de l’auth.** Une requête sans `auth` l’hérite du dossier le plus
proche dont le `folder.toml` en définit une, puis de la collection
(`apix.toml`), puis du projet (`apix-project.toml`) ; s’il n’y en a aucune,
rien n’est envoyé. `auth = "none"` dans une requête (ou un dossier) arrête
l’héritage. `apix_core::project::inherited_auth` fait la recherche pour
l’application de bureau, la CLI et « Copier en » de la même façon.

**Réglages de requête.** Une table `[settings]` (délai d’expiration,
redirections, vérification TLS, proxy, certificat client, CA approuvée,
cookies) existe aux quatre mêmes niveaux. Chaque champ est une `Option` :
`None` hérite champ par champ le long de la même chaîne
(`apix_core::project::inherited_settings`), et ce qui reste non défini retombe
sur les valeurs par défaut de `RequestSettings::effective` — pas de délai
d’expiration, jusqu’à 10 redirections suivies, TLS vérifié, proxy tiré de
l’environnement, cookies activés. Le moteur applique le résultat quand il
construit le `reqwest::Client` de chaque envoi (un fichier de CA approuvée
ajoute ses certificats comme racines) ; la boîte à cookies partagée (un
`cookie_store` que le gestionnaire de cookies peut lister et modifier) est
transmise à ce client, sauf si la requête désactive les cookies.

**Auth signée.** AWS Signature V4, OAuth 1.0a et Digest sont des fonctions
pures dans `apix_core::sign`, vérifiées contre la suite de tests AWS, la
RFC 7616, la RFC 7636 et l’exemple classique d’OAuth 1.0 ; le moteur et
« Copier en » signent donc de la même façon. Le moteur construit d’abord la
`reqwest::Request`, puis la signe (le hash du contenu AWS a besoin du corps
final ; un flux multipart est envoyé en `UNSIGNED-PAYLOAD`). Digest envoie une
fois, lit le défi de la 401 et y répond sur la même connexion. OAuth 2.0 se
trouve dans `apix_http::oauth2` : le jeton est obtenu avant l’envoi
(identifiants client, mot de passe, ou code d’autorisation avec PKCE, où le
navigateur revient vers un écouteur à usage unique sur 127.0.0.1), gardé en
mémoire par serveur, client, type d’octroi, portée et utilisateur, rafraîchi
avec son jeton de rafraîchissement, et jamais écrit sur le disque.

Un fichier de requête :

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # pas de query : elle est dans [[query]]

[[query]]
key = "notify"
value = "yes"

[[path]]                 # la valeur du segment :org
key = "org"
value = "{{org}}"

[[example]]              # une réponse gardée avec la requête
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # ce à partir de quoi elle a été envoyée, tel qu’écrit
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # gardé dans le fichier, pas envoyé

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # une vérification sans code, exécutée après les scripts de test
target = "status"
op = "=="
value = "201"
```

**Vérifications.** Les lignes `[[assert]]` comparent une cible (`status`,
`time`, `size`, `body`, `body.<path>`, `header.<name>`) avec une valeur au
moyen de `==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `not contains`,
`exists`, `not exists`, `matches` ou `is` (un type JSON).
`apix_script::assertions_script` les transforme en un script de test de plus ;
elles produisent donc partout des résultats comme `pm.test`.

**Corps.** `[body]` contient un seul parmi `json`, `text`, `xml`, `form` (lignes
encodées en URL), `multipart` (lignes avec `file = true` pour les champs
fichier et un `content_type` facultatif), `binary = { file = "…" }` ou
`graphql` (`query` et `variables`). Les chemins de fichiers peuvent contenir des
variables et sont relatifs au fichier de requête ;
`Request::with_files_relative_to` les rend absolus une fois les variables
résolues, dans l’application comme dans la CLI. Le moteur ne lit les fichiers
qu’au moment de l’envoi : un fichier manquant est une erreur d’envoi, pas une
erreur de chargement. Un corps GraphQL est envoyé en JSON
`{"query", "variables"}` ; des variables invalides arrêtent l’envoi. Avec un
corps multipart, un en-tête `Content-Type: multipart/…` sans la limite
(boundary) du moteur (fréquent dans les exports Postman) est retiré, pour que
le corps reste lisible.

**gRPC.** Une requête dont l’URL est `grpc://host:port` (ou `grpcs://` pour TLS)
nomme sa méthode dans `[grpc]` ; le corps JSON est le message, et une liste JSON
est un flux de messages pour une méthode qui en accepte un. Sans `protos`, on
interroge le serveur (réflexion). Les chemins sont relatifs au fichier de
requête.

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # facultatif
import_paths = ["../protos"]              # facultatif
```

Au lieu de fichiers, `definition` garde la description dans la requête : du
texte `.proto`, ou un ensemble de descripteurs en JSON. Elle passe avant
`protos`. `guessed = true` marque une définition déduite des types ou du JSON :
la réflexion du serveur passe avant elle, et les différences sont signalées.
`protos` accepte aussi des ensembles de descripteurs, binaires ou `.json`.

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

Questions ouvertes :
- **Secrets.** Les valeurs d’environnement comme les jetons ne doivent pas être
  commitées. `environments/<name>.local.toml` se superpose à `<name>.toml`. En
  écrire un ajoute `*.local.toml` au `.gitignore` voisin de `environments/` s’il
  n’y est pas. Un secret peut garder une valeur de remplacement dans le fichier
  partagé, pour que l’équipe voie le nom. En option (Paramètres › Valeurs
  secrètes des environnements), les secrets vont plutôt dans le trousseau du
  système : `<name>.local.toml` ne contient alors que `keychain = "<id>"` et les
  valeurs vivent dans l’entrée `env:<id>` du service `Apix`
  (`apix_vault::keychain::save_text`, réparties sur plusieurs entrées à cause de
  la limite de 2560 octets de Windows). `apix_core::secrets::use_keychain`
  décide où les nouveaux secrets sont écrits ; la lecture suit le fichier, la
  CLI n’a donc besoin d’aucun réglage. L’identifiant voyage avec le fichier :
  renommer l’environnement ou déplacer le dossier garde le lien. Dans une
  collection chiffrée, les secrets restent dans le coffre.
- **Variables dynamiques.** `{{$name}}` est rempli par `apix_core::dynamic`
  quand aucune variable de ce nom n’est définie, avec une nouvelle valeur à
  chaque occurrence (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …).
  Aucune crate de plus : un petit générateur xorshift et un formateur de dates
  UTC écrit à la main.
- **Ordre dans la barre latérale.** Sans rien d’enregistré, les dossiers
  viennent d’abord, puis les requêtes, chacun par nom. Un glisser-déposer
  enregistre une liste `order` des noms des enfants dans le `folder.toml` du
  dossier (ou l’`apix.toml` de la collection) ; `store::place` place un élément
  avant ou après un autre, et les renommages et déplacements tiennent la liste à
  jour. Les noms que la liste ne connaît pas suivent dans l’ordre par défaut ;
  un nouveau fichier ne disparaît donc jamais. Les collections d’un projet
  utilisent `order` dans `apix-project.toml` (`Project::place_collection`).
  L’importateur Postman n’écrit `order` que si l’ordre de Postman diffère de
  l’ordre par défaut.

### Projets

Un projet regroupe des collections et possède des environnements partagés par
toutes :

```
shop/
├── apix-project.toml       nom, [vars] partagées, ordre des collections, group_by, pins, colors
├── environments/           test.toml, prod.toml (+ secrets *.local.toml)
├── users/                  une collection (apix.toml + fichiers .apix)
└── payments/               une collection ; peut être épinglée à certains environnements
```

`[colors]` associe un dossier de collection à un nom de couleur (`red`,
`orange`, `yellow`, `green`, `teal`, `blue`, `purple`, `pink`) ; l’application
la dessine comme une pastille dans la barre latérale et sur les onglets.
`project::move_collection_into` déplace un dossier de collection dans un projet
(copie puis suppression d’un lecteur à l’autre).

Couches de variables, la dernière l’emporte : variables du projet → variables
de la collection → environnement du projet → environnement de la collection du
même nom (chacun avec ses secrets `.local.toml`). La CLI (`apix run --env`) et
l’application de bureau utilisent la même fonction,
`apix_core::project::variables`. `group_by = "environment"` organise la barre
latérale en Projet → Environnement → Collections, et `[pins]` limite une
collection à certains environnements.

### Collections chiffrées (M4)

Le format et le moteur se trouvent dans `crates/apix-vault`. Le reste d’Apix y
accède via `apix_core::vfs`, une fine couche avec les mêmes appels que
`std::fs` : un chemin sous la racine d’un coffre déverrouillé est servi depuis
le coffre, déchiffré en mémoire ; tout autre chemin va au disque. Ainsi le
stockage, la barre latérale, les onglets et la CLI continuent d’utiliser des
chemins ordinaires, et rien de déchiffré n’est jamais écrit sur le disque. Les
coffres déverrouillés vivent dans un registre à l’échelle du processus ; le
verrouillage (à la main, après 15 minutes d’inactivité ou à la fermeture)
abandonne le coffre et efface sa clé. Supprimer dans une collection chiffrée
est définitif : une corbeille du système contiendrait des fichiers déchiffrés.

```
my-api/
├── apix-vault.toml   en-tête en clair : format, coût et sels Argon2id,
│                     la clé de données enveloppée par le mot de passe et par la clé de récupération
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   les vrais chemins de tous les fichiers, chiffrés
    ├── 3f9c…e1.apixe un fichier chiffré chacun ; noms aléatoires, à plat (pas de forme de dossiers)
    └── local/        secrets *.local.toml, même format, jamais commités
```

- Une **clé de données** aléatoire de 256 bits chiffre chaque fichier avec
  XChaCha20-Poly1305 et un nonce neuf à chaque écriture. L’identifiant de
  l’objet est la donnée associée : on ne peut pas échanger ou renommer des blobs
  sans que cela se voie.
- La clé de données est stockée deux fois : enveloppée par une clé dérivée du
  **mot de passe** (Argon2id, 64 MiB, 3 passes) et par une clé dérivée de la
  **clé de récupération** (160 bits aléatoires, affichée une seule fois sous la
  forme `XXXX-XXXX-…` en base32 de Crockford). Changer le mot de passe
  ré-enveloppe la clé ; les fichiers ne sont pas rechiffrés.
- Les renommages et déplacements ne touchent que l’index chiffré.
- La clé déverrouillée ne vit qu’en mémoire (`zeroize` l’efface au
  verrouillage).
- Git voit toujours quel objet a changé, jamais ce qu’il contient. La sécurité
  vient de l’algorithme de chiffrement et du mot de passe, pas du secret du
  format ; Apix est open source.
- Le mot de passe peut être mémorisé dans le magasin d’identifiants du système
  (`apix_vault::keychain`, via la crate `keyring` : Gestionnaire
  d’identification, Trousseau de macOS, Secret Service). Une entrée par coffre
  sous le service `Apix`, nommée `vault:<absolute folder>` (en minuscules sous
  Windows). C’est à activer depuis la demande de déverrouillage ; l’application
  et la CLI l’essaient avant de demander, un mot de passe qui n’ouvre plus le
  coffre est supprimé et redemandé, changer le mot de passe le met à jour et
  déchiffrer la collection le supprime. Quand aucun magasin n’est disponible,
  Apix demande simplement, comme avant.

## Scripts (M5)

`apix_core::scripts` trouve les scripts d’une requête (`_collection.<event>.js`,
le `_folder.<event>.js` de chaque dossier, puis `<request>.<event>.js` ; les
événements sont `prerequest` et `test`) et contient `ScriptVars`, ce que les
scripts définissent ou retirent par portée. `ScriptVars::apply` les superpose
aux `Variables` de la requête ; la résolution fonctionne donc comme avant. Rien
de ce que définissent les scripts n’est écrit sur le disque.

`apix-script` exécute les scripts d’un événement dans un runtime QuickJS neuf
(`rquickjs` ; limites de 10 s et 256 MB). L’objet `pm` est en JavaScript
(`prelude.js`) : Rust transmet le contexte en JSON (la requête non résolue, la
réponse pour les tests, les valeurs des portées tirées de
`project::scope_values`, les `ScriptVars` précédentes, `pm.info`), les scripts
s’exécutent un par un, enveloppés dans une fonction, et `__apix_output()`
renvoie en JSON la requête et les variables modifiées, les résultats des tests,
les lignes de console, `setNextRequest` et `skipRequest`. Garder l’API en
JavaScript limite le côté Rust à quelques appels et rend le comportement de
Postman facile à reproduire.

`pm.sendRequest` appelle une fonction native `__apix_send` avec la requête en
JSON ; l’appelant d’`apix_script::run` passe un `Sender` qui l’envoie et bloque
jusqu’à l’arrivée de la réponse (la CLI sur un runtime neuf dans le thread du
script, l’application sur son runtime réseau). Les scripts sont enveloppés dans
des fonctions async ; le style callback et `await` fonctionnent donc tous les
deux, et le temps d’attente n’est pas compté dans la limite. `require` vient de
`libs.js` : des sous-ensembles de crypto-js (empreintes et HMAC calculés en Rust
via `__apix_hash` / `__apix_hmac`), uuid, lodash, moment, ajv, tv4 et chai,
écrits pour Apix plutôt que téléchargés.

Un envoi avec scripts : scripts de pré-requête, puis `ScriptVars::apply` et
résolution, la requête HTTP, puis les scripts de test avec une `ResponseView`.
`apix_runner::exchange` fait cela pour `apix send`, `apix run` et le Runner ;
l’envoi simple de l’application de bureau le fait dans `script_run.rs`, avec
les scripts sur un thread d’arrière-plan et les scripts propres à la requête
pris dans l’éditeur, enregistrés ou non.

`apix_runner::run` prend un `Plan` (fichiers de requête dans l’ordre,
variables et portées des scripts, lignes de données, itérations, délai,
`bail`, remplacements du transport, l’état des scripts de départ) et le
parcourt : à chaque itération, il superpose la ligne de données (puis les
valeurs `--var`) aux variables et à `pm.iterationData`, suit `setNextRequest`
par nom de requête, attend entre les requêtes et signale la `Progress`
(itération, envoi, élément terminé) à un callback. Un `Control` le met en pause
ou l’arrête depuis un autre thread. Il renvoie un `RunReport` d’`ItemResult`,
que `report.rs` écrit en JSON, en JUnit XML (une suite par requête et
itération, un cas par test, un cas de statut pour les requêtes sans tests) et
en page HTML autonome. `data.rs` lit le CSV (RFC 4180, BOM, en-têtes avec `;`)
et les tableaux JSON. La CLI affiche la progression au fur et à mesure ; le
Runner de bureau (`workspace/runner_ui.rs`) l’exécute sur le runtime réseau et
copie la progression depuis une structure partagée `Live` quelques fois par
seconde ; la vue reste dans l’espace de travail, si bien qu’une exécution
survit à sa boîte de dialogue.

## Streaming et WebSocket (M6)

`HttpClient::send_with` signale `Progress::Head` (statut et en-têtes) dès
qu’ils sont arrivés, puis chaque `Progress::Chunk` du corps ; `send` est
`send_with` sans personne à l’écoute, et la réponse contient toujours le corps
entier. `apix_http::sse` lit les corps `text/event-stream` morceau par morceau
(`SseParser` : toute fin de ligne, un morceau peut être coupé n’importe où, `id`
se reporte, un dernier événement inachevé est abandonné). Le parseur garde
aussi le dernier id et le dernier `retry:` pour la reconnexion :
`reconnected()` démarre un parseur pour la connexion suivante avec les deux, et
`reconnect_request` ajoute `Last-Event-ID`. L’application et la CLI gèrent
elles-mêmes l’attente et le nouvel envoi ; Esc ou Ctrl+C l’arrête donc
toujours.

`apix_http::ws::connect` construit la poignée de main à partir de la requête
telle que le côté HTTP l’enverrait (`prepare` : paramètres, en-têtes, auth ;
OAuth 2.0 devient d’abord un jeton bearer), ajoute `Sec-WebSocket-Protocol` à
partir de `[websocket] subprotocols`, et se connecte avec tokio-tungstenite.
`wss://` reçoit une configuration rustls d’`apix_http::tls::client_config` (les
racines habituelles plus une CA approuvée, un certificat client ou un
vérificateur qui accepte n’importe quel certificat), le même module que gRPC
utilise sans vérification. `WsConnection::run` envoie ce que demande un canal de
`WsCommand` et signale des `WsEvent` (envoyé, reçu, fermé avec code et raison,
erreur) jusqu’à ce que l’un des côtés ferme ; tungstenite répond aux pings du
serveur. Avec `[websocket] ping_interval`, il envoie aussi des pings, avec un
compteur comme contenu, signale l’aller-retour du pong correspondant
(`WsEvent::Pong`) et se termine en erreur quand un ping est toujours sans
réponse au moment où le suivant est dû. Le `MessageFormat` d’un message (texte,
base64, hex) transforme son texte en `WsData` une fois les variables remplies.

Dans l’application de bureau, le runtime réseau remplit des structures
partagées (`net::Incoming` pour un envoi, `net::SocketIncoming` pour un socket)
et une tâche de l’interface les lit toutes les 60–80 ms. Une réponse
event-stream devient `ResponseState::Streaming` (`workspace/stream_ui.rs`) avec
une liste d’événements en direct qui suit les nouveaux événements, sauf si on a
fait défiler vers le haut ; Esc transforme ce qui est arrivé en réponse
terminée. Une URL `ws://` fait passer le panneau de requête à l’onglet Messages
(`workspace/ws_editor.rs`) et le panneau de réponse à `ResponseState::Socket`
(`workspace/socket_ui.rs`). Les deux états sont dirigés vers leur onglet comme
une réponse terminée ; un flux ou un socket continue donc de tourner dans un
onglet à l’arrière-plan. La CLI affiche événements et messages à partir des
mêmes callbacks.

## Socket.IO (M6)

`apix_http::socketio` parle Socket.IO 3/4 (protocole Engine.IO 4) par-dessus la
connexion WebSocket ci-dessus ; la poignée de main, les en-têtes, l’auth et les
réglages TLS sont donc les mêmes. Une requête est Socket.IO quand elle a une
table `[socketio]` (`SocketIoOptions` : espace de noms, chemin, auth) ;
`handshake_request` transforme `http(s)://` en `ws(s)://`, prend l’espace de
noms dans les options ou dans le chemin de l’URL, met le chemin du serveur
(`/socket.io/`) à sa place et ajoute `EIO=4&transport=websocket`. `connect`
attend le paquet open d’Engine.IO, envoie le CONNECT de l’espace de noms avec le
JSON d’auth et attend la réponse (`40…` avec l’id du socket, ou `44…` avec le
refus) avant de rendre la main ; un mauvais serveur, espace de noms ou jeton
échoue donc comme une poignée de main ratée.

`SioConnection::run` exécute `WsConnection::run` à côté d’un traducteur entre
deux canaux : `SioCommand::Emit` devient un paquet `EVENT` (`42…`, avec un id
d’ack sur demande, les ids comptant à partir de 0) ou un `BINARY_EVENT`
(`45n-…`) dont les valeurs `{"$binary": "<base64>"}` sont remplacées par des
marqueurs et envoyées comme trames binaires après lui ; les paquets reçus
deviennent des `SioEvent` (événements reçus avec l’id d’ack attendu par le
serveur, nos propres acks, le serveur qui nous fait partir, trames illisibles),
les pièces jointes binaires étant rassemblées et remises à la place de leurs
marqueurs. Les pings Engine.IO (`2`) reçoivent un pong (`3`).

L’application de bureau garde un seul onglet Messages pour les deux : `WsEditor`
a un sélecteur WebSocket/Socket.IO et les champs Socket.IO, et `WsMessage` porte
`event` et `ack`. `net::SocketIncoming` est générique sur le type d’événement,
et `socket_ui.rs` suit les deux sortes de connexion avec une seule boucle, en
transformant les `SioEvent` en entrées de journal avec le nom de l’événement et
des pastilles d’ack. Le serveur de test de
`apix-http/tests/common/socketio_server.rs` est aussi l’exemple
`socketio_server` (:8771).

## MQTT (M6)

`apix_proto::mqtt` enveloppe la boucle d’événements de rumqttc (MQTT 3.1.1 ;
son TLS uniquement via la configuration d’`apix_http::tls::client_config`, il
n’apporte donc aucun fournisseur cryptographique à lui). Une URL `mqtt://` ou
`mqtts://` fait d’une requête une requête MQTT, comme `grpc://` pour gRPC ;
`[mqtt]` contient l’id client, la session, le keep alive et les abonnements.
`connect` interroge la boucle d’événements jusqu’au CONNACK du broker ; une
connexion refusée échoue donc comme une poignée de main ratée.

`MqttConnection::run` fait tourner deux futures côte à côte, sans jamais
annuler l’interrogation de la boucle d’événements : l’un transmet les
`MqttCommand` au client (qui peut attendre quand sa file est pleine, file que
l’interrogation vide), l’autre suit la connexion et signale des `MqttEvent`.
rumqttc signale l’id de paquet de chaque paquet sortant dans l’ordre où il lui
a été transmis ; une file rapproche donc publications et abonnements de leurs
ids, et les PUBACK, PUBCOMP, SUBACK et UNSUBACK du broker de ce qu’ils
confirment. Un message est signalé comme envoyé quand son paquet est parti. Un
mauvais topic ou un abonnement refusé est un événement, et la connexion
continue ; la fermeture, et le broker qui raccroche, y mettent fin.

Dans l’application de bureau, `WsEditor` contient aussi les champs MQTT (l’URL
décide, et `Workspace` le lui dit), et `socket_ui.rs` suit la connexion avec la
même boucle que WebSocket et Socket.IO, en transformant les `MqttEvent` en
lignes de journal par topic. Le broker de test de
`apix-proto/tests/common/broker.rs` (jokers, QoS 0–2, messages retenus, refus,
TLS sur n’importe quel flux) est aussi l’exemple `mqtt_broker` (:8772).

## SOAP (M6)

SOAP n’ajoute aucune sorte de requête : `apix_import::wsdl` transforme un
WSDL 1.1 en requêtes POST ordinaires avec des corps XML ; tout ce qui marche sur
une requête marche donc sur elles. `apix_core::soap` contient ce en quoi les
versions diffèrent (en-têtes, enveloppe) et lit les fautes ;
`Response::soap_fault` l’applique aux réponses XML, et le runner fait échouer
une requête sans tests qui en contient une, quel que soit son statut.

L’importateur ne récupère rien lui-même. `Loader` parcourt `wsdl:import`,
`xsd:import` et `xsd:include` : il dit quel emplacement lire ensuite et
reprend le texte ; la CLI et l’application de bureau lisent ainsi fichiers et
réseau chacune à sa manière. Les emplacements se résolvent par rapport à leur
document, un document venu du réseau ne peut pas pointer vers des fichiers
locaux, et une lecture s’arrête à 64 documents. `Model` rassemble ensuite
messages, types de port, liaisons, ports et éléments globaux des schémas de
tous les documents, indexés par espace de noms et nom. `Writer` construit des
exemples à partir des nœuds XSD (roxmltree les garde et résout `prefix:name`
par rapport aux déclarations du nœud lui-même), et décide pour chaque élément
s’il est qualifié d’après `form`, `elementFormDefault` et le fait d’être
global. Les espaces de noms qu’il utilise vont sur l’enveloppe. Une pile des
types nommés en cours d’écriture arrête la récursion.

L’import par URL dans l’application et la CLI lit le texte, puis suit les mêmes
chemins qu’un fichier ; il sert donc tous les formats. Les WSDL de test de
`apix-import/tests/fixtures/` alimentent aussi l’exemple `soap_server` (:8773).

## Schémas GraphQL (M6)

`apix_core::graphql` contient la requête standard `INTROSPECTION_QUERY`, un
`Schema` lu à partir de sa réponse (types avec champs, arguments, champs
d’entrée, valeurs d’enum, interfaces et types possibles ; `TypeRef` garde les
listes et `!`) et `to_sdl`, qui écrit d’abord les types racines et laisse de
côté les scalaires intégrés et les types `__`. Pour l’éditeur, il ne lit une
requête que jusqu’au curseur : `spot_at` découpe le texte en jetons
(commentaires, chaînes et `$variables` retirés) et parcourt une pile de
sélections, listes d’arguments, objets d’entrée et listes, en suivant les
alias, `... on Type`, les fragments et les définitions de variables, pour dire
ce qui convient à cet endroit (`Spot`). `suggestions` en tire des libellés avec
leurs types et descriptions, classés par préfixe, puis sous-chaîne, les
obsolètes en dernier ; `describe_at` donne le Markdown du survol. Rien n’est
validé : une requête cassée se complète quand même du mieux possible.
L’éditeur de variables complète aussi : `variable_types` lit les déclarations
`$name: Type`, un petit analyseur JSON trouve le chemin des clés et des
éléments de liste jusqu’au curseur et si c’est une clé ou une valeur qui va là,
et `variable_suggestions` suit ce chemin à travers les champs d’entrée et les
éléments de liste pour suggérer les variables pas encore données, les champs
d’entrée, les valeurs d’enum (en chaînes JSON) et `true`/`false` ; une chaîne en
cours de saisie est remplacée avec ses guillemets, y compris celui que
l’éditeur a fermé.

`apix_http::graphql::fetch_schema` envoie la requête d’introspection en POST
avec l’URL, les en-têtes, l’auth et les réglages de la requête (la requête est
résolue d’abord) et lit le schéma, ou dit pourquoi il n’y en a pas (un statut
hors 2xx avec le début du corps, ou le `errors[0].message` du serveur).

Dans l’application, `workspace/graphql_ui.rs` implémente le
`CompletionProvider` et le `HoverProvider` de gpui-kit sur un
`Rc<RefCell<Option<Rc<Schema>>>>` partagé que détient l’éditeur de requête ;
les complétions remplacent le mot en cours de saisie par une modification de
texte LSP. **Récupérer le schéma** dans l’éditeur de Corps émet `FetchSchema` ;
l’espace de travail construit la requête comme pour un envoi (auth et réglages
hérités, variables) et la récupère sur le runtime réseau. La réponse de chaque
point de terminaison est gardée dans `<config>/Apix/graphql/<FNV-1a of
the URL>.json` et chargée quand une requête GraphQL s’ouvre. La vue SDL est un
éditeur en lecture seule rempli au moment du dessin, seulement une fois
affiché.

## gRPC (M6)

`apix-grpc` fait des appels sans code généré. La description du service est un
`prost_reflect::DescriptorPool` : à partir des fichiers `.proto` de la requête
via protox (Rust, pas de `protoc` ; les imports sont cherchés dans
`import_paths` et à côté de chaque fichier, les types bien connus de Google
sont intégrés), ou à partir du serveur via la réflexion gRPC (v1, puis
v1alpha ; les fichiers que le serveur omet sont demandés par nom, puis ajoutés
dans l’ordre des dépendances). Les messages sont des `DynamicMessage` : le JSON
entre via la prise en charge serde de prost-reflect, le JSON sort avec les noms
de champs du `.proto` et les valeurs par défaut affichées. `DynamicCodec` les
branche dans tonic ; `tonic::client::Grpc` exécute ainsi les quatre sortes de
méthodes (unaire, streaming serveur, streaming client, bidirectionnelle) sur un
seul `Channel` (HTTP/2 sur les hyper et rustls que le moteur HTTP utilise
déjà).

`apix_grpc::convert` construit une description à partir de ce qu’il y a
d’autre, en essayant tour à tour : un ensemble de descripteurs en JSON ; du
texte `.proto` ; le `FileDescriptorProto` qu’embarque le code généré
(`embedded.rs` sait comment chaque générateur écrit ces octets, et ceux de PHP
peuvent même ne pas être du texte valide) ; puis des déclarations dans un
langage de programmation (`source.rs` : commentaires retirés, blocs trouvés,
membres lus par famille de langages, types normalisés, numéros tirés des
annotations et des encodeurs de ts-proto) ou un exemple JSON (`json.rs`, qui
garde l’ordre des clés, que `serde_json::Value` perdrait). Les deux derniers
passent par un petit `Schema` rendu en `.proto`. `printer.rs` réécrit une
description exacte en texte `.proto`, et on ne s’y fie que si ce texte se
compile vers la même description ; sinon, ou pour plusieurs fichiers, la
définition reste un ensemble de descripteurs en JSON. `compare` confronte une
définition à la description d’un serveur, en rapprochant les champs par nom,
puisque les noms de messages n’apparaissent jamais sur le réseau. `described`
fixe l’ordre : la définition de la requête, ses fichiers, la réflexion ; une
supposition essaie d’abord la réflexion et lui cède la place. `call` signale les
champs qu’un `DynamicMessage` n’a pas pu placer (`CallEvent::Note`), parce que
rien d’autre ne le ferait.

Les en-têtes et l’auth de la requête deviennent des métadonnées (Bearer,
Basic, un en-tête de clé d’API, OAuth 2.0 via `apix_http::oauth2` ; les clés
`-bin` sont en base64) ; les types d’auth qui signent les requêtes HTTP sont
refusés avec une raison. Le délai d’expiration, le certificat client et la CA
approuvée viennent des réglages de la requête. Quand la vérification TLS est
désactivée, tonic (qui n’a pas un tel interrupteur) reçoit une adresse
`http://` simple et la vraie origine `https`, et un connecteur à nous
(`insecure.rs`) fait le TLS : rustls avec l’ALPN h2 et un vérificateur qui
accepte n’importe quel certificat mais contrôle quand même les signatures de la
poignée de main. `call` signale les événements `Sent`, `Headers` et `Received`
au moment où ils se produisent et se termine par un `CallOutcome` : le code de
statut et le message, les métadonnées de réponse, les trailers et tous les
messages. Un statut autre que OK est un résultat, pas une erreur ; les erreurs
sont ce qui empêche un appel d’avoir lieu (pas de connexion, pas de
description, un message qui ne correspond pas à son type).

Dans l’application, une URL `grpc://` transforme l’onglet Corps en onglet
Message (`workspace/grpc_editor.rs`) : un sélecteur de méthode avec recherche
rempli par **Charger les méthodes**, les fichiers `.proto`, et le message avec
un exemple construit à partir du type d’entrée. L’appel tourne sur le runtime
réseau et remplit une structure partagée que l’interface lit toutes les 60 ms ;
le panneau de réponse est un journal (`ResponseState::Grpc`,
`workspace/grpc_ui.rs`) comme celui d’un WebSocket, et Esc abandonne l’appel.
La CLI affiche une réponse unaire en entier et les flux ligne par ligne, et
`apix schema` écrit le service en texte proche du `.proto`.

Les exécutions appellent les requêtes gRPC via `apix_grpc::send_for_tests`, qui
transforme le `CallOutcome` en `apix_http::Response`
(`CallOutcome::to_response`) : le statut HTTP correspondant au code (la
correspondance de Google) avec le nom du code comme raison, les métadonnées et
trailers plus `grpc-status` et `grpc-message` comme en-têtes, et le message
(une liste JSON pour un flux serveur) comme corps. Les scripts de test, les
vérifications et les rapports n’ont alors besoin de rien de propre à gRPC.
L’historique garde un appel avec `Summary::grpc` défini : le statut est le
code gRPC, les trailers et le message de statut ont leurs propres champs, et le
corps est le journal (`[{"at_ms", "sent", "message"}]`), que
`grpc_ui::restored` retransforme en vue de journal.

## Exécutions parallèles et moniteurs (M7)

`Plan::parallel` au-dessus de un exécute autant d’itérations à la fois
(`buffer_unordered` de `futures_util` sur la tâche propre de l’exécution, pas de
threads) : chaque itération envoie ses requêtes dans l’ordre avec sa propre
copie de l’état des scripts, un échec sous `bail` arrête les autres, et le
rapport est trié par itération ensuite. La progression passe par un mutex
autour du callback de l’appelant.

`apix_core::monitor` décrit `[[monitor]]` dans `apix.toml` (nom, `every`,
`run`, environnement, webhook, activé) et lit les intervalles ;
`apix_runner::monitor` en exécute un (`plan` résout les variables comme
`apix run` : l’environnement du moniteur, `APIX_VAR_*`, puis les valeurs
épinglées), le résume (`Outcome`), garde un `Log` en lignes JSON par moniteur
(`<config>/Apix/monitors/<hash of the collection>-<name>.jsonl`, les 500
derniers), décide quand chacun est dû (`Scheduler` : un moniteur vu pour la
première fois est dû dès que son intervalle s’est écoulé depuis sa dernière
exécution journalisée), et prévient un webhook quand l’état bascule (`change`,
`notify`). `apix monitor` et l’application de bureau
(`workspace/monitors_ui.rs` : un minuteur chaque seconde tant que le réglage
est activé, une exécution à la fois sur le runtime réseau) l’utilisent tous les
deux ; ils partagent donc le journal.

## Git (M7)

`apix_core::git` lance le programme `git` déjà installé (pas de bibliothèque
git : rien n’est ajouté au binaire, et la configuration, les hooks et les
identifiants de l’utilisateur s’appliquent). Un `Repo` est ouvert au dossier de
la collection ou du projet et chaque commande s’y exécute avec `--relative` ;
les chemins sont donc relatifs à ce dossier, et une collection dans un dépôt
plus grand ne voit que ses propres fichiers. `changes_since(rev)` est
`git diff --merge-base rev` (là où la branche a quitté `rev`, comme une pull
request) plus les fichiers non suivis ; sans historique (un clone
superficiel), il se rabat sur `rev` lui-même et le dit. `show(rev, path)` lit
un fichier à un commit.

`git::affected` relie les fichiers modifiés aux requêtes selon leur rôle
(`git::role_of`) : un fichier de requête ou ses scripts `<stem>.*.js` → cette
requête ; `folder.toml` ou `_folder.*.js` → les requêtes sous ce dossier ;
`apix.toml`, `_collection.*.js`, `environments/*.toml`, le fichier de projet ou
les environnements du projet → toutes ; tout autre fichier → les requêtes qui
le nomment. `apix run --changed` garde ces requêtes et le rapport enregistre le
choix (`RunReport::selection`).

`apix_core::diff` compare deux versions d’un fichier selon leur sens : les
requêtes sont analysées en `Request` et sérialisées en JSON (valeurs par défaut
et mise en forme n’ont donc pas d’importance), les autres TOML en valeurs, les
scripts et les autres textes ligne par ligne (plus longue sous-suite commune,
début et fin communs retirés). Les listes de lignes ayant une `key` ou un
`name` distincts (en-têtes, query, `[[example]]`) deviennent des tables
indexées par ce champ ; les lignes se rapprochent donc par nom ; les champs
sont nommés par leur chemin (`headers.Accept`, `body.json`). Une requête
supprimée et une requête ajoutée au même contenu deviennent un seul
déplacement. `apix diff` affiche le résultat en texte, en Markdown ou en JSON.

`RunReport::to_markdown` écrit l’exécution pour GitHub (une ligne de tableau
par requête, les échecs dans un `<details>` ouvert), en commençant par un
marqueur caché `<!-- apix-run: <name> -->` que l’étape de commentaire de
l’action recherche pour modifier son commentaire précédent. Il reste sous la
limite de 65 536 caractères de GitHub.

L’application de bureau (`workspace/git_ui.rs`) lit la branche et
`git status` pour le dossier ouvert en arrière-plan après chaque rechargement
et quand la fenêtre est activée, et garde une table des chemins modifiés pour
les marques de la barre latérale ; une requête dont les scripts propres ont
changé compte comme modifiée.

La fenêtre Git (`workspace/git_panel.rs`) utilise le même `Repo` :
`working_changes` pour la liste, `show("HEAD", path)` comparé au fichier sur le
disque via `diff::file` pour le fichier sélectionné, et `commit_folder`
(`git add -A -- .`, puis `git commit -- .`, donc seulement le dossier ouvert),
`branches`, `switch`, `create_branch` (`switch -c`), `pull` (`--ff-only`),
`push` (`-u origin <branch>` quand il n’y a pas d’amont) et `log` (hash, heure,
sujet). Git tourne avec `GIT_TERMINAL_PROMPT=0` ; un dépôt distant qui demande
un mot de passe échoue donc avec le message de git au lieu d’attendre une
saisie ; les assistants d’identifiants (credential helpers) fonctionnent
toujours. Chaque opération tourne en arrière-plan et rafraîchit la barre
d’état.

## Assistant IA

`apix_core::ai` construit à la main la requête pour l’un de trois fournisseurs
(Rust n’a pas de SDK officiel pour eux) et lit la réponse. `Provider`
(`anthropic`, `openai`, `google`) donne le modèle par défaut, le compte de la
clé dans le trousseau (`ai:<provider>-api-key`, via
`apix_vault::keychain::save_text`), sa variable d’environnement et l’URL.
Anthropic : `POST /v1/messages` avec `x-api-key`,
`anthropic-version: 2026-10-21`, `claude-opus-5-5` et, pour le modèle par
défaut, des solutions de repli côté serveur (`fallbacks: "default"`, bêta
`server-side-fallback-2026-07-01`) ; une raison d’arrêt `refusal` devient un
message. OpenAI : `POST /v1/chat/completions` avec une clé bearer, un message
système et un message utilisateur ; un champ `refusal` est décliné. Google :
`POST /v1beta/models/{model}:generateContent` avec `x-goog-api-key`,
`systemInstruction` et `contents` ; les fins `SAFETY` et les prompts bloqués
sont déclinés, les parties de réflexion ignorées. Chaque prompt porte l’échange
(`Context::describe` : en-têtes d’identifiants masqués, corps coupés à 12 KB),
la tâche et le code de langue de l’application. `models_request` / `models`
listent ce qu’une clé peut utiliser (seulement les modèles de chat d’OpenAI,
les modèles Google qui font `generateContent`). `APIX_AI_URL` remplace l’URL de
base pour les tests. L’application (`workspace/ai_ui.rs`) envoie via
`net::send`, ajoute les tests avec `ScriptEditor::append_test` et affiche les
explications dans une boîte de dialogue ; Paramètres garde `ai_provider` et
`ai_model`.

## Serveur MCP

`apix mcp` (`apix-cli/src/mcp.rs`) lit un message JSON-RPC 2.0 par ligne sur
l’entrée standard et écrit une réponse par ligne sur la sortie standard (le
transport stdio de MCP ; une marque d’ordre des octets avant la première ligne
est ignorée). `initialize` répond avec la version de protocole demandée quand
il la connaît (2025-06-18, 2025-03-26, 2024-11-05) et la plus récente sinon, et
ne propose que des outils ; les notifications ne reçoivent pas de réponse, les
méthodes inconnues donnent l’erreur -32601. Les résultats de `tools/call`
portent le JSON à la fois en texte et en `structuredContent`, et un outil qui
échoue est un résultat avec `isError`, pas une erreur de protocole, pour que
l’agent puisse lire pourquoi. Les chemins venus de l’agent sont joints à la
racine de la collection, `..` résolu, et refusés hors d’elle. `send_request`
passe par `apix_runner::exchange` (scripts, variables, tests), `run_requests`
par `apix_runner::run` ; les variables viennent du même `VarArgs::resolve` que
pour les autres commandes, avec les `variables` de l’agent comme des `--var`.
La CLI tourne sur un thread de 16 MB, car le future partagé des commandes a
dépassé la pile du thread principal sous Windows.

## Configuration et Claude

`workspace/setup_ui.rs` ouvre Configurer Apix une fois (`settings.setup_shown`)
et depuis Aide. Le bouton de la base de données exécute
`route_ui::download_geo` (le fichier DB-IP de ce mois, sinon celui du mois
dernier, via le client de l’application, décompressé hors du thread de
l’interface) ; `apix geo download|status|remove` fait la même chose dans un
terminal, et c’est ce qu’appellera l’étape facultative d’un installateur.
`apix_core::claude_mcp` trouve la configuration de Claude Desktop
(`%APPDATA%\Claude`, `~/Library/Application Support/Claude`,
`~/.config/Claude` ; `APIX_CLAUDE_CONFIG` pour les tests), nomme le serveur
`apix-<collection>`, le fait pointer vers l’`apix` situé à côté du programme en
cours (ou dans le PATH) avec `mcp <folder> [--list-only]`, et le fusionne dans
`mcpServers` en gardant tout le reste (l’original copié en `.bak` une fois) ;
`claude_code_command` est `claude mcp add --scope user`.

## Flux

`apix_core::flow` est le fichier : un `Flow` a des nœuds (`NodeKind::Request`,
`Check`, `Set`, `Delay`, aplatis dans chaque table `[[node]]` avec leur id et
leur position) et des arêtes qui vont du résultat d’un nœud (`ok`/`fail`,
`yes`/`no`, `next`) à un autre nœud. `problems()` liste ce qui empêche un flux
de tourner ; `Seen` est la dernière réponse, et `Seen::subject` lit `status`,
`time`, `body`, `header:Name`, ou un filtre JSONPath ou jq via
`apix_core::filter`. Les fichiers `.flow` se trouvent n’importe où dans la
collection ; la barre latérale les ignore.

`apix_runner::flow::run` parcourt le graphe à partir du nœud de départ. Un nœud
de requête passe par `exchange`, comme une exécution ; scripts, tests et
variables `pm.*` fonctionnent donc ; ce qu’il garde est défini dans la portée
locale de l’état des scripts, qui l’emporte sur l’environnement pour les nœuds
suivants, comme le ferait `pm.variables.set`. Une vérification dont le sujet
est du texte résout les `{{variables}}` et n’a pas de valeur quand l’une d’elles
n’est pas définie. L’exécution signale chaque étape (`FlowProgress`) et
s’arrête après 1000 étapes, ou via `Control`. `FlowReport::to_run_report`
réutilise les rapports d’exécution (JUnit, HTML, Markdown). L’éditeur de bureau
(`workspace/flows_ui.rs`) dessine les nœuds comme des éléments positionnés et
les arêtes sur un canevas : les arêtes vers l’avant en courbes, celles qui
reviennent en arrière en lignes qui contournent les nœuds ; il exécute le flux
sur le runtime réseau et lit sa progression avec un minuteur, comme le Runner.

## Extensions

`apix-plugins` exécute les extensions WebAssembly avec `wasmi` 2, un
interpréteur : pas de JIT, donc aucun code natif n’est produit à partir d’une
extension, et il compile partout où Rust compile. Un module ne peut rien
importer ; une extension n’a donc aucune fonction hôte (ni réseau, ni
fichiers, ni horloge, ni aléa) ; elle exporte `memory`, `apix_alloc`,
`apix_manifest` et `apix_call`, et les réponses sont du JSON transmis sous la
forme `ptr << 32 | len`. Chaque appel instancie le module à neuf (aucun état
entre les appels) avec 2·10⁹ unités de carburant et une limite de mémoire de
256 MB, sur un thread avec une pile de 64 MB ; manquer de carburant est signalé
comme « ran too long ». Le manifeste est lu une fois au chargement de
l’extension ; son `name` doit être `[a-z0-9-]`. `installed()` met en cache le
dossier des extensions (`<config>/Apix/plugins`, ou `APIX_PLUGINS_DIR`)
jusqu’à `reload()`.

Les hooks rejoignent le reste d’Apix à trois endroits : `Auth::Plugin { plugin,
config }` dans le modèle, résolu comme les autres auth (variables dans la
config) et appliqué par `apix_http::prepare` à la requête finale ;
`apix_import::plugin::plan`, qui transforme la réponse d’un importateur en
fichiers de requête ; et le visualiseur de la réponse (`VisualMode::Plugin`),
qui exécute l’extension hors du thread de l’interface et dessine un tableau ou
du texte. wasmi est compilé avec `opt-level = 3` et sans assertions de debug,
même en debug, car sa répartition repose sur des appels terminaux qui sinon
épuisent la pile.

## Comparer des réponses

`apix_core::compare` prend deux `Snapshot` (libellé, statut, en-têtes, corps,
temps). Les en-têtes sont comparés par nom en minuscules, plusieurs en-têtes du
même nom étant joints ; ceux de `VOLATILE` (dates, cache, ids de requête et de
trace, limites de débit, mode d’envoi du corps) sont seulement comptés comme
laissés de côté. Deux corps JSON passent par `by_id`, qui transforme les listes
d’enregistrements ayant des valeurs `id` / `_id` / `uuid` / `key` distinctes en
tables indexées `[id=…]`, puis par `diff::fields_as_they_are` (le parcours du
diff de requêtes sans son rapprochement par key/name, qui convient aux fichiers
de requête mais pas aux données) ; les autres textes sont comparés ligne par
ligne, tout le reste seulement par taille. La fenêtre Comparer les réponses de
l’application (`workspace/compare_ui.rs`) prend le côté antérieur dans
l’historique (entrées avec le même fichier de requête, ou la même méthode et la
même URL) ou envoie la requête résolue avec les variables d’un autre
environnement ; `apix compare` l’envoie dans deux environnements.

## Tests de charge

`apix_http::load::Load` construit un client reqwest pour une requête résolue
(les réglages TLS de la requête via `tls::client_config`, son proxy, son délai
d’expiration et sa politique de redirection, un pool de connexions inactives
illimité) et `hit()` l’envoie, en comptant les octets du corps sans les
décoder ; OAuth 2.0 devient un jeton bearer une seule fois, Digest est refusé.
`apix_runner::bench::Bench::run` démarre `concurrency` workers dans un seul
`FuturesUnordered` ; chacun prend le numéro de requête suivant dans un compteur
atomique jusqu’à ce que le nombre, l’échéance ou `stop()` dise non, et avec un
débit fixé attend jusqu’à `start + n / rate`. Chaque envoi est enregistré sous
un mutex : sa latence en microsecondes (un `u32` par requête, donc un million
de requêtes font 4 MB), le statut, l’erreur par sorte (« could not connect »,
« timed out », …), les octets et un compartiment par seconde. `summary()` peut
être appelé pendant l’exécution : il trie une copie des latences pour les
percentiles par rang le plus proche. La CLI exécute le test sur un runtime à
elle avec un thread par cœur et affiche une ligne par seconde ; l’application
l’exécute sur son runtime réseau et redessine quatre fois par seconde.

## Route et carte

`apix-route` lance l’outil du système, comme `apix_core::git` lance git :
`tracert -d -h N -w 1000` sous Windows, `traceroute -n -q 1 -w 1 -m N` sous
macOS et Linux, `tracepath -n -m N` quand traceroute manque. Aucun d’eux n’a
besoin des droits d’administrateur, ce qu’exigerait sous Windows l’envoi d’ICMP
par nous-mêmes. `parse_line` lit une ligne de n’importe lequel d’entre eux
uniquement d’après ses nombres et ses adresses (le numéro du saut d’abord,
chaque `N ms`, le premier mot qui se lit comme une adresse) ; un Windows
traduit dans sa propre page de code se lit donc de la même façon ; le temps
d’un saut est le meilleur de ses essais, `<1 ms` compte pour une demi-
milliseconde. `Trace` parcourt les sauts au fur et à mesure que l’outil les
affiche et le tue quand il est abandonné ou arrêté.

`Geo` ouvre un fichier MaxMind DB (`maxminddb`) et donne un `Place` (latitude,
longitude, nom de ville en anglais, pays ISO) pour les adresses publiques ; les
adresses privées, de bouclage, lien-local, CGNAT et multicast ne sont jamais
recherchées. La base est le premier `.mmdb` trouvé dans `<config>/Apix/geo` ;
`install` décompresse un téléchargement (gzip), vérifie qu’il s’ouvre et
seulement alors remplace l’ancien. `dbip_urls` donne l’adresse DB-IP Lite de ce
mois et du mois dernier (`APIX_GEO_URL` les remplace). La carte est une
projection équirectangulaire de `land::LAND`, continents et grandes îles
dessinés à la main à environ deux degrés près (avec la mer Noire et la mer
Caspienne dans `land::WATER`) ; aucune donnée cartographique n’est donc
téléchargée ni soumise à licence. La fenêtre de bureau
(`workspace/route_ui.rs`) exécute la trace sur un thread, place les sauts à
mesure qu’ils arrivent, lit la base une fois par lancement de l’application et
dessine la carte avec des chemins GPUI. `apix trace` affiche les sauts avec
leurs lieux et la distance de chaque bond de plus de 100 km.

## Vérification des mises à jour

`apix_core::update` construit la requête (le `releases/latest` de GitHub pour
11R5/Apix, `User-Agent: Apix/<version>`, un délai d’expiration de 10 s ;
`APIX_UPDATE_URL` la dirige ailleurs) et lit la réponse : `Version` lit
`v1.2.3`, `1.2` et les préversions (`1.0.0-rc.1` se classe avant `1.0.0`, les
parties numériques par valeur), 404 signifie que rien n’est encore publié, les
autres erreurs portent le message de GitHub. La CLI (`apix update`) l’envoie
avec son propre client ; l’application (`workspace/update_ui.rs`) via
`net::send` au démarrage quand `settings.check_updates` est activé (seule une
version plus récente est affichée) et depuis Aide › Rechercher des mises à
jour… (toute réponse est affichée). Rien n’est téléchargé ni installé.

## Thèmes

`theme.rs` contient les couleurs. Un `ThemeDef` est un thème (intégré, ou lu
dans `<config>/Apix/themes/*.toml` ; les couleurs qu’un fichier omet viennent
d’Obsidian ou de Daylight) avec des `Colors` et un indicateur sombre/clair.
`Palette::build` en fait ce avec quoi les vues dessinent : le réglage d’accent
(une couleur nommée avec une nuance sombre et une nuance claire plus profonde,
ou `#RRGGBB`), le texte sur l’accent choisi selon la luminance relative, les
teintes de survol et de saisie pour le sombre ou le clair, et le réglage de
transparence : l’alpha du fond de la fenêtre est `1 - t`, les panneaux
s’éclaircissent à 60 % de ce rythme, les bordures tirent vers le blanc à mesure
que `t` augmente, et les thèmes clairs utilisent 70 % de `t`. `from_settings`
choisit le thème sombre ou clair selon le réglage d’apparence et, pour « suivre
le système », selon `App::window_appearance`.

La vue racine de gpui-kit peint `Theme::background` sous toute la fenêtre, ce
qui cacherait le flou du système ; avec une transparence quelconque, il est
donc rendu transparent et les boîtes de dialogue reçoivent leur propre fond via
`bidi::DialogBidi`, que toutes les boîtes de dialogue utilisent. C’est le
système qui fait le flou : `WindowBackgroundAppearance::Blurred` est l’effet
acrylique sous Windows (`SetWindowCompositionAttribute`) et la vibrancy sous
macOS.

Les vues ne gardent jamais de copie de la palette : elles appellent
`theme::current()` quand elles dessinent. `theme::set` stocke la nouvelle
palette, y fait pointer le thème des composants de gpui-kit (en changeant son
mode, pour que les couleurs de l’éditeur de code suivent), règle le fond de
chaque fenêtre sur flou ou opaque et rafraîchit les fenêtres ; un changement se
voit donc tout de suite. L’espace de travail observe l’apparence de la fenêtre
et l’appelle de nouveau quand le système bascule entre clair et sombre.

## Langues (M4)

Chaque texte de l’interface de bureau passe par une clé : `tr!("menu.file")`
(une fine enveloppe autour de `rust_i18n::t!` qui renvoie une `SharedString` à
afficher ; `tr_plain!` donne le même texte pour ce qui est enregistré, comme le
nom par défaut d’une nouvelle requête). rust-i18n est le même système que celui
de gpui-kit ; les deux suivent donc une seule locale pour tout le processus ; le
chinois utilise `zh-CN`, le code qu’utilisent les propres textes de gpui-kit.
Chaque langue est un fichier à plat,
`crates/apix-desktop/locales/<code>.yml` (une paire `key: value` par ligne,
triées, marqueurs `%{name}`), compilé dans le binaire ; `build.rs` recompile
quand un fichier change. Un texte manquant retombe sur l’anglais, et un test
unitaire échoue quand une langue n’a pas une clé, en a une inconnue ou utilise
d’autres marqueurs que l’anglais. Les phrases sont des clés entières avec
marqueurs, jamais collées à partir de morceaux, car l’ordre des mots diffère
d’une langue à l’autre.

La langue se choisit dans Paramètres (ou suit la langue du système au premier
démarrage) et change immédiatement : les textes sont lus à chaque rendu, et les
quelques textes gardés dans des widgets (textes indicatifs des champs, entrées
des sélecteurs) sont redéfinis par `Workspace::retranslate`. Les glyphes du
japonais, du chinois et de l’arabe viennent du mécanisme de polices de secours
de la plateforme (DirectWrite, Core Text, fontconfig), à côté de la police Inter
embarquée. Le texte arabe est mis en forme et placé de droite à gauche ; sous
Windows, cela demande `vendor/gpui-pre-windows`, le backend DirectWrite de
gpui 0.3.9 avec le placement des suites de glyphes corrigé (substitué depuis le
`Cargo.toml` de l’espace de travail ; à retirer quand l’amont l’aura corrigé).
gpui ne sait pas faire passer à la ligne du texte de droite à gauche : il met
en forme un paragraphe comme une seule ligne et le coupe à des positions x de
glyphes qu’il suppose croissantes de gauche à droite. Le texte qui peut passer
à la ligne (notes, indications, descriptions, messages) passe donc par
`bidi::para`, qui, en RTL, coupe le texte aux espaces dans l’ordre de lecture,
en mesurant chaque ligne candidate, et dessine chaque ligne sans retour
automatique (`src/bidi/paragraph.rs`). Dans les autres langues, c’est du texte
gpui ordinaire. gpui met aussi chaque ligne en page avec une direction de base
de gauche à droite ; dans une langue RTL, `tr!` enveloppe donc chaque texte
dans un enchâssement de droite à gauche (U+202B…U+202C ; DirectWrite ignore les
isolats plus récents). Une phrase qui commence ou finit par un mot latin
(« URL », « OAuth 2.0 », « Ctrl+S ») se lit alors toujours de droite à gauche,
et `i18n::display` fait de même pour les textes composés comme la barre d’état.
Les marques n’atteignent jamais les fichiers : les textes enregistrés utilisent
`tr_plain!`. Les messages d’erreur des crates du moteur restent en anglais.

Dans une langue de droite à gauche (`Language::rtl`, l’arabe), la fenêtre se
met en miroir : la barre latérale passe à droite, le panneau de réponse à
gauche du panneau de requête, les boutons de la fenêtre à gauche, et les menus,
les barres d’onglets, les lignes de formulaire et la barre d’état partent de la
droite, avec le texte aligné à droite. GPUI n’a pas de direction de mise en
page ; c’est donc `src/bidi.rs` qui s’en charge : un trait `Bidi` sur chaque
élément stylé (`dir_row` inverse une ligne, `text_start` / `text_end`, les
marges `ps` / `pe` et `border_e_1` choisissent le côté selon la direction,
`ltr` fixe le texte à gauche), `tab_bar` / `fill_tabs` pour le `TabBar` de
gpui-kit (onglets donnés dans l’ordre inverse, indices remis en
correspondance), et `bidi_title` pour les boîtes de dialogue (en RTL, le bouton
de fermeture passe dans la ligne de titre, à gauche). Les utilitaires lisent la
langue pendant le rendu d’une vue ; un changement de langue met donc la fenêtre
en miroir immédiatement. Les visionneuses de code et de réponse, la barre
d’URL, les tableaux clé–valeur (paramètres, en-têtes, champs de formulaire,
variables) et les en-têtes de réponse restent de gauche à droite dans toutes
les langues. Les menus contextuels et les boutons de gpui-kit gardent leur
propre disposition interne de gauche à droite (raccourci à droite, icône avant
le libellé) ; les menus s’ouvrent vers la gauche à partir de leur bouton.

Ajouter une langue : copiez `locales/en.yml` en `locales/<code>.yml`, traduisez
les valeurs et ajoutez une ligne à `LANGUAGES` dans `src/i18n.rs`.

La documentation utilisateur vit à part du plan, un dossier par langue
(`Docs/<code>/` : guide de l’utilisateur, architecture, notes techniques,
workflows, images), listés dans `Docs/languages.toml`. L’anglais est la
source ; chaque traduction enregistre la version anglaise à partir de laquelle
elle a été faite, si bien que les traductions périmées se repèrent.

## Modèle de threads (bureau)

- GPUI possède le thread principal, qui fait tourner la boucle d’événements et
  le rendu.
- Un seul **runtime tokio** multi-thread est démarré une fois sur un thread
  d’arrière-plan. reqwest, tonic et tungstenite en ont besoin.
- L’interface envoie le travail à tokio et reçoit le résultat dans une tâche
  GPUI (`cx.spawn_in`). Abandonner cette tâche interrompt la requête ; Esc et un
  nouvel envoi annulent donc proprement.
- Après l’arrivée d’une réponse, le corps est mis en forme et indexé sur
  l’exécuteur d’arrière-plan de GPUI avant que l’interface ne le voie. La mise
  en forme utilise `apix_core::text::pretty_json`, un ré-indenteur en une seule
  passe qui ne construit aucun arbre et garde les nombres exactement tels
  qu’envoyés.

## Visionneuse de grandes réponses

Les corps de 512 KB et plus utilisent `large_view.rs` au lieu de l’éditeur de
code :

- `apix_core::text::RowIndex` enregistre où commence chaque ligne affichée,
  8 octets par ligne. Les lignes de plus de 2 KB (JSON minifié) sont découpées
  en plusieurs lignes affichées.
- La position de défilement est un **numéro de ligne**, pas un décalage en
  pixels. Dix millions de lignes à 20 px feraient environ 200 millions de px,
  bien au-delà de la plage où les pixels en `f32` sont exacts. La visionneuse
  ne met en page que les ~120 lignes qui tiennent à l’écran. Elle gère la
  molette, sa propre barre de défilement et le clavier (↑ ↓ PgUp PgDn,
  Ctrl+Home/End).
- L’onglet Brut d’un grand corps JSON n’est construit que quand on l’ouvre.
- Ctrl+F cherche dans tout le texte sur un thread d’arrière-plan (une `regex`
  insensible à la casse sur la requête littérale) et garde jusqu’à 100,000
  plages d’octets ; `RowIndex::row_of` trouve la ligne d’une correspondance par
  recherche dichotomique, et seules les lignes à l’écran cherchent leurs
  correspondances pour les surligner.

Les corps plus petits sont dans l’éditeur de code, dont le panneau de recherche
propre est activé. Le filtre au-dessus de Formaté (`workspace/filter_ui.rs`)
exécute `apix_core::filter` hors du thread de l’interface, 250 ms après la
dernière frappe : JSONPath via `serde_json_path` pour une expression qui
commence par `$`, jq via `jaq` sinon, au plus 10,000 sorties jq. Son résultat
est un `Document` comme n’importe quel corps ; un grand corps filtré peut donc
arriver dans l’éditeur, et inversement.

Mesuré sous Windows dans une compilation release, pour 100 MB de JSON minifié
venant d’un serveur local
(`cargo run -p apix-http --release --example big_json_server`) : la réponse est
à l’écran environ 0.33 s après Entrée. Cela couvre le transfert, la mise en
forme en 9.6 millions de lignes, l’indexation et la première image. La mémoire
est d’environ 407 MB avec le corps chargé, contre 69 MB au repos.

## Budget de performance

| Mesure                              | Objectif          | Mesuré (Windows, release) |
|-------------------------------------|-------------------|-----------------------------|
| Démarrage à froid jusqu’à la première image | < 150 ms  | 222 ms (de `main` à la première image ; environ 150 ms correspondent au chargement du pilote graphique) |
| RAM au repos                        | < 60 MB           | 88 MB d’ensemble de travail, collection d’exemple ouverte (surtout le pilote graphique) |
| Binaire release                     | < 100 MB          | 54.4 MB |
| Latence de la saisie au dessin      | 1 image (≤ 8 ms à 120 Hz) | non mesurée |
| Ouvrir et faire défiler un corps JSON de 100 MB | aucune image perdue | à l’écran 306 ms après l’envoi (serveur local) ; temps par image non mesurés |

Le temps de démarrage et la mémoire au repos dépassent le budget aujourd’hui ;
leur profilage est prévu. Les chiffres viennent de `scripts/measure.ps1` (voir
les [notes techniques](technical.md#mesurer-les-performances)).

Le profil release utilise déjà le LTO complet (fat), une seule unité de
génération de code et des symboles retirés (voir le `Cargo.toml` racine).
