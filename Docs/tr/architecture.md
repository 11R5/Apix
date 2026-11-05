<!-- translated from en/architecture.md sha256:7c5f04d6b9e2b695 -->

# Apix mimarisi

Apix, Rust ile yazılmış native bir API istemcisidir. Arayüz,
[GPUI](https://github.com/zed-industries/zed) (Zed'in arayüz motoru) ve
[gpui-kit](https://github.com/longbridge/gpui-kit) bileşen kütüphanesiyle GPU
üzerinde çizilir. WebView ya da Electron kullanılmaz.

## İlkeler

1. **Önce yerel, git dostu.** Bir koleksiyon, küçük TOML dosyalarından oluşan bir
   klasördür. Hesap ya da bulut zorunluluğu yoktur. Koleksiyonlar kod gibi
   commit'lenir, diff'lenir ve gözden geçirilir.
2. **Arayüz asla bloklanmaz.** Ağ, disk, ayrıştırma ve betik çalıştırma UI
   thread'inin dışında yürür. UI thread'i yalnızca yerleşim (layout) ve çizim yapar.
3. **Tek motor, çok önyüz.** Masaüstü uygulaması, `apix` CLI'ı ve gelecekteki
   önyüzler (bir TUI, bir MCP sunucusu) aynı crate'leri paylaşır; böylece bir istek
   CI dahil her yerde aynı davranır.
4. **Devasa yanıtlara ölçeklenme.** 100 MB'lık bir JSON yanıtı akıcı biçimde
   kaydırılabilmelidir. Yalnızca görünen satırlar yerleştirilir (sanal listeler) ve
   gövde tembel (lazy) ayrıştırılır.
5. **Hafif ve hızlı açılış.** Hız ve özellikler boyuttan önce gelir. Release
   binary'si 100 MB'ın altında kalır (bugün 54.4 MB; Electron uygulamaları 300 MB+)
   ve pencereyi 150 ms'den kısa sürede açar.

## Crate'ler

```
crates/
├── apix-core     istek modeli, {{variables}} ve {{$dynamic}} değerleri, disk formatı,
│                 projeler, ortamlar ve gizli değerler, ağaç ve dosya işlemleri,
│                 GraphQL şemaları (introspection, SDL) ve sorgu tamamlama,
│                 git: dal, değişen dosyalar, bir değişikliğin dokunduğu istekler, istek düzeyinde fark
├── apix-http     HTTP motoru: reqwest + rustls, zamanlamalar, yanıt yakalama, ağ izi,
│                 sunucu olayları (SSE), WebSocket (tokio-tungstenite), Socket.IO, GraphQL şeması çekme
│                 (DNS, bağlantı kurulumu, yönlendirmeler, sertifika, yerel ağ arayüzü)
├── apix-import   Postman, Insomnia, OpenAPI, HAR ve WSDL → Apix koleksiyonları
├── apix-codegen  istekler ↔ kabuk komutları: curl (bash), curl.exe (cmd), PowerShell; yapıştırılan curl'ü ayrıştırır; 10 dilde kod
├── apix-vault    şifreli koleksiyonlar: dosya başına XChaCha20-Poly1305, Argon2id, kurtarma anahtarı
├── apix-script   istek öncesi ve test scriptleri: pm.* uyumlu API ile QuickJS
├── apix-runner   koleksiyon koşuları: sıra, tekrar (yan yana da), veri dosyaları, bekleme, bail,
│                 raporlar (pull request için Markdown da), monitörler: zamanlama, günlük, webhook
├── apix-grpc     gRPC çağrıları: tonic, JSON mesajlar (prost-reflect), .proto dosyaları (protox), koddan ya da JSON'dan tanım
│                 ya da sunucu reflection'ı
├── apix-proto    diğer protokoller: MQTT (rumqttc)
├── apix-mock     kayıtlı örneklerden yanıt veren yerel mock sunucu (hyper)
├── apix-capture  kaydeden proxy: düz HTTP, CONNECT tünelleri, yerel CA ile açılan HTTPS (rcgen, rustls)
├── apix-route    sunucuya giden rota (sistemin traceroute aracı) ve atlamaların yeri (çevrimdışı .mmdb veritabanı)
├── apix-plugins  WebAssembly eklentileri (wasmi): yetkilendirme türleri, içe aktarıcılar, yanıt görünümleri; apix-plugin-sdk ile Rust'ta yazılır
├── apix-cli      `apix` binary'si: terminalden ve CI'dan send / run / diff / schema / import / shell
└── apix-desktop  GPUI masaüstü uygulaması (tek crate'ten Windows, macOS ve Linux)
tools/
└── icongen       tüm uygulama ikonlarını (ico, icns, hicolor PNG'ler) Logo/LogoFirst.jpg'den üretir
```

`apix-desktop/src` içinde: `workspace.rs` (istek düzenleyici, yanıt paneli,
eylemler ve tuş bağlamaları), `workspace/collections.rs` (kenar çubuğu, projeler,
dosya işlemleri, içe aktarma, dosya izleme), `workspace/env_editor.rs`
(değişkenler penceresi), `workspace/auth_editor.rs` (Yetkilendirme sekmesi ve
klasör/koleksiyon/proje yetkilendirme penceresi), `workspace/palette.rs` (komut
paleti ve bulanık (fuzzy) eşleştiricisi), `workspace/network.rs` (yanıtın Ağ
sekmesi), `workspace/runner_ui.rs` (Runner), `workspace/vault_ui.rs` (kilit açma, kilitleme ve otomatik kilit,
şifreleme, şifrelemeyi kaldırma, parola değiştirme, kurtarma anahtarı),
`assets.rs` (gpui-kit ikonları ve iki kilit ikonu), `workspace/menu_bar.rs`
(başlık çubuğundaki Dosya/İstek/Ortam/Görünüm/Yardım; düğmeler başlık çubuğunun
sürükleme alanını `occlude()` ile örter, böylece Windows tıklamayı onlara
iletir), `workspace/tabs.rs` (açık istekler; öndeki sekme düzenleyici
widget'larında yaşar, diğerleri kendi taslağını, yanıtını ve süren görevini
tutar), `workspace/drag.rs` (istekleri ve klasörleri taşımak için sürükleme),
`workspace/shell_copy.rs` (farklı kopyala / curl yapıştır), `workspace/var_chips.rs`
(URL çubuğundaki `{{variable}}` çipleri ve üzerine gelince açılan kartları),
`workspace/graphql_ui.rs` (GraphQL şemasını çekme, saklama, tamamlama ve açıklama),
`workspace/grpc_editor.rs` ve `workspace/grpc_ui.rs` (gRPC isteğinin Mesaj sekmesi ve
yanıt alanındaki çağrı),
`large_view.rs` (devasa gövdeler için satır tabanlı görüntüleyici), `net.rs`
(tokio köprüsü), `bidi.rs` (sağdan sola yerleşim yardımcıları, bkz. Diller), `state.rs` (kullanıcının yapılandırma klasöründe son kullanılan
öğeler ve klasör başına ortam), `theme.rs` ve `fonts.rs`.

### Üç işletim sistemi için tek masaüstü crate'i

GPUI ve gpui-kit aynı kodla Windows, macOS ve Linux'ta çalışır; Zed de bu şekilde
dağıtılır. Bu yüzden işletim sistemine özel arayüz klasörleri yoktur. Gerçekten
işletim sistemine göre değişen kod,
`crates/apix-desktop/src/platform/{windows,macos,linux}.rs` içinde `cfg` arkasına
konur: başlık çubuğu ve pencere kontrolleri, blur/vibrancy, macOS menü çubuğu,
`.apix` için dosya ilişkilendirmeleri ve anahtar zinciri (keychain) erişimi.
Kurulum paketleri ve ikonlar `packaging/{windows,macos,linux}/` altındadır. Ayrı
bir arayüz ancak bir işletim sistemi bir gün gerçekten farklı bir arayüz
gerektirirse ayrılır.

Bağımlılık yönü: `core` ← `http`/`script`/`proto` ← `cli`/`desktop`.
Hiçbir şey `desktop`'a bağımlı değildir.

## Bir isteğin veri akışı

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                     istek öncesi betik (ileride) ◀─────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio runtime, arka plan thread'i)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                         test betiği (ileride) ◀────────┤
                                                        ▼
                Arayüz: durum satırı, sanallaştırılmış gövde görüntüleyici
```

`{{variables}}` katmanlar hâlinde çözümlenir. Sonraki katmanlar kazanır:
**global değişkenler → koleksiyon (`apix.toml`) → ortam → geçersiz kılmalar (CLI `--env-file`, `APIX_VAR_*` ve `--var`, koleksiyondaki adlarla büyük küçük harf gözetmeden eşleşir; betik `pm.variables.set`)**.
Tanımsız adlar olduğu gibi bırakılır ve raporlanır; böylece arayüz bunların altını
çizebilir, CLI ise göndermeyi reddedebilir.

### Ağ izi

Her gönderim kendi kısa ömürlü reqwest istemcisini kurar; böylece kaydedilen her
şey o isteğe aittir (`apix_http::trace`). Bir çözümleyici DNS'i, reqwest'in
bağlayıcısının etrafındaki bir Tower katmanı her bağlantıyı zamanlar,
yönlendirme politikası her adımı not eder. reqwest TLS bağlantısını dışarı
vermez, kendi bağlayıcımızın döndürmesine de izin vermez; bu yüzden TLS
ayrıntıları rustls'un içinden, Apix'in kendi kurduğu bir istemci
yapılandırmasıyla gelir (`use_preconfigured_tls`, `tls::traced_config`,
kancalar `tls_trace.rs` içinde):

- saran bir sertifika doğrulayıcı sunucunun gönderdiği zinciri ve hangi
  sürümün imzasını denetlediğini (TLS 1.2 ya da 1.3) kaydeder, kararı olağan
  doğrulayıcıya (denetim kapalıysa "hepsini kabul et"e) bırakır;
- hiçbir şey saklamayan bir oturum deposuna istemci merhabası yazılırken
  anahtar değişimi ipucu sorulur; bu, TLS el sıkışmasının başlangıcıdır:
  bağlantının ondan önceki süresi (DNS'ten sonra) TCP, sonrası TLS'tir; depoya
  sunucunun seçtiği grup da bildirilir (TLS 1.3);
- şifre takımları rustls'un kendileridir; bir kez, şifrelemelerinin etrafında
  hangi takımın kurulduğunu not eden ince bir sarmalayıcıyla yapılır. Takımlar
  `'static` ve ortak olduğu için sarmalayıcı isteği, isteğin future'ı poll
  edilirken atanan bir thread-local üzerinden bulur (`Scoped`); el sıkışma o
  poll'un içinde çalışır.

ALPN yanıtın sürümünden çıkar (TLS üzerinden HTTP/2 yalnızca `h2`'de
anlaşıldıktan sonra konuşulur). Sıkıştırılmış gövdeleri reqwest değil Apix açar
(`decode.rs`; gzip, zlib ya da ham deflate, brotli, geldikçe); reqwest
`Content-Encoding`'i ve ağdaki boyutu düşürürdü.

## Disk formatı

```
my-api/
├── apix.toml               ad + koleksiyon düzeyinde [vars] ve [auth]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         isteğe bağlı: bu klasördeki istekler için [auth]
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Yetkilendirme devralma.** `auth` içermeyen bir istek, yetkilendirmeyi
`folder.toml` dosyasında bir yetkilendirme ayarlayan en yakın klasörden, sonra
koleksiyondan (`apix.toml`), sonra projeden (`apix-project.toml`) devralır;
hiçbiri yoksa hiçbir şey gönderilmez. Bir istekte (veya klasörde)
`auth = "none"` devralmayı durdurur. Bu aramayı masaüstü uygulaması, CLI ve
"Komut olarak kopyala" için aynı şekilde `apix_core::project::inherited_auth` yapar.

**İstek ayarları.** Aynı dört düzeyde bir `[settings]` tablosu bulunur (zaman
aşımı, yönlendirmeler, TLS doğrulama, proxy, istemci sertifikası, güvenilen
CA, çerezler).
Her alan bir `Option`'dır: `None`, aynı zincir üzerinden alan alan devralır
(`apix_core::project::inherited_settings`); yine boş kalanlar
`RequestSettings::effective` içindeki varsayılanlara düşer — zaman aşımı yok,
en çok 10 yönlendirme izlenir, TLS doğrulanır, proxy ortamdan, çerezler açık.
Motor sonucu her gönderimin `reqwest::Client`'ını kurarken uygular (güvenilen
CA dosyası sertifikalarını kök olarak ekler); paylaşılan
çerez kavanozu (çerez yöneticisinin listeleyip düzenleyebildiği bir
`cookie_store`) istek çerezleri kapatmadıkça o istemciye verilir.

**İmzalı yetkilendirme.** AWS Signature V4, OAuth 1.0a ve Digest,
`apix_core::sign` içinde saf fonksiyonlardır; AWS test paketine, RFC 7616'ya,
RFC 7636'ya ve klasik OAuth 1.0 örneğine karşı doğrulanır, böylece motor ve
"Komut olarak kopyala" aynı şekilde imzalar. Motor önce `reqwest::Request`'i oluşturur,
sonra onu imzalar (AWS payload hash'i son gövdeye ihtiyaç duyar; multipart akışı
`UNSIGNED-PAYLOAD` olarak gönderilir). Digest bir kez gönderir, 401 yanıtındaki
sınamayı (challenge) okur ve aynı bağlantı üzerinden yanıtlar. OAuth 2.0
`apix_http::oauth2` içindedir: token göndermeden önce alınır (istemci kimlik
bilgileri, parola veya PKCE ile yetkilendirme kodu; bu sonuncusunda tarayıcı
127.0.0.1 üzerindeki tek seferlik bir dinleyiciye geri döner), sunucu, istemci,
grant, kapsam ve kullanıcı başına bellekte tutulur, refresh token'ı ile yenilenir
ve asla diske yazılmaz.

Bir istek dosyası:

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/orgs/:org/users"   # sorgu yok: o [[query]] içinde

[[query]]
key = "notify"
value = "yes"

[[path]]                 # :org parçasının değeri
key = "org"
value = "{{org}}"

[[example]]              # istekle birlikte tutulan bir yanıt
name = "Created"
status = 201
body = '{ "id": 7 }'
[example.request]        # geldiği istek, yazıldığı hâliyle
method = "POST"
url = "{{base_url}}/orgs/:org/users?notify=yes"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # dosyada tutulur, gönderilmez

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"

[[assert]]               # kod yazmadan doğrulama, test scriptlerinden sonra çalışır
target = "status"
op = "=="
value = "201"
```

**Doğrulamalar.** `[[assert]]` satırları bir hedefi (`status`, `time`, `size`,
`body`, `body.<yol>`, `header.<ad>`) bir değerle `==`, `!=`, `>`, `>=`, `<`,
`<=`, `contains`, `not contains`, `exists`, `not exists`, `matches` ya da `is`
(bir JSON türü) ile karşılaştırır. `apix_script::assertions_script` bunları
bir test scripti daha olarak üretir, böylece her yerde `pm.test` gibi
raporlanırlar.

**Gövdeler.** `[body]` şunlardan birini içerir: `json`, `text`, `xml`, `form`
(URL-encoded satırlar), `multipart` (dosya alanları için `file = true` içeren
satırlar ve isteğe bağlı bir `content_type`), `binary = { file = "…" }` ya da
`graphql` (`query` ve `variables`). Dosya yolları değişken içerebilir ve istek
dosyasına göre görelidir; `Request::with_files_relative_to`, değişkenler
çözümlendikten sonra hem uygulamada hem CLI'da bunları mutlak yola çevirir. Motor
dosyaları yalnızca gönderirken okur; bu yüzden eksik bir dosya yükleme hatası
değil, gönderim hatasıdır. GraphQL gövdesi `{"query", "variables"}` JSON'u olarak
gönderilir; geçersiz değişkenler gönderimi durdurur. Multipart gövdede, motorun
boundary'sini içermeyen bir `Content-Type: multipart/…` başlığı (Postman dışa
aktarımlarında yaygındır) atılır, böylece gövde okunabilir kalır.

**gRPC.** Adresi `grpc://host:port` (TLS için `grpcs://`) olan bir istek,
çağıracağı metodu `[grpc]` içinde adlandırır; JSON gövde mesajdır, akış alan
bir metotta JSON listesi bir mesaj akışıdır. `protos` yoksa sunucuya sorulur
(reflection). Yollar istek dosyasına göre görelidir.

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # isteğe bağlı
import_paths = ["../protos"]              # isteğe bağlı
```

Dosyalar yerine `definition` tanımı isteğin içinde tutar: `.proto` metni ya da JSON
olarak descriptor set. `protos`'tan önce gelir. `guessed = true`, tiplerden ya da
JSON'dan çıkarılmış tanımı işaretler: sunucu reflection'ı ondan önce gelir ve farklar
söylenir. `protos` ikili ya da `.json` descriptor set de kabul eder.

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

Açık sorular:
- **Gizli değerler.** Token gibi ortam değerleri commit'lenmemelidir.
  `environments/<name>.local.toml`, `<name>.toml` üzerine katmanlanır. Böyle bir
  dosya yazıldığında, eksikse `environments/` yanındaki `.gitignore` dosyasına
  `*.local.toml` eklenir. Bir gizli değer paylaşılan dosyada bir yer tutucuyla
  kalabilir; böylece ekip arkadaşları adını görür. İstenirse (Ayarlar › Ortam
  gizli değerleri) gizli değerler işletim sistemi anahtarlığına gider:
  `<name>.local.toml` yalnızca `keychain = "<kimlik>"` satırını tutar, değerler
  `Apix` servisindeki `env:<kimlik>` kaydında durur
  (`apix_vault::keychain::save_text`; Windows'un 2560 baytlık sınırı için
  birden çok kayda bölünür). Yeni gizli değerlerin nereye yazılacağına
  `apix_core::secrets::use_keychain` karar verir; okuma dosyayı izler, bu yüzden
  CLI'nin bir ayara ihtiyacı yoktur. Kimlik dosyayla birlikte taşındığından
  ortamı yeniden adlandırmak ya da klasörü taşımak bağlantıyı bozmaz. Şifreli
  koleksiyonlarda gizli değerler kasada kalır.
- **Dinamik değişkenler.** `{{$name}}`, bu adla tanımlı bir değişken yoksa
  `apix_core::dynamic` tarafından doldurulur; her geçişte yeni bir değer üretilir
  (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …). Ek crate yok:
  küçük bir xorshift üreteci ve elle yazılmış bir UTC tarih biçimlendiricisi.
- **Kenar çubuğunda sıralama.** Kaydedilmiş bir şey yoksa önce klasörler, sonra
  istekler gelir; her biri ada göre. Sürüklemek, klasörün `folder.toml`
  dosyasına (ya da koleksiyonun `apix.toml` dosyasına) alt öğe adlarından oluşan
  bir `order` listesi kaydeder; `store::place` bir öğeyi diğerinin önüne ya da
  arkasına koyar, yeniden adlandırma ve taşıma listeyi günceller. Listenin
  bilmediği adlar varsayılan sırayla arkadan gelir, böylece yeni bir dosya hiç
  kaybolmaz. Projedeki koleksiyonlar `apix-project.toml` içindeki `order`
  alanını kullanır (`Project::place_collection`). Postman içe aktarıcısı
  `order` alanını yalnızca Postman'ın sırası varsayılandan farklıysa yazar.

### Projeler

Bir proje koleksiyonları gruplar ve hepsinin paylaştığı ortamlara sahiptir:

```
shop/
├── apix-project.toml       ad, paylaşılan [vars], koleksiyon sırası, group_by, pins, colors
├── environments/           test.toml, prod.toml (+ *.local.toml gizli değerleri)
├── users/                  bir koleksiyon (apix.toml + .apix dosyaları)
└── payments/               bir koleksiyon; bazı ortamlara sabitlenebilir
```

`[colors]` bir koleksiyon klasörünü bir renk adıyla eşler (`red`, `orange`,
`yellow`, `green`, `teal`, `blue`, `purple`, `pink`); uygulama bunu kenar
çubuğunda ve sekmelerde nokta olarak çizer. `project::move_collection_into` bir
koleksiyon klasörünü projeye taşır (farklı sürücüde kopyalayıp siler).

Değişken katmanları, sonraki kazanır: proje değişkenleri → koleksiyon değişkenleri
→ projenin ortamı → koleksiyonun aynı adlı ortamı (her biri kendi `.local.toml`
gizli değerleriyle). CLI (`apix run --env`) ve masaüstü uygulaması aynı fonksiyonu
kullanır: `apix_core::project::variables`. `group_by = "environment"` kenar
çubuğunu Proje → Ortam → Koleksiyonlar şeklinde düzenler; `[pins]` ise bir
koleksiyonu bazı ortamlarla sınırlar.

### Şifreli koleksiyonlar (M4)

Format ve motor `crates/apix-vault` içindedir. Apix'in geri kalanı bunlara,
`std::fs` ile aynı çağrılara sahip ince bir katman olan `apix_core::vfs`
üzerinden erişir: kilidi açık bir vault'un kökü altındaki bir yol, bellekte şifresi
çözülerek vault'tan sunulur; diğer her yol diske gider. Böylece store, kenar
çubuğu, sekmeler ve CLI sıradan yolları kullanmaya devam eder ve şifresi
çözülmüş hiçbir şey diske yazılmaz. Kilidi açık vault'lar süreç genelindeki bir
registry'de tutulur; kilitleme (elle, 15 dakika boşta kalındıktan sonra veya
çıkışta) vault'u bırakır ve anahtarını siler. Şifreli bir koleksiyonun içinde
silme kalıcıdır: sistem çöp kutusu şifresi çözülmüş dosyaları tutardı.

```
my-api/
├── apix-vault.toml   düz metin üst bilgi: format, Argon2id maliyeti ve salt'lar,
│                     parola ve kurtarma anahtarıyla sarılmış veri anahtarı
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   her dosyanın gerçek yolu, şifreli
    ├── 3f9c…e1.apixe her biri tek bir şifreli dosya; rastgele adlar, düz (klasör yapısı yok)
    └── local/        *.local.toml gizli değerleri, aynı format, asla commit'lenmez
```

- Rastgele 256 bitlik bir **veri anahtarı** her dosyayı XChaCha20-Poly1305 ile ve
  her yazımda yeni bir nonce ile şifreler. Nesne kimliği ilişkili veri (associated
  data) olarak kullanılır; böylece blob'lar fark edilmeden yer değiştirilemez
  veya yeniden adlandırılamaz.
- Veri anahtarı iki kez saklanır: **paroladan** türetilen bir anahtarla
  (Argon2id, 64 MiB, 3 geçiş) ve **kurtarma anahtarından** türetilen bir anahtarla
  (160 rastgele bit, bir kez `XXXX-XXXX-…` biçiminde Crockford base32 olarak
  gösterilir) sarılmış olarak. Parolayı değiştirmek anahtarı yeniden sarar;
  dosyalar yeniden şifrelenmez.
- Yeniden adlandırma ve taşıma yalnızca şifreli dizine dokunur.
- Kilidi açılmış anahtar yalnızca bellekte yaşar (`zeroize` kilitlemede onu siler).
- Git yine de hangi nesnenin değiştiğini görür, ama içinde ne olduğunu asla
  görmez. Güvenlik formatın gizli olmasından değil, şifreleme algoritmasından ve
  paroladan gelir; Apix açık kaynaktır.
- Parola işletim sisteminin kimlik bilgisi deposunda hatırlanabilir
  (`apix_vault::keychain`, `keyring` crate'i üzerinden: Credential Manager,
  macOS Anahtar Zinciri, Secret Service). Her kasa için `Apix` servisi altında
  bir kayıt tutulur; adı `vault:<mutlak klasör yolu>` (Windows'ta küçük harf).
  Kilit açma penceresinden isteğe bağlı açılır; uygulama ve CLI sormadan önce
  onu dener, kasayı artık açmayan bir parola silinip yeniden sorulur, parola
  değişince kayıt güncellenir, şifre kaldırılınca silinir. Depo yoksa Apix
  eskisi gibi parolayı sorar.

## Scriptler (M5)

`apix_core::scripts` bir isteğin scriptlerini bulur (`_collection.<olay>.js`,
her klasörün `_folder.<olay>.js` dosyası, sonra `<istek>.<olay>.js`; olaylar
`prerequest` ve `test`) ve scriptlerin kapsam kapsam atadığı ya da sildiği
değerleri `ScriptVars` içinde tutar. `ScriptVars::apply` bunları isteğin
`Variables` katmanlarının üstüne koyar, böylece çözümleme eskisi gibi çalışır.
Scriptlerin atadığı hiçbir şey diske yazılmaz.

`apix-script` bir olayın scriptlerini yeni bir QuickJS çalışma zamanında
çalıştırır (`rquickjs`; 10 sn ve 256 MB sınırı). `pm` nesnesi JavaScript ile
yazılmıştır (`prelude.js`): Rust bağlamı JSON olarak verir (çözülmemiş istek,
testler için yanıt, `project::scope_values` ile kapsam değerleri, önceki
`ScriptVars`, `pm.info`), scriptler birer fonksiyon içine sarılıp sırayla
çalışır ve `__apix_output()` değişen isteği ve değişkenleri, test sonuçlarını,
konsol satırlarını, `setNextRequest` ve `skipRequest` bilgisini JSON olarak
döndürür. API'nin JavaScript'te durması Rust tarafını birkaç çağrıya indirir ve
Postman davranışını yakalamayı kolaylaştırır.

`pm.sendRequest`, isteği JSON olarak yerel `__apix_send` fonksiyonuna verir;
`apix_script::run`'ı çağıran taraf isteği gönderen ve yanıt gelene kadar
bekleyen bir `Sender` sağlar (CLI script thread'inde yeni bir çalışma
zamanıyla, uygulama kendi ağ çalışma zamanıyla). Scriptler async fonksiyonlara
sarılır, böylece hem geri çağırma stili hem `await` çalışır ve beklenen süre
sınıra sayılmaz. `require` `libs.js` dosyasından gelir: crypto-js (özetler ve
HMAC'ler `__apix_hash` / `__apix_hmac` üzerinden Rust'ta hesaplanır), uuid,
lodash, moment, ajv, tv4 ve chai'nin Apix için yazılmış alt kümeleri;
indirilmezler.

Scriptli bir gönderim: istek öncesi scriptler, sonra `ScriptVars::apply` ve
çözümleme, HTTP isteği, ardından `ResponseView` ile test scriptleri.
`apix_runner::exchange` bunu `apix send`, `apix run` ve Runner için yapar;
masaüstü uygulamasındaki tekil gönderim ise `script_run.rs` içinde,
scriptleri arka plan thread'inde çalıştırarak ve isteğin kendi scriptlerini
kaydedilmiş olsun olmasın editörden alarak yapar.

`apix_runner::run` bir `Plan` alır (sıralı istek dosyaları, değişkenler ve
script kapsamları, veri satırları, tekrar sayısı, bekleme, `bail`, bağlantı
ayarı geçersiz kılmaları, başlangıçtaki script durumu) ve onu dolaşır: her
tekrarda veri satırını (ardından `--var` değerlerini) değişkenlerin ve
`pm.iterationData`'nın üstüne koyar, `setNextRequest`'i istek adına göre
izler, istekler arasında bekler ve `Progress` (tekrar, gönderiliyor, istek
bitti) olaylarını bir geri çağırmaya bildirir. Bir `Control` onu başka bir
thread'den duraklatır ya da durdurur. Sonuç `ItemResult`'lardan oluşan bir
`RunReport`'tur; `report.rs` bunu JSON, JUnit XML (istek ve tekrar başına bir
suite, test başına bir case, testi olmayan istekler için bir durum case'i) ve
kendi başına açılan bir HTML sayfası olarak yazar. `data.rs` CSV (RFC 4180,
BOM, `;` başlıklar) ve JSON dizilerini okur. CLI ilerlemeyi geldikçe yazar;
masaüstü Runner'ı (`workspace/runner_ui.rs`) koşuyu ağ çalışma zamanında
yürütür ve ilerlemeyi paylaşılan bir `Live` yapısından saniyede birkaç kez
okur; görünüm workspace'te kaldığı için koşu penceresinden uzun yaşar.

## Akışlar ve WebSocket (M6)

`HttpClient::send_with`, durum ve başlıklar gelir gelmez `Progress::Head`'i,
sonra gövdenin her parçasını `Progress::Chunk` olarak bildirir; `send` hiçbir
şey dinlemeyen bir `send_with`'tir ve yanıt yine bütün gövdeyi tutar.
`apix_http::sse`, `text/event-stream` gövdelerini parça parça okur
(`SseParser`: her satır sonu biçimi, parça her yerden bölünebilir, `id`
sonraki olaylara taşınır, bitmemiş son olay atılır). Ayrıştırıcı yeniden
bağlanmak için son kimliği ve son `retry:` değerini de tutar:
`reconnected()` bir sonraki bağlantı için ikisini de taşıyan bir ayrıştırıcı
başlatır, `reconnect_request` de `Last-Event-ID` ekler. Beklemeyi ve yeniden
göndermeyi uygulama ve CLI kendileri yapar; böylece Esc ya da Ctrl+C her
zaman durdurur.

`apix_http::ws::connect`, el sıkışmayı isteğin HTTP tarafının göndereceği
biçimden kurar (`prepare`: parametreler, başlıklar, yetkilendirme; OAuth 2.0
önce bearer token'a döner), `[websocket] subprotocols`'tan
`Sec-WebSocket-Protocol` ekler ve tokio-tungstenite ile bağlanır. `wss://`
rustls ayarını `apix_http::tls::client_config`'ten alır (bilinen kökler ve
güvenilen CA, istemci sertifikası ya da her sertifikayı kabul eden bir
doğrulayıcı); gRPC de doğrulamasız bağlantıda aynı modülü kullanır.
`WsConnection::run` bir `WsCommand` kanalının istediklerini gönderir ve iki
taraftan biri kapatana kadar `WsEvent`'leri (gönderildi, geldi, kod ve
nedenle kapandı, hata) bildirir; sunucunun ping'lerini tungstenite yanıtlar.
`[websocket] ping_interval` varsa kendisi de yükü bir sayaç olan ping'ler
gönderir, eşleşen pong'un gidiş-dönüş süresini bildirir (`WsEvent::Pong`) ve
sıradaki ping zamanı geldiğinde hâlâ yanıtlanmamış bir ping varsa hatayla biter. Bir
mesajın `MessageFormat`'ı (metin, base64, hex), değişkenler doldurulduktan
sonra metnini `WsData`'ya çevirir.

Masaüstü uygulamasında ağ çalışma zamanı paylaşılan yapıları doldurur (bir
gönderim için `net::Incoming`, bir soket için `net::SocketIncoming`) ve bir
arayüz görevi onları 60–80 ms'de bir okur. Olay akışı olan bir yanıt
`ResponseState::Streaming` olur (`workspace/stream_ui.rs`): yukarı
kaydırılmadıkça yeni olayları izleyen canlı bir liste; Esc gelenleri bitmiş
bir yanıta çevirir. `ws://` adresi istek alanını Mesajlar sekmesine
(`workspace/ws_editor.rs`), yanıt alanını `ResponseState::Socket`'e
(`workspace/socket_ui.rs`) geçirir. İki durum da bitmiş bir yanıt gibi kendi
sekmesine yönlendirilir; bu yüzden akış ya da soket arkadaki bir sekmede de
sürer. CLI olayları ve mesajları aynı geri çağırmalardan yazar.

## Socket.IO (M6)

`apix_http::socketio`, Socket.IO 3/4'ü (Engine.IO protokolü 4) yukarıdaki
WebSocket bağlantısının üstünde konuşur; el sıkışma, başlıklar, auth ve TLS
ayarları aynıdır. `[socketio]` tablosu (`SocketIoOptions`: ad alanı, yol,
auth) olan istek Socket.IO'dur. `handshake_request` `http(s)://` adresini
`ws(s)://` yapar, ad alanını seçeneklerden ya da URL'nin yolundan alır,
yerine sunucunun yolunu (`/socket.io/`) koyar ve
`EIO=4&transport=websocket` ekler. `connect` Engine.IO'nun açılış paketini
bekler, ad alanının CONNECT paketini auth JSON'uyla gönderir ve yanıtı
(soket kimliğiyle `40…` ya da ret nedeniyle `44…`) bekledikten sonra döner;
böylece yanlış sunucu, ad alanı ya da token başarısız bir el sıkışma gibi
hata verir.

`SioConnection::run`, `WsConnection::run`'ı iki kanal arasındaki bir
çeviricinin yanında çalıştırır: `SioCommand::Emit` bir `EVENT` paketi
(`42…`; istenirse ack id'siyle, id'ler 0'dan sayılır) ya da `BINARY_EVENT`
(`45n-…`) olur; ikincisinde `{"$binary": "<base64>"}` değerlerinin yerine
yer tutucular konur ve baytlar paketin ardından ikili çerçeveler olarak
gider. Gelen paketler `SioEvent`'e dönüşür (sunucunun istediği ack id'siyle
gelen olaylar, bizim olaylarımızın ack'leri, sunucunun bizi ad alanından
çıkarması, okunamayan çerçeveler); ikili ekler toplanıp yer tutucularının
yerine konur. Engine.IO ping'lerine (`2`) pong (`3`) gider.

Masaüstü uygulaması ikisi için tek Mesajlar sekmesi kullanır: `WsEditor`'da
WebSocket/Socket.IO seçimi ve Socket.IO alanları vardır, `WsMessage` da
`event` ve `ack` taşır. `net::SocketIncoming` olay türüne göre geneldir;
`socket_ui.rs` iki tür bağlantıyı da tek döngüyle izler ve `SioEvent`'leri
olay adı ve ack etiketli günlük satırlarına çevirir.
`apix-http/tests/common/socketio_server.rs` içindeki test sunucusu aynı
zamanda `socketio_server` örneğidir (:8771).

## MQTT (M6)

`apix_proto::mqtt`, rumqttc'nin olay döngüsünü sarar (MQTT 3.1.1; TLS yalnızca
`apix_http::tls::client_config`'ten gelen yapılandırmayla, böylece kendi kripto
sağlayıcısını getirmez). `grpc://` gRPC'de olduğu gibi `mqtt://` ya da
`mqtts://` adresi isteği MQTT yapar; `[mqtt]` istemci kimliğini, oturumu, keep
alive'ı ve abonelikleri tutar. `connect` olay döngüsünü broker'ın CONNACK'ine
kadar çalıştırır; böylece reddedilen giriş başarısız bir el sıkışma gibi hata
verir.

`MqttConnection::run` iki future'ı yan yana çalıştırır ve olay döngüsünün
poll'unu hiç iptal etmez: biri `MqttCommand`'ları istemciye verir (kuyruğu
doluysa bekleyebilir, o kuyruğu da poll boşaltır), diğeri bağlantıyı izleyip
`MqttEvent` bildirir. rumqttc giden her paketin kimliğini teslim sırasıyla
bildirir; bir kuyruk yayın ve abonelikleri kimlikleriyle, broker'ın PUBACK,
PUBCOMP, SUBACK ve UNSUBACK'lerini de onayladıkları şeyle eşleştirir. Mesaj,
paketi çıktığında gönderilmiş sayılır. Geçersiz konu ya da reddedilen abonelik
bir olaydır ve bağlantı sürer; kapatma ve broker'ın bağlantıyı kesmesi bağlantıyı
bitirir.

Masaüstü uygulamasında MQTT alanları da `WsEditor`'dadır (URL karar verir,
`Workspace` ona bildirir); `socket_ui.rs` bağlantıyı WebSocket ve Socket.IO ile
aynı döngüyle izler ve `MqttEvent`'leri konu etiketli günlük satırlarına çevirir.
`apix-proto/tests/common/broker.rs` içindeki test broker'ı (joker konular, QoS
0–2, retained mesajlar, retler, herhangi bir akış üzerinden TLS) aynı zamanda
`mqtt_broker` örneğidir (:8772).

## SOAP (M6)

SOAP yeni bir istek türü eklemez: `apix_import::wsdl` WSDL 1.1'i XML gövdeli
normal POST isteklerine çevirir, böylece istek üzerinde çalışan her şey bunlarda
da çalışır. `apix_core::soap` sürümlerin farklı olduğu şeyleri (başlıklar, zarf)
tutar ve hataları okur; `Response::soap_fault` bunu XML yanıtlara uygular, koşucu
da testi olmayan isteği durumu ne olursa olsun bu hatada başarısız sayar.

İçe aktarıcı hiçbir şey indirmez. `Loader`, `wsdl:import`, `xsd:import` ve
`xsd:include` zincirini izler: sıradaki konumu söyler ve metni geri alır; CLI ve
masaüstü uygulaması dosyayı ve ağı kendi yollarıyla okur. Konumlar kendi
belgelerine göre çözülür, ağdan gelen belge yerel dosyaya yönlendiremez, okuma 64
belgede durur. `Model` ardından tüm belgelerden mesajları, port tiplerini,
bağlamaları, portları ve şema öğelerini ad alanı ve adla toplar. `Writer` XSD
düğümlerinden örnekleri üretir (roxmltree düğümleri tutar ve `önek:ad`'ı düğümün
kendi bildirimlerine göre çözer) ve her öğenin nitelikli olup olmadığına `form`,
`elementFormDefault` ve genel olup olmamasına göre karar verir. Kullandığı ad
alanları zarfa yazılır. Adlı tiplerin yığını özyinelemeyi durdurur.

Uygulama ve CLI'daki URL içe aktarma metni okur, sonra dosyayla aynı yolu izler;
bu yüzden her biçimde çalışır. `apix-import/tests/fixtures/` içindeki test
WSDL'leri `soap_server` örneğini de besler (:8773).

## GraphQL şemaları (M6)

`apix_core::graphql` standart `INTROSPECTION_QUERY`'yi, onun cevabından
okunan bir `Schema`'yı (alanları, argümanları, girdi alanları, enum
değerleri, arayüzleri ve olası türleriyle türler; `TypeRef` listeleri ve
`!`'i tutar) ve kök türleri başa yazan, yerleşik skalerleri ve `__` türlerini
dışarıda bırakan `to_sdl`'i tutar. Düzenleyici için bir sorguyu yalnızca
imlece kadar okur: `spot_at` metni parçalara ayırır (yorumlar, metinler ve
`$değişkenler` atılır) ve seçimler, argüman listeleri, girdi nesneleri ve
listelerden oluşan bir yığında ilerler; takma adları, `... on Tür`'ü,
fragment'leri ve değişken tanımlarını izleyerek oraya neyin uyduğunu söyler
(`Spot`). `suggestions` bunu türleri ve açıklamalarıyla etiketlere çevirir;
önce önekle, sonra içerikle eşleşenler, kullanımdan kalkmışlar en sonda.
`describe_at` üzerine gelme için Markdown'u verir. Hiçbir şey doğrulanmaz:
bozuk bir sorgu da elinden geldiğince tamamlanır. Değişkenler düzenleyicisi
de tamamlanır: `variable_types` `$ad: Tür` tanımlarını okur, küçük bir JSON
tarayıcısı imlece kadar olan anahtar ve liste öğesi yolunu ve oraya anahtar
mı değer mi geleceğini bulur, `variable_suggestions` da bu yolu girdi
alanları ve liste öğeleri üzerinden izleyerek henüz verilmemiş değişkenleri,
girdi alanlarını, enum değerlerini (JSON metni olarak) ve `true`/`false`'u
önerir; yazılmakta olan bir metin, düzenleyicinin kapattığı tırnak dahil
tırnaklarıyla değiştirilir.

`apix_http::graphql::fetch_schema` introspection sorgusunu isteğin adresi,
başlıkları, yetkilendirmesi ve ayarlarıyla POST olarak gönderir (istek önce
çözülür) ve şemayı okur ya da neden olmadığını söyler (2xx olmayan durum ve
gövdenin başı, ya da sunucunun `errors[0].message`'ı).

Uygulamada `workspace/graphql_ui.rs`, sorgu düzenleyicisinin tuttuğu ortak
bir `Rc<RefCell<Option<Rc<Schema>>>>` üzerinde gpui-kit'in
`CompletionProvider` ve `HoverProvider`'ını uygular; tamamlamalar yazılan
kelimeyi bir LSP metin düzenlemesiyle değiştirir. Gövde düzenleyicisinin
**Şemayı getir** düğmesi `FetchSchema` yayar; çalışma alanı isteği bir
gönderimdeki gibi kurar (devralınan yetkilendirme ve ayarlar, değişkenler) ve
ağ çalışma zamanında çeker. Her uç noktanın cevabı
`<config>/Apix/graphql/<adresin FNV-1a'sı>.json` içinde saklanır ve bir
GraphQL isteği açılınca yüklenir. SDL görünümü salt okunur bir
düzenleyicidir; yalnızca gösterildiğinde, çizim sırasında doldurulur.

## gRPC (M6)

`apix-grpc` çağrıları üretilmiş kod olmadan yapar. Servis tanımı bir
`prost_reflect::DescriptorPool`'dur: ya isteğin `.proto` dosyalarından protox
ile (Rust; `protoc` gerekmez; import'lar `import_paths` içinde ve her dosyanın
yanında aranır, Google'ın bilinen türleri yerleşiktir), ya da sunucudan gRPC
reflection ile (önce v1, sonra v1alpha; sunucunun atladığı dosyalar adıyla
istenir, sonra bağımlılık sırasına göre eklenir). Mesajlar `DynamicMessage`
olur: JSON, prost-reflect'in serde desteğiyle girer; `.proto` alan adlarıyla
ve varsayılan değerler de gösterilerek JSON olarak çıkar. `DynamicCodec`
bunları tonic'e bağlar; böylece `tonic::client::Grpc` dört metot türünü de
(tekli, sunucu akışı, istemci akışı, çift yönlü) tek bir `Channel` üzerinde
çalıştırır (HTTP motorunun zaten kullandığı hyper ve rustls üzerinde HTTP/2).

`apix_grpc::convert` elde başka ne varsa ondan tanım üretir ve sırayla dener: JSON
descriptor set; `.proto` metni; üretilmiş kodun gömdüğü `FileDescriptorProto`
(`embedded.rs` her üreticinin bu baytları nasıl yazdığını bilir; PHP'ninki geçerli metin
bile olmayabilir); sonra programlama dilindeki bildirimler (`source.rs`: yorumlar atılır,
bloklar bulunur, üyeler dil ailesine göre okunur, türler normalleştirilir, numaralar
işaretlemelerden ve ts-proto'nun kodlayıcılarından alınır) ya da örnek JSON (`json.rs`,
`serde_json::Value`'nun kaybedeceği anahtar sırasını korur). Son ikisi `.proto`'ya
çevrilen küçük bir `Schema`'dan geçer. `printer.rs` kesin tanımı `.proto` metni olarak geri
yazar ve yalnızca bu metin aynı tanıma derlenirse ona güvenilir; yoksa ya da birden çok
dosyada tanım JSON descriptor set olarak kalır. `compare` tanımı sunucununkiyle karşılaştırır;
mesaj adları tele çıkmadığı için alanları adla eşler. `described` sırayı tutar: isteğin tanımı,
dosyaları, reflection; tahmin önce reflection'ı dener ve ona yol verir. `call`, bir
`DynamicMessage`'ın yerleştiremediği alanları bildirir (`CallEvent::Note`), çünkü başka hiçbir
şey bildirmez.
İsteğin başlıkları ve yetkilendirmesi metadata olur (Bearer, Basic, başlıkta
API anahtarı, `apix_http::oauth2` ile OAuth 2.0; `-bin` anahtarları
base64'tür); HTTP isteğini imzalayan yetkilendirme türleri gerekçesiyle
reddedilir. Zaman aşımı, istemci sertifikası ve güvenilen CA istek
ayarlarından gelir. TLS doğrulaması kapalıyken tonic'e (böyle bir anahtarı
yoktur) düz bir `http://` adresi ve gerçek `https` kaynağı verilir; TLS'i
kendi bağlayıcımız (`insecure.rs`) yapar: ALPN h2 ile rustls ve her
sertifikayı kabul eden ama el sıkışma imzalarını yine denetleyen bir
doğrulayıcı.
`call`, `Sent`, `Headers` ve `Received` olaylarını oldukları anda bildirir ve
bir `CallOutcome` ile biter: durum kodu ve mesajı, yanıt metadata'sı,
trailer'lar ve gelen her mesaj. OK dışındaki bir durum hata değil, sonuçtur;
hatalar çağrının olmasını engelleyen şeylerdir (bağlantı yok, tanım yok,
türüne uymayan mesaj).

Uygulamada `grpc://` adresi Gövde sekmesini Mesaj sekmesine çevirir
(`workspace/grpc_editor.rs`): **Metotları yükle** ile dolan aranabilir bir
metot seçici, `.proto` dosyaları ve girdi türünden üretilen bir örnekle mesaj.
Çağrı ağ çalışma zamanında koşar ve arayüzün 60 ms'de bir okuduğu ortak bir
yapıyı doldurur; yanıt alanı WebSocket'inki gibi bir günlüktür
(`ResponseState::Grpc`, `workspace/grpc_ui.rs`), Esc çağrıyı bırakır. CLI
tekli cevabı bütün olarak, akışları satır satır yazar; `apix schema` servisi
`.proto` benzeri metin olarak yazar.

Koşular gRPC isteklerini `apix_grpc::send_for_tests` ile çağırır; bu,
`CallOutcome`'u bir `apix_http::Response`'a çevirir (`CallOutcome::to_response`):
kodun karşılığı olan HTTP durumu (Google'ın eşlemesi) ve gerekçe olarak kodun
adı; başlık olarak metadata ve trailer'lar ile `grpc-status` ve
`grpc-message`; gövde olarak mesaj (sunucu akışında JSON listesi). Böylece
test scriptleri, kontroller ve raporlar gRPC'ye özel hiçbir şey istemez.
Geçmiş bir çağrıyı `Summary::grpc` işaretiyle tutar: durum gRPC kodudur,
trailer'lar ve durum mesajı kendi alanlarındadır, gövde de günlüktür
(`[{"at_ms", "sent", "message"}]`); `grpc_ui::restored` onu yeniden günlük
görünümüne çevirir.

## Paralel koşular ve monitörler (M7)

`Plan::parallel` birden büyükse o kadar tekrar aynı anda koşar (koşunun kendi
görevinde `futures_util` `buffer_unordered`, thread yok): her tekrar isteklerini
sırayla, script durumunun kendi kopyasıyla gönderir; `bail` altında bir hata
diğerlerini de durdurur, rapor sonra tekrar sırasına dizilir. İlerleme, çağıranın
geri çağrısının etrafındaki bir mutex üzerinden gider.

`apix_core::monitor`, `apix.toml`'daki `[[monitor]]`'ü (ad, `every`, `run`,
ortam, webhook, açık/kapalı) tanımlar ve aralıkları okur; `apix_runner::monitor`
bir monitörü çalıştırır (`plan` değişkenleri `apix run` gibi çözer: monitörün
ortamı, `APIX_VAR_*`, sonra sabitlenen değerler), sonucu özetler (`Outcome`),
monitör başına JSON satırlarından oluşan bir `Log` tutar
(`<config>/Apix/monitors/<koleksiyonun özeti>-<ad>.jsonl`, son 500), hangisinin
ne zaman sırası geldiğine karar verir (`Scheduler`: ilk kez görülen monitörün
sırası, günlükteki son koşusundan beri aralığı geçince gelir) ve durum
döndüğünde webhook'a haber verir (`change`, `notify`). `apix monitor` de
masaüstü uygulaması da (`workspace/monitors_ui.rs`: ayar açıkken saniyede bir
bakan bir zamanlayıcı, ağ runtime'ında birer birer koşu) bunu kullanır; bu
yüzden günlüğü paylaşırlar.

## Git (M7)

`apix_core::git` zaten kurulu olan `git` programını çalıştırır (git
kütüphanesi yok; binary büyümez, kullanıcının kendi ayarları, hook'ları ve
kimlik bilgileri geçerlidir). Bir `Repo` koleksiyon ya da proje klasöründe
açılır ve her komut orada `--relative` ile çalışır; yollar o klasöre göredir ve
daha büyük bir deponun içindeki koleksiyon yalnızca kendi dosyalarını görür.
`changes_since(rev)`, `git diff --merge-base rev` (dalın `rev`'den ayrıldığı
yer, pull request'teki gibi) ile izlenmeyen dosyalardır; geçmiş yoksa (sığ
klon) doğrudan `rev` ile karşılaştırır ve bunu söyler. `show(rev, path)` bir
dosyayı bir commit'teki hâliyle okur.

`git::affected`, değişen dosyaları rollerine göre (`git::role_of`) isteklere
eşler: istek dosyası ya da `<ad>.*.js` scriptleri → o istek; `folder.toml` ya
da `_folder.*.js` → o klasörün altındaki istekler; `apix.toml`,
`_collection.*.js`, `environments/*.toml`, proje dosyası ya da projenin
ortamları → hepsi; başka bir dosya → dosyası onun adını geçen istekler.
`apix run --changed` bu istekleri tutar, rapor da seçimi kaydeder
(`RunReport::selection`).

`apix_core::diff` bir dosyanın iki sürümünü anlamına göre karşılaştırır:
istekler `Request` olarak okunup JSON'a çevrilir (varsayılanlar ve biçim
önemsizleşir), diğer TOML dosyaları değer olarak, scriptler ve diğer metinler
satır satır (ortak baş ve son kırpılıp en uzun ortak alt dizi). `key` ya da
`name` alanı benzersiz olan satır listeleri (header'lar, sorgu, `[[example]]`)
o alanla anahtarlanan tablolara dönüşür; satırlar adıyla eşleşir, alanlar
yollarıyla adlandırılır (`headers.Accept`, `body.json`). Aynı içerikle silinen
ve eklenen istek tek bir taşıma olur. `apix diff` sonucu metin, Markdown ya da
JSON olarak yazar.

`RunReport::to_markdown` koşuyu GitHub için yazar (istek başına bir tablo
satırı, hatalar açık bir `<details>` içinde) ve gizli bir
`<!-- apix-run: <ad> -->` işaretiyle başlar; action'ın yorum adımı önceki
yorumunu bu işaretle bulup düzenler. GitHub'ın 65 536 karakter sınırının
altında kalır.

Masaüstü uygulaması (`workspace/git_ui.rs`) açık klasörün dalını ve
`git status`'unu her yeniden yüklemeden sonra ve pencere etkinleştiğinde arka
planda okur, kenar çubuğu işaretleri için değişen yolların bir haritasını
tutar; kendi scriptleri değişen istek de değişmiş sayılır.

Git penceresi (`workspace/git_panel.rs`) aynı `Repo`'yu kullanır: liste için
`working_changes`, seçilen dosya için `show("HEAD", yol)` ile diskteki dosya
`diff::file` üzerinden; `commit_folder` (`git add -A -- .`, ardından
`git commit -- .`, yani yalnızca açık klasör), `branches`, `switch`,
`create_branch` (`switch -c`), `pull` (`--ff-only`), `push` (upstream yoksa
`-u origin <dal>`) ve `log` (hash, zaman, konu). Git
`GIT_TERMINAL_PROMPT=0` ile çalışır; parola isteyen bir uzak depo girdi
beklemek yerine git'in mesajıyla başarısız olur, kimlik bilgisi yardımcıları
yine çalışır. Her işlem arka planda koşar ve durum çubuğunu yeniler.

## AI yardımcısı

`apix_core::ai` üç sağlayıcıdan biri için isteği elle kurar (Rust için resmî
SDK'ları yok) ve yanıtı okur. `Provider` (`anthropic`, `openai`, `google`)
varsayılan modeli, anahtarın anahtarlık hesabını (`ai:<sağlayıcı>-api-key`,
`apix_vault::keychain::save_text` ile), ortam değişkenini ve adresi verir.
Anthropic: `x-api-key`, `anthropic-version: 2026-10-21` ile `POST
/v1/messages`, `claude-opus-5-5` ve varsayılan modelde sunucu tarafı
yedekleri (`fallbacks: "default"`, beta `server-side-fallback-2026-07-01`);
`refusal` durdurma nedeni iletiye çevrilir. OpenAI: bearer anahtarla, bir
sistem ve bir kullanıcı mesajıyla `POST /v1/chat/completions`; `refusal`
alanı ret sayılır. Google: `x-goog-api-key`, `systemInstruction` ve
`contents` ile `POST /v1beta/models/{model}:generateContent`; `SAFETY`
bitişleri ve engellenen istemler ret sayılır, düşünce parçaları atlanır. Her
istem alışverişi (`Context::describe`: kimlik bilgisi başlıkları gizli,
gövdeler 12 KB'ta kesik), görevi ve uygulamanın dil kodunu taşır.
`models_request` / `models` bir anahtarın kullanabildiklerini listeler
(OpenAI'da yalnızca sohbet modelleri, Google'da `generateContent`
destekleyenler). `APIX_AI_URL` testler için temel adresin yerine geçer.
Uygulama (`workspace/ai_ui.rs`) `net::send` ile gönderir, testleri
`ScriptEditor::append_test` ile ekler, açıklamaları bir pencerede gösterir;
Ayarlar `ai_provider` ve `ai_model`'i tutar.

## MCP sunucusu

`apix mcp` (`apix-cli/src/mcp.rs`) standart girdiden satır başına bir JSON-RPC
2.0 mesajı okur, standart çıktıya satır başına bir yanıt yazar (MCP'nin stdio
taşıması; ilk satırın başındaki bayt sırası işareti atlanır). `initialize`,
bildiği sürümse istenen protokol sürümüyle (2025-06-18, 2025-03-26,
2024-11-05), değilse en yenisiyle yanıt verir ve yalnızca araç sunar;
bildirimler yanıtsız kalır, bilinmeyen yöntemler -32601 hatası alır.
`tools/call` sonuçları JSON'u hem metin hem `structuredContent` olarak taşır;
başarısız bir araç protokol hatası değil `isError`'lı bir sonuçtur, böylece
ajan nedenini okuyabilir. Ajanın verdiği yollar koleksiyon köküne eklenir,
`..` çözülür ve dışarıdaysa reddedilir. `send_request`
`apix_runner::exchange` (scriptler, değişkenler, testler), `run_requests`
`apix_runner::run` üzerinden gider; değişkenler öteki komutlarla aynı
`VarArgs::resolve`'dan gelir, ajanın `variables`'ı `--var` gibi davranır.
CLI 16 MB'lık bir iş parçacığında çalışır; komutların ortak future'ı
Windows'ta ana iş parçacığının yığınını aşmıştı.

## Kurulum ve Claude

`workspace/setup_ui.rs` Kurulum'u bir kez (`settings.setup_shown`) ve
Yardım'dan açar. Veritabanı düğmesi `route_ui::download_geo`'yu çalıştırır
(bu ayın DB-IP dosyası, yoksa geçen ayınki; uygulamanın istemcisiyle,
arayüz iş parçacığı dışında açılır); `apix geo download|status|remove`
terminalde aynısını yapar ve bir yükleyicinin isteğe bağlı adımı da onu
çağıracak. `apix_core::claude_mcp` Claude Desktop'ın yapılandırmasını bulur
(`%APPDATA%\Claude`, `~/Library/Application Support/Claude`,
`~/.config/Claude`; testler için `APIX_CLAUDE_CONFIG`), sunucuyu
`apix-<koleksiyon>` diye adlandırır, onu çalışan programın yanındaki (ya da
PATH'teki) `apix`'e `mcp <klasör> [--list-only]` ile yöneltir ve geri kalan
her şeyi koruyarak `mcpServers`'a ekler (özgün dosya bir kez `.bak`'a
kopyalanır); `claude_code_command` `claude mcp add --scope user`'dır.

## Akışlar

`apix_core::flow` dosyanın kendisidir: bir `Flow`'un düğümleri
(`NodeKind::Request`, `Check`, `Set`, `Delay`; kimliği ve yeriyle birlikte
her `[[node]]` tablosuna düzleştirilir) ve bir düğümün sonucundan (`ok`/`fail`,
`yes`/`no`, `next`) başka bir düğüme giden bağlantıları vardır. `problems()`
bir akışın çalışmasını engelleyenleri listeler; `Seen` son yanıttır ve
`Seen::subject`, `status`, `time`, `body`, `header:Ad` ya da
`apix_core::filter` üzerinden bir JSONPath veya jq filtresi okur. `.flow`
dosyaları koleksiyonda herhangi bir yerde durabilir; kenar çubuğu onları
yok sayar.

`apix_runner::flow::run`, grafiği başlangıç düğümünden itibaren gezer. Bir
istek düğümü bir koşudaki gibi `exchange`'ten geçer; böylece scriptler,
testler ve `pm.*` değişkenleri çalışır; sakladığı değerler script durumunun
yerel kapsamına yazılır ve sonraki düğümler için ortamın önüne geçer,
`pm.variables.set`'in yapacağı gibi. Konusu metin olan bir kontrol
`{{değişkenleri}}` çözer ve biri tanımsızsa değeri yok sayılır. Çalıştırma
her adımı bildirir (`FlowProgress`) ve 1000 adımdan sonra ya da `Control`
ile durur. `FlowReport::to_run_report` koşu raporlarını (JUnit, HTML,
Markdown) yeniden kullanır. Masaüstü editörü (`workspace/flows_ui.rs`)
düğümleri konumlandırılmış öğeler, bağlantıları bir tuval üzerinde çizer:
ileri giden bağlantılar eğri, geri dönenler düğümlerin etrafından dolaşan
çizgilerdir; akışı ağ çalışma zamanında çalıştırır ve ilerlemesini,
Runner gibi, bir zamanlayıcıyla okur.

## Eklentiler

`apix-plugins`, WebAssembly eklentilerini bir yorumlayıcı olan `wasmi` 2 ile
çalıştırır: JIT yoktur, yani bir eklentiden yerel kod üretilmez ve Rust'ın
derlendiği her yerde derlenir. Bir modül hiçbir şey içe aktaramaz; bu yüzden
eklentinin hiç host fonksiyonu yoktur (ağ, dosya, saat ya da rastgelelik
yok); `memory`, `apix_alloc`, `apix_manifest` ve `apix_call` dışa aktarır ve
yanıtlar `ptr << 32 | len` olarak geçen JSON'dur. Her çağrı modülü sıfırdan
örnekler (çağrılar arasında durum kalmaz); 2·10⁹ birim yakıt ve 256 MB bellek
sınırıyla, 64 MB yığınlı bir thread'de çalışır; yakıtın bitmesi "çok uzun
sürdü" olarak bildirilir. Manifest, eklenti yüklenirken bir kez okunur;
`name` `[a-z0-9-]` olmalıdır. `installed()` eklenti klasörünü
(`<config>/Apix/plugins` ya da `APIX_PLUGINS_DIR`) `reload()`'a kadar
önbellekte tutar.

Kancalar Apix'in geri kalanıyla üç yerde buluşur: modeldeki `Auth::Plugin {
plugin, config }`, diğer yetkilendirmeler gibi çözülür (config'te
değişkenler) ve `apix_http::prepare` tarafından son isteğe uygulanır;
`apix_import::plugin::plan`, bir içe aktarıcının yanıtını istek dosyalarına
çevirir; yanıtın görselleştiricisi (`VisualMode::Plugin`) eklentiyi arayüz
thread'i dışında çalıştırır ve bir tablo ya da metin çizer. wasmi, debug
derlemelerde bile `opt-level = 3` ile ve debug assertion'ları olmadan
derlenir; çünkü dağıtımı, aksi hâlde yığını tüketen kuyruk çağrılarına
dayanır.

## Yanıtları karşılaştırma

`apix_core::compare` iki `Snapshot` alır (etiket, durum, başlıklar, gövde,
süre). Başlıklar küçük harfli adla karşılaştırılır, aynı adlılar birleştirilir;
`VOLATILE` içindekiler (tarihler, önbellek, istek ve iz kimlikleri, hız
sınırları, gövdenin nasıl gönderildiği) yalnızca dışarıda bırakılanlar olarak
sayılır. İki JSON gövde önce `by_id`'den geçer: ayrı `id` / `_id` / `uuid` /
`key` değerli kayıt listelerini `[id=…]` anahtarlı tablolara çevirir; sonra
`diff::fields_as_they_are` (istek farkının yürüyüşü, istek dosyalarına uyan
ama veriye uymayan anahtar/ad eşlemesi olmadan). Öteki metinler satır satır,
geri kalan yalnızca boyutla karşılaştırılır. Uygulamanın Karşılaştır penceresi
(`workspace/compare_ui.rs`) önceki tarafı geçmişten alır (aynı istek dosyalı
ya da aynı yöntem ve URL'li kayıtlar) ya da isteği başka bir ortamın
değişkenleriyle çözüp gönderir; `apix compare` iki ortamda gönderir.

## Yük testi

`apix_http::load::Load` çözülmüş bir istek için tek bir reqwest istemcisi
kurar (isteğin TLS ayarları `tls::client_config` ile, proxy'si, zaman aşımı
ve yönlendirme kuralı, sınırsız boşta bağlantı havuzu); `hit()` onu gönderir
ve gövdenin baytlarını açmadan sayar. OAuth 2.0 bir kez bearer token olur,
Digest reddedilir. `apix_runner::bench::Bench::run`, `concurrency` kadar
işçiyi tek bir `FuturesUnordered` içinde başlatır; her biri sıradaki istek
numarasını atomik bir sayaçtan alır, sayı, süre ya da `stop()` hayır diyene
kadar sürer; hız verilmişse `başlangıç + n / hız` anına kadar bekler. Her
gönderim bir mutex altında kaydedilir: mikrosaniye cinsinden gecikmesi
(istek başına bir `u32`, yani bir milyon istek 4 MB), durumu, türüne göre
hatası ("could not connect", "timed out", …), baytları ve saniyelik bir
kova. `summary()` çalışırken de çağrılabilir: en yakın sıra yüzdelikleri için
gecikmelerin bir kopyasını sıralar. CLI testi çekirdek başına bir iş
parçacıklı kendi çalışma zamanında koşturur ve saniyede bir satır yazar;
uygulama onu ağ çalışma zamanında koşturur ve saniyede dört kez yeniden çizer.

## Rota ve harita

`apix-route`, `apix_core::git`'in git'i başlattığı gibi sistemin aracını
başlatır: Windows'ta `tracert -d -h N -w 1000`, macOS ve Linux'ta
`traceroute -n -q 1 -w 1 -m N`, traceroute yoksa `tracepath -n -m N`.
Hiçbiri yönetici izni istemez; ICMP'yi kendimiz göndermek Windows'ta isterdi.
`parse_line` her birinin satırını yalnızca sayı ve adreslerinden okur (önce
atlama numarası, her `N ms`, adres olarak ayrışan ilk sözcük); bu yüzden
kendi kod sayfasındaki çevrilmiş bir Windows da aynı okunur. Bir atlamanın
süresi denemelerinin en iyisidir, `<1 ms` yarım milisaniye sayılır. `Trace`
atlamaları araç yazdıkça verir, bırakılınca ya da durdurulunca aracı kapatır.

`Geo` bir MaxMind DB dosyasını açar (`maxminddb`) ve genel adresler için bir
`Place` verir (enlem, boylam, İngilizce şehir adı, ISO ülke kodu); özel,
loopback, link-local, CGNAT ve multicast adreslere hiç bakılmaz. Veritabanı
`<config>/Apix/geo` içindeki ilk `.mmdb` dosyasıdır; `install` bir indirmeyi
açar (gzip), açılabildiğini denetler ve ancak ondan sonra eskisinin yerine
koyar. `dbip_urls` bu ayın ve geçen ayın DB-IP Lite adresini verir
(`APIX_GEO_URL` onların yerine geçer). Harita, `land::LAND`'in eşdikdörtgen
izdüşümüdür: kıtalar ve büyük adalar elle, yaklaşık iki derece doğrulukla
çizilmiştir (Karadeniz ve Hazar `land::WATER` içinde); bu yüzden harita verisi
indirilmez ya da lisanslanmaz. Masaüstü penceresi (`workspace/route_ui.rs`)
izlemeyi bir iş parçacığında koşturur, atlamaları geldikçe yerleştirir,
veritabanını uygulama açıkken bir kez okur ve haritayı GPUI yollarıyla çizer.
`apix trace` atlamaları yerleriyle ve 100 km'yi aşan her sıçramanın
uzaklığıyla yazar.

## Güncelleme denetimi

`apix_core::update` isteği kurar (11R5/Apix için GitHub'ın
`releases/latest` adresi, `User-Agent: Apix/<sürüm>`, 10 sn zaman aşımı;
`APIX_UPDATE_URL` onu başka yere yöneltir) ve yanıtı okur: `Version`
`v1.2.3`, `1.2` ve ön sürümleri ayrıştırır (`1.0.0-rc.1`, `1.0.0`'dan önce
gelir, sayısal parçalar değerine göre), 404 henüz bir şey yayımlanmadığı
anlamına gelir, öteki hatalar GitHub'ın mesajını taşır. CLI (`apix update`)
isteği kendi istemcisiyle, uygulama (`workspace/update_ui.rs`) `net::send`
ile gönderir: `settings.check_updates` açıksa açılışta (yalnızca daha yeni
bir sürüm gösterilir) ve Yardım › Güncellemeleri denetle… ile (her yanıt
gösterilir). Hiçbir şey indirilmez ya da kurulmaz.

## Temalar

Renkler `theme.rs` içindedir. Bir `ThemeDef` bir temadır (yerleşik ya da
`<config>/Apix/themes/*.toml` dosyasından okunmuş; dosyanın vermediği renkler
Obsidyen'den ya da Gün ışığı'ndan gelir); `Colors` ve koyu/açık bilgisini
taşır. `Palette::build` onu görünümlerin boyadığı renklere çevirir: vurgu
ayarı (koyu ve daha koyu bir açık tonu olan adlı bir renk ya da `#RRGGBB`),
göreli parlaklığa göre seçilen vurgu üzerindeki yazı rengi, koyu ya da açık
temaya göre üzerine gelme ve giriş alanı tonları ve saydamlık ayarı:
pencere arka planının alfası `1 - t`'dir, paneller bu hızın %60'ıyla açılır,
`t` büyüdükçe kenarlar beyaza doğru parlar, açık temalar `t`'nin %70'ini
kullanır. `from_settings` görünüm ayarına göre koyu ya da açık temayı,
"sistemi izle" için `App::window_appearance` ile seçer.

gpui-kit'in kök görünümü bütün pencerenin altına `Theme::background` boyar;
bu sistem bulanıklığını örterdi. Bu yüzden saydamlık sıfırdan büyükken o renk
saydam yapılır ve pencereler (dialog) kendi arka planlarını her dialogun
kullandığı `bidi::DialogBidi` üzerinden alır. Bulanıklığı işletim sistemi
yapar: `WindowBackgroundAppearance::Blurred` Windows'ta akrilik
(`SetWindowCompositionAttribute`), macOS'ta vibrancy'dir.

Görünümler paletin bir kopyasını tutmaz: boyarken `theme::current()` çağırır.
`theme::set` yeni paleti saklar, gpui-kit'in bileşen temasını ona yöneltir
(kipini de değiştirir, böylece kod editörünün renkleri de uyar), her
pencerenin arka planını bulanık ya da opak yapar ve pencereleri yeniler; bu
yüzden değişiklik hemen görünür. Workspace pencere görünümünü izler ve sistem
açık ile koyu arasında geçtiğinde onu yeniden çağırır.

## Diller (M4)

Masaüstü arayüzündeki her metin bir anahtardan geçer: `tr!("menu.file")`
(`rust_i18n::t!` üzerine kurulu, gösterilecek bir `SharedString` döndüren ince
bir sarmalayıcı; kaydedilen metinler, örneğin yeni bir isteğin varsayılan adı,
için aynı metni `tr_plain!` verir).
rust-i18n, gpui-kit'in kullandığı sistemin aynısıdır; bu yüzden ikisi de süreç
genelinde tek bir locale'i izler. Çince için gpui-kit'in kendi metinlerinin
kullandığı kod olan `zh-CN` kullanılır. Her dil tek bir düz dosyadır:
`crates/apix-desktop/locales/<code>.yml` (satır başına bir `key: value`, sıralı,
`%{name}` yer tutucuları); binary'ye derlenir ve bir dosya değiştiğinde
`build.rs` yeniden derler. Eksik bir metin İngilizceye düşer; bir dilde bir
anahtar eksikse, bilinmeyen bir anahtar varsa ya da İngilizceden farklı yer
tutucular kullanılıyorsa bir birim testi başarısız olur. Cümleler asla
parçalardan yapıştırılmaz, yer tutuculu bütün anahtarlardır; çünkü sözcük sırası
diller arasında farklıdır.

Dil Ayarlar'da seçilir (veya ilk açılışta sistem dilini izler) ve anında değişir:
metinler her render'da okunur, widget'ların içinde tutulan birkaç metin (input
yer tutucuları, seçici öğeleri) ise `Workspace::retranslate` tarafından yeniden
ayarlanır. Japonca, Çince ve Arapça glifleri, gömülü Inter'in yanında platformun
font fallback mekanizmasından (DirectWrite, Core Text, fontconfig) gelir. Arapça
metin sağdan sola şekillendirilir ve yerleştirilir; Windows'ta bunun için glyph
run yerleşimi düzeltilmiş gpui 0.3.9 DirectWrite backend'i olan
`vendor/gpui-pre-windows` gerekir (workspace `Cargo.toml` üzerinden patch'lenir;
upstream düzelttiğinde kaldırılacak). gpui sağdan sola metni satıra
kaydıramaz: bir paragrafı tek satır olarak şekillendirir ve soldan sağa arttığını
varsaydığı glif x konumlarından keser. Bu yüzden satıra kayabilecek metinler
(notlar, ipuçları, açıklamalar, iletiler) `bidi::para` üzerinden geçer; o da
RTL'de metni okuma sırasıyla boşluklardan böler, her aday satırı ölçer ve her
satırı kaydırmadan ayrı çizer (`src/bidi/paragraph.rs`). Diğer dillerde düz gpui
metnidir. gpui her satırı ayrıca soldan sağa temel yönle yerleştirir; bu yüzden
RTL bir dilde `tr!` her metni sağdan sola gömmeyle (U+202B…U+202C; DirectWrite
yeni yalıtım karakterlerini yok sayar) sarar. Böylece Latin bir sözcükle
("URL", "OAuth 2.0", "Ctrl+S") başlayan ya da biten bir cümle de sağdan sola
okunur; durum çubuğu gibi birleştirilmiş metinler için aynısını `i18n::display`
yapar. Bu işaretler dosyalara asla ulaşmaz: kaydedilen metinler `tr_plain!`
kullanır. Motor crate'lerinden gelen hata iletileri İngilizce kalır.

Sağdan sola bir dilde (`Language::rtl`, Arapça) pencere aynalanır: kenar çubuğu
sağa, yanıt paneli istek panelinin soluna, pencere düğmeleri sola geçer; menüler,
sekme şeritleri, form satırları ve durum çubuğu sağdan başlar, metin sağa
hizalanır. GPUI'de yerleşim yönü yoktur, bu yüzden bunu `src/bidi.rs` yapar:
stil alan her öğe için bir `Bidi` trait'i (`dir_row` bir satırı ters çevirir;
`text_start` / `text_end`, `ps` / `pe` dolguları ve `border_e_1` tarafı yöne göre
seçer; `ltr` metni sola sabitler), gpui-kit'in `TabBar`'ı için `tab_bar` /
`fill_tabs` (sekmeler ters sırayla verilir, indeksler geri eşlenir) ve diyaloglar
için `bidi_title` (RTL'de kapatma düğmesi başlık satırının soluna taşınır).
Yardımcılar dili görünüm render edilirken okur, böylece dil değişince pencere
anında aynalanır. Kod ve yanıt görüntüleyicileri, URL çubuğu, anahtar–değer
tabloları (parametreler, başlıklar, form alanları, değişkenler) ve yanıt
başlıkları her dilde soldan sağa kalır. gpui-kit'in açılır menüleri ve düğmeleri
kendi soldan sağa iç yerleşimlerini korur (kısayol sağda, ikon etiketin önünde);
menüler düğmelerinden sola doğru açılır.

Dil eklemek: `locales/en.yml` dosyasını `locales/<code>.yml` olarak kopyalayın,
değerleri çevirin ve `src/i18n.rs` içindeki `LANGUAGES` listesine bir satır ekleyin.

Kullanıcı belgeleri plandan ayrı, her dil için bir klasörde durur
(`Docs/<code>/`: kullanıcı kılavuzu, mimari, teknik, iş akışları, görseller) ve
`Docs/languages.toml` içinde listelenir. Kaynak İngilizcedir; her çeviri hangi
İngilizce sürümden yapıldığını kaydeder, böylece eskimiş çeviriler ortaya çıkar.

## Thread modeli (masaüstü)

- Ana thread GPUI'ye aittir; olay döngüsünü ve render'ı o çalıştırır.
- Tek bir çok thread'li **tokio runtime**, bir arka plan thread'inde bir kez
  başlatılır. reqwest, tonic ve tungstenite buna ihtiyaç duyar.
- Arayüz işi tokio'ya gönderir ve sonucu bir GPUI görevinde (`cx.spawn_in`)
  alır. Bu görev bırakıldığında (drop) istek iptal edilir; böylece Esc ve yeniden
  gönderme temiz biçimde iptal eder.
- Yanıt geldikten sonra gövde, arayüz onu görmeden önce GPUI'nin arka plan
  executor'ında biçimlendirilir ve indekslenir. Biçimlendirme, ağaç kurmayan ve
  sayıları tam gönderildiği gibi koruyan tek geçişli bir yeniden girintileyici
  olan `apix_core::text::pretty_json` ile yapılır.

## Büyük yanıt görüntüleyici

512 KB ve üzeri gövdeler kod düzenleyici yerine `large_view.rs` kullanır:

- `apix_core::text::RowIndex`, her görüntü satırının nerede başladığını satır
  başına 8 bayt ile kaydeder. 2 KB'tan uzun satırlar (minify edilmiş JSON)
  birkaç satıra bölünür.
- Kaydırma konumu piksel ofseti değil, bir **satır numarasıdır**. Satır başına
  20 px ile on milyon satır yaklaşık 200 milyon px eder; bu, `f32` piksellerin
  tam olduğu aralığın çok ötesindedir. Görüntüleyici yalnızca ekrana sığan ~120
  satırı yerleştirir. Fare tekerleğini, kendi kaydırma çubuğunu ve klavyeyi
  (↑ ↓ PgUp PgDn, Ctrl+Home/End) kendisi yönetir.
- Büyük bir JSON gövdesinin Ham sekmesi yalnızca açıldığında oluşturulur.
- Ctrl+F metnin tamamını arka plandaki bir iş parçacığında arar (aranan metin
  için büyük-küçük harf ayırmayan bir `regex`) ve en çok 100.000 bayt aralığı
  tutar; `RowIndex::row_of` bir eşleşmenin satırını ikili aramayla bulur ve
  yalnızca ekrandaki satırlar vurgulamak için kendi eşleşmelerine bakar.

Daha küçük gövdeler kod editöründedir; editörün kendi arama paneli açıktır.
Biçimli'nin üstündeki süzme (`workspace/filter_ui.rs`) `apix_core::filter`'ı son
tuştan 250 ms sonra arayüz iş parçacığı dışında çalıştırır: `$` ile başlayan ifade
için `serde_json_path` ile JSONPath, diğerleri için `jaq` ile jq; jq en çok 10.000
çıktı verir. Sonuç her gövde gibi bir `Document`'tır; böylece süzülmüş büyük bir
gövde editöre düşebilir, tersi de olabilir.

Windows'ta release derlemede, yerel bir sunucudan gelen 100 MB minify edilmiş
JSON için ölçülmüştür (`cargo run -p apix-http --release --example big_json_server`):
yanıt, Enter'dan yaklaşık 0.33 s sonra ekrandadır. Bu süre aktarımı, 9.6 milyon
satıra biçimlendirmeyi, indekslemeyi ve ilk kareyi kapsar. Bellek, gövde
yüklüyken yaklaşık 407 MB'tır; boştayken 69 MB.

## Performans bütçesi

| Metrik                              | Hedef             | Ölçülen (Windows, release)  |
|-------------------------------------|-------------------|-----------------------------|
| Soğuk başlangıçtan ilk kareye       | < 150 ms          | 222 ms (`main`'den ilk kareye; yaklaşık 150 ms'si ekran kartı sürücüsünün yüklenmesi) |
| Boştaki RAM                         | < 60 MB           | 88 MB working set, örnek koleksiyon açık (çoğu ekran kartı sürücüsü) |
| Release binary'si                   | < 100 MB          | 54.4 MB |
| Girdiden çizime gecikme             | 1 kare (120 Hz'de ≤ 8 ms) | ölçülmedi |
| 100 MB JSON gövdesini açma ve kaydırma | kare düşmesi yok | gönderdikten 306 ms sonra ekranda (yerel sunucu); kare süreleri ölçülmedi |

Başlangıç süresi ve boştaki bellek bugün bütçenin üzerinde; bunların profilinin
çıkarılması planlanıyor. Rakamlar `scripts/measure.ps1`'den gelir (bkz.
[teknik notlar](technical.md#performans-ölçümü)).

Release profili zaten fat LTO, tek codegen unit ve strip edilmiş semboller
kullanır (bkz. kök `Cargo.toml`).
