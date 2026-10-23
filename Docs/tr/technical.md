<!-- translated from en/technical.md sha256:98566cf1ac9907ff -->

# Teknik notlar

Apix'in nasıl derlendiği, nelere bağımlı olduğu ve nasıl ölçüleceği.
[Mimari](architecture.md) tasarımı açıklar; bu sayfa işin pratik tarafıdır.

## Depo yapısı

```
crates/
├── apix-core       istek modeli, değişkenler, disk formatı, projeler, imzalama (sign.rs)
├── apix-http       HTTP motoru (reqwest), ağ izi, OAuth 2.0 token'ları
├── apix-import     Postman koleksiyonu ve ortamı içe aktarma
├── apix-codegen    "Komut veya kod olarak kopyala" (curl, curl.exe, PowerShell, 10 dilde kod) ve curl yapıştırma
├── apix-vault      şifreli koleksiyonlar
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
└── jsonplaceholder repodan çalıştırıldığında uygulamanın açtığı küçük bir koleksiyon
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
| Ağ ayrıntıları | `x509-parser` (sertifikalar), `netdev` (ağ arayüzleri, ağ geçidi, DNS sunucuları) |
| İmzalama | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Şifreleme | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize` |
| Dosyalar | `serde`, `toml`, `serde_json`, `notify` (dosya izleme), `trash` |
| Diller | `rust-i18n` (gpui-kit ile ortak), `sys-locale` |

## Apix'in koleksiyonlar dışında yazdığı dosyalar

| Dosya | İçerik |
|---|---|
| `<config>/Apix/settings.toml` | dil, tema, otomatik kilit süresi |
| `<config>/Apix/state.toml` | son açılan koleksiyonlar, koleksiyon başına seçilen ortam |
| `<config>/Apix/cookies.json` | çerez kavanozu (oturum çerezleri kalıcı değildir) |
| `<config>/Apix/history/` | gönderilen istekler (`{{değişkenler}}` çözülmeden) ve yanıtları: `index.jsonl`, her kayıt için bir `<id>.json` ve `<id>.body`; son 300 kayıt, gövdeler 256 KB'ta kesilir |

`<config>`, Windows'ta `%APPDATA%`, macOS'te `~/Library/Application Support`,
Linux'ta `~/.config` klasörüdür. OAuth 2.0 token'ları ve şifreli koleksiyonların
parolaları asla diske yazılmaz.

## Ortam değişkenleri

| Değişken | Etkisi |
|---|---|
| `APIX_PASSWORD` | CLI için şifreli bir koleksiyonun parolası (`--password-env` başka bir değişken seçer). |
| `APIX_THEME` | `obsidian` veya `cyber`; tema ayarını geçersiz kılar. |
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

## Testler

| Crate | Kapsananlar |
|---|---|
| apix-core | model ve dosya formatı için gidiş-dönüş (round trip) testleri, değişkenler, AWS test paketine, RFC 7616'ya (Digest), RFC 7636'ya (PKCE) ve OAuth 1.0 örneğine karşı imzalama |
| apix-http | istek oluşturma, her gövde türü, OAuth 2.0, Digest, AWS ve OAuth 1.0 için yerel bir sunucu, ağ izi |
| apix-codegen | bash, cmd ve PowerShell için kabuk tırnaklama; curl yapıştırma; gidiş-dönüş testleri; her dilde URL, başlıklar, gövdeler, dize kaçışları ve ayarlar |
| apix-import | Postman v2.0/v2.1 fixture'ları |
| apix-vault | şifreleme formatı, kurcalama (tampering), parola değişikliği, kurtarma anahtarı |
| apix-desktop | çevirilerin eksiksizliği, dil yardımcıları, dosya yolları, vurgulama sorgusu |
| docs-check | belge yapısı ve damgalar |
