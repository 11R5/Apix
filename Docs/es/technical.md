<!-- translated from en/technical.md sha256:40c5e638c106f35d -->

# Notas técnicas

Cómo está construido Apix, de qué depende y cómo medirlo. La
[arquitectura](architecture.md) explica el diseño; esta página es la parte
práctica.

## Estructura del repositorio

```
crates/
├── apix-core       modelo de solicitud, variables, formato en disco, proyectos, firma (sign.rs), esquemas GraphQL, git (git.rs, diff.rs)
├── apix-http       el motor HTTP (reqwest), traza de red, tokens OAuth 2.0, SSE, WebSocket, Socket.IO, descarga del esquema GraphQL
├── apix-import     importación de Postman, Insomnia, OpenAPI, HAR y WSDL
├── apix-codegen    documentación de la API (HTML, Markdown), "Copiar como" (curl, curl.exe, PowerShell, código en 10 lenguajes) y pegado de curl
├── apix-vault      colecciones cifradas
├── apix-script     scripts previos a la solicitud y de prueba (QuickJS)
├── apix-runner     ejecuciones de colecciones, archivos de datos, informes JUnit/JSON/HTML/Markdown
├── apix-grpc       llamadas gRPC: archivos .proto, reflection del servidor o definiciones a partir de código y JSON, mensajes como JSON
├── apix-proto      otros protocolos: MQTT
├── apix-mock       un servidor simulado local que responde con los ejemplos guardados
├── apix-capture    un proxy de grabación que guarda las solicitudes que pasan por él
├── apix-route      la ruta hasta un servidor y dónde están sus saltos
├── apix-plugins    complementos WebAssembly (wasmi): carga, el aislamiento, ganchos de autenticación, importación y vistas
├── apix-plugin-sdk escribir complementos en Rust; sus examples/ son los complementos de plugins/
├── apix-cli        el comando `apix`
└── apix-desktop    la aplicación de escritorio (GPUI); textos de la interfaz en locales/
tools/
├── icongen         genera los iconos de la aplicación a partir de Logo/
└── docs-check      lista las traducciones de la documentación desactualizadas
vendor/
└── gpui-pre-windows  el backend de Windows de gpui con una corrección para texto de derecha a izquierda
scripts/
├── package-*.sh/.ps1  paquetes de versión: instalador y zip de Windows, .dmg de macOS, .deb, tarball, paquete de Arch
└── measure.ps1     mediciones de rendimiento para el README
examples/
├── jsonplaceholder una pequeña colección que la aplicación abre cuando se ejecuta desde el repositorio
└── ci/             ejemplos de GitHub Actions (también monitores programados), GitLab CI y Azure Pipelines
plugins/            complementos de ejemplo (.wasm), compilados a partir de los ejemplos de apix-plugin-sdk
site/               la página de descarga (GitHub Pages), que lee la última versión desde GitHub
action.yml          la GitHub Action (`uses: 11R5/Apix@…`)
Dockerfile          la línea de comandos `apix` como imagen de contenedor
.github/workflows/  CI en tres sistemas, y la imagen publicada con las etiquetas de versión
Docs/
├── languages.toml  idiomas de la documentación
├── en/ tr/         guía de usuario, arquitectura, notas técnicas, flujos de trabajo
└── IS-PLANI.md, ROADMAP.md   el plan (detalle en turco, resumen en inglés)
```

## Compilación

Requisitos: [Rust](https://rustup.rs) estable (el repositorio se prueba con la 1.99).

- **Windows:** la cadena de herramientas MSVC (Visual Studio o Build Tools con
  "Desarrollo para el escritorio con C++" y un SDK de Windows 10/11).
- **macOS:** las herramientas de línea de comandos de Xcode.
- **Linux:** los paquetes de desarrollo de X11/Wayland, Vulkan y fontconfig que
  necesita GPUI (el README de gpui-kit los lista por distribución); aún sin
  probar.

```sh
cargo test                       # motor, CLI, importación, codegen, vault, comprobación de la documentación
cargo run -p apix-desktop        # la aplicación de escritorio (debug)
cargo test -p apix-desktop       # pruebas unitarias del escritorio (traducciones, rutas, …)
cargo build --release            # CLI optimizada
cargo build --release -p apix-desktop
```

`cargo build` y `cargo test` sin `-p` omiten la aplicación de escritorio, que
arrastra toda la pila de GPU; compílala de forma explícita. Las dependencias se
optimizan incluso en las compilaciones de depuración para que la interfaz siga
fluida durante el desarrollo.

Los complementos de ejemplo de `plugins/` necesitan una vez el destino
WebAssembly (`rustup target add wasm32-unknown-unknown`) y después
`cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown`;
copia `target/wasm32-unknown-unknown/release/examples/*.wasm` en `plugins/`
(con `-` en lugar de `_` en los nombres). Las pruebas de `apix-plugins` cargan
esos archivos. `wasmi` se optimiza y se compila sin aserciones de depuración
también en las compilaciones de depuración (`[profile.dev.package.wasmi]`): su
bucle de intérprete depende de llamadas de cola que, de otro modo, desbordan la
pila.

El perfil de versión usa LTO completo (fat), una sola unidad de generación de
código y símbolos eliminados. Una compilación de versión completa de la
aplicación de escritorio tarda unos minutos.

## Dependencias principales

| Área | Crates |
|---|---|
| Interfaz | `gpui-kit` 0.7 (GPUI y su biblioteca de componentes), gramáticas tree-sitter para JSON y GraphQL |
| HTTP | `reqwest` 0.12 con rustls, `tokio`, `hyper-util`, capas `tower` para los tiempos |
| gRPC | `tonic` 0.14 (cliente, sobre el mismo hyper y rustls), `prost-reflect` (mensajes conocidos en tiempo de ejecución, JSON), `protox` (lee archivos .proto sin `protoc`), `regex` (lectura del código y los tipos generados), `tonic-reflection` (el cliente de reflection), `rustls` y `tokio-rustls` para gRPC con la comprobación de certificados desactivada; `rcgen` crea certificados de prueba (solo en las pruebas) |
| Detalles de red | `x509-parser` (certificados), `sha2` (huellas), `netdev` (interfaces, puerta de enlace, servidores DNS); detalles de TLS mediante los propios ganchos de rustls (sin crate adicional); `flate2` y `brotli-decompressor` descomprimen los cuerpos comprimidos (ya compilados para reqwest); la ruta a partir de `tracert`, `traceroute` o `tracepath` del sistema, los lugares a partir de `maxminddb` |
| Firma | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Cifrado | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize`, `keyring` (almacén de credenciales del sistema) |
| Scripts | `rquickjs` (QuickJS, compilado desde C con el compilador del sistema); las bibliotecas de `require` son JavaScript propio de Apix, con los hashes mediante `sha1`, `sha2`, `md-5`, `hmac` |
| Archivos | `serde`, `toml`, `serde_json`, `notify` (vigilancia de archivos), `trash`; git mediante el programa `git` instalado (sin biblioteca) |
| Respuestas | `serde_json_path` (JSONPath, RFC 9535) y `jaq-core`, `jaq-std`, `jaq-json` (jq) para el filtro; `regex` para buscar en cuerpos grandes |
| Complementos | `wasmi` 2 (un intérprete de WebAssembly con límites de combustible y de memoria) |
| Idiomas | `rust-i18n` (compartido con gpui-kit), `sys-locale` |

## Archivos que Apix escribe fuera de las colecciones

| Archivo | Contenido |
|---|---|
| `<config>/Apix/settings.toml` | idioma, apariencia, tema oscuro y claro, color de acento, transparencia, tiempo de bloqueo automático, historial activado o no, dónde van los valores secretos de los entornos, si los monitores se ejecutan mientras la aplicación está abierta |
| `<config>/Apix/capture-ca.pem`, `capture-ca.key` | la autoridad de certificación local del proxy de captura, creada la primera vez que se abre HTTPS; solo es de confianza si la añades tú mismo |
| `<config>/Apix/geo/*.mmdb` | la base de datos de ubicación de IP para el mapa de la ruta (descargada desde la ventana Ruta y mapa, o colocada allí) |
| `<config>/Claude/claude_desktop_config.json` | escrito solo por Conectar con Claude: la entrada `apix-<collection>` bajo `mcpServers` (el original se guarda como `.json.bak`) |
| `<config>/Apix/monitors/` | los últimos 500 resultados de cada monitor, una línea JSON por ejecución (`<hash of the collection folder>-<monitor name>.jsonl`), escritos por `apix monitor` y la aplicación |
| `<config>/Apix/state.toml` | colecciones abiertas recientemente, el entorno elegido y las pestañas abiertas por colección o proyecto (no para colecciones cifradas) |
| `<config>/Apix/cookies.json` | el almacén de cookies (las cookies de sesión no se conservan) |
| `<config>/Apix/graphql/*.json` | el último esquema GraphQL obtenido de cada endpoint (la respuesta de introspección del servidor), con un hash de la URL como nombre |
| `<config>/Apix/themes/*.toml` | tus propios temas (solo se escribe cuando pulsas **Abrir la carpeta** en Configuración y la carpeta está vacía: un ejemplo) |
| Almacén de credenciales del sistema, servicio `Apix` | solo cuando se pide: contraseñas recordadas de colecciones cifradas (`vault:<folder>`) y valores secretos de los entornos (`env:<id>`, más `env:<id>#1`, … para valores largos), claves del asistente de IA (`ai:anthropic-api-key`, `ai:openai-api-key`, `ai:google-api-key`) |
| `<config>/Apix/plugins/*.wasm` | complementos instalados (la ventana Complementos, `apix plugins install`) |
| `<config>/Apix/history/` | solicitudes enviadas (con las `{{variables}}` sin resolver) y sus respuestas: `index.jsonl`, un `<id>.json` y un `<id>.body` por entrada; las últimas 300, con los cuerpos recortados a 256 KB; una llamada gRPC guarda su registro como cuerpo |

`<config>` es `%APPDATA%` en Windows, `~/Library/Application Support` en macOS
y `~/.config` en Linux. Los tokens OAuth 2.0 y las contraseñas de las
colecciones cifradas nunca se escriben en el disco.

## Variables de entorno

| Variable | Efecto |
|---|---|
| `APIX_PASSWORD` | Contraseña de una colección cifrada para la CLI (`--password-env` elige otra variable). |
| `APIX_VAR_<NAME>` | Una variable para la CLI: `APIX_VAR_TOKEN` rellena `{{token}}` (se compara con los nombres de la colección sin importar mayúsculas y minúsculas); tiene prioridad sobre los entornos, y `--var` tiene prioridad sobre ella. |
| `APIX_THEME` | un id de tema (`obsidian`, `cyber`, `dracula`, `nord`, `solarized-dark`, `daylight`, `solarized-light` o el nombre de un archivo de tema); muestra ese tema diga lo que diga la configuración. |
| `APIX_BENCH` | `startup` o `json:<url>`: mide y sale (ver más abajo). |
| `APIX_BENCH_OUT` | Archivo al que se añaden las mediciones. |
| `APIX_BENCH_PHASES` | Con `APIX_BENCH=startup`: informa también de cuándo se alcanzó cada paso del arranque (líneas `phase_…`), para ver dónde se va el tiempo. |
| `APIX_UPDATE_URL` | Dónde pregunta la búsqueda de actualizaciones en lugar de la última versión de GitHub (un fork, un espejo, un servidor de prueba). |
| `OPENAI_API_KEY`, `GEMINI_API_KEY` (o `GOOGLE_API_KEY`), `ANTHROPIC_API_KEY` | La clave del asistente de IA cuando no hay ninguna guardada en el llavero para ese proveedor. |
| `APIX_CLAUDE_CONFIG` | El archivo de configuración de Claude Desktop que escribe Conectar con Claude, en lugar del real (para pruebas). |
| `APIX_AI_URL` | Adónde envía el asistente de IA sus solicitudes a la Messages API en lugar de a Anthropic (un proxy, un servidor de prueba). |
| `APIX_GEO_URL` | De dónde descarga **Descargar DB-IP Lite** en lugar de DB-IP. |
| `APIX_PLUGINS_DIR` | La carpeta de complementos, en lugar de `<config>/Apix/plugins`. |
| `GITHUB_BASE_REF` | La define GitHub Actions en los pull requests: `apix run --changed` sin valor compara con `origin/<it>`. |

## Medir el rendimiento

`scripts/measure.ps1` compila la aplicación de versión y mide:

- **Binario de versión:** el tamaño.
- **Del inicio al primer fotograma:** desde `main` hasta que se dibuja el
  primer fotograma (`APIX_BENCH=startup`), mediana de varias ejecuciones.
- **Memoria en reposo:** el conjunto de trabajo cinco segundos después de
  arrancar con la colección de ejemplo abierta.
- **JSON grande en pantalla:** un GET de un cuerpo JSON de 100 MB desde el
  ejemplo local `big_json_server`, desde el envío hasta que se dibuja la
  respuesta (`APIX_BENCH=json:<url>`), para que la red no forme parte de la
  cifra.

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7, también en macOS/Linux
```

El script imprime una tabla Markdown con la máquina en la que se ejecutó; el
README muestra la más reciente. Los tiempos vienen de
`crates/apix-desktop/src/bench.rs`, que no hace nada si `APIX_BENCH` no está
definida.

Otras utilidades:

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # 100 MB de JSON en :8765
cargo run -p apix-http --example oauth_server                       # servidor de prueba OAuth 2.0 en :8766
cargo run -p apix-http --example sse_server                         # server-sent events en :8767 (se reanuda tras Last-Event-ID)
cargo run -p apix-http --example ws_server                          # eco WebSocket (también binario) y ticker en :8768
cargo run -p apix-http --example graphql_server                     # un endpoint GraphQL (esquema y algunos usuarios) en :8769
cargo run -p apix-http --example socketio_server                    # Socket.IO 4: namespaces /, /chat, /ticker, /private (auth) en :8771
cargo run -p apix-grpc --example grpc_server                        # un Greeter gRPC con reflection en :8770
cargo run -p apix-http --example soap_server                        # una calculadora SOAP en :8773 (WSDL en ?wsdl; faults con 500 y 200)
cargo run -p apix-proto --example mqtt_broker                       # un broker MQTT en :8772 (`clock` cada segundo)
```

## Idiomas de la interfaz

Los textos de la interfaz están en `crates/apix-desktop/locales/<code>.yml`, un
`key: value` plano por línea, que rust-i18n lee mediante la macro `tr!`. Una
prueba unitaria falla cuando a un idioma le falta una clave o usa
`%{placeholders}` distintos de los del inglés. Para añadir un idioma, copia
`en.yml`, tradúcelo y añade una línea a `LANGUAGES` en `src/i18n.rs`.

## Idiomas de la documentación

`Docs/languages.toml` lista los idiomas de la documentación. Las traducciones
empiezan con un sello que nombra el texto en inglés del que proceden:

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check                      # lista las traducciones que faltan o están desactualizadas y los enlaces rotos
cargo run -p docs-check -- stamps            # imprime el sello actual de cada documento en inglés
cargo run -p docs-check -- site site/docs    # la documentación como páginas web (GitHub Pages)
```

Las imágenes de la guía de uso las toma `scripts/docs-tour.ps1`. Arranca los servidores de ejemplo y ejecuta la aplicación de escritorio una vez por idioma con `APIX_TOUR=Docs/tour/scenes.toml`: la aplicación prepara cada escena por sí misma (abre solicitudes de la colección de demostración en `Docs/tour/demo`, las envía, abre una ventana) y el script guarda la ventana como `Docs/<idioma>/images/<escena>.png`. `APIX_CONFIG_DIR` guarda los ajustes, el historial y las cookies de la aplicación en una carpeta aparte durante el recorrido, así que los tuyos no se tocan.

## El backend de gpui incluido en el repositorio

El renderizador de texto de Windows de GPUI (crate `gpui-pre-windows` 0.3.9)
colocaba cada tramo de glifos de izquierda a derecha, así que las palabras en
árabe aparecían invertidas. `vendor/gpui-pre-windows` es ese crate con
`DrawGlyphRun` corregido (el cambio está marcado como "Apix patch"), y el
`Cargo.toml` del workspace lo aplica como parche. Se puede quitar cuando se
corrija el crate original.

## Integración continua

`.github/workflows/ci.yml` se ejecuta en cada push y pull request:

| Trabajo | Qué comprueba |
|---|---|
| Formato y documentación | `cargo fmt --check`; `docs-check` (las traducciones siguen a la documentación en inglés) |
| Motor y CLI | `cargo clippy -D warnings` y `cargo test` para todo menos la aplicación de escritorio, en Windows, macOS y Linux |
| Aplicación de escritorio | lo mismo para `apix-desktop` en los tres sistemas (en Linux se instalan antes las bibliotecas de ventanas, fuentes y llavero) |
| GitHub Action | la propia action del repositorio (`uses: ./`) contra `examples/jsonplaceholder`, en Linux y Windows, y que haya escrito sus informes |

`.github/workflows/release.yml` compila los paquetes de versión con una
etiqueta de versión (`v1.2.3`, que debe coincidir con `Cargo.toml`) y los
publica como una versión de GitHub con `SHA256SUMS.txt`; si se lanza a mano,
solo los compila, como artefactos de la ejecución. Windows:
`scripts/package-windows.ps1` (Inno Setup 6, `packaging/windows/apix.iss`:
instalación por usuario o por equipo, PATH y asociación de `.apix` opcionales,
desinstalador). macOS: `scripts/package-macos.sh` compila ambas
arquitecturas, las une con `lipo` en `Apix.app` (firmada ad hoc) y crea un
`.dmg`. Linux (en Ubuntu 22.04, así que basta con glibc 2.35):
`scripts/package-linux.sh` crea el tarball con `install.sh`, el `.deb`
(dependencias de `dpkg-shlibdeps`, más `libvulkan1`) y un `PKGBUILD`
rellenado; `scripts/package-arch.sh` compila con ellos el paquete de Arch en
un contenedor `archlinux`. El `.deb` y el paquete de Arch se instalan en el
workflow y se comprueba que no falten bibliotecas. `.github/workflows/pages.yml`
publica `site/` en https://11r5.github.io/Apix/ .
Cada versión incluye además un cask de Homebrew rellenado (`apix.rb`, a partir
de `packaging/homebrew`) y manifiestos de winget (`AhmetDemir.Apix*.yaml`, a
partir de `packaging/winget`), creados por `scripts/channel-manifests.sh`, para
copiarlos al repositorio tap ([11R5/homebrew-apix](https://github.com/11R5/homebrew-apix))
y a un pull request a `microsoft/winget-pkgs`. `scripts/action-repo.sh` escribe
los archivos de [11R5/apix-action](https://github.com/11R5/apix-action), la
copia de `action.yml` para el Marketplace que siempre descarga una versión
publicada (el Marketplace no lista un repositorio con archivos de workflow);
tras cada versión recibe una etiqueta `v<version>` y `v1` se mueve al nuevo
commit. `.github/workflows/docker.yml` publica la imagen de contenedor con las
etiquetas de versión. La action guarda en caché el `apix` que compila bajo un
hash de las fuentes del motor, así que un commit nuevo lo vuelve a compilar y
uno sin cambios lo reutiliza.

## Pruebas

| Crate | Qué se cubre |
|---|---|
| apix-core | ida y vuelta del modelo y del formato de archivo, variables, valores secretos de los entornos en el llavero (se omite sin almacén de credenciales), firma contra el conjunto de pruebas de AWS, RFC 7616 (Digest), RFC 7636 (PKCE) y el ejemplo de OAuth 1.0; esquemas GraphQL (lectura de introspección, SDL) y lo que se sugiere y describe en cada lugar de una consulta (campos, alias, argumentos, valores de enum, objetos de entrada, listas, fragmentos, tipos de variables, comentarios y cadenas); la tabla `[grpc]`; entradas del historial, llamadas gRPC incluidas; declaraciones de variables GraphQL y autocompletado en las variables (JSON); el ajuste de CA de confianza; filtrado de JSON con JSONPath y jq (coincidencias, salidas en orden, errores con palabras, filtros infinitos detenidos); la query de la URL y sus filas en ambos sentidos, variables de ruta `:name` encontradas y rellenadas (solo en la ruta, codificadas, un segundo relleno sin cambiar nada), ordenar la URL de una solicitud; nombres y valores de encabezados sugeridos (los que empiezan antes que los que contienen, ninguno una vez escrito entero); ejemplos (ida y vuelta de texto y bytes, la solicitud tal como se escribió, sin Set-Cookie); git contra un repositorio real (cambios desde un punto de ramificación, sin confirmar, entre commits, un archivo en un commit, una referencia desconocida), qué afecta un archivo cambiado (scripts, carpetas, entornos, el proyecto, archivos que nombra una solicitud), diferencias de solicitudes (filas por nombre, formato ignorado, cuerpo línea a línea, movimientos); intervalos de monitores e ida y vuelta de `[[monitor]]`; solicitudes del asistente de IA (credenciales ocultas, cuerpos recortados, el idioma, alternativas solo para el modelo predeterminado) y respuestas (bloques de texto, rechazos, tipos de error, el script en un bloque de código); comparación de respuestas (encabezados por nombre dejando fuera los volátiles, JSON campo a campo con los registros emparejados por id, formato ignorado, cuerpos de texto y binarios); registros encontrados en JSON para tablas (envoltorios, valores simples, columnas de números y de etiquetas, texto alineado) |
| apix-http | construcción de solicitudes, todos los tipos de cuerpo, un servidor local para OAuth 2.0, Digest, AWS y OAuth 1.0, la traza de red; el analizador SSE (trozos cortados en cualquier punto), streaming contra un servidor que envía despacio, WebSocket (eco, autenticación y parámetros en el handshake, cierre desde cualquiera de los lados, un handshake rechazado), obtención de un esquema GraphQL con la autenticación de la solicitud y los errores cuando no la hay, HTTPS con una autoridad de certificación propia; subprotocolos WebSocket y mensajes binarios, `wss://` con una CA propia o sin comprobar; lo que un flujo de eventos traslada a una reconexión; paquetes Socket.IO, direcciones del handshake y adjuntos binarios, y contra un servidor Socket.IO 4 local: eventos, acks en ambos sentidos, pings, namespaces con autenticación y rechazos, adjuntos binarios, el servidor haciendo salir al cliente, un servidor WebSocket simple distinguido; pings WebSocket a intervalos con sus pongs cronometrados, un ping sin respuesta que termina la conexión, `ping_interval` en el archivo; detalles TLS del propio handshake de la solicitud contra un servidor HTTPS local (versión, conjunto de cifrado, grupo, la cadena enviada con clave y huella, el tiempo de handshake coincidiendo con la división TCP/TLS); cuerpos gzip, zlib, deflate sin envoltorio y brotli descomprimidos por trozos, cuerpos cortados rechazados, `Accept-Encoding` pedido salvo que la solicitud defina uno, `Content-Encoding` conservado |
| apix-grpc | direcciones, códigos de estado, encabezados y autenticación como metadatos; contra un Greeter local (construido con mensajes dinámicos, con y sin reflection): listar y describir el servicio, los cuatro tipos de método, la autenticación llegando al servidor, un estado de fallo, archivos .proto con imports, y los errores para ninguna descripción, un método desconocido, un mensaje que no encaja, una lista para un método unario, un archivo que falta y ningún servidor; mensajes de ejemplo; una llamada como respuesta HTTP para las ejecuciones (correspondencia de estados, `grpc-status`, un stream como lista); TLS contra un servidor con un certificado de su propia autoridad (rechazado, de confianza con la CA, aceptado sin comprobar); definiciones: salida real de generadores guardada en `tests/fixtures/generated` (protoc 3.19 para Python, Java, C#, C++, PHP; protoc-gen-es; ts-proto) debe devolver el servicio original, Go, Rust, betterproto y las anotaciones dan números y tipos, el mismo tipo leído desde 10 lenguajes, ejemplos JSON, comparación con un servidor, una suposición que cede ante reflection, campos desconocidos señalados, un tipo de respuesta erróneo explicado |
| apix-proto | direcciones, inicios de sesión y opciones de MQTT; contra un broker local: todos los niveles de calidad de servicio, comodines, suscribirse y salir estando conectado, suscripciones rechazadas y topics incorrectos, mensajes retenidos, inicios de sesión y sesiones conservadas, el broker colgando, `mqtts://` con una CA propia o sin comprobar |
| apix-codegen | documentación de la API (carpetas, credenciales ocultas, sin valores de `.local.toml`, descripciones Markdown sin HTML en bruto, anclas); entrecomillado de shell para bash, cmd y PowerShell; pegado de curl; ida y vuelta; URL, encabezados, cuerpos, escape de cadenas y ajustes de cada lenguaje |
| apix-import | fixtures de Postman v2.0/v2.1; WSDL: ambas versiones de SOAP, puertos y direcciones, estilos document y rpc, encabezados, ejemplos a partir del esquema (extensión, enums, choices, `form`, recursión), imports y lo que no se pudo leer, ubicaciones, WSDL 2.0 rechazado |
| apix-script | variables y cambios de la solicitud desde scripts previos a la solicitud, resultados y mensajes de `pm.test` / `pm.expect`, la API antigua `tests[...]`, errores que nombran el archivo, scripts infinitos detenidos, `setNextRequest`, `atob`/`btoa`, `pm.sendRequest` (callback y `await`, sin contar la espera), las bibliotecas integradas contra resúmenes conocidos, comprobaciones sin código, valores y expresiones para destinos de la respuesta |
| apix-runner | archivos de datos CSV (comillas, saltos de línea, `;`, BOM) y JSON, archivos dotenv y nombres `APIX_VAR_*`, número de iteraciones y filas de datos, tiempos de pausa/detención, salida JUnit/JSON/HTML/Markdown y escape (Markdown para pull requests: marcador, escape, lo que falló, una selección con `--changed`), fechas; ejecuciones completas contra un servidor local (filas de datos, orden de la barra lateral, `bail`, `setNextRequest`), iteraciones en paralelo contra un servidor lento (más rápido que una tras otra, listadas por iteración); monitores: cuándo cambia el estado, registros limitados a sus últimos resultados y separados por colección, programaciones que siguen el registro, mensajes de webhook |
| apix-runner (pruebas de carga) | percentiles por rango más cercano, lectura de duraciones; contra un servidor local con keep-alive: un número de solicitudes sobre conexiones mantenidas (alrededor de una por worker para 400 solicitudes), una tasa constante, un límite de tiempo, detener desde otro lugar, errores 500 y conexiones rechazadas contados por tipo |
| apix-runner (flujos) | archivos de flujo leídos y escritos, nodo inicial, aristas reemplazadas y eliminadas, problemas nombrados, sujetos de una respuesta (estado, tiempo, encabezados, JSONPath, jq, nada allí), comparaciones; contra un servidor local: un valor guardado por una solicitud y usado en la URL de la siguiente, un bucle de sondeo con una comprobación y una espera, una solicitud que falla con y sin arista de fallo, un valor que no está, un archivo de solicitud desconocido, un bucle infinito detenido a los 1000 pasos, un flujo que no se puede ejecutar |
| apix-cli (MCP) | el protocolo (versiones, notificaciones, métodos desconocidos, la lista de herramientas con y sin `--list-only`); contra un servidor local: listar solicitudes y entornos (nombres, sin valores), enviar con un entorno y con las variables del agente, las pruebas de los scripts en el resultado, ejecutar todas las solicitudes, rutas fuera de la colección y archivos que faltan rechazados |
| apix-mock | patrones de ruta (`{{base_url}}/users/:id`, hosts, queries), la coincidencia más literal, elección por `x-mock-response-name` y `-code`, 2xx primero, cuerpos descomprimidos sin `Content-Encoding`, la lista del 404, preflights; un servidor real en un puerto respondiendo a través del cliente HTTP, con CORS, y sirviendo un ejemplo añadido mientras se ejecuta |
| apix-capture | las solicitudes guardadas no conservan cookies ni credenciales, filtros de host; a través de servidores locales reales: HTTP simple grabado y guardado, HTTPS abierto con la autoridad local (el cliente confiando solo en ella), túneles HTTPS pasados sin leer |
| apix-route | líneas de tracert (también una traducida), traceroute y tracepath; direcciones locales; la proyección del mapa y las distancias; una base de datos escrita en el formato MaxMind DB: lugares, direcciones que no conoce, privadas, elección del archivo; instalación de una descarga (gzip, rechazada si no es una base de datos, la antigua reemplazada solo después); la herramienta del propio sistema trazando 127.0.0.1 |
| apix-plugins | los complementos de ejemplo cargados con sus manifiestos; la firma HMAC frente a una calculada de forma independiente; importación de CSV y estadísticas de campos; módulos escritos como texto: un bucle infinito detenido, imports rechazados, un nombre incorrecto, un manifiesto que falta, una respuesta fuera de la memoria, un archivo que no es WebAssembly; también ida y vuelta de `[auth.plugin]` (apix-core) y la respuesta de un importador como archivos de solicitud (apix-import) |
| apix-vault | formato de cifrado, manipulación, cambio de contraseña, clave de recuperación, entradas del llavero (contra el almacén de credenciales real con un nombre de servicio de prueba; se omite donde no existe) |
| apix-desktop | traducciones completas, utilidades de idioma, rutas de archivo, consulta de resaltado, archivos de tema (lectura, errores, ida y vuelta), colores de acento y de transparencia, nombres de archivo de esquemas GraphQL, el registro de una llamada gRPC en el historial |
| docs-check | estructura de la documentación y sellos |
