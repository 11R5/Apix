<!-- translated from en/technical.md sha256:86b614e97d3156ce -->

# Teknik notlar

Apix'in nasıl derlendiği, nelere bağımlı olduğu ve nasıl ölçüleceği.
[Mimari](architecture.md) tasarımı açıklar; bu sayfa işin pratik tarafıdır.

## Depo yapısı

```
crates/
├── apix-core       istek modeli, değişkenler, disk formatı, projeler, imzalama (sign.rs), GraphQL şemaları
├── apix-http       HTTP motoru (reqwest), ağ izi, OAuth 2.0 token'ları, SSE, WebSocket, Socket.IO, GraphQL şeması çekme
├── apix-import     Postman, Insomnia, OpenAPI, HAR ve WSDL içe aktarma
├── apix-codegen    "Komut veya kod olarak kopyala" (curl, curl.exe, PowerShell, 10 dilde kod) ve curl yapıştırma
├── apix-vault      şifreli koleksiyonlar
├── apix-script     istek öncesi ve test scriptleri (QuickJS)
├── apix-runner     koleksiyon koşuları, veri dosyaları, JUnit/JSON/HTML raporlar
├── apix-grpc       gRPC çağrıları: .proto dosyaları, sunucu reflection'ı ya da koddan ve JSON'dan tanım, JSON mesajlar
├── apix-proto      diğer protokoller: MQTT
├── apix-cli        `apix` komutu
└── apix-desktop    masaüstü uygulaması (GPUI); arayüz metinleri locales/ içinde
tools/
├── icongen         uygulama ikonlarını Logo/ klasöründen üretir
└── docs-check      eskimiş belge çevirilerini listeler
vendor/
└── gpui-pre-windows  sağdan sola metin düzeltmesi içeren gpui Windows backend'i
scripts/
└── measure.ps1     README için performans ölçümleri
examples/
├── jsonplaceholder repodan çalıştırıldığında uygulamanın açtığı küçük bir koleksiyon
└── ci/             GitHub Actions, GitLab CI ve Azure Pipelines örnekleri
action.yml          GitHub Action (`uses: 11R5/Apix@…`)
Dockerfile          konteyner imajı olarak `apix` komutu
.github/workflows/  üç sistemde CI ve sürüm etiketlerinde imaj yayımı
Docs/
├── languages.toml  belge dilleri
├── en/ tr/         kullanıcı kılavuzu, mimari, teknik notlar, iş akışları
└── IS-PLANI.md, ROADMAP.md   plan (Türkçe ayrıntı, İngilizce özet)
```

## Derleme

Gereksinimler: [Rust](https://rustup.rs) stable (repo 1.99 ile test edilmiştir).

- **Windows:** MSVC toolchain'i ("Desktop development with C++" iş yükü ve bir
  Windows 10/11 SDK'sı ile Visual Studio veya Build Tools).
- **macOS:** Xcode komut satırı araçları.
- **Linux:** GPUI'nin ihtiyaç duyduğu X11/Wayland, Vulkan ve fontconfig geliştirme
  paketleri (gpui-kit README'si bunları dağıtıma göre listeler); henüz test edilmedi.

```sh
cargo test                       # motor, CLI, içe aktarma, codegen, vault, belge kontrolü
cargo run -p apix-desktop        # masaüstü uygulaması (debug)
cargo test -p apix-desktop       # masaüstü birim testleri (çeviriler, yollar, …)
cargo build --release            # optimize edilmiş CLI
cargo build --release -p apix-desktop
```

`-p` olmadan `cargo build` ve `cargo test`, tüm GPU yığınını içeri çeken masaüstü
uygulamasını atlar; onu açıkça derleyin. Bağımlılıklar debug derlemelerde bile
optimize edilir, böylece geliştirme sırasında arayüz akıcı kalır.

Release profili fat LTO, tek codegen unit ve strip edilmiş semboller kullanır.
Masaüstü uygulamasının tam bir release derlemesi birkaç dakika sürer.

## Ana bağımlılıklar

| Alan | Crate'ler |
|---|---|
| Arayüz | `gpui-kit` 0.7 (GPUI ve bileşen kütüphanesi), JSON ve GraphQL için tree-sitter gramerleri |
| HTTP | rustls ile `reqwest` 0.12, `tokio`, `hyper-util`, zamanlama için `tower` katmanları |
| gRPC | `tonic` 0.14 (istemci, aynı hyper ve rustls üzerinde), `prost-reflect` (çalışma anında bilinen mesajlar, JSON), `protox` (.proto dosyalarını `protoc` olmadan okur), `regex` (üretilmiş kod ve tipleri okuma), `tonic-reflection` (reflection istemcisi), sertifika denetimi kapalı gRPC için `rustls` ve `tokio-rustls`; `rcgen` test sertifikaları üretir (yalnızca testlerde) |
| Ağ ayrıntıları | `x509-parser` (sertifikalar), `netdev` (ağ arayüzleri, ağ geçidi, DNS sunucuları) |
| İmzalama | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Şifreleme | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize`, `keyring` (işletim sistemi kimlik bilgisi deposu) |
| Scriptler | `rquickjs` (QuickJS, sistemin C derleyicisiyle derlenir); `require` kütüphaneleri Apix'in kendi JavaScript kodu, özetler `sha1`, `sha2`, `md-5`, `hmac` ile |
| Dosyalar | `serde`, `toml`, `serde_json`, `notify` (dosya izleme), `trash` |
| Diller | `rust-i18n` (gpui-kit ile ortak), `sys-locale` |

## Apix'in koleksiyonlar dışında yazdığı dosyalar

| Dosya | İçerik |
|---|---|
| `<config>/Apix/settings.toml` | dil, görünüm, koyu ve açık tema, vurgu rengi, saydamlık, otomatik kilit süresi, geçmişin açık ya da kapalı olması, ortam gizli değerlerinin nereye yazılacağı |
| `<config>/Apix/state.toml` | son açılan koleksiyonlar, koleksiyon ya da proje başına seçilen ortam ve açık sekmeler (şifreli koleksiyonlar hariç) |
| `<config>/Apix/cookies.json` | çerez kavanozu (oturum çerezleri kalıcı değildir) |
| `<config>/Apix/graphql/*.json` | her uç noktadan en son çekilen GraphQL şeması (sunucunun introspection cevabı), adresin özetiyle adlandırılır |
| `<config>/Apix/themes/*.toml` | kendi temaların (yalnızca Ayarlar'da **Klasörü aç**'a basıldığında ve klasör boşsa yazılır: bir örnek) |
| İşletim sistemi kimlik bilgisi deposu, `Apix` servisi | yalnızca istenirse: şifreli koleksiyonların hatırlanan parolaları (`vault:<klasör>`) ve ortam gizli değerleri (`env:<kimlik>`, uzun değerler için ayrıca `env:<kimlik>#1`, …) |
| `<config>/Apix/history/` | gönderilen istekler (`{{değişkenler}}` çözülmeden) ve yanıtları: `index.jsonl`, her kayıt için bir `<id>.json` ve `<id>.body`; son 300 kayıt, gövdeler 256 KB'ta kesilir; bir gRPC çağrısı gövde olarak günlüğünü tutar |

`<config>`, Windows'ta `%APPDATA%`, macOS'te `~/Library/Application Support`,
Linux'ta `~/.config` klasörüdür. OAuth 2.0 token'ları ve şifreli koleksiyonların
parolaları asla diske yazılmaz.

## Ortam değişkenleri

| Değişken | Etkisi |
|---|---|
| `APIX_PASSWORD` | CLI için şifreli bir koleksiyonun parolası (`--password-env` başka bir değişken seçer). |
| `APIX_VAR_<AD>` | CLI için bir değişken: `APIX_VAR_TOKEN`, `{{token}}`'ı doldurur (koleksiyondaki adla büyük küçük harf gözetmeden eşleşir); ortamlardan üstündür, `--var` ondan üstündür. |
| `APIX_THEME` | bir tema kimliği (`obsidian`, `cyber`, `dracula`, `nord`, `solarized-dark`, `daylight`, `solarized-light` ya da bir tema dosyasının adı); ayarlar ne derse desin o temayı gösterir. |
| `APIX_BENCH` | `startup` veya `json:<url>`: ölç ve çık (aşağıya bakın). |
| `APIX_BENCH_OUT` | Ölçümlerin sonuna ekleneceği dosya. |

## Performans ölçümü

`scripts/measure.ps1` release uygulamasını derler ve şunları ölçer:

- **Release binary'sinin** boyutu.
- **Başlangıçtan ilk kareye:** `main`'den ilk kare çizilene kadar geçen süre
  (`APIX_BENCH=startup`), birkaç çalıştırmanın medyanı.
- **Boştaki bellek:** örnek koleksiyon açık olarak başlattıktan beş saniye sonraki
  working set.
- **Büyük JSON'un ekrana gelmesi:** yerel `big_json_server` örneğinden 100 MB'lık
  bir JSON gövdesinin GET'i, göndermeden yanıt çizilene kadar
  (`APIX_BENCH=json:<url>`); böylece ağ bu sayıya dahil olmaz.

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7, macOS/Linux'ta da
```

Betik, çalıştığı makineyle birlikte bir Markdown tablosu yazdırır; README en
sonuncusunu gösterir. Zamanlamalar `crates/apix-desktop/src/bench.rs`'den gelir;
bu dosya `APIX_BENCH` ayarlanmadıkça hiçbir şey yapmaz.

Diğer yardımcılar:

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # :8765 üzerinde 100 MB JSON
cargo run -p apix-http --example oauth_server                       # :8766 üzerinde OAuth 2.0 test sunucusu
cargo run -p apix-http --example sse_server                         # :8767 üzerinde sunucu olayları (SSE; Last-Event-ID'den devam eder)
cargo run -p apix-http --example ws_server                          # :8768 üzerinde WebSocket yankı (ikili de) ve ticker
cargo run -p apix-http --example graphql_server                     # :8769 üzerinde bir GraphQL uç noktası (şema ve birkaç kullanıcı)
cargo run -p apix-http --example socketio_server                    # :8771 üzerinde Socket.IO 4: /, /chat, /ticker, /private (auth) ad alanları
cargo run -p apix-grpc --example grpc_server                        # :8770 üzerinde reflection'lı bir gRPC Greeter
cargo run -p apix-http --example soap_server                        # :8773 üzerinde bir SOAP hesap makinesi (WSDL ?wsdl adresinde; 500 ve 200 ile hatalar)
cargo run -p apix-proto --example mqtt_broker                       # :8772 üzerinde bir MQTT broker'ı (`clock` her saniye yayın yapar)
```

## Arayüz dilleri

Arayüz metinleri `crates/apix-desktop/locales/<code>.yml` içindedir; satır başına
bir düz `key: value` bulunur ve rust-i18n bunları `tr!` makrosu üzerinden okur.
Bir dilde bir anahtar eksikse veya İngilizceden farklı `%{placeholders}`
kullanılıyorsa bir birim testi başarısız olur. Dil eklemek için `en.yml`'yi
kopyalayın, çevirin ve `src/i18n.rs` içindeki `LANGUAGES` listesine bir satır
ekleyin.

## Belge dilleri

`Docs/languages.toml` belge dillerini listeler. Çeviriler, kaynak aldıkları
İngilizce metni belirten bir damgayla başlar:

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check            # eksik veya güncel olmayan çevirileri listeler
cargo run -p docs-check -- stamps  # her İngilizce belgenin güncel damgasını yazdırır
```

## Vendor'a alınmış gpui backend'i

GPUI'nin Windows metin çizicisi (`gpui-pre-windows` 0.3.9 crate'i) her glyph
run'ı soldan sağa yerleştiriyordu; bu yüzden Arapça sözcükler ters görünüyordu.
`vendor/gpui-pre-windows`, `DrawGlyphRun`'ı düzeltilmiş hâliyle aynı crate'tir
(değişiklik "Apix patch" olarak işaretlenmiştir) ve workspace `Cargo.toml`'u onu
patch ile devreye alır. Upstream crate düzeltildiğinde kaldırılabilir.

## Sürekli entegrasyon

`.github/workflows/ci.yml` her push ve pull request'te çalışır:

| İş | Neyi denetler |
|---|---|
| Biçim ve belgeler | `cargo fmt --check`; `docs-check` (çeviriler İngilizce belgeleri izliyor mu) |
| Motor ve CLI | masaüstü uygulaması dışındaki her şey için `cargo clippy -D warnings` ve `cargo test`; Windows, macOS ve Linux'ta |
| Masaüstü uygulaması | aynısı `apix-desktop` için üç sistemde (Linux önce pencere, yazı tipi ve anahtarlık kütüphanelerini kurar) |
| GitHub Action | deponun kendi action'ı (`uses: ./`) `examples/jsonplaceholder` ile, Linux ve Windows'ta; raporların yazıldığı da denetlenir |

`.github/workflows/docker.yml` sürüm etiketlerinde konteyner imajını
yayımlar. Action derlediği `apix`'i motor kaynaklarının özetiyle önbelleğe
alır; yeni bir commit yeniden derler, değişmeyen kaynak öncekini kullanır.

## Testler

| Crate | Kapsananlar |
|---|---|
| apix-core | model ve dosya formatı için gidiş-dönüş (round trip) testleri, değişkenler, anahtarlıktaki ortam gizli değerleri (kimlik bilgisi deposu yoksa atlanır), AWS test paketine, RFC 7616'ya (Digest), RFC 7636'ya (PKCE) ve OAuth 1.0 örneğine karşı imzalama; GraphQL şemaları (introspection okuma, SDL) ve bir sorgunun her yerinde neyin önerilip açıklandığı (alanlar, takma adlar, argümanlar, enum değerleri, girdi nesneleri, listeler, fragment'ler, değişken türleri, yorumlar ve metinler); `[grpc]` tablosu; gRPC çağrıları dahil geçmiş kayıtları; GraphQL değişken tanımları ve değişkenler (JSON) içinde tamamlama; güvenilen CA ayarı |
| apix-http | istek oluşturma, her gövde türü, OAuth 2.0, Digest, AWS ve OAuth 1.0 için yerel bir sunucu, ağ izi; SSE ayrıştırıcı (parçaların her yerden bölünmesi dahil), yavaş gönderen bir sunucuya karşı akış, WebSocket (yankı, el sıkışmada auth ve parametreler, iki taraftan kapanış, reddedilen el sıkışma), isteğin yetkilendirmesiyle GraphQL şeması çekme ve şema yokken verilen hatalar, kendi sertifika yetkilisiyle HTTPS; WebSocket alt protokolleri ve ikili mesajlar, kendi CA'sıyla ya da denetimsiz `wss://`; bir olay akışının yeniden bağlanmaya taşıdıkları; Socket.IO paketleri, el sıkışma adresleri ve ikili ekler, yerel bir Socket.IO 4 sunucusuna karşı: olaylar, iki yönde ack'ler, ping'ler, auth'lu ad alanları ve retler, ikili ekler, sunucunun istemciyi çıkarması, düz WebSocket sunucusunun ayırt edilmesi |
| apix-grpc | adresler, durum kodları, metadata olarak başlıklar ve yetkilendirme; yerel bir Greeter'a karşı (dinamik mesajlarla kurulmuş, reflection'lı ve reflection'sız): servisi listeleme ve tanımlama, dört metot türünün hepsi, yetkilendirmenin sunucuya ulaşması, başarısız bir durum, import'lu .proto dosyaları ve tanım yokken, bilinmeyen metotta, türüne uymayan mesajda, tekli metoda liste verilince, eksik dosyada ve sunucu yokken verilen hatalar; örnek mesajlar; koşular için HTTP yanıtı olarak çağrı (durum eşlemesi, `grpc-status`, liste olarak akış); kendi sertifika yetkilisinden sertifikalı bir sunucuya karşı TLS (reddedilir, CA ile güvenilir, denetlenmeden kabul edilir); tanımlar: `tests/fixtures/generated` içinde saklanan gerçek üretici çıktıları (Python, Java, C#, C++, PHP için protoc 3.19; protoc-gen-es; ts-proto) özgün servisi geri vermeli, Go, Rust, betterproto ve işaretlemeler numara ve türü verir, aynı tip 10 dilden okunur, örnek JSON, sunucuyla karşılaştırma, tahminin reflection'a yol vermesi, bilinmeyen alanların bildirilmesi, yanlış yanıt türünün açıklanması |
| apix-proto | MQTT adresleri, girişler ve seçenekler; yerel bir broker'a karşı: her QoS, joker konular, bağlıyken abone olma ve bırakma, reddedilen abonelikler ve geçersiz konular, retained mesajlar, girişler ve saklanan oturumlar, broker'ın bağlantıyı kesmesi, kendi CA'sıyla ya da denetimsiz `mqtts://` |
| apix-codegen | bash, cmd ve PowerShell için kabuk tırnaklama; curl yapıştırma; gidiş-dönüş testleri; her dilde URL, başlıklar, gövdeler, dize kaçışları ve ayarlar |
| apix-import | Postman v2.0/v2.1 fixture'ları; WSDL: iki SOAP sürümü, portlar ve adresler, document ve rpc stilleri, başlıklar, şemadan örnekler (extension, enum, choice, `form`, özyineleme), içe aktarılanlar ve okunamayanlar, konumlar, reddedilen WSDL 2.0 |
| apix-script | istek öncesi scriptlerden değişken ve istek değişiklikleri, `pm.test` / `pm.expect` sonuçları ve mesajları, eski `tests[...]` API'si, dosyayı adlandıran hatalar, sonsuz scriptlerin durdurulması, `setNextRequest`, `atob`/`btoa`, `pm.sendRequest` (geri çağırma ve `await`, bekleme sayılmaz), bilinen özet değerlerine karşı hazır kütüphaneler, kodsuz doğrulamalar, yanıt hedefleri için değer ve ifade üretimi |
| apix-runner | CSV (tırnaklar, satır sonları, `;`, BOM) ve JSON veri dosyaları, dotenv dosyaları ve `APIX_VAR_*` adları, tekrar sayıları ve veri satırları, duraklatma/durdurma zamanlaması, JUnit/JSON/HTML çıktısı ve kaçışlar, tarihler; yerel bir sunucuya karşı bütün koşular (veri satırları, kenar çubuğu sırası, `bail`, `setNextRequest`) |
| apix-vault | şifreleme formatı, kurcalama (tampering), parola değişikliği, kurtarma anahtarı, anahtarlık kayıtları (gerçek kimlik bilgisi deposunda, test servis adıyla; depo yoksa atlanır) |
| apix-desktop | çevirilerin eksiksizliği, dil yardımcıları, dosya yolları, vurgulama sorgusu, tema dosyaları (okuma, hatalar, gidiş-dönüş), vurgu ve saydamlık renkleri, GraphQL şema dosyası adları, geçmişte bir gRPC çağrısının günlüğü |
| docs-check | belge yapısı ve damgalar |
