<!-- translated from en/user-guide.md sha256:302bcb1b0e41cc49 -->

# Guide d'utilisation d'Apix

Apix est un client d'API de bureau : vous construisez des requêtes HTTP, vous
les envoyez et vous lisez les réponses. Les requêtes sont enregistrées sous
forme de petits fichiers texte dans un dossier, si bien qu'une collection peut
vivre dans git à côté de votre code. Ce guide présente l'application ; les
[workflows](workflows.md) décrivent pas à pas les tâches courantes, et la ligne
de commande `apix` exécute les mêmes requêtes dans un terminal ou en CI.

![La fenêtre d'Apix](../en/images/main.png)

## Sommaire

- [Installer Apix](#installer-apix)
- [Ouvrir une collection](#ouvrir-une-collection)
- [La fenêtre](#la-fenêtre)
- [Envoyer une requête](#envoyer-une-requête)
- [Paramètres et en-têtes](#paramètres-et-en-têtes)
- [Corps](#corps)
- [Authentification](#authentification)
- [Réglages de requête](#réglages-de-requête)
- [Cookies](#cookies)
- [Variables et environnements](#variables-et-environnements)
- [La réponse](#la-réponse)
- [L'onglet Réseau](#longlet-réseau)
- [WebSocket et événements envoyés par le serveur (SSE)](#websocket-et-événements-envoyés-par-le-serveur-sse)
- [Socket.IO](#socketio)
- [MQTT](#mqtt)
- [SOAP](#soap)
- [gRPC](#grpc)
- [Scripts et tests](#scripts-et-tests)
- [Runner](#runner)
- [Flux](#flux)
- [Tests de charge](#tests-de-charge)
- [Moniteurs](#moniteurs)
- [Serveur fictif](#serveur-fictif)
- [Documentation de l'API](#documentation-de-lapi)
- [Proxy de capture](#proxy-de-capture)
- [Onglets](#onglets)
- [Organiser les requêtes](#organiser-les-requêtes)
- [Projets](#projets)
- [Git](#git)
- [Collections chiffrées](#collections-chiffrées)
- [Copier en et coller curl](#copier-en-et-coller-curl)
- [Importer depuis Postman](#importer-depuis-postman)
- [Importer depuis Insomnia, OpenAPI et HAR](#importer-depuis-insomnia-openapi-et-har)
- [Historique](#historique)
- [Comparer des réponses](#comparer-des-réponses)
- [Assistant IA](#assistant-ia)
- [Extensions](#extensions)
- [Palette de commandes](#palette-de-commandes)
- [Configuration](#configuration)
- [Paramètres et langue](#paramètres-et-langue)
- [Raccourcis clavier](#raccourcis-clavier)

## Installer Apix

La page de téléchargement, **https://11r5.github.io/Apix/**, propose la
dernière version pour le système que vous utilisez ; tous les fichiers sont
aussi sur la [page des versions](https://github.com/11R5/Apix/releases).
Chacun contient l'application de bureau et la ligne de commande `apix`.

| Système | Fichier | Comment |
|---|---|---|
| Windows 10/11 | `Apix-<version>-windows-x64-setup.exe` | Lancez-le. Il installe Apix pour votre utilisateur sans droits d'administrateur (ou pour tout le monde, si vous le choisissez), l'ajoute au menu Démarrer et peut mettre `apix` dans le PATH et ouvrir les fichiers `.apix`. `…-windows-x64.zip` est la même chose sans installation. |
| macOS 11+ | `Apix-<version>-macos-universal.dmg` | Ouvrez-le et faites glisser Apix dans Applications ; une seule application pour Apple silicon et Intel. La ligne de commande est `/Applications/Apix.app/Contents/MacOS/apix`. Ou bien `brew install --cask 11r5/apix/apix`, qui met aussi `apix` dans le PATH. |
| Debian, Ubuntu, Mint, … | `apix_<version>_amd64.deb` | `sudo apt install ./apix_<version>_amd64.deb` |
| Arch, Manjaro, EndeavourOS, … | `apix-<version>-1-x86_64.pkg.tar.zst` | `sudo pacman -U ./apix-<version>-1-x86_64.pkg.tar.zst`, ou `makepkg -si` avec le `PKGBUILD` de la version |
| Autres Linux (glibc 2.35+) | `apix-<version>-linux-x64.tar.gz` | Décompressez-le et lancez `./install.sh` (pour votre utilisateur ; `--system` pour tout le monde, `--uninstall` pour désinstaller) |
| Tout Linux, un seul fichier | `Apix-<version>-x86_64.AppImage` | Rendez-le exécutable avec `chmod +x` et lancez-le ; `Apix-….AppImage cli …` lance la ligne de commande |

Les builds ne sont pas signés (la signature de code coûte de l'argent chaque
année, elle n'est donc pas prévue pour l'instant). Sous Windows, SmartScreen
peut bloquer l'installateur : **Informations complémentaires › Exécuter quand
même**. Sous macOS, ouvrez Apix la première fois avec clic droit › **Ouvrir**,
ou lancez `xattr -dr com.apple.quarantine
/Applications/Apix.app`. Sous Linux, Apix dessine avec Vulkan, le pilote
graphique doit donc prendre en charge Vulkan (Mesa le fournit sur la plupart
des systèmes). `SHA256SUMS.txt` dans chaque version liste les sommes de
contrôle des fichiers.

## Ouvrir une collection

Une **collection** est un dossier qui contient un fichier `apix.toml` et vos
requêtes (fichiers `.apix`), éventuellement dans des sous-dossiers. Ouvrez-en
une avec **Fichier › Ouvrir…** (Ctrl+O), ou commencez-en une nouvelle avec
**Fichier › Nouvelle collection…**. Si vous choisissez un dossier qui n'est pas
encore une collection, Apix propose d'en faire une.

Les collections ouvertes récemment se trouvent sous **Fichier › Ouvrir
récent**. Lancer l'application avec un dossier
(`apix-desktop path/to/collection`) l'ouvre directement.

## La fenêtre

- **Barre latérale (à gauche) :** les dossiers et les requêtes de la
  collection, lus depuis les fichiers sur le disque et tenus à jour quand ils
  changent. Le sélecteur sous le nom de la collection choisit l'environnement
  actif.
- **Onglets (en haut) :** chaque requête que vous ouvrez a son onglet.
- **Volet de requête :** la méthode, l'URL et les onglets Paramètres,
  En-têtes, Auth, Corps et Réglages.
- **Volet de réponse (à droite) :** le statut, le temps, la taille et le corps
  de la réponse.
- **Barre d'état (en bas) :** la collection, le fichier de la requête ouverte
  et les raccourcis les plus utiles.
- **Barre de menus :** Fichier, Requête, Environnement, Affichage et Aide
  contiennent toutes les commandes.

## Envoyer une requête

1. Choisissez la méthode à gauche de la barre d'URL (ou appuyez sur Alt+1…7
   pour GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS).
2. Saisissez l'URL. Les `{{variables}}` sont permises partout (voir
   [Variables](#variables-et-environnements)).
3. Appuyez sur **Envoyer** ou sur Ctrl+Enter. Esc annule une requête encore en
   cours.

Tant qu'un environnement dont le nom contient « prod » est actif, Apix demande
confirmation avant d'envoyer DELETE, PUT ou PATCH et affiche l'URL complète
visée. **Paramètres › Confirmer en production** désactive cette demande.

**Nouvelle requête** (Ctrl+N) ouvre une requête GET ; le menu **+** de la barre
latérale propose toutes les méthodes, et POST, PUT et PATCH commencent avec un
corps JSON. Enregistrez avec Ctrl+S. Un point sur l'onglet et dans la barre
latérale signale des modifications non enregistrées.

## Paramètres et en-têtes

Les **Paramètres** forment la chaîne de requête (query string) de l'URL ; les
**En-têtes** sont envoyés tels quels. Décochez une ligne pour la garder dans le
fichier sans l'envoyer. Une ligne sans clé est ignorée. `Content-Type` est
déduit du type de corps, sauf si vous ajoutez le vôtre. Les `{{variables}}`
dans les clés et les valeurs s'affichent sous forme de pastilles, comme dans la
barre d'URL ; survolez-en une pour voir sa valeur.

La barre d'URL et le tableau des Paramètres restent synchronisés : saisir
`?page=2&sort=name` dans l'URL remplit le tableau, et modifier, ajouter,
décocher ou supprimer une ligne réécrit l'URL. Les valeurs s'affichent décodées
(`%20` devient une espace) et sont encodées à l'envoi de la requête. Le fichier
garde l'URL sans sa chaîne de requête, et les lignes sous forme de `[[query]]`.

**Variables de chemin.** Un segment de chemin écrit `:name`, comme dans
`{{base}}/users/:id/posts`, obtient une ligne sous **Variables de chemin** dans
l'onglet Paramètres. Sa valeur remplace le segment à l'envoi de la requête,
encodée comme un seul segment (`a b/c` devient `a%20b%2Fc`), et les
`{{variables}}` y fonctionnent. Une variable laissée vide est envoyée telle
qu'elle est écrite. Les valeurs sont gardées dans le fichier sous forme de
`[[path]]`, et les collections Postman apportent les leurs (et les récupèrent à
l'export).

![Paramètres et variables de chemin](../en/images/path-variables.png)

**Suggestions d'en-têtes.** Pendant la saisie du nom d'un en-tête, les noms
d'en-têtes usuels qui correspondent s'affichent sous la ligne (d'abord ceux
qui commencent par ce qui est saisi, et aucun qui soit déjà dans le tableau) ;
sa valeur reçoit les valeurs usuelles de cet en-tête, comme `application/json`
pour `Content-Type`. Enter prend la première suggestion, un clic prend
n'importe laquelle, et après un nom, c'est au tour de la valeur.

![Noms d'en-têtes suggérés pendant la saisie](../en/images/header-hints.png)

## Corps

L'onglet Corps propose :

| Type | Ce qui est envoyé |
|---|---|
| Aucun | Pas de corps. |
| JSON, Texte, XML, HTML | Le texte tel qu'il est écrit, avec coloration syntaxique (`application/json`, `text/plain`, `application/xml`, `text/html`, sauf si un en-tête dit autre chose). |
| Formulaire | Des lignes clé–valeur en `application/x-www-form-urlencoded`. |
| Multipart | `multipart/form-data`. Chaque champ est **Texte** ou **Fichier** ; le bouton dossier choisit un ou plusieurs fichiers. Le champ de type à côté fixe le Content-Type propre au champ (comme `application/json` ou `image/png`) ; vide, il laisse Apix choisir. |
| Binaire | Les octets d'un fichier, en `application/octet-stream` sauf si un en-tête dit autre chose. Apix affiche la taille du fichier, ou « Fichier introuvable ». |
| GraphQL | Un éditeur de requête et un éditeur JSON de variables ; envoyé sous forme de JSON `{"query", "variables"}`. |

![Corps multipart](../en/images/body.png)

Les chemins de fichiers situés dans la collection sont enregistrés relativement
au fichier de la requête, si bien que la collection continue de fonctionner sur
une autre machine.

### Schéma GraphQL et complétion

**Récupérer le schéma**, au-dessus de la requête, demande son schéma au serveur
(la requête d'introspection standard), avec l'URL, les en-têtes,
l'authentification et l'environnement de la requête, si bien qu'un schéma
protégé par un jeton fonctionne. Ensuite :

- La saisie dans la requête suggère ce qui convient à la position du curseur :
  les champs du type sélectionné (avec leurs types), les arguments d'un champ
  après `(`, les valeurs d'énumération et `true`/`false` après `:`, les champs
  d'un objet d'entrée, les fragments `... on Type` pour les unions et les
  interfaces, et les noms de types après `on` ou dans `$variable: Type`. Les
  champs obsolètes viennent en dernier. Ctrl+Space ouvre les suggestions sans
  rien saisir.
- Survoler un champ, un argument ou une valeur d'énumération affiche son type
  et sa description.
- Dans l'éditeur de variables, les variables que la requête déclare
  (`$role: Role`) sont suggérées comme clés, puis les champs des objets
  d'entrée, les valeurs d'énumération (en chaînes JSON) et `true`/`false` pour
  leurs valeurs.
- **Afficher le schéma** place tout le schéma à côté de la requête, en SDL.

![Complétion à partir du schéma](../en/images/graphql.png)

Apix garde le schéma de chaque point d'accès (dans le dossier de
configuration, sous `graphql/`) et le recharge quand vous ouvrez une requête
vers cette URL ; **Actualiser le schéma** n'est donc utile que lorsque l'API
change. Un serveur dont l'introspection est désactivée répond par une erreur,
affichée à la place de l'état du schéma.

En ligne de commande, `apix schema request.apix` affiche le schéma en SDL
(`--json` affiche la réponse du serveur telle qu'elle est arrivée).

## Authentification

L'onglet Auth définit comment la requête prouve son identité :

| Type | Remarques |
|---|---|
| Hériter du parent | Par défaut : utiliser l'authentification du dossier, de la collection ou du projet le plus proche qui en définit une. L'onglet indique laquelle et d'où elle vient. |
| Aucune authentification | Ne rien envoyer, même si un parent définit une authentification. |
| Jeton Bearer | `Authorization: Bearer <token>`. |
| Authentification Basic | Nom d'utilisateur et mot de passe. |
| Clé d'API | Un en-tête ou un paramètre de requête de votre choix. |
| Authentification Digest | Apix répond au défi Digest du serveur (MD5 ou SHA-256). |
| OAuth 1.0 | HMAC-SHA1, HMAC-SHA256 ou PLAINTEXT ; dans l'en-tête ou dans la chaîne de requête ; un nouveau nonce et un nouvel horodatage à chaque envoi. |
| OAuth 2.0 | Client credentials, mot de passe, ou authorization code avec PKCE. Apix obtient le jeton lui-même et l'envoie comme jeton Bearer. |
| AWS Signature | AWS Signature Version 4 avec clé d'accès, clé secrète, jeton de session facultatif, région et service. |

![OAuth 2.0 dans l'onglet Auth](../en/images/auth.png)

**Jetons OAuth 2.0.** Le premier envoi récupère un jeton ; les envois suivants
le réutilisent et le renouvellent avec le refresh token quand il expire.
**Obtenir un nouveau jeton** en récupère un tout de suite, **Oublier** le
supprime. Les jetons sont gardés en mémoire seulement et jamais écrits sur le
disque. Pour l'authorization code grant, Apix ouvre la page de connexion dans
votre navigateur et reçoit la réponse sur `http://127.0.0.1` (un port libre, ou
l'**URL de redirection** que vous avez enregistrée pour le client).

**Authentification partagée.** Faites un clic droit sur un dossier ou une
collection et choisissez **Authentification…** pour définir l'authentification
une seule fois pour tout ce qu'il contient ; **Environnement ›
Authentification du projet…** fait de même pour tout un projet. Les requêtes
dont l'authentification est *Hériter du parent* l'utilisent.

## Réglages de requête

L'onglet Réglages contrôle la façon dont la requête voyage :

| Réglage | Par défaut |
|---|---|
| Délai (secondes) | aucun — une requête tourne jusqu'à ce qu'elle se termine ou que vous l'annuliez ; `0` signifie aussi aucun délai |
| Suivre les redirections | activé |
| Redirections max | 10 |
| Vérifier le certificat TLS | activé — ne le désactivez que pour des serveurs de test de confiance |
| Proxy | celui de l'environnement (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`) ; *Désactivé* les ignore ; une URL en désigne un explicitement |
| Certificat client | aucun — un fichier PEM, avec la clé dans le même fichier ou dans un fichier séparé |
| AC de confiance | seulement les certificats racines usuels — un fichier PEM ajoute vos propres autorités de certification (celle d'une entreprise ou une AC auto-signée), si bien que le serveur est quand même vérifié |
| Envoyer les cookies | activé |

Tout champ laissé vide (ou sur *Hériter*) reprend la valeur du dossier, de la
collection ou du projet le plus proche qui le définit, et en dernier lieu les
valeurs par défaut ci-dessus — la même chaîne que pour l'authentification.
Faites un clic droit sur un dossier ou une collection et choisissez
**Réglages de requête…** pour les définir une seule fois pour tout ce qu'il
contient ; **Environnement › Réglages de requête du projet…** fait de même pour
tout un projet. La ligne sous le formulaire montre ce que la requête utilise au
final. Les `{{variables}}` fonctionnent dans les champs du proxy et des
certificats, et les chemins de certificats relatifs sont résolus par rapport
au fichier qui les définit.

Dans un fichier de requête (et dans `folder.toml`, `apix.toml` et
`apix-project.toml`), les réglages forment une table `[settings]` :

```toml
[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""          # "" = pas de proxy, même si l'environnement en définit un
client_cert = "certs/client.pem"
ca_cert = "certs/company-ca.pem"
```

**Copier en** écrit les options curl correspondantes (`-m`, `-L --max-redirs`,
`-k`, `-x`, `--cert`, `--key`, `--cacert`), et coller une commande curl avec
ces options remplit l'onglet. `apix send` et `apix run` ont `--timeout`,
`--insecure`, `--proxy` et `--cacert`, qui priment sur le fichier. Ces réglages
s'appliquent aussi aux requêtes gRPC. Les options par requête de Postman
(`protocolProfileBehavior`) sont importées.

## Cookies

Apix tient un seul pot à cookies : les réponses `Set-Cookie` y sont stockées
automatiquement et renvoyées avec les requêtes suivantes vers le même site, y
compris après un redémarrage (`cookies.json` à côté des paramètres de
l'application ; les cookies de session disparaissent avec l'application).
**Requête › Cookies…** les liste, supprime des cookies un par un ou vide le
pot. **Envoyer les cookies** dans l'onglet Réglages désactive le pot pour une
requête, un dossier, une collection ou un projet. La CLI commence avec un pot
vide à chaque exécution.

## Variables et environnements

Écrivez `{{name}}` dans une URL, un en-tête, un corps ou un champ
d'authentification, et Apix y met la valeur à l'envoi. Dans la barre d'URL,
chaque variable est une pastille colorée : survolez-la pour voir sa valeur et
d'où elle vient ; une pastille rouge n'est définie nulle part.

Les valeurs viennent par couches, la dernière l'emportant :

1. Les variables du **projet** (partagées par toutes les collections d'un projet)
2. Les variables de la **collection** (`apix.toml`)
3. L'**environnement** actif (`environments/<name>.toml`)

Ouvrez **Environnement › Variables et environnements…** (Ctrl+E) pour les
modifier. Cochez **Secret** pour les jetons et les mots de passe : les valeurs
secrètes sont stockées dans `environments/<name>.local.toml`, qu'Apix ajoute à
`.gitignore`, si bien qu'elles ne sont jamais commitées.

Pour les garder aussi hors du disque, choisissez **Paramètres › Valeurs
secrètes des environnements › Trousseau du système**. Les valeurs vont alors
dans le magasin d'identifiants du système (le Gestionnaire d'identification
sous Windows, le Trousseau sous macOS, le Secret Service sous Linux) et
`<name>.local.toml` ne garde que l'identifiant de cette entrée, si bien que
renommer l'environnement ou déplacer le dossier continue de fonctionner. La
ligne de commande `apix` les lit de la même façon. Changer ce réglage déplace
tout de suite les secrets de la collection ou du projet ouvert ; les autres
sont déplacés la prochaine fois que vous les enregistrez. Les secrets d'une
collection chiffrée restent toujours à l'intérieur de celle-ci.

![La fenêtre Variables](../en/images/variables.png)

Les **variables dynamiques** prennent une nouvelle valeur à chaque
utilisation : `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`,
`{{$randomInt}}`, `{{$randomEmail}}` et d'autres ; la page **Variables
dynamiques** de la fenêtre Variables les liste toutes avec un exemple de
valeur.

## La réponse

Le volet de réponse affiche le statut, le temps jusqu'au premier octet et le
temps total, et la taille. **Formaté** met en forme le JSON (repliez les blocs
avec les flèches) et indente et colore le XML et le HTML ; **Brut** affiche le
corps tel qu'il a été reçu, **En-têtes** liste les en-têtes de la réponse et
**Hex** affiche les octets. Les très gros corps (des dizaines de mégaoctets)
s'ouvrent dans une visionneuse rapide qui ne dessine que les lignes visibles.

Apix déduit la nature du corps de son `Content-Type`, ou des octets quand le
type ne dit rien d'utile. Les images (PNG, JPEG, GIF, WebP, BMP, SVG)
s'affichent dans **Formaté** ; les autres contenus binaires (un PDF, un zip…)
ont un court résumé au lieu d'un texte illisible. Le HTML qui omet des balises
fermantes facultatives est affiché tel qu'il est arrivé plutôt que
reformaté.

Le bouton **↓** au bout de la ligne d'état enregistre le corps, octet pour
octet, dans un fichier. Le nom proposé vient du `Content-Disposition` du
serveur ou de l'URL, avec une extension adaptée au contenu.

**Recherche.** Ctrl+F dans le corps, ou la loupe de la ligne d'état, cherche
dans **Formaté**, **Brut** et **Hex**, sans tenir compte de la casse. Enter
(ou F3) va à l'occurrence suivante et Shift+Enter (Shift+F3) revient en
arrière ; le compteur indique où vous en êtes. La visionneuse rapide des très
gros corps cherche dans tout le corps en arrière-plan et arrête de compter à
100 000 occurrences.

**Filtre.** Une réponse JSON a une zone de filtre au-dessus de **Formaté**.
Une expression qui commence par `$` est du JSONPath (RFC 9535), tout le reste
est du jq ; ces deux expressions fonctionnent donc :

```text
$.users[?@.active].email
.users[] | select(.active) | {name, email}
```

![Une réponse JSON filtrée avec JSONPath](../en/images/response-filter.png)

Le résultat remplace le corps dans **Formaté** tant que la zone contient une
expression (le compteur à côté indique combien de valeurs) ; **Brut** et
**Hex** affichent toujours le corps entier, et ✕ le rétablit. JSONPath donne
la liste de ses correspondances ; jq donne ses sorties l'une après l'autre. Le
filtre reste pour la réponse suivante de la même requête, qu'on peut donc
renvoyer encore et encore en surveillant une valeur.
`apix send --filter '<expression>'` n'affiche que ce que le filtre retient.

**Tableau et graphique.** **Texte**, **Tableau** et **Graphique**, au bout de
la ligne du filtre, affichent une réponse JSON (ou ce que le filtre a retenu)
sous forme de texte, de tableau de ses enregistrements ou de graphique en
barres. Les enregistrements sont le tableau du corps, ou le plus long tableau
d'objets qu'il contient (`$.items`, `$.data`) ; chaque clé est une colonne,
les objets imbriqués s'affichent en JSON compact. Le graphique trace une
colonne numérique (**Valeur**) en fonction d'une colonne qui nomme les lignes
(**Libellé**) ; les deux peuvent être choisies. Les corps de plus de 8 Mo ne
sont pas transformés en tableau : filtrez-les d'abord. `apix send --table`
affiche le même tableau dans un terminal (avec `--filter`, celui de ce qu'il
retient). Les modèles `pm.visualizer.set` de Postman ne sont pas dessinés,
car Apix n'a pas de vue web ; les scripts qui l'appellent obtiennent une note
dans la console.

![La vue Tableau d'une réponse filtrée](../en/images/visual-table.png)

![La vue Graphique](../en/images/visual-chart.png)

**Exemples.** Le bouton plateau de la ligne d'état garde la réponse comme
exemple de la requête : son statut, ses en-têtes et son corps, ainsi que la
requête dont elle provient telle qu'elle est écrite (les `{{variables}}`
restent, si bien qu'aucun jeton n'atterrit dans le fichier). Les cookies
définis par le serveur sont omis, et les corps de plus de 2 Mo sont refusés.
L'onglet **Exemples** de la requête les liste : renommez-en un sur place, ▷
l'affiche dans le volet de réponse (marqué « Exemple : … »), et la corbeille
le retire. Ils font partie de la requête : Ctrl+S les écrit donc dans son
fichier sous forme de `[[example]]`, et les réponses enregistrées de Postman
arrivent comme exemples à l'import et repartent à l'export.
`apix send --save-example '<name>'` garde la réponse depuis la ligne de
commande.

![Un exemple affiché dans le volet de réponse](../en/images/examples.png)

Quand le volet est trop étroit pour tous les onglets de la requête, la flèche
au bout de la barre d'onglets les liste tous.

## L'onglet Réseau

L'onglet **Réseau** explique où le temps est passé et ce qu'était la
connexion :

- une barre de temps découpée en DNS, connexion TCP, poignée de main TLS,
  attente et téléchargement
- les réponses DNS, l'adresse du serveur et l'adresse locale, l'interface
  réseau (avec une indication quand un VPN est utilisé), la passerelle, les
  serveurs DNS et le proxy
- chaque redirection
- un corps compressé : son encodage, sa taille sur le réseau et une fois
  décodé
- le chiffrement : version de TLS, suite de chiffrement, échange de clés, ALPN
  (`h2` quand le serveur parle HTTP/2), le nom de serveur envoyé (SNI) et la
  durée de la poignée de main
- la chaîne de certificats telle que le serveur l'a envoyée : pour chaque
  certificat, à qui il est destiné et qui l'a émis, sa validité et les jours
  restants, la clé ; pour celui du serveur, aussi ses noms et son empreinte
  SHA-256

![L'onglet Réseau](../en/images/network.png)

![Le chiffrement et la chaîne de certificats](../en/images/network-tls.png)

Tout cela vient de la connexion de la requête elle-même, pas d'une seconde
connexion ouverte pour regarder : chaque envoi ouvre une nouvelle connexion et
Apix enregistre ce que cette poignée de main a négocié. Apix demande des
réponses compressées (`gzip`, `deflate`, `br`), sauf si la requête définit son
propre `Accept-Encoding`, et affiche les en-têtes `Content-Encoding` et
`Content-Length` tels que le serveur les a envoyés.

### Route et carte

**Tracer la route et la voir sur une carte…** sous l'adresse du serveur (ou
**Affichage › Route et carte…**) liste les routeurs entre cet ordinateur et le
serveur, avec le temps jusqu'à chacun, et les dessine sur une carte du monde.
Les sauts viennent de l'outil du système (`tracert` sous Windows, `traceroute`
ou `tracepath` ailleurs), si bien qu'aucun droit d'administrateur n'est
nécessaire ; un routeur qui ne répond pas s'affiche comme `*`. Les adresses de
votre propre réseau sont marquées comme telles.

L'emplacement de chaque routeur vient d'une base de géolocalisation IP
présente sur cet ordinateur ; rien n'est consulté en ligne. Sans base, la
liste fonctionne quand même et la carte reste vide : **Télécharger DB-IP
Lite** récupère la base gratuite de DB-IP (environ 130 Mo, CC BY 4.0) dans
`<config>/Apix/geo` (la Configuration et `apix geo download` font de même), ou
placez-y n'importe quelle base de villes `.mmdb` (DB-IP Lite, MaxMind GeoLite2
City). Ces bases situent au mieux une adresse dans une ville et se trompent
parfois pour les routeurs de cœur de réseau. Dans un terminal,
`apix trace <url or host>` affiche la même liste.

![La route sur une carte](../en/images/route.png)

La capture d'écran utilise une petite base de démonstration (`cargo run -p
apix-route --example demo_geo`), pas de vraies recherches.

## WebSocket et événements envoyés par le serveur (SSE)

**WebSocket.** Une requête dont l'URL commence par `ws://` ou `wss://` est un
WebSocket : **Envoyer** devient **Se connecter**, et l'onglet Corps devient
**Messages**. La poignée de main utilise les paramètres, les en-têtes et
l'authentification de la requête (Bearer, Basic, clé d'API, OAuth 2.0 …),
comme n'importe quelle requête.

![Une connexion WebSocket et son onglet Messages](../en/images/websocket.png)

- Saisissez un message et appuyez sur **Envoyer** (ou Ctrl+Enter). Les
  `{{variables}}` sont remplies à l'envoi. Sans connexion, Apix se connecte
  d'abord et envoie le message une fois la connexion ouverte.
- **Enregistrer le message** le garde dans le fichier de la requête sous forme
  de `[[message]]` ; Ctrl+S enregistre le fichier. Cliquez sur un message
  enregistré pour le modifier, ou sur ▷ pour l'envoyer tel quel.
- Le volet de réponse est un journal : ↑ envoyé, ↓ reçu, avec le temps écoulé
  depuis la connexion, ainsi que le moment de la connexion et la façon dont
  elle s'est fermée (code et raison). Il suit les nouveaux messages, sauf si
  vous remontez.
- **Se déconnecter** (ou Esc) ferme la connexion ; le journal reste jusqu'à la
  connexion suivante. Une connexion continue de tourner tant que son onglet
  est à l'arrière-plan.
- **Messages binaires :** choisissez **Base64** ou **Hex** à côté des boutons
  et écrivez les octets de cette façon (`01 02 ff`, espaces et `0x` permis) ;
  un message qui ne se lit pas comme des octets ne peut pas être envoyé. Les
  messages enregistrés gardent leur format. Les messages binaires s'affichent
  dans le journal par leur taille et leurs premiers octets.
- Les **Sous-protocoles** (en haut de l'onglet Messages, séparés par des
  virgules) sont proposés lors de la poignée de main ; le journal indique celui
  que le serveur a choisi. Un serveur qui n'en choisit aucun est refusé, comme
  le font les navigateurs.
- **Ping toutes les (s)**, en dessous, envoie un ping à cet intervalle tant que
  la connexion est ouverte (`[websocket] ping_interval` dans le fichier).
  Chaque réponse s'affiche dans le journal sous la forme `pong after 2 ms` ;
  un ping encore sans réponse quand le suivant est dû ferme la connexion avec
  une erreur, si bien qu'une connexion morte sans bruit apparaît comme morte.
  Les pings du serveur reçoivent toujours une réponse. `apix send --ping 30`
  fait de même (`--ping 0` désactive un intervalle enregistré) et se termine
  avec un code non nul sur une connexion morte.
- `wss://` suit les réglages de la requête : une AC de confiance, un certificat
  client, ou « Vérifier le certificat TLS » désactivé.

**Événements envoyés par le serveur.** Quand une réponse est
`text/event-stream`, ses événements s'affichent au fur et à mesure qu'ils
arrivent, avec l'heure, le type et l'id de l'événement, et les données.
**Arrêter** (ou Esc) met fin au flux et garde ce qui est arrivé ; un flux que
le serveur termine affiche la même liste dans l'onglet Formaté, avec Brut,
En-têtes, Réseau et Hex comme pour toute réponse. Envoyez
`Accept: text/event-stream` si le serveur en a besoin. Un délai de requête dans
les Réglages met fin à un flux lui aussi : laissez-le vide pour les flux qui
restent ouverts.

- Avec plus d'un type d'événement, les types apparaissent au-dessus de la
  liste avec le nombre reçu pour chacun ; cliquez sur l'un d'eux pour
  n'afficher que ceux-là, sur **Toutes** pour tous les événements.
- **Reconnecter** (dans la vue en direct) reprend le flux quand il se termine
  ou que la connexion tombe : après le délai `retry:` du serveur (3 s s'il
  n'en a pas donné), avec `Last-Event-ID` égal au dernier id reçu, comme le
  fait un navigateur. La liste note chaque reconnexion. Une réponse qui n'est
  pas un flux d'événements l'arrête, de même qu'Esc.

En ligne de commande, `apix send` affiche les événements au fur et à mesure
(`--event TYPE`, répétable, n'affiche que ceux-là ; `--reconnect` se reconnecte
sans cesse jusqu'à Ctrl+C). Pour un WebSocket, il se connecte, envoie les
messages enregistrés (ou les textes de `--message`, lus selon
`--format text|base64|hex`), affiche les deux sens et ferme après `--listen`
secondes (3 par défaut, `0` attend le serveur). `apix run` ignore les requêtes
WebSocket.

## Socket.IO

Une requête Socket.IO dialogue avec des serveurs Socket.IO 3 et 4 via
WebSocket. Ouvrez l'onglet **Messages** d'une requête (donnez-lui une URL
`ws://` pour que l'onglet apparaisse) et choisissez **Socket.IO** en haut ;
l'URL peut alors être `http://`, `https://`, `ws://` ou `wss://`, comme dans un
client Socket.IO. Le fichier de la requête reçoit une table `[socketio]`, et
une requête Socket.IO s'ouvre sur son onglet Messages.

![Une connexion Socket.IO : événements, acks et une pièce jointe binaire](../en/images/socketio.png)

- **Espace de noms :** celui à rejoindre, comme `/chat`. Laissé vide, c'est le
  chemin de l'URL (`http://localhost:3000/chat` rejoint `/chat`), ou `/`.
  **Chemin** est l'endroit où le serveur écoute, `/socket.io/` sauf si le
  serveur l'a déplacé. Une adresse copiée depuis l'onglet réseau du navigateur
  (`…/socket.io/?EIO=4&transport=websocket`) fonctionne telle quelle.
- **Auth :** un objet JSON envoyé en rejoignant (l'option `auth` du client),
  avec les `{{variables}}` remplies. Quand le serveur refuse, la raison qu'il a
  donnée s'affiche comme erreur.
- **Événement** indique sous quel nom un message est émis (`message` si vide).
  Le texte est un seul argument : du JSON tel quel, tout autre texte comme une
  chaîne, et rien du tout s'il est vide. **Base64** et **Hex** envoient les
  octets comme pièce jointe binaire. **Demander un accusé (ack)** demande au
  serveur d'accuser réception. Les messages enregistrés gardent leur événement
  et leur ack.
- Le journal affiche chaque événement avec son nom : ↑ émis (avec `ack 0`,
  `ack 1` … quand un ack a été demandé), ↓ reçu (avec `wants ack 9` quand le
  serveur en demande un), et ✓ pour les acks du serveur avec ce qu'il a
  répondu. Les pièces jointes binaires s'affichent par leur taille et leurs
  premiers octets. Les en-têtes, les paramètres et l'authentification de la
  poignée de main ainsi que les réglages TLS fonctionnent comme pour un
  WebSocket.
- Apix répond aux pings du serveur. Quand le serveur fait quitter l'espace de
  noms au client, le journal le signale et la connexion se ferme ;
  **Se déconnecter** (ou Esc) quitte l'espace de noms et ferme.

`apix send` rejoint, émet les messages enregistrés et affiche les événements,
les acks et les pièces jointes binaires au fur et à mesure ; `--message TEXT`
(répétable) émet plutôt des textes, sous l'événement donné par
`--event NAME` (`message` par défaut). `apix run` ignore les requêtes
Socket.IO. Les pièces jointes reçues sont écrites sous la forme
`{"$binary": "<base64>"}`. Pas encore pris en charge : les serveurs Socket.IO 2
(Engine.IO 3), le transport long-polling, et la réponse à un ack demandé par
le serveur.

## MQTT

Une requête dont l'URL commence par `mqtt://` (ou `mqtts://` pour TLS) se
connecte à un broker MQTT 3.1.1 : la barre d'URL et la barre latérale
affichent **MQTT**, **Envoyer** devient **Se connecter**, et l'onglet Corps
devient **Messages**. Le port est 1883, ou 8883 avec TLS, sauf si l'URL en
indique un.

![Une connexion MQTT : abonnements, messages publiés et reçus](../en/images/mqtt.png)

- **Identifiants :** le nom d'utilisateur et le mot de passe du type Basic de
  l'onglet Auth, ou de l'URL (`mqtt://user:password@host`). Les autres types
  d'authentification ne s'appliquent pas à MQTT. Quand le broker refuse, il
  dit pourquoi. Un mot de passe dans l'URL est omis de ce qu'Apix affiche.
- **ID client** est nouveau à chaque connexion s'il est laissé vide.
  **Garder la session** demande au broker de garder la session entre les
  connexions, ce qui nécessite un ID client ; le journal indique quand le
  broker en avait encore une. **Keep-alive** est le nombre de secondes entre
  les pings en période d'inactivité (60 par défaut, 0 pour aucun).
- Les **Abonnements** sont faits à la connexion, chacun avec sa qualité de
  service ; `+` et `#` fonctionnent dans les sujets. Une fois connecté, ▷
  abonne tout de suite à une ligne et ✕ la quitte. Le journal note ce que le
  broker a accepté ou refusé.
- Pour publier, remplissez le **Sujet**, choisissez la qualité de service,
  cochez **Retenir** si le broker doit garder le message pour les abonnés
  futurs, et appuyez sur **Envoyer** (ou Ctrl+Enter). Les `{{variables}}` sont
  remplies. **Base64** et **Hex** envoient des octets. Un message vide est
  permis : avec Retenir, il efface ce que le broker avait gardé.
  **Enregistrer le message** le garde dans le fichier de la requête.
- Le journal affiche chaque message sous son sujet : ↑ publié, ↓ reçu, avec sa
  qualité de service et son indicateur retain, et ✓ quand le broker a
  confirmé un message de QoS 1 ou 2. Les contenus qui ne sont pas du texte
  s'affichent par leur taille et leurs premiers octets.
- `mqtts://` suit les réglages de la requête : une AC de confiance, un
  certificat client, ou « Vérifier le certificat TLS » désactivé. Le délai de
  la requête limite la connexion (10 s sans délai défini).

`apix send` se connecte, s'abonne, publie les messages enregistrés et affiche
ce qui passe dans les deux sens, puis ferme après `--listen` secondes (3 par
défaut, `0` attend le broker). `--subscribe TOPIC` (répétable) ajoute des
abonnements ; `--message TEXT` avec `--topic TOPIC` publie à la place des
messages enregistrés ; `--qos` et `--retain` s'appliquent à ceux-ci. Le code
de sortie est 1 quand le broker a refusé un abonnement, ou a raccroché sans
répondre à un abonnement ou sans confirmer un message. `apix run` ignore les
requêtes MQTT. Pas encore pris en charge : MQTT 5, le dernier testament (last
will) et MQTT sur WebSocket.

## SOAP

Un appel SOAP est une enveloppe XML envoyée en POST ; une requête SOAP est donc
une requête ordinaire : les scripts, les tests, les exécutions et l'historique
fonctionnent dessus comme sur n'importe quelle autre.

**À partir d'un WSDL.** Importez un fichier WSDL 1.1 avec **Fichier ›
Importer…**, ou son adresse avec **Fichier › Importer depuis une URL…**
(`http://host/service?wsdl`). Vous obtenez une collection avec une requête par
opération, et un dossier par port quand il y en a plusieurs, en général un
pour SOAP 1.1 et un pour 1.2. L'adresse du port est la variable `baseUrl` (une
variable par port quand elles diffèrent), et les documents que le WSDL importe
sont lus aussi. Ce qui n'a pas pu être lu, et les ports qui ne sont pas SOAP,
sont listés à la fin de l'import.

![Une requête SOAP issue d'un WSDL, avec le fault renvoyé par le serveur](../en/images/soap.png)

- Les **en-têtes** suivent la version : SOAP 1.1 envoie
  `Content-Type: text/xml` et `SOAPAction` ; SOAP 1.2 envoie
  `application/soap+xml` avec l'action à l'intérieur.
- **Le corps** est une enveloppe avec des valeurs d'exemple construites à
  partir du schéma : `1` pour les nombres, `string` pour le texte, la première
  valeur d'une énumération, la première option d'un choix. Des commentaires
  indiquent ce qui est facultatif ou répété et quelles sont les autres
  valeurs. Les attributs facultatifs sont omis. Remplacez les valeurs, ou
  mettez-y des `{{variables}}`, et supprimez ce dont vous n'avez pas besoin.
- Un type qui se contient lui-même s'arrête au deuxième niveau, avec un
  commentaire.

**Sans WSDL.** Choisissez **XML** comme corps ; tant qu'il est vide,
**SOAP 1.1** et **SOAP 1.2** à côté démarrent une enveloppe et définissent les
en-têtes. Renseignez l'opération et l'action (`SOAPAction`, ou `action=` dans
le `Content-Type`).

**Faults.** Quand une réponse contient un fault SOAP, une ligne rouge
au-dessus de la réponse affiche son code et son message, et `apix send`
l'affiche. Une requête sans tests échoue sur un fault même si le statut est
200, comme certains serveurs l'envoient, dans `apix send`, `apix run` et le
Runner. Les scripts de test décident par eux-mêmes.

Pas encore pris en charge : WSDL 2.0, qui est refusé avec cette raison,
WS-Security, les pièces jointes MTOM, et `xsi:type` dans les exemples pour
l'encodage SOAP, ce dont l'import avertit.

## gRPC

Une requête dont l'URL commence par `grpc://` (ou `grpcs://` pour TLS) appelle
une méthode gRPC : la barre d'URL affiche **gRPC**, et l'onglet Corps devient
**Message**.

![Un appel gRPC](../en/images/grpc.png)

1. Écrivez l'adresse, par exemple `grpc://localhost:50051`.
2. **Charger les méthodes** demande au serveur ce qu'il propose (gRPC server
   reflection). Si le serveur n'a pas la réflexion, **Ajouter un fichier**
   choisit les fichiers `.proto` du service ou des descriptor sets
   (`protoc --descriptor_set_out`, `buf build`, binaires ou JSON) ; les imports
   sont cherchés à côté d'eux, et les well-known types de Google sont intégrés.
   Rien ne nécessite `protoc`. Sans un tel fichier, voir
   [Sans fichier .proto](#sans-fichier-proto).
3. Choisissez la méthode. Son genre est affiché à côté : unaire, flux serveur,
   flux client ou bidirectionnelle.
4. Écrivez le message en JSON, ou appuyez sur **Message d'exemple** pour en
   obtenir un avec tous les champs. Pour une méthode qui prend un flux, une
   liste JSON envoie un message par élément.
5. **Envoyer** (Ctrl+Enter). Le volet de réponse liste ce qui est parti (↑) et
   ce qui est revenu (↓) avec les temps, puis le statut (`0 OK`, ou un code
   comme `5 NOT_FOUND` avec son message), les métadonnées de la réponse et les
   trailers. Un flux affiche ses messages au fur et à mesure ; Esc l'arrête.

Les en-têtes sont envoyés comme métadonnées, et Auth fonctionne comme pour
HTTP avec Bearer, Basic, une clé d'API dans un en-tête et OAuth 2.0. Le délai,
le certificat client, l'AC de confiance et « Vérifier le certificat TLS » des
réglages de requête s'appliquent (désactivé, le certificat d'un serveur
`grpcs://` n'est pas vérifié). Un en-tête `-bin` est lu comme du base64.

En ligne de commande, `apix send` appelle la méthode (les flux sont affichés un
message par ligne au fur et à mesure ; le code de sortie est non nul sauf si le
statut est OK), et `apix schema` affiche le service et ses messages.

**Dans les exécutions et les tests.** `apix run` et le Runner appellent aussi
les requêtes gRPC, et leurs scripts de test et leurs vérifications voient
l'appel comme une réponse HTTP : le statut est le statut HTTP qui correspond au
code gRPC (`OK` vaut 200, `INVALID_ARGUMENT` 400, `NOT_FOUND` 404,
`UNAUTHENTICATED` 401, …), le code et le message gRPC sont dans les en-têtes
`grpc-status` et `grpc-message` à côté des métadonnées et des trailers, et le
corps est la réponse, ou une liste JSON des messages pour une méthode qui
répond par un flux. `pm.response.json().message` ou une vérification sur
`body.message` fonctionne donc comme pour n'importe quelle requête. Un simple
**Envoyer** affiche l'appel sans exécuter ses scripts.

Chaque appel est gardé dans l'**Historique** (Ctrl+H) avec son journal, son
statut et ses métadonnées ; le choisir réaffiche l'appel tel qu'il s'est
terminé.

### Sans fichier .proto

Sur le réseau, gRPC envoie des numéros de champs, pas des noms, et appelle
`package.Service/Method`. Quel que soit votre point de départ, ces éléments
doivent donc être justes à l'arrivée. L'onglet **Définition**, à côté de
**Message**, construit une définition à partir de ce que vous avez.
Collez-le (ou **Lire un fichier**, ou **Utiliser le message**), indiquez la
**Méthode à appeler** si la source ne la donne pas, et appuyez sur
**Convertir**. Le résultat est un texte `.proto` modifiable, gardé dans la
requête ; **Enregistrer dans un fichier** l'écrit sur le disque.

![Une définition construite à partir d'un exemple JSON, et l'avertissement quand elle ne correspond pas](../en/images/grpc-definition.png)

| Vous avez | Ce qu'on en tire |
|---|---|
| Du texte `.proto`, un descriptor set (binaire ou JSON) | Exact. |
| Du code généré qui contient la description : `_pb2.py`, la classe externe Java, Kotlin à travers elle, `.cs`, `.pb.cc`, `.pb.go`, `_pb.rb`, `GPBMetadata` de PHP, `_pb.ts` de protobuf-es | Exact : la description qu'il contient est lue, quel que soit le langage. |
| Du code avec les numéros mais sans description : tags Go `protobuf:"…"`, Rust `#[prost(…)]`, ts-proto, betterproto, `[ProtoMember(n)]`, `@ProtoNumber(n)` | Les numéros et les types sont justes ; vérifiez les appels quand la source ne les liste pas. |
| Des types écrits à la main : TypeScript, Go, Rust, C#, Java, Kotlin, Python, Swift, Dart, C++ | **Une supposition.** Les champs sont numérotés dans l'ordre d'apparition. |
| Un message d'exemple en JSON (la seconde zone, facultative, reçoit la réponse) | **Une supposition.** Les types viennent des valeurs ; une liste JSON donne un flux. |

Une supposition ne vaut que ce que valent ses numéros, et **des numéros faux
n'échouent pas : ils remplissent les mauvais champs.** Apix ne le cache donc
jamais :

- Si le serveur propose la réflexion, sa description est utilisée à la place
  d'une supposition, et chaque différence est listée
  (`number 1 here, 7 on the server`).
- Sinon, chaque appel signale que les numéros ne sont pas vérifiés. Les champs
  de la réponse que la définition ne connaît pas s'affichent avec leurs
  numéros et leurs valeurs (`1:"o-1"`), ce qui permet aussi de déchiffrer une
  réponse dont vous n'avez pas le type. Une réponse qui ne correspond pas du
  tout échoue avec une ligne indiquant que la définition en est probablement
  la cause.

Les autres fichiers que `protoc` génère se contentent d'utiliser la
description (`.pb.h`, les builders Kotlin, `.pyi`), et les stubs gRPC ne
contiennent que les appels ; Apix indique quel fichier utiliser à la place. Ne
sont pas lus : les éditions Protobuf sous forme de texte `.proto` (elles
restent un descriptor set), les oneofs depuis des tags ou des types (leurs
champs deviennent des champs simples, identiques sur le réseau), et les
génériques.

En ligne de commande :

```bash
apix proto service_pb2.py
```

```bash
apix proto order.json --method shop.v1.Orders/Create --response answer.json --out orders.proto
```

```bash
apix proto types.ts --method shop.v1.Orders/Create --into create.apix
```

Le `.proto` part sur la sortie standard ou dans `--out`, `--into` le garde dans
une requête, et ce qui a dû être choisi part sur la sortie d'erreur. `apix
send`, `apix run` et `apix schema` utilisent une définition de la même façon,
et `apix send` affiche les mêmes avertissements ; le rapport d'une exécution
ne les inclut pas.

## Scripts et tests

L'onglet **Scripts** contient deux scripts JavaScript pour la requête :

- **Avant la requête** s'exécute avant l'envoi de la requête. Il peut modifier
  la requête (`pm.request.headers.upsert(…)`, `pm.request.url.query.add(…)`,
  `pm.request.body.raw = …`) et définir des variables
  (`pm.environment.set("token", …)`).
- **Tests** s'exécute après l'arrivée de la réponse et la vérifie :

```js
pm.test("status is 200", () => pm.response.to.have.status(200));
pm.test("has users", () => {
  const body = pm.response.json();
  pm.expect(body.users).to.be.an("array").that.is.not.empty;
});
```

L'API est celle de Postman, `pm.*` (aussi disponible sous le nom `apix.*`) ;
les scripts importés de Postman s'exécutent donc tels quels :
`pm.environment`, `pm.collectionVariables`, `pm.globals` (les variables du
projet), `pm.variables`, `pm.request`, `pm.response`, `pm.test`, `pm.expect`
avec des assertions à la Chai, `pm.info`, `console.log`, et les anciennes
formes `tests["…"] = …` et `postman.setEnvironmentVariable(…)`.

`pm.sendRequest` envoie une autre requête depuis un script, par exemple pour
obtenir d'abord un jeton ; `pm.sendRequest(req, (err, res) => …)` et
`const res = await pm.sendRequest(req)` fonctionnent tous les deux.
`require(…)` propose les parties de `crypto-js` (MD5, SHA-1, SHA-2 et leurs
HMAC, encodeurs Base64/Hex/UTF-8), `uuid`, `lodash` (aussi `_`), `moment`,
`ajv`, `tv4` et `chai` qu'utilisent les scripts d'API ; elles sont intégrées,
rien n'est donc téléchargé. Une fonction qu'une bibliothèque n'a pas échoue
avec un message clair.

La troisième partie de l'onglet Scripts, **Vérifications**, teste la réponse
sans code : chaque ligne compare une cible à une valeur. Les cibles sont
`status`, `time` (ms), `size` (octets), `body`, un chemin dans un corps JSON
comme `body.users[0].id`, ou `header.Content-Type` ; les comparaisons sont
`==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `not contains`, `exists`,
`not exists`, `matches` (une expression régulière comme `/^ok/i`) et `is` (un
type JSON). Les valeurs peuvent utiliser des `{{variables}}`. Les
vérifications sont enregistrées dans le fichier de la requête sous forme de
`[[assert]]` et apparaissent comme des tests, dans l'application et dans
`apix run`.

Pour garder une valeur d'une réponse, utilisez le bouton **{x}** à côté du
statut : indiquez la valeur (mêmes cibles que dans Vérifications) et un nom de
variable, et elle est enregistrée dans l'environnement actif (ou, s'il n'y en
a pas, dans les variables de la collection). Cochez **La mettre à jour à
chaque envoi** pour ajouter aussi la ligne `pm.environment.set(…)`
correspondante au script Tests.

Les scripts sont des fichiers placés à côté de ce à quoi ils appartiennent :
`<request>.prerequest.js` et `<request>.test.js` à côté de la requête,
`_folder.prerequest.js` / `_folder.test.js` dans un dossier,
`_collection.prerequest.js` / `_collection.test.js` à la racine de la
collection. Ils s'exécutent de l'extérieur vers l'intérieur : la collection,
les dossiers, puis la requête. Ctrl+S enregistre les scripts de la requête
avec elle ; le texte de l'éditeur est utilisé même avant l'enregistrement.

Après l'envoi, le volet de réponse reçoit un onglet **Tests** (✓ réussi,
✗ échoué avec la raison) et un onglet **Console** avec la sortie de
`console.*`. Si un script d'avant-requête lève une exception, la requête n'est
pas envoyée et l'erreur indique quel fichier a échoué. Les scripts s'arrêtent
au bout de 10 secondes.

Les valeurs qu'un script définit ne sont écrites dans aucun fichier. Elles
durent tant qu'Apix est ouvert (changer d'environnement supprime ce que
`pm.environment.set` a défini) et apparaissent dans les fiches des variables
avec la mention « Défini par un script ». `pm.variables.set` dure le temps
d'un envoi.

## Runner

Le **Runner** envoie de nombreuses requêtes d'un coup et les vérifie toutes.
Ouvrez-le avec **Affichage › Runner…** ou Ctrl+Shift+R pour la collection où
vous êtes, ou avec **Exécuter…** sur une collection ou un dossier dans la barre
latérale pour cette partie seulement.

![Le Runner après une exécution pilotée par les données](../en/images/runner.png)

- **Requêtes** : toutes les requêtes dans l'ordre de la barre latérale,
  cochées. Décochez celles à laisser de côté, et déplacez-en une avec ↑ ↓ pour
  changer l'ordre de cette exécution.
- **Environnement** : l'environnement dont les variables sont utilisées.
- **Itérations** : combien de fois parcourir les requêtes. Laissé vide, c'est
  une fois, ou une fois par ligne quand il y a un fichier de données.
- **Délai (ms)** : une pause entre deux requêtes, pour les API à débit
  limité.
- **En même temps** : combien d'itérations tournent côte à côte, comme
  plusieurs utilisateurs de l'API en même temps. Chacune envoie ses requêtes
  dans l'ordre (le jeton d'une connexion atteint donc toujours la requête
  suivante) et garde ce que ses propres scripts définissent ; les résultats
  sont listés par itération. Laissé vide, l'une après l'autre.
- **Fichier de données** : un fichier CSV ou JSON pour une exécution pilotée
  par les données. Chaque ligne est une itération et ses colonnes sont des
  variables : `{{email}}` dans la requête et `pm.iterationData.get("email")`
  dans les scripts. Le CSV nécessite une ligne d'en-tête ; les guillemets et
  les fichiers séparés par des points-virgules (Excel dans de nombreuses
  langues) sont compris. Le JSON est un tableau d'objets, comme
  `[{"id": 1, "email": "a@x.test"}, …]`.
- **S'arrêter au premier échec** : terminer l'exécution à la première requête
  qui échoue.

Une requête réussit quand ses tests (scripts et Vérifications) réussissent ;
une requête sans tests réussit sur un statut 2xx ou 3xx. Pendant l'exécution,
vous pouvez faire **Pause**, **Reprendre** et **Arrêter** (après la requête en
cours d'envoi). Fermer la fenêtre n'arrête pas l'exécution : rouvrez le Runner
pour la voir. Cliquez sur un résultat pour voir ses tests, pourquoi il n'a pas
été envoyé, et ses lignes de console. `pm.execution
.setNextRequest("name")` saute à une autre requête de l'exécution et
`setNextRequest(null)` termine l'itération, comme dans Postman.

Le Runner envoie la version enregistrée de chaque requête. Les valeurs que des
scripts ont définies avant l'exécution sont utilisées ; ce que les scripts de
l'exécution définissent reste à l'intérieur de l'exécution.

Une fois l'exécution terminée, **Enregistrer le rapport** l'écrit en **JUnit
XML** (pour les vues de tests des CI), en **JSON** (tout ce qui s'est passé) ou
en **HTML** (une page qui s'ouvre dans n'importe quel navigateur, en clair ou
en sombre). `apix run` fait de même depuis un terminal ; voir
[Exécuter une collection en CI](workflows.md#exécuter-une-collection-en-ci).
Un mot de passe écrit dans une URL (`https://user:password@host`) est omis des
rapports, de la sortie de la ligne de commande, des journaux et des messages
d'erreur. Le fichier de la requête et la barre d'URL le contiennent toujours :
pour de vrais mots de passe, préférez une variable ou l'onglet Auth.

## Flux

Un **flux** relie des requêtes en un graphe : se connecter, garder le jeton,
interroger jusqu'à ce qu'une tâche soit terminée, bifurquer selon ce qui est
revenu. Chaque flux est un fichier `.flow` dans la collection, à côté des
requêtes ; il va donc dans git avec elles. Ouvrez **Affichage › Flux…** ;
saisissez un nom et appuyez sur **Créer** pour en créer un.

![La fenêtre Flux](../en/images/flows.png)

Il existe quatre sortes de nœuds :

- **Requête** envoie une requête de la collection, avec ses scripts et ses
  tests. Son résultat est **ok** quand elle a réussi (un statut de succès, ou
  tous les tests réussis) et **fail** sinon. **Garder de la réponse** transforme
  des valeurs de la réponse en variables pour les nœuds suivants, une par
  ligne : `token = $.token`.
- **Vérification** compare un sujet à une valeur : **yes** ou **no**. Le sujet
  est `status`, `time` (ms), `body`, `header:Name`, un JSONPath
  (`$.items[0].id`) ou un filtre jq (`.items | length`) sur la dernière
  réponse, ou un texte avec des `{{variables}}`. Comparaisons : égal, différent,
  contient, supérieur à, inférieur à (en nombres quand les deux côtés en
  sont), existe, absent.
- **Définir des variables** définit des variables (lignes `name = value` ; les
  `{{variables}}` fonctionnent).
- **Attendre** fait une pause, pour les boucles d'interrogation.

Faites glisser un nœud pour le déplacer. Pour relier, cliquez sur le point
d'un résultat à droite d'un nœud, puis sur le nœud vers lequel il mène ;
cliquer de nouveau sur le point le redirige, et **×** dans le panneau le
supprime. Le panneau de droite modifie le nœud sélectionné : son id, ses
réglages, où mène chaque résultat, **Commencer ici**, **Supprimer le nœud**.
Une exécution commence au nœud de **Début** (sans lui, au premier nœud vers
lequel rien ne mène), suit chaque résultat et se termine sur un résultat qui
ne mène nulle part. Elle échoue quand une requête échoue sans arête **fail**,
ou quand une valeur à garder est absente. Les boucles sont permises ; une
exécution s'arrête après 1000 étapes.

**Lancer** exécute le flux avec les variables de la collection et
l'environnement choisi ; chaque nœud devient vert, jaune ou rouge selon son
résultat, et la liste sous le canevas montre chaque étape avec ce qui a été
gardé ou comparé. Les modifications sont enregistrées avec **Enregistrer**, et
à la fermeture de la fenêtre.

Depuis un terminal, le même flux s'exécute en CI :

```sh
apix flow list my-api
apix flow run my-api/new-user.flow --env staging --junit flow.xml
```

`--junit`, `--html` et `--markdown` font un rapport sur les requêtes du flux
comme `apix run` ; `--json` écrit chaque étape. La collection d'exemple
contient `new-user.flow`.

Un fichier de flux :

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

## Tests de charge

**Requête › Test de charge…** (ou la palette de commandes) envoie la requête
ouverte depuis de nombreux workers à la fois et montre comment le serveur
tient le coup : requêtes et requêtes par seconde, échecs, les 50e, 95e et 99e
centiles et le temps le plus lent, et un graphique seconde par seconde des
requêtes (celles en échec en rouge) avec la latence moyenne marquée d'un trait
jaune.

![Un test de charge](../en/images/load-test.png)

- **En même temps** : requêtes en vol au même moment (1 à 10 000).
- **Requêtes** : s'arrêter après ce nombre ; vide pour aucune limite.
- **Pendant** : s'arrêter après cette durée (`30s`, `2m`, `1500ms`) ; sans
  l'un ni l'autre, 10 s.
- **Par seconde** : au plus ce nombre de requêtes par seconde, réparties
  régulièrement ; vide pour aller aussi vite que le serveur répond.

Les connexions restent ouvertes et sont réutilisées, comme le font de vrais
clients sous charge (un envoi simple ouvre une nouvelle connexion pour que
son onglet Réseau montre la poignée de main). Les variables sont remplies une
seule fois ; les scripts ne s'exécutent pas, OAuth 2.0 récupère son jeton une
seule fois et l'authentification Digest n'est pas prise en charge. Une requête
compte comme échouée quand aucune réponse n'est arrivée ou que le statut est
400 ou plus. **Arrêter** termine le test, et fermer la fenêtre l'arrête aussi.
Ne testez en charge que des serveurs pour lesquels vous en avez le droit.

Dans un terminal, `apix bench` fait de même et peut faire échouer un job de
CI :

```bash
apix bench my-api/users/list.apix -c 50 -d 30s --max-p95 300 --max-failed 1 --json bench.json
```

Il se termine avec 1 quand le 95e centile dépasse `--max-p95` millisecondes,
que plus de `--max-failed` pour cent des requêtes ont échoué, ou qu'aucune n'a
réussi.

## Moniteurs

Un **moniteur** exécute la collection, ou l'un de ses dossiers ou l'une de ses
requêtes, selon un planning, et le signale quand elle commence à échouer ou
réussit de nouveau. Ouvrez **Affichage › Moniteurs…**.

![Moniteurs](../en/images/monitors.png)

- **Ajouter un moniteur** demande un nom, une fréquence (`30s`, `5m`, `1h`,
  `1d`, ou combinés comme dans `1h30m` ; au moins `10s`), un dossier ou une
  requête (vide pour toute la collection), un environnement et un webhook. Le
  bouton ⚙ modifie un moniteur, le bouton ▷ l'exécute tout de suite.
- Chaque moniteur affiche son dernier résultat (réussi, ou combien ont échoué
  et les premiers échecs) et, tant que la planification est active, sa
  prochaine exécution.
- **Exécuter les moniteurs selon leur planning tant qu’Apix est ouvert** active
  la planification (elle est désactivée jusqu'à ce que vous la choisissiez).
  Apix exécute alors les moniteurs dus en arrière-plan, un à la fois, et
  affiche une notification quand l'un commence à échouer ou réussit de
  nouveau.
- Un **webhook** est prévenu de la même chose : Apix envoie en POST un JSON
  avec une ligne `text` (et `content`) que Slack, Discord, Mattermost et Google
  Chat affichent comme un message, plus les détails (`monitor`, `status`,
  `passed`, `failed`, `failures`). Les `{{variables}}` sont remplies, si bien
  que l'adresse peut rester dans les secrets d'un environnement.

Les moniteurs sont gardés dans le `apix.toml` de la collection ; ils sont donc
partagés via git :

```toml
[[monitor]]
name = "Health"
every = "5m"
run = "users"            # un dossier ou une requête ; omettez-le pour toute la collection
environment = "staging"
webhook = "{{alerts_url}}"
# enabled = false        # listé, mais non exécuté selon son planning
```

Pour surveiller une API quand l'application est fermée, lancez `apix monitor`
dans un terminal, depuis cron ou le Planificateur de tâches, ou selon un
planning de CI ; voir
[Surveiller une API avec des moniteurs](workflows.md#surveiller-une-api-avec-des-moniteurs).
L'application et `apix monitor` tiennent le même journal de résultats ; chacun
montre donc ce que l'autre a trouvé.

## Serveur fictif

Le **serveur fictif** répond avec les exemples enregistrés avec les requêtes
de la collection (voir Exemples), si bien qu'on peut construire un front-end
ou lancer un test avant que la vraie API n'existe. Ouvrez **Affichage ›
Serveur fictif…**, choisissez un port et **Démarrer**.

![Le serveur fictif](../en/images/mock.png)

- Une requête est mise en correspondance par méthode et par chemin : l'URL de
  la requête après son hôte ou après une `{{variable}}` initiale, si bien que
  `{{base_url}}/users/:id` sert `/users/7`. Les segments `:name` et `{{name}}`
  correspondent à n'importe quel segment, et le chemin le plus littéral
  l'emporte. Un exemple dont la requête enregistrée a une autre URL sert cette
  URL.
- Avec plusieurs exemples, le premier ayant un statut 2xx répond. Une requête
  peut en choisir un autre avec l'en-tête `x-mock-response-name: <example>` ou
  `x-mock-response-code: 404` (ceux que lisent les serveurs fictifs de
  Postman).
- Chaque réponse autorise toutes les origines (CORS) et indique de quel
  exemple elle vient dans `X-Apix-Mock` ; une requête préliminaire `OPTIONS`
  reçoit aussi une réponse.
- Sans correspondance, on obtient `404` avec une liste JSON de ce qui est
  servi.
- La fenêtre liste ce qui est servi et les dernières requêtes. Les exemples
  ajoutés ou modifiés sont servis immédiatement ; le serveur continue de
  tourner quand la fenêtre se ferme, jusqu'à **Arrêter** ou jusqu'à ce que
  l'application quitte.

Pour l'utiliser, ajoutez un environnement (disons `mock`) dont `base_url` vaut
`http://127.0.0.1:4010`. Depuis un terminal : `apix mock my-api --port 4010`
(`--host 0.0.0.0` pour être joignable depuis d'autres ordinateurs,
`--delay 300` pour un serveur lent).

## Documentation de l'API

**Fichier › Exporter la documentation…** (ou **Exporter la documentation…** sur
une collection dans la barre latérale) écrit la collection sous forme de
documentation d'API : une page HTML qui s'ouvre dans n'importe quel navigateur,
en clair ou en sombre, avec une table des matières ; ou du Markdown pour un
dépôt quand le nom du fichier se termine par `.md`.

![Documentation générée](../en/images/api-docs.jpg)

Elle montre la description de la collection (Markdown), ses variables et les
variables commitées dans chaque environnement, puis chaque requête dans
l'ordre de la barre latérale sous ses dossiers : méthode, URL, description,
variables de chemin, paramètres de requête, en-têtes, type d'authentification,
corps, vérifications et exemples enregistrés. Rien de secret n'est imprimé :
les fichiers `.local.toml` et le trousseau ne sont pas lus, l'authentification
n'affiche que son type, et les en-têtes `Authorization`, `Cookie` et de clé
d'API n'affichent leur valeur que lorsqu'il s'agit d'une `{{variable}}`. Le
HTML brut dans les descriptions est omis.

Depuis un terminal : `apix docs my-api --out api.html` (ou `--out API.md` ;
sans `--out`, du Markdown sur la sortie standard).

## Proxy de capture

Le **proxy de capture** transforme du trafic réel en requêtes. Ouvrez
**Affichage › Proxy de capture…**, choisissez un port (8888) et **Démarrer**,
puis définissez `http://127.0.0.1:8888` comme proxy d'un navigateur, d'une
application ou d'un script (`HTTP_PROXY` et `HTTPS_PROXY`). Chaque requête qui
passe est enregistrée dans le dossier `captured/` de la collection sous forme
de fichier de requête, avec sa réponse comme exemple `Captured`, et listée
dans la fenêtre.

![Le proxy de capture](../en/images/capture.png)

- **Seulement** limite l'enregistrement à certains hôtes (`example.com` prend
  aussi `api.example.com`) ; le reste passe sans être enregistré.
- Rien de secret n'est écrit : `Cookie` est supprimé, et les valeurs des
  en-têtes `Authorization` et de clé d'API deviennent des variables
  (`{{authorization}}`).
- Le HTTPS voyage dans un tunnel que le proxy ne peut pas lire ; par défaut, il
  est donc transmis et seul son hôte est listé. Avec **Ouvrir le HTTPS**
  coché, Apix répond à la poignée de main TLS avec un certificat pour l'hôte
  signé par une autorité locale (`<config>/Apix/capture-ca.pem`) et enregistre
  aussi les requêtes. Cela ne fonctionne que là où cette autorité est
  approuvée : la fenêtre affiche la commande qui l'approuve (sous Windows
  `certutil -user -addstore Root …`) ; Apix ne l'installe jamais, et Firefox
  garde sa propre liste.

Depuis un terminal : `apix capture my-api/captured --port 8888 --only example.com`
(`--https` pour ouvrir le HTTPS, `--no-save` pour seulement observer).

## Onglets

Chaque onglet garde ses modifications non enregistrées, sa dernière réponse,
et même une requête encore en cours. Ctrl+Tab et Ctrl+Shift+Tab passent d'un
onglet à l'autre, Ctrl+W ou un clic du milieu en ferme un ; Apix demande
confirmation avant de fermer des modifications non enregistrées. Les onglets
qui étaient ouverts reviennent la prochaine fois que vous ouvrez la même
collection ou le même projet (les onglets des collections chiffrées ne sont pas
mémorisés, car leurs noms de fichiers sont secrets eux aussi).

## Organiser les requêtes

La **recherche** en haut de la barre latérale la restreint aux requêtes dont
le nom, la méthode ou l'URL contiennent chaque mot saisi (`get users` trouve
une requête GET vers `/users`), avec les dossiers qui y mènent ouverts. Un
dossier dont le nom correspond montre tout ce qu'il contient. Esc ou ✕ efface
la recherche, et les dossiers redeviennent comme avant.

Faites un clic droit dans la barre latérale pour créer, renommer, dupliquer ou
supprimer des requêtes et des dossiers ; les fichiers supprimés vont dans la
corbeille du système. Faites glisser une requête ou un dossier au milieu d'un
autre dossier pour l'y déplacer. Déposez-le plutôt sur le bord supérieur ou
inférieur d'une ligne pour le placer avant ou après cette ligne ; un trait
montre où il va atterrir. L'ordre est enregistré dans `apix.toml` (pour le
premier niveau de la collection) et `folder.toml` (pour un dossier) sous forme
de liste `order` ; il est donc partagé via git ; les éléments qu'elle ne liste
pas suivent par nom. Les imports Postman gardent l'ordre de Postman, et les
exports vers Postman suivent le vôtre.

## Projets

Un **projet** regroupe plusieurs collections qui vont ensemble et partage
entre elles les environnements et l'authentification. Créez-en un avec
**Fichier › Nouveau projet…**. Dans un projet, **Environnement › Grouper la
barre latérale par** peut afficher la barre latérale sous la forme Projet ›
Environnement › Collections.

- Faites glisser l'en-tête d'une collection au-dessus ou en dessous d'une
  autre pour changer leur ordre.
- Clic droit sur une collection › **Couleur** lui donne une couleur ; un point
  l'affiche dans la barre latérale et sur les onglets de ses requêtes. Les
  couleurs sont enregistrées dans `apix-project.toml`.
- **Fichier › Déplacer dans un projet…** (avec une seule collection ouverte)
  déplace le dossier de cette collection dans un projet que vous choisissez.
  Avec un projet ouvert, déposer le dossier d'une collection depuis le
  gestionnaire de fichiers sur la fenêtre propose la même chose.

## Git

Une collection n'est faite que de fichiers ordinaires ; elle vit donc dans git
comme du code. Quand la collection (ou le projet) ouverte se trouve dans un
dépôt git, la barre d'état affiche la branche et combien de ses fichiers ont
changé depuis le dernier commit, et la barre latérale marque chaque requête
modifiée : **M** modifiée (son fichier ou ses propres scripts), **A** nouvelle.
Ces marques suivent chaque enregistrement et chaque changement sur le disque,
et sont relues quand la fenêtre revient au premier plan, si bien qu'un commit
fait dans un terminal les efface.

![Branche et requêtes modifiées](../en/images/git-status.png)

Cliquez sur la branche dans la barre d'état (ou **Affichage › Git…**) pour
ouvrir la fenêtre Git. À gauche se trouvent les fichiers modifiés depuis le
dernier commit et les derniers commits ; choisissez un fichier pour voir ce qui
y a changé, de la même façon que `apix diff` le montre : une requête champ par
champ (`query._limit: 5 → 10`, un en-tête ajouté), les scripts et les autres
textes ligne par ligne, et rien du tout quand seule la mise en forme a changé.
En dessous, écrivez un message et faites **Commit** : il prend toutes les
modifications du dossier ouvert et rien en dehors. Le menu des branches change
de branche, **Nouvelle branche** en crée une à partir du nom saisi à côté et
passe dessus, et **Pull** (fast-forward seulement) et **Push** (en définissant
l'upstream la première fois) lancent git avec vos propres réglages, hooks et
identifiants. La barre latérale suit immédiatement un changement de branche.

![La fenêtre Git](../en/images/git-window.png)

En ligne de commande, `apix run --changed` n'envoie que les requêtes touchées
par une modification et `apix diff` montre ce qui y a changé champ par champ ;
sur une pull request, la GitHub Action peut commenter avec le résultat. Voir
[Seulement ce qui a changé](workflows.md#seulement-ce-qui-a-changé--exécutions-qui-connaissent-git).

## Collections chiffrées

Une collection chiffrée garde chaque requête, chaque environnement et même
chaque nom de fichier scellés sur le disque (XChaCha20-Poly1305 avec une clé de
mot de passe Argon2id). Git voit seulement quel blob chiffré a changé.

- **Fichier › Chiffrer cette collection…** ou clic droit sur une collection ›
  **Chiffrer…** chiffre sur place une collection existante.
- **Fichier › Nouvelle collection chiffrée…** en crée une dans un projet.
- Vous recevez une seule fois une **clé de récupération**. Gardez-la en lieu
  sûr : elle ouvre la collection si le mot de passe est perdu.
- Ouvrir la collection demande le mot de passe. Elle se verrouille de nouveau
  quand vous choisissez **Fichier › Verrouiller les collections chiffrées**,
  quand vous quittez Apix, ou quand vous la laissez inactive pendant la durée
  définie dans les Paramètres (15 minutes par défaut).
- Cochez **Se souvenir sur cet ordinateur** dans la demande de
  déverrouillage pour garder le mot de passe dans le magasin d'identifiants du
  système (le Gestionnaire d'identification sous Windows, le Trousseau sous
  macOS, le Secret Service sous Linux). La collection s'ouvre alors sans rien
  demander tant que vous êtes connecté à votre session, et la ligne de
  commande `apix` utilise aussi le mot de passe enregistré. **Fichier ›
  Oublier le mot de passe enregistré** (ou un clic droit sur la collection) le
  supprime. Si le mot de passe a été changé ailleurs, celui qui est enregistré
  est abandonné et Apix le redemande.

## Copier en et coller curl

Le bouton de copie à côté d'**Envoyer** (ou **Requête › Copier en**) copie la
requête sous forme de `curl` pour bash ou fish, de `curl.exe` pour l'invite de
commandes Windows, d'un appel PowerShell `Invoke-RestMethod`, d'une commande
HTTPie `http` (HTTPie 3.0 ou plus récent) ou d'une commande GNU `wget`, avec
les variables remplies. wget ne sait pas envoyer de formulaires multipart ; la
commande copiée le signale. Les requêtes signées à chaque envoi (OAuth 1.0, AWS
pour PowerShell et le code) sont signées au moment de la copie.

Le même menu copie la requête sous forme de programme : JavaScript `fetch`,
Node.js axios, Python requests, Go `net/http`, Rust reqwest, C# `HttpClient`,
Java `HttpClient`, PHP cURL, Swift `URLSession` ou Kotlin OkHttp. Chacun envoie
ce qu'Apix enverrait — URL, en-têtes, authentification et tous les types de
corps — ainsi que les réglages de requête que le langage sait exprimer (délai,
redirections, vérification TLS, proxy, certificat client). Un réglage pour
lequel un langage n'a pas d'option simple est listé dans un commentaire
`Not in this snippet` en tête au lieu d'être abandonné en silence. Dans la
barre latérale, faites un clic droit sur une requête et utilisez **Copier en
code**.

La CLI écrit la même chose : `apix code request.apix --as python`
(`apix shell` est la même commande ; `--as` accepte `bash`, `cmd`,
`powershell`, `fetch`, `axios`, `python`, `go`, `rust`, `csharp`, `java`,
`php`, `swift` ou `kotlin`).

Collez une commande `curl` (par exemple issue de « Copy as cURL » de votre
navigateur) dans le champ d'URL et Apix remplit toute la requête : méthode,
URL, paramètres de requête, en-têtes, authentification et corps.

## Importer depuis Postman

**Fichier › Importer…**, ou déposez un export Postman sur la fenêtre. Les
collections (v2.0 et v2.1) et les environnements sont pris en charge. Les
dossiers, les requêtes, les corps, l'authentification et les variables sont
repris ; tout ce qu'Apix ne sait pas encore reprendre est listé dans un
rapport. Les scripts d'avant-requête et de test sont gardés sous forme de
fichiers `.js` à côté de leurs requêtes, et les secrets des environnements vont
dans le `.local.toml` ignoré par git.

**Plusieurs fichiers à la fois.** Ouvrez ou déposez un dossier qui contient des
exports Postman — ou le zip que produit *Export data* de Postman — et Apix
reconnaît les fichiers par leur contenu, pas par leur nom, et propose de tout
importer en un seul projet : une collection par collection Postman, les
environnements partagés par toutes, et les globales comme variables du projet
(les globales secrètes sont omises, puisque les variables du projet sont
commitées). Deux environnements de même nom sont gardés sous les noms `name` et
`name-2`.

**Exporter vers Postman.** Faites un clic droit sur une collection et
choisissez **Exporter vers Postman…** (ou **Fichier › Exporter vers
Postman…**) pour écrire un fichier de collection Postman v2.1 et un fichier
d'environnement par environnement, afin que les coéquipiers sous Postman
puissent continuer à travailler avec. Les dossiers, l'authentification, les
corps, les scripts, les variables et les réglages de redirection, de TLS et de
cookies sont repris. Postman n'a pas de délai, de proxy ni de certificat client
par requête, et les valeurs secrètes des environnements ne sont exportées
qu'avec leur nom ; la notification liste ce qui a été omis.

En ligne de commande : `apix import <file, folder or zip>` et
`apix export <collection> --out <folder>`.

## Importer depuis Insomnia, OpenAPI et HAR

**Fichier › Importer…** (ou déposer le fichier sur la fenêtre) lit aussi ces
formats ; le format est reconnu par le contenu, et un dossier ou un zip peut
les mélanger :

| Format | Ce que vous obtenez |
|---|---|
| Insomnia v4 (export JSON) | Une collection par espace de travail, des dossiers avec leur authentification, l'environnement de base comme variables de la collection et chaque sous-environnement comme environnement. `{{ _.name }}` devient `{{name}}`. |
| OpenAPI 3 / Swagger 2 (JSON ou YAML) | Une requête par opération, un dossier par tag. Le serveur devient la variable `baseUrl` (plusieurs serveurs : un environnement chacun), les paramètres de chemin deviennent des variables avec leurs valeurs d'exemple, les paramètres de requête et d'en-tête obligatoires sont activés, les corps viennent des exemples ou sont construits à partir du schéma, et les schémas de sécurité deviennent une authentification avec des variables d'identifiants vides (`bearerToken`, `apiKey`, …) à remplir dans un environnement. |
| WSDL 1.1 | Une requête par opération avec une enveloppe d'exemple ; voir [SOAP](#soap). |
| HAR (« Save all as HAR » des DevTools du navigateur) | Les appels d'API seulement, un dossier par hôte. Les scripts, les styles, les images et les polices, les URL `data:` et les répétitions sont omis, de même que les en-têtes `Cookie` : ils portent la session du navigateur. |

Une collection issue d'un seul fichier est importée comme collection ;
plusieurs deviennent un projet. Le rapport liste tout ce qui n'a pas pu être
repris.

## Historique

Chaque requête que vous envoyez est gardée avec sa réponse. **Affichage ›
Historique…** (Ctrl+H) les liste, de la plus récente à la plus ancienne, avec
la méthode, le nom, l'URL, le statut et le moment de l'envoi ; saisissez du
texte pour chercher selon n'importe lequel de ces éléments. Choisissez une
entrée et la requête revient dans l'onglet d'où elle a été envoyée, sous forme
de modifications non enregistrées, avec la réponse qu'elle a obtenue. Appuyez
sur Ctrl+Enter pour la renvoyer, sur Ctrl+S pour garder cette version. Si le
fichier de la requête n'existe plus, l'entrée s'ouvre dans l'éditeur sans
onglet.

L'historique reste sur cet ordinateur (le dossier `history` à côté des
paramètres), jamais dans une collection. Les requêtes sont stockées telles
qu'elles sont écrites, avec les `{{variables}}` non résolues ; les secrets des
environnements n'y sont donc pas copiés. Les 300 dernières requêtes sont
gardées, avec des corps de réponse de 256 Ko au plus chacun. Les requêtes des
collections chiffrées ne sont jamais écrites dans l'historique.
**Effacer l’historique** dans la fenêtre le vide ; **Fichier › Paramètres… ›
Historique** le désactive.

## Comparer des réponses

**⇄** à côté de la réponse (ou **Requête › Comparer les réponses…**) place la
réponse que vous avez maintenant à côté d'une autre réponse de la même
requête :

- **Réponses antérieures** : les réponses de la requête issues de
  l'historique, de la plus récente à la plus ancienne. Choisissez-en une pour
  voir ce qui a changé depuis ; c'est ainsi que vous remontez le temps
  jusqu'à la réponse d'hier.
- **Autres environnements** : la requête renvoyée dans un autre environnement
  (staging contre production, un mock contre la vraie API).

L'affichage indique combien de choses diffèrent, puis le statut, les en-têtes
par nom et le corps : le JSON champ par champ (les enregistrements des listes
sont appariés par leur `id`, `_id`, `uuid` ou `key`, si bien qu'une liste dans
un autre ordre n'est pas un changement, et `[id=1].email` désigne le champ),
les autres textes ligne par ligne. Les en-têtes qui changent à chaque réponse
(Date, identifiants de requête, mise en cache, façon dont le corps a été
envoyé) sont omis et listés à la fin. **Copier en texte** copie le résultat.

![Comparaison avec une réponse antérieure](../en/images/compare.png)

Dans un terminal, `apix compare <request> --env staging --against production`
envoie la requête dans les deux et affiche les différences ; il se termine avec
1 quand elles diffèrent, si bien qu'un pipeline peut vérifier que deux
environnements répondent pareil. L'historique garde les corps jusqu'à 256 Ko ;
les corps plus longs sont donc comparés jusqu'à la partie gardée.

## Assistant IA

L'assistant IA est désactivé jusqu'à ce que vous l'activiez dans **Fichier ›
Paramètres… › Assistant IA**. Apix est open source et n'a pas de compte IA à
lui : vous choisissez un fournisseur et donnez votre propre clé d'API.

- **Fournisseur d’IA** : Claude (Anthropic), ChatGPT (OpenAI) ou Gemini
  (Google).
- **Modèle** : celui par défaut du fournisseur (Claude Opus 5.5, `gpt-5.5`,
  `gemini-2.5-flash`), ou **↻** charge les modèles que votre clé peut utiliser
  et vous en choisissez un.
- **Clé d’API** : pour le fournisseur ci-dessus, gardée dans le trousseau du
  système, jamais dans un fichier ; enregistrer un champ vide la supprime. Sans
  clé, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` ou `GEMINI_API_KEY` est utilisée.

La réponse reçoit alors un bouton de plus :

- **Tests IA** sur une réponse réussie : l'IA écrit un script Tests qui vérifie
  le statut, le type de contenu et la forme du corps, et Apix l'ajoute aux
  Tests de la requête (marqué comme écrit par elle, non enregistré), si bien
  que vous le lisez avant de le garder.
- **Expliquer** sur une réponse en échec (400 et plus) : ce que signifie la
  réponse, les causes probables et ce qu'il faut vérifier, dans la langue de
  l'application.

![Explication d'une réponse en échec](../en/images/ai-explain.png)

Ce qui est envoyé au fournisseur : la méthode, l'URL, les en-têtes et le corps
de la requête, et le statut, les en-têtes et le corps de la réponse (coupé à
12 Ko). Authorization, les cookies et les en-têtes dont le nom contient key,
token, secret ou signature sont masqués en •••. L'IA peut se tromper ; rien
n'est modifié ni envoyé ailleurs sans vous. La capture d'écran a été faite
avec un serveur de test local, pas une vraie réponse.

## Extensions

Les extensions ajoutent ce qu'Apix n'a pas en standard : un type
d'authentification, un importateur pour un autre format de fichier, ou une vue
d'une réponse. Une extension est un fichier WebAssembly (`.wasm`), et elle
tourne isolée : elle reçoit du JSON et répond en JSON, sans réseau, sans
fichiers et sans horloge, et elle est arrêtée quand elle tourne trop longtemps
ou utilise trop de mémoire. Une extension écrite par quelqu'un d'autre peut
modifier la requête qu'elle signe ou la collection qu'elle importe, et rien
d'autre.

**Affichage › Extensions…** liste ce qui est installé et ce que propose chaque
extension. **Installer depuis un fichier…** vérifie un fichier `.wasm` et le
copie dans le dossier des extensions (`<config>/Apix/plugins`), **Retirer** le
supprime, **Ouvrir le dossier** montre le dossier et **Recharger** le relit
après que vous y avez copié des fichiers vous-même.

![La fenêtre Extensions](../en/images/plugins.png)

- Les **types d'authentification** apparaissent dans la liste de l'onglet Auth
  sous la forme « … (extension) », avec les champs que l'extension demande
  (les `{{variables}}` y fonctionnent). À l'envoi de la requête, l'extension
  reçoit la requête finale (méthode, URL, en-têtes, corps) et ajoute des
  en-têtes ou des paramètres de requête. Une requête dont l'extension n'est
  pas installée le signale et n'est pas envoyée.

  ![Une requête signée par une extension](../en/images/plugin-auth.png)

- Les **vues** apparaissent à côté de Texte, Tableau et Graphique au-dessus
  d'une réponse JSON ; l'extension lit le statut, les en-têtes et le corps et
  répond par un tableau ou du texte.

  ![Une vue fournie par une extension](../en/images/plugin-view.png)

- Les **importateurs** ont un bouton **Importer un fichier…** dans la fenêtre
  Extensions : le fichier devient une nouvelle collection à côté, qui s'ouvre
  ensuite.

Dans un fichier de requête, une authentification par extension s'écrit :

```toml
[auth.plugin]
plugin = "hmac-signature"

[auth.plugin.config]
key_id = "k1"
secret = "{{signing_secret}}"
```

Depuis un terminal :

```sh
apix plugins list                     # extensions installées et ce qu'elles proposent
apix plugins install my-plugin.wasm
apix plugins remove my-plugin
apix import endpoints.csv --plugin csv-endpoints --out shop
apix send posts/list-posts.apix --view json-stats
```

Le dossier `plugins/` du dépôt contient trois exemples, construits à partir de
`crates/apix-plugin-sdk/examples` : **hmac-signature** (signe les requêtes avec
HMAC-SHA256 sur la méthode, le chemin et le hachage du corps),
**csv-endpoints** (une liste CSV de `method,url,name,folder` devient des
requêtes) et **json-stats** (statistiques des champs d'une liste JSON : types,
présence, valeurs distinctes, plages).

### Écrire une extension

En Rust, ajoutez la dépendance `apix-plugin-sdk`, implémentez son trait
`Plugin` et appelez `export!` :

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

Compilez la crate en `cdylib` avec `cargo build --release --target
wasm32-unknown-unknown` et installez le `.wasm`. Le `name` du manifeste
(lettres minuscules, chiffres et `-`) est le nom par lequel les requêtes
désignent l'extension ; elle propose n'importe lequel de ces éléments :

| Clé du manifeste | Le hook reçoit | Il répond |
|---|---|---|
| `auth: {label, fields: [{name, label, secret}]}` | `{method, url, headers, body, config}` | `{headers: [[name, value]], query: [[name, value]]}` |
| `import: {label, extensions}` | `{file_name, content}` | `{name, requests: [{folder, name, method, url, headers, body}]}` |
| `visualize: {label}` | `{status, headers, body}` | `{title, text}` ou `{title, columns, rows}` |

Tout langage qui compile vers WebAssembly fonctionne. Le module exporte
`memory`, `apix_alloc(len) -> ptr`, `apix_manifest() -> i64` et
`apix_call(hook_ptr, hook_len, input_ptr, input_len) -> i64`, où le `i64` vaut
`ptr << 32 | len` d'une réponse JSON en UTF-8, `{"ok": …}` ou
`{"error": "…"}`. Il ne peut rien importer, et chaque appel tourne dans une
nouvelle instance.

## Palette de commandes

Ctrl+K (ou Ctrl+P) ouvre la palette de commandes : saisissez quelques lettres
d'un nom de requête ou d'une commande et appuyez sur Enter. Les commandes
peuvent être trouvées par leur nom anglais dans toutes les langues.

![La palette de commandes](../en/images/palette.png)

## Configuration

Au premier démarrage, Apix ouvre la **Configuration**, les extras facultatifs
réunis en un seul endroit ; **Aide › Configuration…** la rouvre. Apix
fonctionne sans aucun d'eux.

- **Base de géolocalisation IP** : situe chaque routeur sur la carte de la
  route et dans `apix trace`. **Télécharger** récupère la base gratuite de
  villes de DB-IP (environ 130 Mo, CC BY 4.0) dans `<config>/Apix/geo` ;
  **Mettre à jour** récupère le mois le plus récent. Rien n'est consulté en
  ligne. Dans un terminal : `apix geo download`, `apix geo status`,
  `apix geo remove`.
- **Assistant IA** : indique s'il est activé et avec quel fournisseur ;
  **Ouvrir les paramètres** mène à ses lignes.
- **Claude (MCP)** : **Connecter à Claude** (aussi **Aide › Connecter à Claude
  (MCP)…**) pour la collection ouverte, voir ci-dessous.

![Configuration](../en/images/setup.png)

### Connecter Claude

**Connecter à Claude** fait de la collection ouverte un serveur MCP pour
Claude, si bien que Claude peut lister, envoyer et exécuter ses requêtes avec
leurs tests sans voir les valeurs des variables (voir
[MCP](workflows.md#laisser-un-agent-ia-utiliser-votre-collection-mcp)).

- **Ajouter à Claude Desktop** écrit l'entrée `apix-<collection>` dans le
  `claude_desktop_config.json` de Claude Desktop. Ses autres serveurs et
  réglages sont conservés, et le fichier d'origine est copié en `.bak` la
  première fois ; redémarrez Claude Desktop pour l'utiliser. **Retirer** enlève
  de nouveau l'entrée.
- **Ajouter à Claude Code** lance `claude mcp add --scope user …`, si bien que
  la collection est disponible dans tous les projets.
- **Liste seulement** ne donne à Claude que les outils de listage
  (`--list-only`).
- L'entrée et la commande sont toutes deux affichées avec **Copier**, pour les
  ajouter à la main.

![Connecter à Claude](../en/images/claude-connect.png)

La même chose depuis un terminal : `apix mcp my-api --add-to claude-desktop`,
`apix mcp my-api --add-to claude-code`, `apix mcp my-api
--remove-from-claude-desktop`.

## Paramètres et langue

**Fichier › Paramètres…** (Ctrl+,) :

- **Langue :** English, Türkçe, العربية, Deutsch, Français, Русский, 日本語,
  简体中文 ou Español. Le changement s'applique immédiatement. Au premier
  démarrage, Apix suit la langue du système. En arabe, la fenêtre est en
  miroir : la barre latérale est à droite et les menus, les onglets et les
  formulaires vont de droite à gauche, tandis que les URL, le code et les
  tableaux clé–valeur restent de gauche à droite.
- **Verrouiller les collections chiffrées :** après 5, 15 ou 30 minutes,
  1 heure, ou jamais.
- **Valeurs secrètes des environnements :** dans un fichier `.local.toml` ou
  dans le trousseau du système (voir
  [Variables et environnements](#variables-et-environnements)).
- **Confirmer en production :** demander confirmation avant DELETE, PUT ou
  PATCH tant qu'un environnement « prod » est actif (activé par défaut).
- **Rechercher des mises à jour :** au démarrage, demander à GitHub si une
  version plus récente d'Apix est sortie (activé par défaut). Seule cette
  requête est envoyée, sans rien qui vous identifie, et rien n'est
  téléchargé : une version plus récente affiche une notification avec un
  bouton **Télécharger** qui ouvre sa page. **Aide › Rechercher des mises à
  jour…** pose la question à tout moment, et `apix update` fait de même dans
  un terminal.

![Une nouvelle version annoncée](../en/images/update.png)

- **Historique :** activé ou désactivé (voir [Historique](#historique)).

La partie **Thème** des Paramètres change l'apparence d'Apix, immédiatement,
sans redémarrage :

- **Apparence :** sombre, clair, ou **Suivre le système**, qui bascule quand
  Windows ou macOS passe du mode clair au mode sombre et inversement.
- **Thème sombre :** Obsidian (par défaut), Cyber glass, Dracula, Nord ou
  Solarized Dark. **Thème clair :** Daylight ou Solarized Light. Choisir un
  thème de l'autre sorte fait passer l'apparence à celle-ci.
- **Couleur d'accent :** celle du thème, ou cyan, bleu, violet, rose, rouge,
  orange, jaune ou vert, pour les boutons, les sélections, les contours de
  focus et les pastilles de variables. Les thèmes clairs reçoivent une nuance
  plus profonde, et le texte sur l'accent est sombre ou blanc, selon ce qui se
  lit le mieux.
- **Transparence :** un curseur de 0 (une fenêtre opaque) à 90 % : la part du
  bureau visible au travers, floutée comme du verre dépoli (acrylic sous
  Windows, vibrancy sous macOS). Le fond de la fenêtre s'éclaircit avec elle,
  les panneaux plus lentement pour que le texte garde un fond, et les bordures
  s'éclairent comme le bord d'un verre ; les menus et les fenêtres de dialogue
  restent opaques. Les thèmes clairs s'éclaircissent plus doucement, car un
  texte sombre a besoin d'un fond clair. La valeur par défaut est 20 %.
- **Taille du texte :** 90 %, 100 %, 110 %, 125 % ou 150 % de la taille
  normale, pour tout le texte de la fenêtre, immédiatement.
- **Taille de la police du code :** de 11 à 18 px pour les éditeurs de code
  (corps, scripts, vue Formaté de la réponse) ; la visionneuse des très gros
  corps garde 13 px.
- **Densité :** lignes compactes, normales ou aérées dans la barre latérale.
- Les éditeurs de code prennent leurs couleurs dans le thème : les chaînes,
  les nombres et les mots-clés comme le thème les donne, les types et les
  fonctions dans la couleur d'accent, les commentaires et la ponctuation
  atténués.

![Taille du texte, taille de la police du code et densité](../en/images/look.png)

- **Vos thèmes :** **Ouvrir le dossier** ouvre `<config>/Apix/themes` et, la
  première fois, y écrit `my-theme.toml` comme point de départ. Chaque fichier
  `.toml` de ce dossier est un thème dans les listes ; un fichier nommé comme
  un thème intégré (`nord.toml`) le remplace. Un fichier illisible est affiché
  en rouge avec la raison.

![Paramètres](../en/images/settings.png)

Un fichier de thème définit un nom, s'il est clair, et n'importe laquelle de
ces couleurs (`#RRGGBB`, ou `#RRGGBBAA` avec opacité) ; celles qu'il omet
viennent d'Obsidian, ou de Daylight pour un thème clair :

```toml
name = "Deep sea"
light = false

[colors]
background = "#03141FEE"      # la fenêtre, un dégradé vers background_end
background_end = "#062A3AEE"
surface = "#05202DB3"         # panneaux
raised = "#0A2B3BF7"          # menus et fenêtres de dialogue
border = "#FFFFFF14"
accent = "#FF9F43"
text = "#E8F6FF"
muted = "#7FA3B8"
ok = "#34D399"                # 2xx, tests réussis
warning = "#FBBF24"
error = "#F87171"
string = "#86EFAC"            # JSON dans la visionneuse des gros corps
number = "#FDBA74"
keyword = "#C4B5FD"
```

Les fichiers de thème sont du texte brut ; une équipe peut donc les partager.
Apix les lit au démarrage et à l'ouverture des Paramètres.

Les paramètres sont enregistrés dans `settings.toml`, dans le dossier Apix du
répertoire de configuration de votre système (`%APPDATA%\Apix` sous Windows,
`~/Library/Application Support/Apix` sous macOS, `~/.config/Apix` sous Linux).

## Raccourcis clavier

| Touches | Action |
|---|---|
| Ctrl+Enter | Envoyer |
| Esc | Annuler la requête |
| Ctrl+S | Enregistrer |
| Ctrl+N | Nouvelle requête GET |
| Ctrl+O | Ouvrir un dossier, un projet ou une collection |
| Ctrl+K ou Ctrl+P | Palette de commandes |
| Ctrl+E | Variables et environnements |
| Ctrl+, | Paramètres |
| Ctrl+H | Historique |
| Ctrl+Shift+R | Runner |
| Alt+1…7 | Changer de méthode |
| Ctrl+W | Fermer l'onglet |
| Ctrl+Tab / Ctrl+Shift+Tab | Onglet suivant / précédent |
| Ctrl+Space | Suggestions dans la requête GraphQL (après avoir récupéré le schéma) |

Sous macOS, utilisez ⌘ au lieu de Ctrl (Ctrl+Tab et Ctrl+Space restent Ctrl).
