<!-- translated from en/architecture.md sha256:75b4d247b3b61d0d -->

# Arquitectura de Apix

Apix es un cliente de API nativo escrito en Rust. La interfaz se dibuja en la
GPU con [GPUI](https://github.com/zed-industries/zed) (el motor de interfaz de
Zed) y la biblioteca de componentes
[gpui-kit](https://github.com/longbridge/gpui-kit). No tiene WebView ni
Electron.

## Principios

1. **Local primero, amigable con git.** Una colección es una carpeta de
   archivos TOML pequeños. No hace falta cuenta ni nube. Las colecciones se
   suben con commit, se comparan y se revisan como el código.
2. **La interfaz nunca se bloquea.** La red, el disco, el análisis y los
   scripts se ejecutan fuera del hilo de la interfaz. El hilo de la interfaz
   solo distribuye y pinta.
3. **Un motor, muchas interfaces.** La aplicación de escritorio, la CLI `apix`
   y las interfaces futuras (una TUI, un servidor MCP) comparten los mismos
   crates, así que una solicitud se comporta igual en todas partes, también en
   CI.
4. **Escalar a respuestas enormes.** Una respuesta JSON de 100 MB tiene que
   desplazarse con fluidez. Solo se distribuyen las filas visibles (listas
   virtuales) y el cuerpo se analiza de forma perezosa.
5. **Ligera y rápida al arrancar.** La velocidad y las funciones van antes que
   el tamaño. El binario de versión se mantiene por debajo de 100 MB (54.4 MB
   hoy; las aplicaciones Electron ocupan 300 MB o más) y abre una ventana en
   menos de 150 ms.

## Crates

```
crates/
├── apix-core     modelo de solicitud, valores {{variables}} y {{$dynamic}}, formato en disco,
│                 proyectos, entornos y secretos, operaciones de árbol y de archivos,
│                 esquemas GraphQL (introspección, SDL) y autocompletado de consultas,
│                 git: rama, archivos cambiados, a qué afecta un cambio, diferencias a nivel de solicitud
├── apix-http     motor HTTP: reqwest + rustls, tiempos, captura de respuestas, traza de red,
│                 server-sent events, WebSocket (tokio-tungstenite), Socket.IO, descarga del esquema GraphQL
│                 (DNS, establecimiento de la conexión, redirecciones, certificado, interfaz local)
├── apix-import   Postman, Insomnia, OpenAPI, HAR y WSDL → colecciones de Apix
├── apix-codegen  solicitudes ↔ comandos de shell: curl (bash), curl.exe (cmd), PowerShell; analiza curl pegado; código en 10 lenguajes
├── apix-vault    colecciones cifradas: XChaCha20-Poly1305 por archivo, Argon2id, clave de recuperación
├── apix-script   scripts previos a la solicitud y de prueba: QuickJS con una API compatible con pm.*
├── apix-runner   ejecuta colecciones: orden, iteraciones (también en paralelo), archivos de datos, espera, bail,
│                 informes (también Markdown para pull requests), monitores: programación, registro, webhooks
├── apix-grpc     llamadas gRPC: tonic, mensajes como JSON (prost-reflect), archivos .proto (protox), definiciones a partir de código o JSON
│                 o reflection del servidor
├── apix-proto    otros protocolos: MQTT (rumqttc)
├── apix-mock     un servidor simulado local que responde con los ejemplos guardados (hyper)
├── apix-capture  un proxy de grabación: HTTP simple, túneles CONNECT, HTTPS abierto con una CA local (rcgen, rustls)
├── apix-route    la ruta hasta un servidor (el traceroute del sistema) y dónde están sus saltos (una base de datos .mmdb sin conexión)
├── apix-plugins  complementos WebAssembly (wasmi): tipos de autenticación, importadores, vistas de respuesta; apix-plugin-sdk los escribe en Rust
├── apix-cli      binario `apix`: send / run / diff / schema / import / shell desde la terminal y CI
└── apix-desktop  aplicación de escritorio GPUI (Windows, macOS y Linux desde un solo crate)
tools/
└── icongen       genera todos los iconos de la aplicación (ico, icns, PNG de hicolor) a partir de Logo/LogoFirst.jpg
```

Dentro de `apix-desktop/src`: `workspace.rs` (editor de solicitudes, panel de
respuesta, acciones y atajos de teclado), `workspace/collections.rs` (barra
lateral, proyectos, operaciones de archivos, importación, vigilancia de
archivos), `workspace/env_editor.rs` (el diálogo de variables),
`workspace/auth_editor.rs` (la pestaña Autenticación y el diálogo de
autenticación de carpeta/colección/proyecto), `workspace/palette.rs` (la
paleta de comandos y su buscador difuso), `workspace/network.rs` (la pestaña
Red de la respuesta), `workspace/runner_ui.rs` (el Runner),
`workspace/vault_ui.rs` (desbloquear, bloquear y bloqueo automático, cifrar,
descifrar, cambiar la contraseña, clave de recuperación), `assets.rs` (iconos
de gpui-kit más los dos iconos de candado), `workspace/menu_bar.rs`
(Archivo/Solicitud/Entorno/Ver/Ayuda en la barra de título; los botones
llaman a `occlude()` sobre la zona de arrastre de la barra de título para que
Windows les entregue el clic), `workspace/tabs.rs` (solicitudes abiertas; la
pestaña del frente vive en los widgets del editor, las demás conservan su
borrador, su respuesta y su tarea en curso), `workspace/drag.rs` (arrastrar
solicitudes y carpetas para moverlas), `workspace/shell_copy.rs` (copiar como /
pegar curl), `workspace/var_chips.rs` (chips de `{{variable}}` en la barra de
URL y su tarjeta al pasar el ratón), `workspace/graphql_ui.rs` (descarga,
caché, autocompletado y tarjeta del esquema GraphQL),
`workspace/grpc_editor.rs` y `workspace/grpc_ui.rs` (la pestaña Mensaje de una
solicitud gRPC, y la llamada en el panel de respuesta), `large_view.rs` (visor
por filas para cuerpos enormes), `net.rs` (puente con tokio), `bidi.rs`
(utilidades de diseño de derecha a izquierda, ver Idiomas), `state.rs`
(elementos recientes y entorno por carpeta en la carpeta de configuración del
usuario), `theme.rs` y `fonts.rs`.

### Un crate de escritorio para tres sistemas operativos

GPUI y gpui-kit funcionan en Windows, macOS y Linux con el mismo código, que es
como Zed se distribuye. Por eso no hay carpetas de interfaz por sistema. El
código que de verdad difiere por sistema va detrás de `cfg` en
`crates/apix-desktop/src/platform/{windows,macos,linux}.rs`: barra de título y
controles de ventana, desenfoque/vibrancy, la barra de menús de macOS,
asociaciones de archivos para `.apix` y acceso al llavero. Los instaladores y
los iconos están en `packaging/{windows,macos,linux}/`. Solo se separaría una
interfaz aparte si algún sistema necesitara alguna vez una interfaz realmente
distinta.

Dirección de las dependencias: `core` ← `http`/`script`/`proto` ←
`cli`/`desktop`. Nada depende de `desktop`.

## Flujo de datos de una solicitud

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
              script previo a la solicitud (luego) ◀────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (runtime de tokio, hilo en segundo plano)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                       script de prueba (luego) ◀───────┤
                                                        ▼
            Interfaz: línea de estado, visor de cuerpo virtualizado
```

Las `{{variables}}` se resuelven por capas. Las capas posteriores tienen
prioridad:
**globales → colección (`apix.toml`) → entorno → sobrescrituras (CLI `--env-file`, `APIX_VAR_*` y `--var`, comparadas con los nombres de la colección sin importar mayúsculas y minúsculas; script `pm.variables.set`)**.
Los nombres no definidos se dejan tal cual y se informan, para que la interfaz
pueda subrayarlos y la CLI pueda negarse a enviar.

### La traza de red

Cada envío construye su propio cliente reqwest de vida corta, así que todo lo
que registra pertenece a esa solicitud (`apix_http::trace`). Un resolver mide
el DNS, una capa de Tower alrededor del conector de reqwest mide cada conexión
y la política de redirecciones anota cada salto. reqwest no entrega su
conexión TLS ni deja que un conector propio devuelva una, así que los detalles
de TLS vienen del interior de rustls, mediante una configuración de cliente que
construye Apix (`use_preconfigured_tls`, `tls::traced_config`, ganchos en
`tls_trace.rs`):

- un verificador de certificados envolvente registra la cadena que envió el
  servidor y qué firma de versión comprueba (TLS 1.2 o 1.3), y luego deja
  decidir al verificador habitual (o a "aceptar cualquiera" con las
  comprobaciones desactivadas);
- a un almacén de sesiones que no guarda nada se le pide una pista de
  intercambio de claves mientras se escribe el client hello, lo que marca el
  inicio del handshake TLS: el tiempo de la conexión antes de ese punto
  (después del DNS) es TCP, y después, TLS; además se le comunica el grupo que
  eligió el servidor (TLS 1.3);
- los conjuntos de cifrado son los propios de rustls, creados una vez con un
  envoltorio fino alrededor de su cifrado que anota qué conjunto se configuró.
  Los conjuntos son `'static` y compartidos, así que el envoltorio encuentra la
  solicitud mediante un conjunto thread-local mientras se sondea el future de
  la solicitud (`Scoped`); el handshake se ejecuta dentro de ese sondeo.

ALPN se deduce de la versión de la respuesta (HTTP/2 sobre TLS solo después de
acordar `h2`). Los cuerpos comprimidos los descomprime Apix (`decode.rs`, gzip,
zlib o deflate sin envoltorio, brotli, a medida que llegan) en lugar de
reqwest, que descartaría `Content-Encoding` y el tamaño transferido.

## Formato en disco

```
my-api/
├── apix.toml               nombre + [vars] y [auth] a nivel de colección
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         opcional: [auth] para las solicitudes de esta carpeta
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Herencia de la autenticación.** Una solicitud sin `auth` la hereda de la
carpeta más cercana cuyo `folder.toml` defina una, después de la colección
(`apix.toml`) y después del proyecto (`apix-project.toml`); si no hay ninguna,
no se envía nada. `auth = "none"` en una solicitud (o carpeta) detiene la
herencia. `apix_core::project::inherited_auth` hace la búsqueda por igual para
la aplicación de escritorio, la CLI y "Copiar como".

**Ajustes de la solicitud.** Una tabla `[settings]` (tiempo de espera,
redirecciones, verificación TLS, proxy, certificado de cliente, CA de
confianza, cookies) existe en los mismos cuatro niveles. Cada campo es un
`Option`: `None` hereda campo a campo por la misma cadena
(`apix_core::project::inherited_settings`), y lo que sigue sin definir recurre
a los valores predeterminados de `RequestSettings::effective`: sin tiempo de
espera, seguir hasta 10 redirecciones, verificar TLS, proxy del entorno,
cookies activadas. El motor aplica el resultado cuando construye el
`reqwest::Client` de cada envío (un archivo de CA de confianza añade sus
certificados como raíces); el almacén de cookies compartido (un
`cookie_store` que el gestor de cookies puede listar y editar) se entrega a
ese cliente salvo que la solicitud desactive las cookies.

**Autenticación firmada.** AWS Signature V4, OAuth 1.0a y Digest son
funciones puras en `apix_core::sign`, comprobadas con el conjunto de pruebas
de AWS, RFC 7616, RFC 7636 y el ejemplo clásico de OAuth 1.0, así que el motor
y "Copiar como" firman de la misma forma. El motor construye primero el
`reqwest::Request` y después lo firma (el hash del payload de AWS necesita el
cuerpo final; un stream multipart se envía como `UNSIGNED-PAYLOAD`). Digest
envía una vez, lee el desafío del 401 y lo responde en la misma conexión.
OAuth 2.0 vive en `apix_http::oauth2`: el token se obtiene antes de enviar
(credenciales de cliente, contraseña o código de autorización con PKCE, donde
el navegador vuelve a un listener de un solo uso en 127.0.0.1), se guarda en
memoria por servidor, cliente, concesión, ámbito y usuario, se renueva con su
refresh token y nunca se escribe en el disco.

Un archivo de solicitud:

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # sin query: eso va en [[query]]

[[query]]
key = "notify"
value = "yes"

[[path]]                 # el valor del segmento :org
key = "org"
value = "{{org}}"

[[example]]              # una respuesta guardada con la solicitud
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # desde qué se envió, tal como se escribió
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # se guarda en el archivo, no se envía

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # una comprobación sin código, ejecutada después de los scripts de prueba
target = "status"
op = "=="
value = "201"
```

**Comprobaciones.** Las filas `[[assert]]` comparan un destino (`status`,
`time`, `size`, `body`, `body.<path>`, `header.<name>`) con un valor usando
`==`, `!=`, `>`, `>=`, `<`, `<=`, `contains`, `not contains`, `exists`,
`not exists`, `matches` o `is` (un tipo JSON).
`apix_script::assertions_script` las convierte en un script de prueba más, así
que informan como `pm.test` en todas partes.

**Cuerpos.** `[body]` contiene uno de `json`, `text`, `xml`, `form` (filas
codificadas como URL), `multipart` (filas con `file = true` para los campos de
archivo y un `content_type` opcional), `binary = { file = "…" }` o `graphql`
(`query` y `variables`). Las rutas de archivo pueden contener variables y son
relativas al archivo de la solicitud; `Request::with_files_relative_to` las
hace absolutas después de resolver las variables, por igual en la aplicación y
en la CLI. El motor solo lee los archivos al enviar, así que un archivo que
falta es un error de envío, no de carga. Un cuerpo GraphQL se envía como JSON
`{"query", "variables"}`; unas variables no válidas detienen el envío. Con un
cuerpo multipart, se descarta un encabezado `Content-Type: multipart/…` sin el
boundary del motor (habitual en las exportaciones de Postman) para que el
cuerpo siga siendo legible.

**gRPC.** Una solicitud cuya URL es `grpc://host:port` (o `grpcs://` para TLS)
nombra su método en `[grpc]`; el cuerpo JSON es el mensaje, y una lista JSON es
un stream de mensajes para un método que lo acepta. Sin `protos` se pregunta al
servidor (reflection). Las rutas son relativas al archivo de la solicitud.

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # opcional
import_paths = ["../protos"]              # opcional
```

En lugar de archivos, `definition` guarda la descripción en la solicitud:
texto `.proto` o un descriptor set como JSON. Tiene prioridad sobre `protos`.
`guessed = true` marca una deducida a partir de tipos o de JSON: la reflection
del servidor tiene prioridad sobre ella y se informa de las diferencias.
`protos` también acepta descriptor sets, binarios o `.json`.

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

Cuestiones abiertas:
- **Secretos.** Los valores de entorno como los tokens no deben subirse con
  commit. `environments/<name>.local.toml` se superpone a `<name>.toml`.
  Escribir uno añade `*.local.toml` al `.gitignore` junto a `environments/`
  si falta. Un secreto puede dejar un marcador en el archivo compartido, para
  que los compañeros vean el nombre. De forma opcional (Configuración › Valores
  secretos de los entornos) los secretos van en su lugar al llavero del
  sistema: `<name>.local.toml` contiene entonces solo `keychain = "<id>"` y los
  valores viven en la entrada `env:<id>` del servicio `Apix`
  (`apix_vault::keychain::save_text`, repartidos en varias entradas por el
  límite de 2560 bytes de Windows). `apix_core::secrets::use_keychain` decide
  dónde se escriben los secretos nuevos; la lectura sigue al archivo, así que
  la CLI no necesita ningún ajuste. El id viaja con el archivo, así que
  renombrar el entorno o mover la carpeta conserva el vínculo. Dentro de una
  colección cifrada los secretos se quedan en la bóveda.
- **Variables dinámicas.** `{{$name}}` lo rellena `apix_core::dynamic` cuando
  no hay ninguna variable definida con ese nombre, con un valor nuevo en cada
  aparición (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …). Sin
  crates adicionales: un pequeño generador xorshift y un formateador de fechas
  UTC escrito a mano.
- **Orden en la barra lateral.** Si no hay nada guardado, primero van las
  carpetas y después las solicitudes, cada grupo por nombre. Al arrastrar se
  guarda una lista `order` de nombres de hijos en el `folder.toml` de la
  carpeta (o en el `apix.toml` de la colección); `store::place` pone un
  elemento antes o después de otro, y los cambios de nombre y los movimientos
  mantienen la lista al día. Los nombres que la lista no conoce siguen en el
  orden predeterminado, así que un archivo nuevo nunca desaparece. Las
  colecciones de un proyecto usan `order` en `apix-project.toml`
  (`Project::place_collection`). El importador de Postman solo escribe `order`
  cuando el orden de Postman difiere del predeterminado.

### Proyectos

Un proyecto agrupa colecciones y posee entornos compartidos por todas ellas:

```
shop/
├── apix-project.toml       nombre, [vars] compartidas, orden de las colecciones, group_by, pins, colores
├── environments/           test.toml, prod.toml (+ secretos *.local.toml)
├── users/                  una colección (apix.toml + archivos .apix)
└── payments/               una colección; puede estar fijada a algunos entornos
```

`[colors]` asigna a una carpeta de colección un nombre de color (`red`,
`orange`, `yellow`, `green`, `teal`, `blue`, `purple`, `pink`); la aplicación
lo dibuja como un punto en la barra lateral y en las pestañas.
`project::move_collection_into` mueve una carpeta de colección a un proyecto
(copiando y borrando entre unidades distintas).

Capas de variables, la posterior tiene prioridad: variables del proyecto →
variables de la colección → el entorno del proyecto → el entorno de la
colección con el mismo nombre (cada uno con sus secretos `.local.toml`). La
CLI (`apix run --env`) y la aplicación de escritorio usan la misma función,
`apix_core::project::variables`. `group_by = "environment"` organiza la barra
lateral como Proyecto → Entorno → Colecciones, y `[pins]` limita una colección
a algunos entornos.

### Colecciones cifradas (M4)

El formato y el motor están en `crates/apix-vault`. El resto de Apix llega a
ellos mediante `apix_core::vfs`, una capa fina con las mismas llamadas que
`std::fs`: una ruta bajo la raíz de una bóveda desbloqueada se sirve desde la
bóveda, descifrada en memoria; cualquier otra ruta va al disco. Así, el
almacén, la barra lateral, las pestañas y la CLI siguen usando rutas normales,
y nunca se escribe nada descifrado en el disco. Las bóvedas desbloqueadas
viven en un registro de todo el proceso; al bloquear (a mano, tras 15 minutos
de inactividad o al salir) se descarta la bóveda y se borra su clave. Eliminar
dentro de una colección cifrada es permanente: una papelera del sistema
guardaría archivos descifrados.

```
my-api/
├── apix-vault.toml   encabezado en texto plano: formato, coste y sales de Argon2id,
│                     la clave de datos envuelta por la contraseña y por la clave de recuperación
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   las rutas reales de todos los archivos, cifradas
    ├── 3f9c…e1.apixe un archivo cifrado cada uno; nombres aleatorios, plano (sin forma de carpetas)
    └── local/        secretos *.local.toml, mismo formato, nunca con commit
```

- Una **clave de datos** aleatoria de 256 bits cifra cada archivo con
  XChaCha20-Poly1305 y un nonce nuevo en cada escritura. El id del objeto son
  los datos asociados, así que los blobs no se pueden intercambiar ni
  renombrar sin que se note.
- La clave de datos se guarda dos veces: envuelta por una clave derivada de la
  **contraseña** (Argon2id, 64 MiB, 3 pasadas) y por una derivada de la
  **clave de recuperación** (160 bits aleatorios, mostrada una sola vez como
  `XXXX-XXXX-…` en base32 de Crockford). Cambiar la contraseña vuelve a
  envolver la clave; los archivos no se vuelven a cifrar.
- Los cambios de nombre y los movimientos solo tocan el índice cifrado.
- La clave desbloqueada vive solo en memoria (`zeroize` la borra al
  bloquear).
- Git sigue viendo qué objeto cambió, nunca lo que contiene. La seguridad
  viene del cifrado y de la contraseña, no de que el formato sea secreto; Apix
  es de código abierto.
- La contraseña se puede recordar en el almacén de credenciales del sistema
  (`apix_vault::keychain`, mediante el crate `keyring`: Administrador de
  credenciales, Llavero de macOS, Secret Service). Una entrada por bóveda bajo
  el servicio `Apix`, llamada `vault:<absolute folder>` (en minúsculas en
  Windows). Es opcional y se activa desde la petición de desbloqueo; la
  aplicación y la CLI la prueban antes de preguntar, una contraseña que ya no
  abre la bóveda se elimina y se vuelve a pedir, cambiar la contraseña la
  actualiza y descifrar la colección la elimina. Cuando no hay almacén
  disponible, Apix simplemente pregunta, como antes.

## Scripts (M5)

`apix_core::scripts` encuentra los scripts de una solicitud
(`_collection.<event>.js`, el `_folder.<event>.js` de cada carpeta y después
`<request>.<event>.js`; los eventos son `prerequest` y `test`) y guarda
`ScriptVars`, lo que los scripts definen o eliminan en cada ámbito.
`ScriptVars::apply` los superpone a las `Variables` de la solicitud, así que
la resolución funciona como antes. Nada de lo que definen los scripts se
escribe en el disco.

`apix-script` ejecuta los scripts de un evento en un runtime de QuickJS nuevo
(`rquickjs`; límites de 10 s y 256 MB). El objeto `pm` es JavaScript
(`prelude.js`): Rust pasa el contexto como JSON (la solicitud sin resolver, la
respuesta para las pruebas, los valores de ámbito de `project::scope_values`,
las `ScriptVars` anteriores, `pm.info`), los scripts se ejecutan uno a uno
envueltos en una función, y `__apix_output()` devuelve como JSON la solicitud
y las variables cambiadas, los resultados de las pruebas, las líneas de
consola, `setNextRequest` y `skipRequest`. Mantener la API en JavaScript
reduce el lado de Rust a unas pocas llamadas y facilita igualar el
comportamiento de Postman.

`pm.sendRequest` llama a una función nativa `__apix_send` con la solicitud
como JSON; quien llama a `apix_script::run` pasa un `Sender` que la envía y se
bloquea hasta que llega la respuesta (la CLI en un runtime nuevo en el hilo
del script, la aplicación en su runtime de red). Los scripts se envuelven en
funciones async, así que funcionan tanto el estilo con callback como `await`,
y el tiempo de espera no cuenta para el límite. `require` viene de
`libs.js`: subconjuntos de crypto-js (resúmenes y HMAC hechos en Rust mediante
`__apix_hash` / `__apix_hmac`), uuid, lodash, moment, ajv, tv4 y chai,
escritos para Apix en lugar de descargados.

Un envío con scripts: scripts previos a la solicitud, después
`ScriptVars::apply` y la resolución, la solicitud HTTP, y después los scripts
de prueba con un `ResponseView`. `apix_runner::exchange` hace esto para
`apix send`, `apix run` y el Runner; el envío individual de la aplicación de
escritorio lo hace en `script_run.rs`, con los scripts en un hilo en segundo
plano y los scripts propios de la solicitud tomados del editor, guardados o
no.

`apix_runner::run` recibe un `Plan` (archivos de solicitud en orden,
variables y ámbitos de script, filas de datos, iteraciones, espera, `bail`,
sobrescrituras de transporte, el estado de script del que partir) y lo
recorre: en cada iteración superpone la fila de datos (y después los valores
de `--var`) a las variables y a `pm.iterationData`, sigue `setNextRequest`
por nombre de solicitud, espera entre solicitudes e informa del `Progress`
(iteración, envío, elemento terminado) a un callback. Un `Control` lo pausa o
lo detiene desde otro hilo. Devuelve un `RunReport` de `ItemResult`s, que
`report.rs` escribe como JSON, JUnit XML (una suite por solicitud e
iteración, un caso por prueba, un caso de estado para las solicitudes sin
pruebas) y una página HTML autónoma. `data.rs` lee CSV (RFC 4180, BOM,
encabezados con `;`) y arrays JSON. La CLI imprime el progreso según llega; el
Runner de escritorio (`workspace/runner_ui.rs`) lo ejecuta en el runtime de
red y copia el progreso desde una estructura `Live` compartida unas cuantas
veces por segundo; la vista se queda en el workspace, así que una ejecución
sobrevive a su diálogo.

## Streams y WebSocket (M6)

`HttpClient::send_with` informa de `Progress::Head` (estado y encabezados) en
cuanto llegan y después de cada `Progress::Chunk` del cuerpo; `send` es
`send_with` sin nadie escuchando, y la respuesta sigue conteniendo el cuerpo
completo. `apix_http::sse` lee los cuerpos `text/event-stream` trozo a trozo
(`SseParser`: cualquier final de línea, un trozo puede cortarse en cualquier
punto, `id` se mantiene, un último evento sin terminar se descarta). El
analizador también guarda el último id y el último `retry:` para reconectar:
`reconnected()` crea un analizador para la siguiente conexión con ambos, y
`reconnect_request` añade `Last-Event-ID`. La aplicación y la CLI se encargan
ellas mismas de esperar y volver a enviar, así que Esc o Ctrl+C siempre lo
detienen.

`apix_http::ws::connect` construye el handshake a partir de la solicitud tal
como lo enviaría el lado HTTP (`prepare`: parámetros, encabezados,
autenticación; OAuth 2.0 se convierte antes en un token Bearer), añade
`Sec-WebSocket-Protocol` a partir de `[websocket] subprotocols` y conecta con
tokio-tungstenite. `wss://` recibe una configuración de rustls de
`apix_http::tls::client_config` (las raíces habituales más una CA de
confianza, un certificado de cliente o un verificador que acepta cualquier
certificado), el mismo módulo que usa gRPC sin verificación.
`WsConnection::run` envía lo que pide un canal de `WsCommand`s e informa de
`WsEvent`s (enviado, recibido, cerrado con código y motivo, error) hasta que
cualquiera de los lados cierra; tungstenite responde a los pings del
servidor. Con `[websocket] ping_interval` también envía pings, con un contador
como payload, informa del tiempo de ida y vuelta del pong correspondiente
(`WsEvent::Pong`) y termina con un error cuando un ping sigue sin respuesta
al llegar el momento del siguiente. El `MessageFormat` de un mensaje (text,
base64, hex) convierte su texto en `WsData` después de rellenar las
variables.

En la aplicación de escritorio, el runtime de red rellena estructuras
compartidas (`net::Incoming` para un envío, `net::SocketIncoming` para un
socket) y una tarea de la interfaz las lee cada 60–80 ms. Una respuesta de
flujo de eventos se convierte en `ResponseState::Streaming`
(`workspace/stream_ui.rs`) con una lista de eventos en vivo que sigue a los
eventos nuevos salvo que se haya desplazado hacia arriba; Esc convierte lo
recibido en una respuesta terminada. Una URL `ws://` cambia el panel de la
solicitud a la pestaña Mensajes (`workspace/ws_editor.rs`) y el panel de
respuesta a `ResponseState::Socket` (`workspace/socket_ui.rs`). Ambos estados
se dirigen a su pestaña como una respuesta terminada, así que un stream o un
socket sigue funcionando en una pestaña en segundo plano. La CLI imprime los
eventos y los mensajes desde los mismos callbacks.

## Socket.IO (M6)

`apix_http::socketio` habla Socket.IO 3/4 (protocolo Engine.IO 4) sobre la
conexión WebSocket descrita arriba, así que el handshake, los encabezados, la
autenticación y los ajustes de TLS son los mismos. Una solicitud es Socket.IO
cuando tiene una tabla `[socketio]` (`SocketIoOptions`: namespace, ruta,
autenticación); `handshake_request` convierte `http(s)://` en `ws(s)://`,
toma el namespace de las opciones o de la ruta de la URL, pone en su lugar la
ruta del servidor (`/socket.io/`) y añade `EIO=4&transport=websocket`.
`connect` espera el paquete open de Engine.IO, envía el CONNECT del namespace
con el JSON de autenticación y espera la respuesta (`40…` con el id del
socket, o `44…` con el rechazo) antes de volver, así que un servidor,
namespace o token incorrecto falla como un handshake fallido.

`SioConnection::run` ejecuta `WsConnection::run` junto a un traductor entre
dos canales: `SioCommand::Emit` se convierte en un paquete `EVENT` (`42…`, con
un id de ack cuando se pide, con ids contados desde 0) o en un `BINARY_EVENT`
(`45n-…`) cuyos valores `{"$binary": "<base64>"}` se reemplazan por
marcadores y se envían después como tramas binarias; los paquetes recibidos se
convierten en `SioEvent`s (eventos recibidos con el id de ack que quiere el
servidor, acks nuestros, el servidor haciéndonos salir, tramas ilegibles), con
los adjuntos binarios recogidos y devueltos a donde estaban sus marcadores.
Los pings de Engine.IO (`2`) reciben un pong (`3`).

La aplicación de escritorio mantiene una sola pestaña Mensajes para ambos:
`WsEditor` tiene un selector WebSocket/Socket.IO y los campos de Socket.IO, y
`WsMessage` lleva `event` y `ack`. `net::SocketIncoming` es genérico sobre el
tipo de evento, y `socket_ui.rs` sigue ambos tipos de conexión con un único
bucle, convirtiendo los `SioEvent`s en entradas del registro con el nombre
del evento y chips de ack. El servidor de prueba de
`apix-http/tests/common/socketio_server.rs` es también el ejemplo
`socketio_server` (:8771).

## MQTT (M6)

`apix_proto::mqtt` envuelve el bucle de eventos de rumqttc (MQTT 3.1.1; su TLS
solo mediante la configuración de `apix_http::tls::client_config`, así que no
trae proveedor criptográfico propio). Una URL `mqtt://` o `mqtts://` hace que
una solicitud sea MQTT, igual que `grpc://` para gRPC; `[mqtt]` contiene el id
de cliente, la sesión, el keep alive y las suscripciones. `connect` sondea el
bucle de eventos hasta el CONNACK del broker, así que un inicio de sesión
rechazado falla como un handshake fallido.

`MqttConnection::run` ejecuta dos futures en paralelo, sin cancelar nunca el
sondeo del bucle de eventos: uno entrega `MqttCommand`s al cliente (que puede
esperar cuando su cola está llena, y esa cola la vacía el sondeo), y el otro
sigue la conexión e informa de `MqttEvent`s. rumqttc informa del id de
paquete de cada paquete saliente en el orden en que se entregó, así que una
cola empareja publicaciones y suscripciones con sus ids, y los PUBACK,
PUBCOMP, SUBACK y UNSUBACK del broker con lo que confirman. Un mensaje se
informa como enviado cuando su paquete ha salido. Un topic incorrecto o una
suscripción rechazada es un evento y la conexión continúa; cerrar, y que el
broker cuelgue, la terminan.

En la aplicación de escritorio, `WsEditor` también contiene los campos de MQTT
(decide la URL, y `Workspace` se lo indica), y `socket_ui.rs` sigue la
conexión con el mismo bucle que WebSocket y Socket.IO, convirtiendo los
`MqttEvent`s en líneas del registro por topic. El broker de prueba de
`apix-proto/tests/common/broker.rs` (comodines, QoS 0–2, mensajes retenidos,
rechazos, TLS sobre cualquier stream) es también el ejemplo `mqtt_broker`
(:8772).

## SOAP (M6)

SOAP no añade ningún tipo de solicitud: `apix_import::wsdl` convierte un WSDL
1.1 en solicitudes POST normales con cuerpos XML, así que todo lo que funciona
con una solicitud funciona con ellas. `apix_core::soap` contiene aquello en lo
que difieren las versiones (encabezados, sobre) y lee los faults;
`Response::soap_fault` lo aplica a las respuestas XML, y el runner hace
fallar con uno de ellos a una solicitud sin pruebas, sea cual sea su estado.

El importador no descarga nada. `Loader` recorre `wsdl:import`, `xsd:import` y
`xsd:include`: dice qué ubicación leer a continuación y recibe el texto, así
que la CLI y la aplicación de escritorio leen los archivos y la red cada una a
su manera. Las ubicaciones se resuelven respecto a su documento, un documento
de la red no puede apuntar a archivos locales, y la lectura se detiene en 64
documentos. Después, `Model` reúne los mensajes, port types, bindings, ports y
globales de esquema de todos los documentos, indexados por namespace y
nombre. `Writer` construye ejemplos a partir de los nodos XSD (roxmltree los
conserva y resuelve `prefix:name` con las declaraciones del propio nodo), y
decide para cada elemento si está cualificado a partir de `form`,
`elementFormDefault` y de si es global. Los namespaces que usa van en el
sobre. Una pila de los tipos con nombre que se están escribiendo detiene la
recursión.

La importación desde URL en la aplicación y en la CLI lee el texto y después
sigue los mismos caminos que un archivo, así que sirve para todos los
formatos. Los WSDL de prueba de `apix-import/tests/fixtures/` también
alimentan el ejemplo `soap_server` (:8773).

## Esquemas GraphQL (M6)

`apix_core::graphql` contiene la `INTROSPECTION_QUERY` estándar, un `Schema`
leído de su respuesta (tipos con campos, argumentos, campos de entrada,
valores de enum, interfaces y tipos posibles; `TypeRef` conserva las listas y
`!`) y `to_sdl`, que escribe primero los tipos raíz y deja fuera los escalares
integrados y los tipos `__`. Para el editor lee una consulta solo hasta el
cursor: `spot_at` divide el texto en tokens (descartando comentarios, cadenas
y `$variables`) y recorre una pila de selecciones, listas de argumentos,
objetos de entrada y listas, siguiendo alias, `... on Type`, fragmentos y
definiciones de variables, para decir qué encaja ahí (`Spot`). `suggestions`
lo convierte en etiquetas con sus tipos y descripciones, ordenadas por
prefijo, después por subcadena, con los obsoletos al final; `describe_at` da
el Markdown para la tarjeta al pasar el ratón. No se valida nada: una consulta
rota se sigue completando lo mejor posible. El editor de variables también
autocompleta: `variable_types` lee las declaraciones `$name: Type`, un pequeño
escáner de JSON encuentra la ruta de claves y elementos de lista hasta el
cursor y si ahí va una clave o un valor, y `variable_suggestions` recorre esa
ruta por los campos de entrada y los elementos de lista para sugerir las
variables aún no dadas, los campos de entrada, los valores de enum (como
cadenas JSON) y `true`/`false`; una cadena que se está escribiendo se
reemplaza con sus comillas, incluida una que cerró el editor.

`apix_http::graphql::fetch_schema` envía la consulta de introspección como un
POST con la URL, los encabezados, la autenticación y los ajustes de la
solicitud (la solicitud se resuelve primero) y lee el esquema, o dice por qué
no lo hay (un estado que no es 2xx con el principio del cuerpo, o el
`errors[0].message` del servidor).

En la aplicación, `workspace/graphql_ui.rs` implementa `CompletionProvider` y
`HoverProvider` de gpui-kit sobre un `Rc<RefCell<Option<Rc<Schema>>>>`
compartido que guarda el editor de consultas; los autocompletados reemplazan
la palabra que se está escribiendo mediante una edición de texto de LSP.
**Obtener esquema** del editor de Cuerpo emite `FetchSchema`; el workspace
construye la solicitud como para un envío (autenticación y ajustes heredados,
variables) y la obtiene en el runtime de red. La respuesta de cada endpoint
se guarda en `<config>/Apix/graphql/<FNV-1a of the URL>.json` y se carga
cuando se abre una solicitud GraphQL. La vista SDL es un editor de solo
lectura que se rellena al dibujar, solo una vez que se muestra.

## gRPC (M6)

`apix-grpc` hace llamadas sin código generado. La descripción del servicio es
un `prost_reflect::DescriptorPool`: a partir de los archivos `.proto` de la
solicitud mediante protox (Rust, sin `protoc`; los imports se buscan en
`import_paths` y junto a cada archivo, los tipos conocidos de Google vienen
integrados), o a partir del servidor mediante gRPC reflection (v1, después
v1alpha; los archivos que el servidor omite se piden por nombre y después se
añaden en orden de dependencias). Los mensajes son `DynamicMessage`s: JSON de
entrada mediante el soporte serde de prost-reflect, JSON de salida con los
nombres de campo del `.proto` y los valores predeterminados visibles.
`DynamicCodec` los conecta a tonic, así que `tonic::client::Grpc` ejecuta los
cuatro tipos de método (unario, streaming de servidor, streaming de cliente,
bidireccional) sobre un `Channel` (HTTP/2 sobre el mismo hyper y rustls que ya
usa el motor HTTP).

`apix_grpc::convert` crea una descripción a partir de lo que haya, probando
por orden: un descriptor set en JSON; texto `.proto`; el
`FileDescriptorProto` que incrusta el código generado (`embedded.rs` sabe
cómo escribe esos bytes cada generador, y los de PHP pueden incluso no ser
texto válido); después, declaraciones en un lenguaje de programación
(`source.rs`: fuera comentarios, bloques encontrados, miembros leídos por
familia de lenguaje, tipos normalizados, números tomados de las anotaciones y
de los codificadores de ts-proto) o un ejemplo JSON (`json.rs`, que conserva
el orden de las claves, que `serde_json::Value` perdería). Los dos últimos
pasan por un pequeño `Schema` convertido en `.proto`. `printer.rs` vuelve a
escribir una descripción exacta como texto `.proto` y solo se da por buena si
ese texto compila a la misma descripción; si no, o si hay varios archivos, la
definición se queda como descriptor set en JSON. `compare` recorre una
definición frente a la descripción de un servidor, emparejando los campos por
nombre, ya que los nombres de los mensajes nunca viajan por la red.
`described` mantiene el orden: la definición de la solicitud, sus archivos,
reflection; una suposición prueba primero reflection y le cede el paso.
`call` informa de los campos que un `DynamicMessage` no pudo ubicar
(`CallEvent::Note`), porque nada más lo haría.

Los encabezados y la autenticación de la solicitud se convierten en metadatos
(Bearer, Basic, un encabezado de clave de API, OAuth 2.0 mediante
`apix_http::oauth2`; las claves `-bin` van en base64); los tipos de
autenticación que firman solicitudes HTTP se rechazan con un motivo. El tiempo
de espera, el certificado de cliente y la CA de confianza vienen de los
ajustes de la solicitud. Con la verificación TLS desactivada, a tonic (que no
tiene ese interruptor) se le da una dirección `http://` simple y el origen
`https` real, y un conector propio (`insecure.rs`) hace el TLS: rustls con
ALPN h2 y un verificador que acepta cualquier certificado pero sigue
comprobando las firmas del handshake. `call` informa de los eventos `Sent`,
`Headers` y `Received` a medida que ocurren y termina con un `CallOutcome`: el
código y el mensaje de estado, los metadatos de la respuesta, los trailers y
todos los mensajes. Un estado distinto de OK es un resultado, no un error; los
errores son lo que impide que una llamada ocurra (sin conexión, sin
descripción, un mensaje que no encaja con su tipo).

En la aplicación, una URL `grpc://` convierte la pestaña Cuerpo en la pestaña
Mensaje (`workspace/grpc_editor.rs`): un selector de métodos con búsqueda que
rellena **Cargar métodos**, los archivos `.proto` y el mensaje con un ejemplo
construido a partir del tipo de entrada. La llamada se ejecuta en el runtime
de red y rellena una estructura compartida que la interfaz lee cada 60 ms; el
panel de respuesta es un registro (`ResponseState::Grpc`,
`workspace/grpc_ui.rs`) como el de un WebSocket, y Esc descarta la llamada. La
CLI imprime una respuesta unaria completa y los streams línea a línea, y
`apix schema` escribe el servicio como texto parecido a `.proto`.

Las ejecuciones llaman a las solicitudes gRPC mediante
`apix_grpc::send_for_tests`, que convierte el `CallOutcome` en un
`apix_http::Response` (`CallOutcome::to_response`): el estado HTTP que
corresponde al código (la correspondencia de Google) con el nombre del código
como motivo, los metadatos y los trailers más `grpc-status` y `grpc-message`
como encabezados, y el mensaje (una lista JSON para un stream de servidor)
como cuerpo. Así, los scripts de prueba, las comprobaciones y los informes no
necesitan nada específico de gRPC. El historial guarda una llamada con
`Summary::grpc` definido: el estado es el código gRPC, los trailers y el
mensaje de estado tienen sus propios campos, y el cuerpo es el registro
(`[{"at_ms", "sent", "message"}]`), que `grpc_ui::restored` vuelve a convertir
en la vista de registro.

## Ejecuciones en paralelo y monitores (M7)

`Plan::parallel` mayor que uno ejecuta ese número de iteraciones a la vez
(`buffer_unordered` de `futures_util` en la propia tarea de la ejecución, sin
hilos): cada iteración envía sus solicitudes en orden con su propia copia del
estado de los scripts, un fallo con `bail` detiene a las demás, y el informe
se ordena después por iteración. El progreso pasa por un mutex alrededor del
callback de quien llama.

`apix_core::monitor` describe `[[monitor]]` en `apix.toml` (nombre, `every`,
`run`, entorno, webhook, activado) y lee los intervalos;
`apix_runner::monitor` ejecuta uno (`plan` resuelve las variables como
`apix run`: el entorno del monitor, `APIX_VAR_*` y después los valores
fijados), lo resume (`Outcome`), mantiene un `Log` en líneas JSON por monitor
(`<config>/Apix/monitors/<hash of the collection>-<name>.jsonl`, los últimos
500), decide cuándo toca cada uno (`Scheduler`: un monitor visto por primera
vez toca una vez que ha pasado su intervalo desde su última ejecución
registrada) y avisa a un webhook cuando cambia el estado (`change`,
`notify`). `apix monitor` y la aplicación de escritorio
(`workspace/monitors_ui.rs`: un temporizador cada segundo mientras el ajuste
está activado, una ejecución cada vez en el runtime de red) lo usan, así que
comparten el registro.

## Git (M7)

`apix_core::git` ejecuta el programa `git` que ya está instalado (sin
biblioteca de git, así que no se añade nada al binario y se aplican la
configuración, los hooks y las credenciales del propio usuario). Un `Repo` se
abre en la carpeta de la colección o del proyecto y cada comando se ejecuta
allí con `--relative`, así que las rutas son relativas a esa carpeta y una
colección dentro de un repositorio más grande solo ve sus propios archivos.
`changes_since(rev)` es `git diff --merge-base rev` (donde la rama salió de
`rev`, como en un pull request) más los archivos sin seguimiento; sin
historial (un clon superficial) recurre al propio `rev` y lo indica.
`show(rev, path)` lee un archivo en un commit.

`git::affected` asigna los archivos cambiados a solicitudes según su papel
(`git::role_of`): un archivo de solicitud o sus scripts `<stem>.*.js` → esa
solicitud; `folder.toml` o `_folder.*.js` → las solicitudes bajo esa carpeta;
`apix.toml`, `_collection.*.js`, `environments/*.toml`, el archivo del
proyecto o los entornos del proyecto → todas; cualquier otro archivo → las
solicitudes cuyo archivo lo nombra. `apix run --changed` conserva esas
solicitudes y el informe registra la selección (`RunReport::selection`).

`apix_core::diff` compara dos versiones de un archivo por su significado: las
solicitudes se analizan como `Request` y se serializan a JSON (así no importan
los valores predeterminados ni el formato), el resto de TOML como valores, y
los scripts y el resto del texto línea a línea (subsecuencia común más larga,
recortando el principio y el final comunes). Las listas de filas con un `key`
o un `name` distintos (encabezados, query, `[[example]]`) se convierten en
tablas indexadas por él, así que las filas se emparejan por nombre; los
campos se nombran por su ruta (`headers.Accept`, `body.json`). Una solicitud
eliminada y otra añadida con el mismo contenido se convierten en un único
movimiento. `apix diff` imprime el resultado como texto, Markdown o JSON.

`RunReport::to_markdown` escribe la ejecución para GitHub (una fila de tabla
por solicitud, los fallos en un `<details>` abierto), empezando por un
marcador oculto `<!-- apix-run: <name> -->` que busca el paso de comentario
de la action para editar su comentario anterior. Se mantiene por debajo del
límite de 65 536 caracteres de GitHub.

La aplicación de escritorio (`workspace/git_ui.rs`) lee la rama y
`git status` de la carpeta abierta en segundo plano después de cada recarga y
cuando se activa la ventana, y mantiene un mapa de rutas cambiadas para las
marcas de la barra lateral; una solicitud cuyos propios scripts cambiaron
cuenta como cambiada.

La ventana Git (`workspace/git_panel.rs`) usa el mismo `Repo`:
`working_changes` para la lista, `show("HEAD", path)` frente al archivo en el
disco mediante `diff::file` para el archivo seleccionado, y `commit_folder`
(`git add -A -- .` y después `git commit -- .`, así que solo la carpeta
abierta), `branches`, `switch`, `create_branch` (`switch -c`), `pull`
(`--ff-only`), `push` (`-u origin <branch>` cuando no hay upstream) y `log`
(hash, hora, asunto). Git se ejecuta con `GIT_TERMINAL_PROMPT=0`, así que un
remoto que necesita contraseña falla con el mensaje de git en lugar de
esperar una entrada; los credential helpers siguen funcionando. Cada
operación se ejecuta en segundo plano y actualiza la barra de estado.

## Asistente de IA

`apix_core::ai` construye a mano la solicitud para uno de tres proveedores
(Rust no tiene un SDK oficial para ellos) y lee la respuesta. `Provider`
(`anthropic`, `openai`, `google`) da el modelo predeterminado, la cuenta del
llavero para la clave (`ai:<provider>-api-key`, mediante
`apix_vault::keychain::save_text`), su variable de entorno y la URL.
Anthropic: `POST /v1/messages` con `x-api-key`,
`anthropic-version: 2026-10-21`, `claude-opus-5-5` y, para el modelo
predeterminado, alternativas en el servidor (`fallbacks: "default"`, beta
`server-side-fallback-2026-07-01`); un motivo de parada `refusal` se
convierte en un mensaje. OpenAI: `POST /v1/chat/completions` con una clave
Bearer, un mensaje de sistema y uno de usuario; un campo `refusal` se trata
como rechazo. Google: `POST /v1beta/models/{model}:generateContent` con
`x-goog-api-key`, `systemInstruction` y `contents`; los finales `SAFETY` y
los prompts bloqueados se tratan como rechazo, y las partes de razonamiento se
omiten. Cada prompt lleva el intercambio (`Context::describe`: encabezados de
credenciales ocultos, cuerpos recortados a 12 KB), la tarea y el código de
idioma de la aplicación. `models_request` / `models` listan lo que una clave
puede usar (solo modelos de chat de OpenAI, modelos de Google que admiten
`generateContent`). `APIX_AI_URL` reemplaza la URL base para las pruebas. La
aplicación (`workspace/ai_ui.rs`) envía mediante `net::send`, añade pruebas
con `ScriptEditor::append_test` y muestra las explicaciones en un diálogo;
Configuración guarda `ai_provider` y `ai_model`.

## Servidor MCP

`apix mcp` (`apix-cli/src/mcp.rs`) lee un mensaje JSON-RPC 2.0 por línea de
la entrada estándar y escribe una respuesta por línea en la salida estándar
(el transporte stdio de MCP; se omite una marca de orden de bytes antes de la
primera línea). `initialize` responde con la versión de protocolo pedida
cuando la conoce (2025-06-18, 2025-03-26, 2024-11-05) y con la más reciente
en caso contrario, y solo ofrece herramientas; las notificaciones no reciben
respuesta y los métodos desconocidos dan el error -32601. Los resultados de
`tools/call` llevan el JSON como texto y como `structuredContent`, y una
herramienta que falla es un resultado con `isError`, no un error de
protocolo, para que el agente pueda leer el motivo. Las rutas que da el
agente se unen a la raíz de la colección, con `..` resuelto, y se rechazan si
quedan fuera de ella. `send_request` pasa por `apix_runner::exchange`
(scripts, variables, pruebas) y `run_requests` por `apix_runner::run`; las
variables vienen del mismo `VarArgs::resolve` que en los demás comandos, con
las `variables` del agente como `--var`s. La CLI se ejecuta en un hilo de
16 MB, ya que el future compartido de los comandos superó la pila del hilo
principal en Windows.

## Configuración inicial y Claude

`workspace/setup_ui.rs` abre la Configuración inicial una vez
(`settings.setup_shown`) y desde Ayuda. El botón de la base de datos ejecuta
`route_ui::download_geo` (el archivo de DB-IP de este mes, o si no el del mes
pasado, mediante el cliente de la aplicación, descomprimido fuera del hilo de
la interfaz); `apix geo download|status|remove` hace lo mismo en una
terminal, que es lo que llamará el paso opcional de un instalador.
`apix_core::claude_mcp` encuentra la configuración de Claude Desktop
(`%APPDATA%\Claude`, `~/Library/Application Support/Claude`,
`~/.config/Claude`; `APIX_CLAUDE_CONFIG` para las pruebas), llama al servidor
`apix-<collection>`, lo apunta al `apix` que está junto al programa en
ejecución (o en el PATH) con `mcp <folder> [--list-only]` y lo fusiona en
`mcpServers` conservando todo lo demás (el original se copia a `.bak` una
vez); `claude_code_command` es `claude mcp add --scope user`.

## Flujos

`apix_core::flow` es el archivo: un `Flow` tiene nodos (`NodeKind::Request`,
`Check`, `Set`, `Delay`, aplanados en cada tabla `[[node]]` con su id y su
posición) y aristas desde un resultado de un nodo (`ok`/`fail`, `yes`/`no`,
`next`) hasta otro nodo. `problems()` lista lo que impide ejecutar un flujo;
`Seen` es la última respuesta, y `Seen::subject` lee `status`, `time`,
`body`, `header:Name`, o un filtro JSONPath o jq mediante
`apix_core::filter`. Los archivos `.flow` pueden estar en cualquier lugar de
la colección; la barra lateral los ignora.

`apix_runner::flow::run` recorre el grafo desde el nodo inicial. Un nodo de
solicitud pasa por `exchange`, como en una ejecución, así que funcionan los
scripts, las pruebas y las variables `pm.*`; lo que guarda se define en el
ámbito local del estado de los scripts, que tiene prioridad sobre el entorno
para los nodos posteriores, como haría `pm.variables.set`. Una comprobación
cuyo sujeto es texto resuelve las `{{variables}}` y no tiene valor cuando una
no está definida. La ejecución informa de cada paso (`FlowProgress`) y se
detiene tras 1000 pasos, o mediante `Control`.
`FlowReport::to_run_report` reutiliza los informes de las ejecuciones (JUnit,
HTML, Markdown). El editor de escritorio (`workspace/flows_ui.rs`) dibuja los
nodos como elementos posicionados y las aristas en un canvas: las aristas
hacia delante como curvas, las que vuelven atrás como líneas alrededor de los
nodos; ejecuta el flujo en el runtime de red y lee su progreso con un
temporizador, como el Runner.

## Complementos

`apix-plugins` ejecuta complementos WebAssembly con `wasmi` 2, un intérprete:
sin JIT, así que no se genera código nativo a partir de un complemento, y
compila allí donde compile Rust. Un módulo no puede importar nada, así que un
complemento no tiene ninguna función del host (ni red, ni archivos, ni reloj,
ni aleatoriedad); exporta `memory`, `apix_alloc`, `apix_manifest` y
`apix_call`, y las respuestas son JSON pasado como `ptr << 32 | len`. Cada
llamada instancia el módulo desde cero (sin estado entre llamadas) con 2·10⁹
unidades de combustible y un límite de memoria de 256 MB, en un hilo con una
pila de 64 MB; quedarse sin combustible se informa como "se ejecutó demasiado
tiempo". El manifiesto se lee una vez al cargar el complemento; su `name`
debe ser `[a-z0-9-]`. `installed()` guarda en caché la carpeta de
complementos (`<config>/Apix/plugins`, o `APIX_PLUGINS_DIR`) hasta
`reload()`.

Los ganchos se encuentran con el resto de Apix en tres lugares:
`Auth::Plugin { plugin, config }` en el modelo, resuelto como el resto de la
autenticación (variables en la configuración) y aplicado por
`apix_http::prepare` a la solicitud final; `apix_import::plugin::plan`, que
convierte la respuesta de un importador en archivos de solicitud; y el
visualizador de la respuesta (`VisualMode::Plugin`), que ejecuta el
complemento fuera del hilo de la interfaz y dibuja una tabla o texto. wasmi se
compila con `opt-level = 3` y sin aserciones de depuración incluso en las
compilaciones de depuración, ya que su despacho depende de llamadas de cola
que de otro modo agotan la pila.

## Comparar respuestas

`apix_core::compare` recibe dos `Snapshot`s (etiqueta, estado, encabezados,
cuerpo, tiempo). Los encabezados se comparan por nombre en minúsculas, uniendo
varios con el mismo nombre; los de `VOLATILE` (fechas, caché, ids de
solicitud y de traza, límites de tasa, cómo se envió el cuerpo) solo se
cuentan como omitidos. Dos cuerpos JSON pasan por `by_id`, que convierte las
listas de registros con valores `id` / `_id` / `uuid` / `key` distintos en
tablas indexadas `[id=…]`, y después por `diff::fields_as_they_are` (el
recorrido del diff de solicitudes sin su emparejamiento por key/name, que
sirve para archivos de solicitud pero no para datos); el resto del texto se
compara línea a línea, y cualquier otra cosa solo por tamaño. La ventana
Comparar respuestas de la aplicación (`workspace/compare_ui.rs`) toma el lado
anterior del historial (entradas con el mismo archivo de solicitud, o el mismo
método y URL) o envía la solicitud resuelta con las variables de otro
entorno; `apix compare` la envía en dos entornos.

## Pruebas de carga

`apix_http::load::Load` construye un cliente reqwest para una solicitud
resuelta (los ajustes de TLS de la solicitud mediante `tls::client_config`,
su proxy, su tiempo de espera y su política de redirecciones, un pool inactivo
ilimitado) y `hit()` la envía, contando los bytes del cuerpo sin
descomprimirlo; OAuth 2.0 se convierte una vez en un token Bearer, y Digest se
rechaza. `apix_runner::bench::Bench::run` arranca `concurrency` workers en un
`FuturesUnordered`; cada uno toma el siguiente número de solicitud de un
contador atómico hasta que el recuento, el plazo o `stop()` dicen que no, y
con una tasa espera hasta `start + n / rate`. Cada envío se registra bajo un
mutex: su latencia en microsegundos (un `u32` por solicitud, así que un millón
de solicitudes son 4 MB), el estado, el error por tipo ("could not connect",
"timed out", …), los bytes y un cubo por segundo. `summary()` se puede llamar
mientras se ejecuta: ordena una copia de las latencias para los percentiles
por rango más cercano. La CLI ejecuta la prueba en un runtime propio con un
hilo por núcleo e imprime una línea por segundo; la aplicación la ejecuta en
su runtime de red y repinta cuatro veces por segundo.

## Ruta y mapa

`apix-route` lanza la herramienta del sistema, igual que `apix_core::git`
lanza git: `tracert -d -h N -w 1000` en Windows,
`traceroute -n -q 1 -w 1 -m N` en macOS y Linux, y `tracepath -n -m N` cuando
falta traceroute. Ninguna necesita permisos de administrador, que sí haría
falta en Windows para enviar ICMP por nuestra cuenta. `parse_line` lee una
línea de cualquiera de ellas solo por sus números y direcciones (primero el
número de salto, cada `N ms`, la primera palabra que se puede leer como
dirección), así que un Windows traducido en su propia página de códigos se lee
igual; el tiempo de un salto es el mejor de sus intentos, y `<1 ms` cuenta
como medio milisegundo. `Trace` recorre los saltos a medida que la
herramienta los imprime y la termina cuando se descarta o se detiene.

`Geo` abre un archivo MaxMind DB (`maxminddb`) y da un `Place` (latitud,
longitud, nombre de la ciudad en inglés, país ISO) para las direcciones
públicas; las direcciones privadas, de loopback, de enlace local, de CGNAT y
de multidifusión nunca se buscan. La base de datos es el primer `.mmdb` que
aparece en `<config>/Apix/geo`; `install` descomprime una descarga (gzip),
comprueba que se abre y solo entonces reemplaza la antigua. `dbip_urls` da la
dirección de DB-IP Lite de este mes y la del mes pasado (`APIX_GEO_URL` las
reemplaza). El mapa es una proyección equirrectangular de `land::LAND`, con
los continentes y las islas grandes dibujados a mano con una precisión de
unos dos grados (con los mares Negro y Caspio en `land::WATER`), así que no se
descargan ni se licencian datos de mapas. La ventana de escritorio
(`workspace/route_ui.rs`) ejecuta la traza en un hilo, coloca los saltos a
medida que llegan, lee la base de datos una vez por ejecución de la
aplicación y pinta el mapa con paths de GPUI. `apix trace` imprime los saltos
con sus lugares y la distancia de cada salto de más de 100 km.

## Búsqueda de actualizaciones

`apix_core::update` construye la solicitud (`releases/latest` de GitHub para
11R5/Apix, `User-Agent: Apix/<version>`, un tiempo de espera de 10 s;
`APIX_UPDATE_URL` la dirige a otro lugar) y lee la respuesta: `Version`
analiza `v1.2.3`, `1.2` y las versiones preliminares (`1.0.0-rc.1` va antes
que `1.0.0`, las partes numéricas por valor), un 404 significa que aún no hay
nada publicado, y los demás errores llevan el mensaje de GitHub. La CLI
(`apix update`) la envía con su propio cliente; la aplicación
(`workspace/update_ui.rs`) mediante `net::send` al iniciar cuando
`settings.check_updates` está activado (solo se muestra una versión más
nueva) y desde Ayuda › Buscar actualizaciones… (se muestra cualquier
respuesta). No se descarga ni se instala nada.

## Temas

`theme.rs` contiene los colores. Un `ThemeDef` es un tema (integrado, o leído
de `<config>/Apix/themes/*.toml`; los colores que un archivo omite vienen de
Obsidian o de Daylight) con `Colors` y una marca de oscuro/claro.
`Palette::build` lo convierte en lo que usan las vistas para pintar: el ajuste
de acento (un color con nombre con un tono oscuro y un tono claro más
intenso, o `#RRGGBB`), el texto sobre el acento elegido por luminancia
relativa, los tintes de hover y de los campos para oscuro o claro, y el
ajuste de transparencia: el alfa del fondo de la ventana es `1 - t`, los
paneles se aclaran al 60% de ese ritmo, los bordes se acercan al blanco a
medida que crece `t`, y los temas claros usan el 70% de `t`.
`from_settings` elige el tema oscuro o claro según el ajuste de apariencia y,
para "seguir el sistema", según `App::window_appearance`.

La vista raíz de gpui-kit pinta `Theme::background` bajo toda la ventana, lo
que ocultaría el desenfoque del sistema, así que con cualquier transparencia
se hace transparente y los diálogos reciben su propio fondo mediante
`bidi::DialogBidi`, que usan todos los diálogos. El desenfoque lo hace el
sistema: `WindowBackgroundAppearance::Blurred` es acrílico en Windows
(`SetWindowCompositionAttribute`) y vibrancy en macOS.

Las vistas nunca guardan una copia de la paleta: llaman a `theme::current()`
cuando pintan. `theme::set` guarda la nueva paleta, apunta a ella el tema de
componentes de gpui-kit (cambiando su modo, para que los colores del editor
de código la sigan), pone el fondo de cada ventana en desenfocado u opaco y
refresca las ventanas, así que un cambio se ve al instante. El workspace
observa la apariencia de la ventana y lo vuelve a llamar cuando el sistema
cambia entre claro y oscuro.

## Idiomas (M4)

Cada texto de la interfaz de escritorio pasa por una clave: `tr!("menu.file")`
(un envoltorio fino sobre `rust_i18n::t!` que devuelve un `SharedString` para
mostrar; `tr_plain!` da el mismo texto para lo que se guarda, como el nombre
predeterminado de una solicitud nueva). rust-i18n es el mismo sistema que usa
gpui-kit, así que ambos siguen una única configuración regional para todo el
proceso; el chino usa `zh-CN`, el código que usan los propios textos de
gpui-kit. Cada idioma es un archivo plano,
`crates/apix-desktop/locales/<code>.yml` (un `key: value` por línea,
ordenado, con marcadores `%{name}`), compilado dentro del binario; `build.rs`
vuelve a compilar cuando cambia un archivo. Un texto que falta recurre al
inglés, y una prueba unitaria falla cuando a un idioma le falta una clave,
tiene una desconocida o usa marcadores distintos de los del inglés. Las
frases son claves completas con marcadores, nunca pegadas a partir de trozos,
porque el orden de las palabras difiere entre idiomas.

El idioma se elige en Configuración (o sigue el idioma del sistema en el
primer inicio) y cambia al instante: los textos se leen en cada renderizado, y
los pocos textos guardados dentro de widgets (marcadores de los campos,
entradas de los selectores) los vuelve a definir `Workspace::retranslate`. Los
glifos de japonés, chino y árabe vienen de la sustitución de fuentes de la
plataforma (DirectWrite, Core Text, fontconfig) junto a la Inter incrustada.
El texto árabe se compone y se coloca de derecha a izquierda; en Windows eso
necesita `vendor/gpui-pre-windows`, el backend DirectWrite de gpui 0.3.9 con
la colocación de los tramos de glifos corregida (aplicado como parche desde
el `Cargo.toml` del workspace; se puede quitar cuando se corrija en el
original). gpui no sabe ajustar texto de derecha a izquierda: compone un
párrafo como una sola línea y lo corta en posiciones x de glifos que supone
crecientes de izquierda a derecha. Por eso el texto que puede ocupar varias
líneas (notas, pistas, descripciones, mensajes) pasa por `bidi::para`, que en
RTL corta el texto por los espacios en orden de lectura, midiendo cada línea
candidata, y pinta cada línea sin ajuste (`src/bidi/paragraph.rs`). En los
demás idiomas es texto normal de gpui. gpui además distribuye cada línea con
una dirección base de izquierda a derecha, así que en un idioma RTL `tr!`
envuelve cada texto en una incrustación de derecha a izquierda
(U+202B…U+202C; DirectWrite ignora los aislamientos más recientes). Así, una
frase que empieza o termina con una palabra latina ("URL", "OAuth 2.0",
"Ctrl+S") se sigue leyendo de derecha a izquierda, y `i18n::display` hace lo
mismo con los textos compuestos como la barra de estado. Las marcas nunca
llegan a los archivos: los textos guardados usan `tr_plain!`. Los mensajes de
error de los crates del motor se quedan en inglés.

En un idioma de derecha a izquierda (`Language::rtl`, árabe) la ventana se
refleja: la barra lateral pasa a la derecha, el panel de respuesta a la
izquierda del panel de la solicitud, los controles de la ventana a la
izquierda, y los menús, las tiras de pestañas, las filas de formularios y la
barra de estado van desde la derecha, con el texto alineado a la derecha. GPUI
no tiene dirección de diseño, así que de esto se encarga `src/bidi.rs`: un
trait `Bidi` en cada elemento con estilo (`dir_row` invierte una fila,
`text_start` / `text_end`, los rellenos `ps` / `pe` y `border_e_1` eligen el
lado según la dirección, `ltr` fija el texto a la izquierda), `tab_bar` /
`fill_tabs` para el `TabBar` de gpui-kit (las pestañas se dan en orden
inverso y los índices se vuelven a asignar), y `bidi_title` para los diálogos
(en RTL el botón de cerrar pasa a la fila del título, a la izquierda). Las
utilidades leen el idioma mientras se renderiza una vista, así que un cambio
de idioma refleja la ventana al instante. Los visores de código y de
respuesta, la barra de URL, las tablas clave–valor (parámetros, encabezados,
campos de formulario, variables) y los encabezados de la respuesta se quedan
de izquierda a derecha en todos los idiomas. Los menús emergentes y los
botones de gpui-kit mantienen su propio diseño interior de izquierda a
derecha (el atajo a la derecha, el icono antes de la etiqueta); los menús se
abren hacia la izquierda desde su botón.

Añadir un idioma: copia `locales/en.yml` a `locales/<code>.yml`, traduce los
valores y añade una línea a `LANGUAGES` en `src/i18n.rs`.

La documentación de usuario vive aparte del plan, una carpeta por idioma
(`Docs/<code>/`: guía de usuario, arquitectura, notas técnicas, flujos de
trabajo, imágenes), listadas en `Docs/languages.toml`. El inglés es la
fuente; cada traducción registra la versión en inglés de la que se hizo, así
que las traducciones desactualizadas salen a la luz.

## Modelo de hilos (escritorio)

- GPUI es dueño del hilo principal, que ejecuta el bucle de eventos y el
  renderizado.
- Un único **runtime de tokio** multihilo se arranca una vez en un hilo en
  segundo plano. reqwest, tonic y tungstenite lo necesitan.
- La interfaz envía el trabajo a tokio y recibe el resultado en una tarea de
  GPUI (`cx.spawn_in`). Descartar esa tarea aborta la solicitud, así que Esc y
  volver a enviar cancelan limpiamente.
- Cuando llega una respuesta, el cuerpo se formatea y se indexa en el
  ejecutor en segundo plano de GPUI antes de que lo vea la interfaz. El
  formateo usa `apix_core::text::pretty_json`, un reindentador de una sola
  pasada que no construye ningún árbol y conserva los números exactamente
  como se enviaron.

## Visor de respuestas grandes

Los cuerpos de 512 KB o más usan `large_view.rs` en lugar del editor de
código:

- `apix_core::text::RowIndex` registra dónde empieza cada fila visible, 8
  bytes por fila. Las líneas de más de 2 KB (JSON minificado) se dividen en
  varias filas.
- La posición de desplazamiento es un **número de fila**, no un
  desplazamiento en píxeles. Diez millones de filas a 20 px serían unos 200
  millones de px, muy por encima del rango en el que los píxeles en `f32` son
  exactos. El visor distribuye solo las ~120 filas que caben en pantalla.
  Gestiona la rueda, su propia barra de desplazamiento y el teclado
  (↑ ↓ RePág AvPág, Ctrl+Inicio/Fin).
- La pestaña Sin formato de un cuerpo JSON grande solo se construye cuando se
  abre.
- Ctrl+F busca en todo el texto en un hilo en segundo plano (un `regex` sin
  distinción de mayúsculas y minúsculas sobre la consulta literal) y guarda
  hasta 100,000 rangos de bytes; `RowIndex::row_of` encuentra la fila de una
  coincidencia por búsqueda binaria, y solo las filas en pantalla buscan sus
  coincidencias para resaltarlas.

Los cuerpos más pequeños están en el editor de código, que tiene activado su
propio panel de búsqueda. El filtro sobre Formateado
(`workspace/filter_ui.rs`) ejecuta `apix_core::filter` fuera del hilo de la
interfaz, 250 ms después de la última pulsación: JSONPath mediante
`serde_json_path` para una expresión que empieza por `$`, y jq mediante `jaq`
en otro caso, con 10,000 salidas de jq como máximo. Su resultado es un
`Document` como cualquier cuerpo, así que un cuerpo grande filtrado puede
acabar en el editor y al revés.

Medido en Windows en una compilación de versión, para 100 MB de JSON
minificado desde un servidor local
(`cargo run -p apix-http --release --example big_json_server`): la respuesta
está en pantalla unos 0.33 s después de pulsar Enter. Eso incluye la
transferencia, el formateo en 9.6 millones de líneas, la indexación y el
primer fotograma. La memoria ronda los 407 MB con el cuerpo cargado, frente a
69 MB en reposo.

## Presupuesto de rendimiento

| Métrica                              | Objetivo            | Medido (Windows, versión) |
|-------------------------------------|-------------------|-----------------------------|
| Arranque en frío hasta el primer fotograma | < 150 ms          | 222 ms (de `main` al primer fotograma; unos 150 ms de ellos son la carga del controlador gráfico) |
| RAM en reposo                       | < 60 MB           | 88 MB de conjunto de trabajo, con la colección de ejemplo abierta (sobre todo el controlador gráfico) |
| Binario de versión                  | < 100 MB          | 54.4 MB |
| Latencia de la entrada al pintado   | 1 fotograma (≤ 8 ms a 120 Hz) | sin medir |
| Abrir y desplazar un cuerpo JSON de 100 MB | sin fotogramas perdidos | en pantalla 306 ms después de enviar (servidor local); tiempos de fotograma sin medir |

El tiempo de arranque y la memoria en reposo superan hoy el presupuesto; está
previsto perfilarlos. Las cifras vienen de `scripts/measure.ps1` (consulta las
[notas técnicas](technical.md#medir-el-rendimiento)).

El perfil de versión ya usa LTO completo (fat), una sola unidad de generación
de código y símbolos eliminados (consulta el `Cargo.toml` raíz).
