# Apix İş Planı

> Bu belge Apix'in ayrıntılı ve Türkçe iş planıdır. Özellikler tek tek yazılmıştır.
> Özet İngilizce yol haritası [ROADMAP.md](ROADMAP.md), teknik mimari [en/architecture.md](en/architecture.md) dosyasındadır; kullanıcı belgeleri dil başına [en/](en/) ve [tr/](tr/) klasörlerindedir.
> Tasarım referansı: [design/mockup-v1.webp](design/mockup-v1.webp).

**İşaretler:** ✅ yapıldı · 🔜 sıradaki · ⬜ planlandı · 💭 sonra karar verilecek
**Boyut:** S = birkaç saat · M = 1–3 gün · L = 1 hafta ve üstü

### Durum özeti (24 Ekim 2026)
| Aşama | Durum |
|---|---|
| M0 Temel | ✅ |
| M1 Arayüzden istek | ✅ 100 MB JSON ~0.33 s'de açılıyor |
| M2 Koleksiyonlar, Postman içe aktarma | ✅ (sekmeler ve sürükle-bırak M3'te tamamlandı) |
| M3 Simge, projeler, ortamlar, kopyalama, auth | ✅ 9/9 · bitti: simge, ortam seçici ve editörü, dinamik değişkenler, projeler, kabuk komutu olarak kopyalama, metotla yeni istek, değişken çipleri, Auth sekmesi ve miras, komut paleti, sekmeler, sürükle-bırak ile taşıma · küçük kalanlar M4'te yapıldı (yalnız ham gövde editöründe değişken çipi kaldı) |
| M4 Günlük kullanım | ✅ · bitti: menü çubuğu, şifreli koleksiyonlar (motor, uygulama, CLI), ağ denetçisi (Network sekmesi), multipart/binary/GraphQL gövdeleri, auth tipleri (Digest, OAuth 1.0, OAuth 2.0, AWS SigV4), ayarlar ekranı ve 9 dilli arayüz, Arapça için sağdan sola düzen, dokümantasyon yapısı (İngilizce, Türkçe), README karşılaştırması ve performans ölçümü, istek ayarları (zaman aşımı, yönlendirmeler, TLS doğrulama, proxy, istemci sertifikaları; klasör/koleksiyon/proje kalıtımıyla) ve çerez yöneticisi, 10 dilde kod üretimi, geçmiş, Postman otomatik tanıma, toplu içe aktarma ve Postman'a dışa aktarma, içerik tipleri (HTML gövde, XML/HTML renklendirme ve biçimlendirme, resim önizleme, Hex, yanıtı kaydetme), Insomnia/OpenAPI/HAR içe aktarma, şifreli koleksiyon parolasını ve ortam gizli değerlerini işletim sistemi anahtarlığında saklama, M3'ten kalan küçük işler (elle sıralama, sekmeleri hatırlama, proje renkleri, koleksiyonu projeye taşıma, prod onayı, fish/HTTPie/wget kopyalama, Params/Headers/form gövdesinde değişken çipleri) · gRPC (not 17) sonraki görevlere bırakıldı |
| M5 Scriptler, testler, runner | ✅ `apix-script` (QuickJS, `pm.*` uyumlu API), istek öncesi ve test scriptleri (koleksiyon → klasör → istek), `pm.test` / `pm.expect` (Chai tarzı), eski `tests[...]` / `postman.*` API'si, konsol, uygulamada Scripts sekmesi ile Tests ve Console sekmeleri, `apix send` / `apix run` script çalıştırma, test sonuçları ve `setNextRequest`, `pm.sendRequest`, hazır kütüphaneler (`require`), kodsuz doğrulamalar, yanıttan değişken kaydetme, Runner penceresi (sıra, tekrar, bekleme, veri dosyası, ilk hatada dur, duraklat/devam/durdur), CSV/JSON veri dosyalı koşular, JUnit XML / JSON / HTML raporlar, `apix run --data/--iterations/--delay/--bail/--junit/--json/--html`, GitHub Actions örneği |
| Tema paketi (M5 sonrası) | ✅ anında tema değişimi, açık tema ve "sistemi izle", vurgu rengi, saydamlık kaydırıcısı (%0–90, buzlu cam), Dracula/Nord/Solarized/Gün ışığı, `<config>/Apix/themes/*.toml` dosyalarından temalar |
| CLI ve CI/CD | ✅ `APIX_VAR_<AD>` ve `--env-file` ile gizli değerler, GitHub Action (`uses: 11R5/Apix@…`), Docker imajı, ortam matrisli örnek  |
| CI temeli | ✅ üç sistemde CI (biçim, docs-check, clippy, testler; motor ve masaüstü), action'ın kendini sınaması, GitLab ve Azure örnekleri; ilk koşu Windows, macOS ve Linux'ta yeşil |
| M6 Protokoller | 🔜 · bitti: WebSocket (uygulamada bağlan/kes, Mesajlar sekmesi ve kayıtlı `[[message]]`'lar, iki yönlü canlı günlük; `apix send` ile `--message`/`--listen`), SSE (olaylar geldikçe uygulamada ve `apix send`'de, Esc gelenleri tutar) · sıradaki: GraphQL introspection ve otomatik tamamlama, gRPC, Socket.IO, MQTT, SOAP |
| M7–M9 | ⬜ |

Release ikili dosyası 38.9 MB (üst sınır 100 MB; QuickJS ve JavaScript renklendirmesi yaklaşık 3.6 MB, Runner 0.8 MB, tema paketi ile WebSocket ve SSE 0.9 MB ekledi). 24 Ekim ölçümü: açılış 278 ms, boşta 84 MB, 100 MB JSON 306 ms. Testler: core 58, script 10, runner 16, vault 11, desktop 16, codegen 24, import 26, http 28 (hepsi geçiyor, clippy uyarısız).

### Kullanıcı notları (17 Ekim 2026) ve plana nasıl işlendikleri
| # | Not | Nereye işlendi |
|---|---|---|
| 1 | Postman koleksiyonlarını tanıyabilmeli | §2-D. İçe aktarma ✅ yapıldı. Otomatik tanıma ve toplu dışa aktarma desteği eklendi. |
| 2 | Şifreli koleksiyonlar oluşturulup açılabilmeli, dışarıdan açılamamalı | §2-C2 (yeni), M4 |
| 3 | Tailscale ile ekip modu | §7 💭. Tüm geliştirme bitince hatırlatılacak. |
| 4 | Logo: `Logo/` klasöründeki görselden Windows, macOS ve Linux simgeleri | §2-M, M3 (ilk iş), M9 |
| 5 | Her işletim sistemi için ayrı klasör mü, tek klasör mü | §6, karar 6: tek masaüstü crate'i, işletim sistemine özel kısımlar `platform/` altında |
| 6 | Aç, yeni koleksiyon, yeni proje; Proje → Ortam → Koleksiyonlar gruplaması | §2-C1 (yeni), M3 |
| 7 | Koleksiyona metot seçerek istek eklemek gibi temel özellikler | §2-C |
| 8 | İsteği bash/sh, terminal, PowerShell ve cURL olarak kopyalamak | §2-I, M3'e öne alındı |
| 9 | Hem yerel tarafın hem hedefin ayrıntılı ağ bilgisi ve haritası | §2-B2 (yeni), M4 ve M7 |
| 10 | Program boyutu 15 MB'ı geçebilir | §0 ve M9 güncellendi: üst sınır yaklaşık 40 MB (18 Ekim'de 100 MB'a çıktı, not 11) |

### Kullanıcı notları (18 Ekim 2026) ve plana nasıl işlendikleri
| # | Not | Nereye işlendi |
|---|---|---|
| 11 | Boyut sınırı 100 MB; yeter ki masaüstünde hızlı, akıcı, basit ve güçlü olsun | §0 "Hafif" kuralı ve M9 güncellendi |
| 12 | Ağ denetçisinde sonraya kalan TLS ayrıntıları (sürüm, şifre takımı, tam zincir, TCP/TLS ayrı süreler) zamanı gelince yapılsın | §B2, M7 (ağ haritasıyla birlikte, kendi bağlayıcımızla) |
| 13 | Ayarlardan arayüz dili seçilebilmeli: İngilizce, Türkçe, Arapça, Almanca, Fransızca, Rusça, Japonca, Çince, İspanyolca | §M, §N, M4'e öne alındı (M7'deydi): ayarlar ekranı, çeviri altyapısı, 9 dil, Arapça için sağdan sola düzen |
| 14 | Dokümantasyon ayrı olsun; kullanım kılavuzu, mimari, teknik altyapı ve iş akışları bu dillerde ayrı belgeler olsun; sonradan yeni dil eklenebilsin | §N (yeni), M4 (yapı + İngilizce ve Türkçe), M9 (tüm diller) |
| 15 | Her dil için görsellerle desteklenmiş kullanım kılavuzu | §N, M9 (ekran görüntüleri her dil için otomatik alınır) |
| 16 | GitHub sayfasında benzer uygulamalardan farklar, avantajlar ve performans | §N, M4 (README karşılaştırması ve tekrarlanabilir ölçümler) |

### Kullanıcı notları (23 Ekim 2026) ve plana nasıl işlendikleri
| # | Not | Nereye işlendi |
|---|---|---|
| 17 | XML, HTML, binary ve gRPC desteği olmalı | Kontrol edildi (23 Ekim): XML gövde vardı ama renklendirilmiyordu, HTML gövde tipi yoktu, XML/HTML yanıtlar biçimlendirilmiyordu, ikili yanıtlar bozuk metin olarak görünüyordu, gRPC yoktu. §A4 ve §B: HTML gövde tipi, XML/HTML renklendirme ve biçimlendirme, resim önizleme, Hex sekmesi, gövdeyi dosyaya kaydetme M4'te ✅. gRPC §H'de (L), M4'teki içe aktarıcılardan sonra ele alınacak. |
| 18 | Her geliştirmeden sonra açıklamalar ve ilerleme md dosyalarına (bu plan, ROADMAP, README, kullanım kılavuzları) işlensin | Çalışma kuralı: her özellik, birleştirilmeden önce bu dosyalara yazılır. |
| 19 | GitHub geçmişinde yalnızca kullanıcının hesabı görünsün (kkmamedaga ve "claude" ortak yazar satırı kaldırılsın) | Yapıldı (23 Ekim): geçmiş yeniden yazıldı; commit'ler yalnızca AhmetVDemir adına, ortak yazar satırı eklenmez. |

---

## 0. Vizyon ve kurallar

Apix, Postman'in yaptığı her şeyi yapan bir API istemcisi olacak. Postman'den daha hızlı, daha küçük ve daha sade olacak.

| Kural | Ne anlama geliyor |
|---|---|
| **Minimalist** | Ekranda sadece o an gereken şey görünür. Gerisi Ctrl/⌘+K komut paletinde durur. |
| **Hafif** | Hız, akıcılık ve özellik boyuttan önce gelir. Program dosyası için üst sınır 100 MB (not 11; şu an 38.9 MB; Postman ve Electron uygulamaları 300 MB'ı aşıyor). Açılış 150 ms'nin, boşta bellek kullanımı 60 MB'ın altında kalır (şu an 69 MB ölçüldü). |
| **Hızlı** | Arayüz hiç donmaz. 100 MB'lık bir yanıt bile kare kaybetmeden kaydırılır. |
| **Local-first** | Hesap ve bulut zorunlu değildir. Koleksiyonlar düz dosya olarak durur ve git ile versiyonlanır. |
| **Postman'den kolay geçiş** | Postman koleksiyonları, ortamları ve scriptleri içe aktarılınca çalışır. |
| **Dürüst arayüz** | Ekrandaki her etiket gerçeği söyler. Desteklenmeyen bir özellik "hazır" diye gösterilmez. |

---

## 1. Tasarım dili (Gemini mockup'ı temel alınarak)

### 1.1 Mockup'tan alınanlar
- **Cam görünüm.** Koyu lacivertten petrol mavisine geçen gradyan bir arka plan var, üstündeki paneller yarı saydam.
- **Çok ince ayraçlar.** Paneller kalın kutular yerine 1 px'lik yarı saydam çizgilerle ayrılır.
- **Renkli metot rozetleri.** POST, GET, DELETE, PUT, gRPC ve GQL için her protokolün kendi rengi var.
- **Yapı.** Üstte komut çubuğu, solda koleksiyon ağacı, ortada istek editörü, sağda yanıt paneli, altta durum çubuğu.
- **Sekmeler.** İstek tarafında Params · Headers · Auth · Body · Pre-request & Tests. Yanıt tarafında Pretty · Timing · Tests · Code.
- **Sayı rozetleri.** Sekme başlıklarında `Params 2`, `Headers 3` gibi sayılar görünür.
- **Değişken vurgusu.** URL içindeki `{{base_url}}` ayrı bir çip olarak çizilir. Fareyle üstüne gelince çözülmüş değeri gösterir (`→ https://api.apix.dev`).
- **Durum çubuğu.** Git branch'i, dosya sayısı, ortam ve kısayol ipuçları burada görünür.
- **Cam ayarı.** Saydamlık slider'ı ve tema seçici (Obsidian / Cyber).

### 1.2 Mockup'ta değiştireceklerimiz
| Mockup'ta | Apix'te | Neden |
|---|---|---|
| `SIMD Validated`, `HTTP/3 QUIC Ready`, `zero_copy_parser` gibi etiketler | Kaldırılacak. Yalnızca gerçekten çalışan şeyler gösterilecek. | Dürüst arayüz kuralı. HTTP/3 henüz deneysel. |
| `120 FPS` sayacı her zaman açık | Sadece geliştirici modunda görünecek (Ctrl+Shift+P). | Minimalizm |
| Her yerde rozet | Rozet sadece bilgi taşıyorsa kullanılacak. | Görsel gürültüyü azaltmak |
| macOS trafik ışıkları | Windows ve Linux'ta sağda küçült/büyüt/kapat düğmeleri (✅ yapıldı) | Her platform kendi düzenini kullanır. |
| `.apix` uzantısı | ✅ Kullanılıyor. İçerik TOML. | |

### 1.3 Renk paleti (tasarım token'ları)
| Token | Değer | Kullanım |
|---|---|---|
| `bg.gradient` | `#0A1626` → `#0F2A3D` (sol üst) → `#161E3A` (sağ alt) | Pencere zemini, blur'un üstünde |
| `surface` | `#0E1726`, %70 opak | Panel, editör, yanıt alanı |
| `surface.raised` | `#132033`, %80 opak | URL çubuğu, açılır menüler |
| `hairline` | beyaz, %8 opak | Panel ayraçları |
| `border.active` | `#22D3EE`, %60 opak | Seçili öğe, odaklanmış alan |
| `accent` | `#22D3EE` (elektrik camgöbeği) | SEND düğmesi, linkler, seçili sekme |
| `text` | `#E6EDF3` | Ana metin |
| `text.muted` | `#8B9BB0` | Yardımcı metin, dosya adları |
| `status.ok` | `#34D399` | 2xx yanıtlar |
| `status.warn` | `#FBBF24` | 3xx yanıtlar, uyarılar |
| `status.err` | `#F87171` | 4xx ve 5xx yanıtlar, hatalar |

**Metot renkleri:**
| Metot | Renk |
|---|---|
| GET | `#34D399` yeşil |
| POST | `#22D3EE` camgöbeği |
| PUT | `#FBBF24` sarı |
| PATCH | `#FB923C` turuncu |
| DELETE | `#F87171` kırmızı |
| HEAD, OPTIONS | `#A78BFA` mor |
| gRPC | `#818CF8` çivit |
| GraphQL | `#F472B6` pembe |
| WebSocket | `#2DD4BF` turkuaz |

**Tipografi:**
- Arayüz: **Inter** veya **Geist Sans**, 13 px
- Kod, URL ve JSON: **JetBrains Mono** veya **Geist Mono**, 12.5 px
- Fontlar uygulamanın içine gömülür, böylece her işletim sisteminde aynı görünür.

**Temalar:**
- **Obsidian:** koyu ve sade. Varsayılan tema.
- **Cyber:** mockup'taki camgöbeği gradyan.
- **Dracula, Nord, Solarized Dark:** hazır koyu temalar.
- **Gün ışığı (Daylight), Solarized Light:** açık temalar; görünüm koyu, açık ya da "sistemi izle".
- **Kendi temaların:** `<config>/Apix/themes/*.toml`.
- **Saydamlık:** %0 (düz renk) ile %90 arası kaydırıcı; masaüstü buzlu cam gibi bulanık görünür (Windows'ta akrilik), paneller daha yavaş açılır, kenarlar parlar, menü ve pencereler düz kalır. Varsayılan %20.

### 1.4 Ekran anatomisi
```
┌───────────────────────────────────────────────────────────────────────────┐
│ [≡] apix   [ 🔍 Uç noktaları ara, test çalıştır, ortam değiştir   ⌘K ]   [Prod ▾]  – □ ✕ │  ← Başlık + komut çubuğu
├──────────────┬────────────────────────────────────────────────────────────┤
│ 🔍 Filtre     │ workspace / auth / verify.apix                     ⋯     │  ← Breadcrumb
│ ▾ Auth        │ [POST ▾] [{{base_url}}]/v2/auth/session        [ SEND ] │  ← URL çubuğu
│   POST Verify │ ─────────────────────────────┬──────────────────────────│
│   DEL  Revoke │ Params Headers Auth Body Tests│ ● 200 OK  11 ms  1.3 KB  │
│ ▾ Users       │                              │ Pretty Raw Headers Timing│
│   GET  List   │   (istek editörü)            │   (yanıt görüntüleyici)  │
│   PUT  Update │                              │                          │
├──────────────┴──────────────────────────────┴──────────────────────────┤
│ ⎇ main*  6 dosya  ✓ git       ·  Production       ⌘B Kenar  ⌘K Palet  │  ← Durum çubuğu
└───────────────────────────────────────────────────────────────────────────┘
```
- Kenar çubuğu Ctrl/⌘+B ile tamamen gizlenir.
- Editör ile yanıt paneli arasındaki ayraç sürüklenebilir. Pencere daralınca paneller alt alta dizilir.
- Kullanılacak gpui-kit bileşenleri: `TitleBar`, `Tree`, `Tabs`, `Table`, `Input` (tree-sitter destekli kod editörü), `VirtualList`, `Resizable`, `Command`, `Select`, `Slider`, `StatusBar`, `Notification`, `Chart`, `Diff`.

---

## 2. Özellik kataloğu

### A. İstek oluşturma

#### A1. HTTP metotları
| Özellik | Açıklama | Durum |
|---|---|---|
| **GET** | Veri okuma. Gövde (body) gönderilmez. Query parametreleri URL ile iki yönlü senkron çalışır. | ✅ · ⬜ query senkronu |
| **POST** | Yeni kayıt oluşturma. Bütün gövde tiplerini destekler (§A4). | ✅ |
| **PUT** | Kaydın tamamını değiştirme. Gövde tipleri POST ile aynıdır. | ✅ |
| **PATCH** | Kaydın bir kısmını güncelleme. JSON Patch ve Merge Patch şablonları gelir. | ✅ · ⬜ şablonlar |
| **DELETE** | Kayıt silme. İsteğe bağlı gövde gönderilebilir. İstenirse "prod ortamında DELETE göndermeden önce onay iste" koruması açılabilir. | ✅ · ⬜ prod onayı |
| **HEAD** | Sadece başlıkları getirir. Yanıt panelinde gövde sekmesi gizlenir. | ✅ · ⬜ gövde sekmesini gizleme |
| **OPTIONS** | CORS preflight yardımcısı. İzin verilen metotları ve başlıkları tablo olarak gösterir. | ✅ motor · ⬜ yardımcı |
| **Özel metot** | `PROPFIND`, `PURGE` gibi elle yazılan metotlar. | ⬜ S |
| **Metot seçici** | Açılır liste. Ayrıca URL çubuğunda `Alt+1…7` ile hızlı seçim. | ✅ |

#### A2. URL çubuğu
- ✅ Düzenlenebilir URL alanı. `{{değişken}}` çip olarak çizilir: tanımlı (camgöbeği), dinamik (mor), tanımsız (kırmızı). Çip tek parça silinir, Ctrl+Z ile metne geri döner.
- ✅ Fareyle değişkenin üstüne gelince çözülmüş değer ve hangi kapsamdan geldiği görünür. Gizli değerler maskelenir. ✅ Params, Headers ve Body'nin form/multipart alanlarında da çip · ⬜ ham gövde editöründe (JSON, XML, metin; gpui-kit kod editörü satır içi token desteklemiyor).
- ⬜ **Path değişkenleri:** `/users/:id` yazılınca Params sekmesinde `id` alanı kendiliğinden açılır.
- ⬜ **Query senkronu:** `?a=1&b=2` yazılınca tabloya, tabloya yazılınca URL'ye eklenir.
- ✅ **cURL yapıştırma:** URL çubuğuna bir cURL komutu yapıştırılırsa istek otomatik oluşturulur.
- ✅ **Ctrl/⌘+Enter** isteği gönderir. **Esc** devam eden isteği iptal eder. **Alt+1…7** metodu seçer.

#### A3. Params ve Headers
- ✅ Anahtar–değer tablosu, her satırda aç/kapa kutucuğu. ⬜ Açıklama alanı ve sürükleyerek sıralama.
- ⬜ **Toplu düzenleme:** tablo ile `key: value` metin modu arasında geçiş.
- ⬜ **Header otomatik tamamlama:** standart header adları ve yaygın değerleri önerilir (`Content-Type`, `Accept` ...).
- ⬜ **Otomatik header'lar:** `Content-Type`, `User-Agent`, `Accept-Encoding` gibi otomatik eklenen başlıklar soluk renkte gösterilir. Kullanıcı bunları ezebilir. Motorda kullanıcının header'ını öne alma kuralı hazır (✅).
- ⬜ **Header ön ayarları:** sık kullanılan header gruplarını kaydedip tek tıkla ekleme.

#### A4. Body (gövde) tipleri
| Tip | Ayrıntı | Durum |
|---|---|---|
| none | | ✅ |
| raw / JSON | Renklendirme, biçimlendirme (Ctrl+Shift+F), anlık doğrulama, yorum satırı desteği (gönderirken silinir) | ✅ renkli editör · ⬜ biçimlendirme, doğrulama, yorumlar |
| raw / Text, XML, HTML, JavaScript | Her biri kendi renklendirmesiyle | ✅ Text, XML, HTML (XML ve HTML, HTML grameriyle renklendirilir; HTML gövde tipi 23 Ekim'de eklendi, `text/html` gönderir; motor, CLI, "Copy as", curl yapıştırma, Postman içe/dışa aktarma) · ⬜ JavaScript |
| x-www-form-urlencoded | Anahtar–değer tablosu | ✅ |
| multipart/form-data | Metin ve dosya alanları (alan başına Text/File), çoklu dosya seçimi, alan başına içerik tipi. Koleksiyon içindeki dosyalar isteğe göre göreli yolla kaydedilir. | ✅ · ⬜ içerik tipini arayüzde düzenleme |
| binary | Dosya seçip ham olarak gönderme; boyut ve "dosya yok" uyarısı | ✅ |
| GraphQL | Sorgu (renkli) ve değişkenler (JSON, anlık doğrulama) editörü · şemadan otomatik tamamlama (§H1) | ✅ editörler · ⬜ otomatik tamamlama (M6) |

#### A5. Kimlik doğrulama (Auth)
| Tip | Durum |
|---|---|
| No Auth / Inherit from parent (klasörden, koleksiyondan veya projeden miras al). Auth sekmesinde "Inherit" seçiliyken neyin, nereden geldiği yazar. | ✅ |
| Bearer Token | ✅ |
| Basic Auth | ✅ |
| API Key (header veya query) | ✅ |
| Digest Auth: sunucunun meydan okumasına (401) MD5 veya SHA-256 ile tek seferde cevap | ✅ |
| OAuth 1.0a: HMAC-SHA1, HMAC-SHA256, PLAINTEXT; header veya query; her gönderimde yeni nonce ve zaman damgası | ✅ |
| OAuth 2.0: Authorization Code + PKCE (tarayıcıda giriş, loopback yönlendirme), Client Credentials, Password, token yenileme, bellekte token önbelleği ("Get new token", "Forget"). Token'lar diske yazılmaz. Implicit grant güvensiz olduğu için yok. | ✅ |
| AWS Signature v4 (geçici kimlik bilgileri, S3 için içerik özeti) | ✅ |
| Hawk, NTLM, Akamai EdgeGrid, JWT Bearer (JWT'yi Apix imzalar) | ⬜ M her biri |

#### A6. İstek ayarları
- ✅ Zaman aşımı, yönlendirmeleri takip et/etme ve üst sınır, TLS doğrulamasını kapatma, istek başına proxy (boş dize proxy'yi tümden kapatır), çerez gönderimini kapatma. `.apix` dosyasında `[settings]` tablosu; alan alan klasör → koleksiyon → proje kalıtımı (auth ile aynı zincir), Ayarlar sekmesi + paylaşılan ayar diyalogları, CLI (`--timeout`, `--insecure`, `--proxy`), "Copy as" bayrakları, curl yapıştırma ve Postman `protocolProfileBehavior` içe aktarma.
- ✅ İstemci sertifikaları (mTLS): PEM sertifika + anahtar dosyası.
- ⬜ HTTP sürümü seçimi (1.1 / 2). HTTP/3, reqwest'te kararlı hâle gelince eklenecek.
- ⬜ Özel CA sertifikası.

### B. Yanıt paneli
| Özellik | Açıklama | Durum |
|---|---|---|
| Durum satırı | Durum kodu (renkli), süre, TTFB, boyut, HTTP sürümü | ✅ |
| **Pretty** | JSON, XML ve HTML biçimlendirilmiş ve renkli gösterilir. JSON'da düğümler katlanabilir. | ✅ JSON, XML, HTML (`apix_core::text::pretty_xml` / `pretty_html`: girinti, yalnız metin içeren öğe tek satır, yorum/CDATA/DOCTYPE korunur, HTML'de boş öğeler ve script/style; dengesiz etiketlerde ham gösterilir) |
| **Raw** | Ham metin | ✅ |
| **Preview** | HTML'i basit hâliyle, resim, PDF ve ses dosyalarını kendi görünümüyle gösterir. | ✅ resim (PNG, JPEG, GIF, WebP, BMP, SVG; 32 MB'a kadar), ikili içerik için tür/boyut özeti · ⬜ HTML'i sayfa olarak gösterme, PDF, ses |
| **Hex** | İkili yanıtlar için onaltılık görünüm | ✅ her yanıt için Hex sekmesi (satır başına 8 bayt, ilk 4 MB, sanal görüntüleyici) |
| **Gövdeyi dosyaya kaydet** | Yanıt gövdesini bayt bayt bir dosyaya yazar; ad `Content-Disposition`'dan ya da URL'den, uzantı içerik türünden | ✅ (durum satırındaki ↓ düğmesi) |
| **Yanıt türünü tanıma** | Önce `Content-Type`, sonra baytlara bakarak (PNG/JPEG/GIF/WebP/BMP imzaları, UTF-8 ve NUL denetimi, `<?xml`, `<!doctype html>`) | ✅ `apix_http::Response::body_kind` |
| **Headers** | Tablo, kopyala ve ara | ✅ tablo · ⬜ kopyala, ara |
| **Cookies** | Yanıtla gelen çerezler | ⬜ |
| **Timing** | DNS, TCP, TLS, TTFB ve indirme sürelerini şelale grafiği olarak gösterir. Mockup'taki "Latency Visualizer" budur. | ✅ DNS, bağlantı (TCP+TLS), bekleme, indirme şelalesi (Network sekmesi) · ⬜ TCP ve TLS ayrı |
| **Büyük yanıt** | 100 MB+ yanıtlarda sanal liste kullanılır, sadece ekrandaki satırlar çizilir. | ✅ (100 MB JSON ~0.33 s) |
| **Gövdede arama** | Ctrl+F, regex desteğiyle | ⬜ |
| **Filtre** | JSONPath ve jq ile yanıtı süzme | ⬜ M |
| **Örnek olarak kaydet** | Yanıtı isteğin yanına "example" olarak kaydeder. Mock sunucu ve dokümantasyon bunu kullanır. | ⬜ |
| **Dosyaya kaydet ve kopyala** | | ⬜ S |
| **Visualizer** | Yanıtı tablo veya grafik olarak gösterme (Postman Visualizer karşılığı) | ⬜ L |

### B2. Ağ denetçisi ve ağ haritası (not 9)
Her istek için "Network" sekmesi: istek gerçekte nasıl ve nereden geçti?

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Yerel taraf** | Kullanılan ağ arayüzü, yerel IP (IPv4/IPv6), varsayılan ağ geçidi, DNS sunucuları, kullanılan proxy, VPN/tünel olup olmadığı | ✅ arayüz ve türü, yerel IP ve port, ağ geçidi, DNS sunucuları, VPN tahmini (tünel arayüzü veya adı), ortamdaki proxy · ⬜ Windows sistem proxy'si |
| **DNS** | Çözülen tüm A/AAAA kayıtları, bağlanılan IP, DNS süresi | ✅ |
| **Bağlantı** | TCP bağlantı süresi, bağlantının yeniden kullanılıp kullanılmadığı, HTTP sürümü ve ALPN | ✅ kurulum süresi (TCP+TLS birlikte), uzak adres, HTTP sürümü · ⬜ TCP ve TLS ayrı süreler |
| **TLS** | Sürüm, şifre takımı, sertifika zinciri (veren, geçerlilik ve kalan gün, SAN listesi). Süresi yaklaşan sertifika için uyarı. | ✅ sunucu sertifikası (veren, geçerlilik, kalan gün, adlar; 30 günden azsa uyarı rengi) · ⬜ sürüm, şifre takımı, tam zincir (reqwest vermiyor) |
| **Zaman şelalesi** | DNS → TCP → TLS → ilk bayt → indirme, çubuk grafik olarak (§B "Timing" ile aynı veri) | ✅ |
| **Yönlendirmeler** | 301/302 zinciri, her adımın süresi ve adresi | ✅ zincir (durum ve adres) · ⬜ adım başına süre |
| **Boyut** | Sıkıştırılmış ve açılmış boyut, kullanılan sıkıştırma (gzip, br, zstd) | ⬜ S |
| **Rota (traceroute)** | Hedefe giden atlamalar ve gecikmeleri. ICMP yönetici izni istediğinde TCP traceroute'a geçilir. | ⬜ L |
| **Harita** | Atlamalar ve hedef, dünya haritası üzerinde çizgi olarak gösterilir. Konumlar çevrimdışı bir IP konum veritabanından gelir (ör. DB-IP Lite, CC-BY). Veritabanı isteğe bağlı indirilir. Çevrimiçi sorgu varsayılan olarak kapalıdır, gizlilik için. | ⬜ L |

**Teknik not:** reqwest DNS, TCP ve TLS sürelerini ayrı ayrı vermiyor. Bunun için zamanlayan bir connector yazılacak (hyper-util ile). TLS sertifika bilgisi reqwest'in `tls_info` özelliğinden alınacak.

### C. Koleksiyonlar
| Özellik | Açıklama | Durum |
|---|---|---|
| **Klasör = koleksiyon** | Bir klasör içindeki `apix.toml` ve istek dosyaları bir koleksiyon oluşturur. | ✅ |
| **Ağaç görünümü** | Klasörler açılıp kapanır, her istekte metot rozeti görünür. | ✅ |
| **Oluştur / yeniden adlandır / çoğalt / sil** | Sağ tık menüsünden ve kısayollardan. Silinen dosyalar çöp kutusuna gider, hemen kalıcı silinmez. | ✅ |
| **Metot seçerek yeni istek** (not 7) | "+" menüsünde GET, POST, PUT, PATCH, DELETE … seçilir; istek o metotla ve uygun gövde tipiyle açılır (POST/PUT/PATCH için JSON). | ✅ |
| **Sürükle-bırak** | İstekleri ve klasörleri taşıma ve sıralama. Bir istek veya klasör, başka bir klasörün, koleksiyonun ya da isteğin üstüne bırakılınca oraya taşınır (scriptler ve `folder.toml` da gider; koleksiyonlar arası da olur). | ✅ taşıma · ✅ elle sıralama (satırın üst/alt kenarına bırakınca önüne/arkasına, klasörün ortasına bırakınca içine; sıra `apix.toml` ve `folder.toml` içinde `order` listesi olarak saklanır, yeniden adlandırma ve taşımada güncellenir; Postman içe aktarma Postman'daki sırayı korur, dışa aktarma bu sırayı izler) |
| **Miras alma** | Klasör ve koleksiyon seviyesinde auth, header, değişken ve script tanımlanabilir. Alt öğeler bunları devralır. Auth için klasörde `folder.toml`, koleksiyonda `apix.toml`, projede `apix-project.toml` kullanılır (sağ tık → Auth…). | ✅ auth · ⬜ header ve script M |
| **Dokümantasyon** | Her koleksiyon, klasör ve istek için Markdown açıklama | ⬜ S |
| **Örnekler (examples)** | Kaydedilmiş istek ve yanıt çiftleri | ⬜ M |
| **Filtre ve arama** | Kenar çubuğunda anlık filtre, komut paletinde bulanık (fuzzy) arama | ⬜ S |
| **Çoklu koleksiyon** | Bir çalışma alanında birden fazla koleksiyon açılabilir. | ✅ projelerle (§C1) |
| **Dosya izleme** | Dosya diskte değişirse (git pull veya başka bir editör) uygulama anında günceller. | ✅ |
| **Git durumu** | Kenar çubuğunda değişen dosyalar işaretlenir. Durum çubuğunda branch adı görünür. | ⬜ M |
| **Sekmeler** | Birden çok istek sekmede açık durur. Kaydedilmemiş değişiklik işareti olur. Ctrl+Tab ile sekmeler arası geçilir. Her sekme kendi taslağını, yanıtını ve sürmekte olan isteğini saklar; Ctrl+W ve orta tık kapatır. | ✅ · ✅ açık sekmeleri oturumlar arasında hatırlama (proje ya da koleksiyon başına `state.toml` içinde; şifreli koleksiyonların sekmeleri dosya adları gizli kalsın diye yazılmaz) |
| **Geçmiş** | Gönderilen her istek yanıtıyla birlikte kaydedilir. Aranabilir ve tekrar gönderilebilir. | ✅ Ctrl+H penceresi (arama, geri yükleme, temizleme); istek `{{değişkenler}}` çözülmeden saklanır; son 300 kayıt, gövde 256 KB; şifreli koleksiyonlar kaydedilmez; Ayarlar'dan kapatılabilir · ⬜ yanıtları karşılaştırma (§K) |

### C1. Projeler: Proje → Ortam → Koleksiyonlar (not 6)
Koleksiyonlar projeler altında toplanır, ortamlar projeye aittir.

```
A Projesi/                      apix-project.toml  (ad, renk, koleksiyon sırası)
├── environments/
│   ├── test.toml               ← projedeki tüm koleksiyonlarda geçerli
│   └── prod.toml
├── kullanicilar/               ← koleksiyon (apix.toml)
├── odemeler/                   ← koleksiyon
└── raporlar/                   ← koleksiyon; "sadece Test'te göster" ile işaretlenebilir
```

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Aç / Yeni koleksiyon / Yeni proje** | Başlangıç ekranı ve komut paletinde üç seçenek. "Yeni proje" sihirbazı ad, klasör ve ilk ortamları (Test, Prod) sorar. | ✅ Kenar çubuğundaki proje/koleksiyon menüsünden (yeni proje Test ve Prod ortamlarıyla oluşur). Komut paletinde de var. |
| **Son açılanlar** | Projeler ve koleksiyonlar, son açılma sırasıyla. Boş ekranda "RECENT" listesi ve proje menüsünde "Recent"; kullanıcı ayarlarında `state.toml`. | ✅ |
| **Proje seçici** | Kenar çubuğunun üstündeki "Shop ▾" menüsü: son açılanlar, Aç, Yeni proje. Windows'ta başlık çubuğu sürükleme alanı olduğu için oraya konmadı. | ✅ |
| **Proje ortamları** | Projenin `environments/` klasörü, içindeki tüm koleksiyonlara uygulanır. Koleksiyonun kendi ortamı varsa onun üstüne eklenir. Katman sırası: proje → koleksiyon → ortam → yerel. | ✅ (CLI `--env` de aynı katmanları kullanıyor) |
| **Özelleştirilebilir gruplama** | Kenar çubuğu iki şekilde gösterilebilir: "Proje → Ortam → Koleksiyonlar" ya da "Proje → Koleksiyonlar". Bir koleksiyon belirli ortamlara bağlanabilir, örneğin "Raporlar sadece Test'te". Gruplara renk ve sıra verilebilir. | ✅ gruplama ve "Show in environment" sabitleme · ✅ renk (koleksiyon sağ tık › Renk; 8 renk, `apix-project.toml` içinde `[colors]`; kenar çubuğunda ve sekmede nokta) ve sürükleyerek sıra (`order`) |
| **Var olan koleksiyonu projeye ekleme** | Sürükle-bırak ile ya da "Projeye taşı" menüsüyle | ✅ Dosya › Bir projeye taşı… (tek koleksiyon açıkken) ve proje açıkken koleksiyon klasörünü pencereye bırakma (onay sorar); farklı sürücüde kopyalayıp asılını siler (`project::move_collection_into`) |
| **Prod koruması** | Prod ortamı seçiliyken pencere kenarında kırmızı bir çizgi görünür. DELETE, PUT ve PATCH göndermeden önce onay istenir (açılıp kapatılabilir). | Kırmızı çizgi ✅ (adında "prod" geçen ortamlar) · onay ✅ (çözülmüş URL'yi gösterir; Ayarlar › Üretimde onay iste) |

### C2. Şifreli koleksiyonlar 🔐 (not 2)
Bir koleksiyon (veya proje) parola ile şifrelenir. Parolayı bilmeyen hiçbir araç, dosyalar elinde olsa bile içeriği okuyamaz.

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Şifreli koleksiyon oluşturma** | "Yeni koleksiyon" ekranında "Şifrele" seçeneği ve parola. Parola gücü göstergesi. | ✅ File › New encrypted collection… (projede) |
| **Var olanı şifreleme / şifreyi kaldırma** | Sağ tık → "Şifrele…" / "Şifreyi kaldır…" (parola sorulur) | ✅ (düz dosyalar çöp kutusuna gider) |
| **Kilit açma** | Açarken parola sorulur. Anahtar sadece bellekte tutulur. İsteğe bağlı olarak işletim sistemi anahtarlığında hatırlanır. Belirli bir süre işlem yapılmazsa otomatik kilitlenir. | ✅ parola veya kurtarma anahtarıyla, 15 dk otomatik kilit, isteğe bağlı anahtarlıkta hatırlama (Windows Credential Manager, macOS Keychain, Linux Secret Service; kilit açma penceresinde "Bu bilgisayarda hatırla", Dosya menüsünde ve koleksiyon menüsünde "Kayıtlı parolayı unut", CLI de kullanır) |
| **Ne şifrelenir** | İstek dosyaları, ortamlar, scriptler ve klasör/istek adları. Dosya adları anlamsız kimliklere dönüşür, adlar şifreli bir dizinde tutulur. Diskte sadece "bu şifreli bir Apix koleksiyonudur" bilgisi açık kalır. | ✅ motor (`apix-vault`) · ⬜ uygulamaya bağlama |
| **Algoritma** | XChaCha20-Poly1305 (dosya başına rastgele nonce) ve Argon2id ile paroladan anahtar türetme (RustCrypto crate'leri) Veri anahtarı parola ve kurtarma anahtarıyla ayrı ayrı sarılır; parola değişince dosyalar yeniden şifrelenmez. | ✅ |
| **Kurtarma anahtarı** | Oluştururken bir kez gösterilen kurtarma anahtarı. Parola ve kurtarma anahtarı ikisi birden kaybolursa veri açılamaz; bu, arayüzde açıkça yazar. | ✅ motor (160 bit, `XXXX-XXXX-…`) · ⬜ arayüz |
| **CLI ve CI** | `apix run --password-env APIX_PASSWORD` (varsayılan `APIX_PASSWORD`; yoksa terminalde sorulur), `apix encrypt`, `apix decrypt` | ✅ |
| **Git** | Her dosya ayrı şifrelendiği için git hâlâ hangi isteğin değiştiğini görür, ama içeriğini göremez. `.local.toml` gizli değerleri `objects/local/` altında ayrı tutulur ve git'e girmez. | ✅ motor |

**Dürüst sınır:** Apix açık kaynak olduğu için güvenlik, formatın gizli olmasından değil algoritmadan ve parolanın gücünden gelir. Kilidi açık koleksiyonun ekran görüntüsü veya panoya kopyalanan bir değer elbette korunmaz.

### D. Postman uyumluluğu ve içe/dışa aktarma ⭐ (öncelikli)
| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Postman Collection v2.1 ve v2.0 içe aktarma** | Klasörler, istekler, header'lar, raw, urlencoded, form-data (dosyalarla), binary ve GraphQL gövdeleri, bearer, basic, API key, Digest, OAuth 1.0, OAuth 2.0 ve AWS auth (klasör ve koleksiyon auth'u olduğu yerde kalır, istekler miras alır), koleksiyon değişkenleri, açıklamalar, path değişkenleri. NTLM, Hawk gibi desteklenmeyen auth tipleri ve örnek yanıtlar raporda listelenir; dosyası seçilmemiş alanlar ve loopback olmayan OAuth yönlendirme adresleri için uyarı verilir. | ✅ |
| **Postman Environment içe aktarma** | `*.postman_environment.json`. `secret` tipindeki değerler `*.local.toml` dosyasına yazılır, git'e girmez. | ✅ |
| **Postman Globals içe aktarma** | Tanınıyor, ortam olarak aktarılıyor. Toplu içe aktarmada projenin `[vars]` bölümüne yazılır (gizli globals commit'lenmesin diye alınmaz, raporda belirtilir). | ✅ |
| **Postman scriptleri** | İsteklerin yanına `.js` olarak kaydediliyor ✅. `pm.*` uyumlu API ile çalıştırılması M5'te (§F). | ✅ çalışıyor (§F), `pm.sendRequest` ve `require` dahil |
| **İçe aktarma raporu** | Neyin aktarıldığı, neyin desteklenmediği ve hangi istekte olduğu listelenir. Hiçbir şey sessizce kaybolmaz. | ✅ |
| **Sürükle-bırak ile içe aktarma** | `.json` dosyasını pencereye bırakmak yeterli. | ✅ kod · ⬜ elle test |
| **Otomatik tanıma** (not 1) | Postman export'u içeren bir klasör açıldığında ya da sürüklendiğinde Apix bunu fark eder ve "Postman koleksiyonu bulundu, içe aktarılsın mı?" diye sorar. Dosya adına değil içeriğe bakarak tanır (v1, v2.0, v2.1, ortam, globals). | ✅ koleksiyon olmayan bir klasör açılınca/bırakılınca 3 seviye derinliğe kadar taranır (arka planda, en çok 400 dosya) |
| **Postman toplu dışa aktarma** | Postman "Export data" ile verilen, tüm koleksiyon ve ortamları içeren klasör veya zip tek seferde bir projeye (§C1) dönüştürülür. | ✅ klasör ve zip; aynı adlı ortamlar `-2` ile ayrılır; CLI `apix import <klasör\|zip>` |
| **Postman v1** | Tanınıyor; kullanıcıya v2.1 olarak yeniden dışa aktarması söyleniyor. | ✅ |
| **Postman'e dışa aktarma** | Apix koleksiyonu Postman v2.1 formatına dönüştürülür, böylece ekip arkadaşı Postman kullanmaya devam edebilir. | ✅ koleksiyon + ortam dosyaları; içe aktarmanın tersi (gidiş-dönüş testiyle aynı sonuç); Postman'ın tutamadığı ayarlar ve gizli ortam değerleri raporlanır; sağ tık, Dosya menüsü, palet, CLI `apix export` |
| **CLI ile içe aktarma** | `apix import <dosya> [--out klasör] [--into koleksiyon]` | ✅ |
| Insomnia v4, Bruno, Hoppscotch | | ✅ Insomnia v4 JSON (`apix_import::insomnia`): çalışma alanı → koleksiyon, istek grubu → klasör (auth'uyla), temel ortam → koleksiyon değişkenleri, alt ortamlar → ortamlar (iç içe değerler `a.b`), `{{ _.x }}` → `{{x}}`, bearer/basic/digest/API anahtarı/OAuth 1-2/AWS; gRPC ve WebSocket istekleri ile `{% %}` etiketleri raporlanır · ⬜ Insomnia v5 YAML, Bruno, Hoppscotch |
| OpenAPI 3 / Swagger 2 | İçe aktarma ve koleksiyon üretme | ✅ JSON ve YAML (`apix_import::openapi`): işlem başına istek, etiket başına klasör, `baseUrl` (her sunucu bir ortam), yol parametreleri `{{değişken}}` (örnek değerleriyle), sorgu/başlık parametreleri (zorunlu olanlar açık), örnekten ya da şemadan gövde (`$ref`, `allOf`, `oneOf`, biçimler, readOnly hariç), form/multipart/XML, güvenlik → auth (bearer, basic, API anahtarı, OAuth2), kimlik bilgileri boş değişken; gerçek API'ye karşı denendi |
| HAR | Tarayıcı kayıtlarını içe aktarma | ✅ (`apix_import::har`): ana makine başına klasör, statik dosyalar ve tekrarlar atlanır, `Cookie` başlığı oturum taşıdığı için alınmaz, Bearer → auth, JSON/form/multipart/XML/HTML gövdeler |
| **Biçimi içerikten tanıma** | Tek dosya, klasör ya da zip: Postman, Insomnia, OpenAPI/Swagger, HAR hepsi içerikten tanınır (`apix_import::detect`); tek dosyadan tek koleksiyon çıkarsa koleksiyon, birden fazlaysa proje oluşturulur | ✅ masaüstü (Dosya › İçe aktar, sürükle-bırak, klasör açınca öneri) ve CLI `apix import` |
| cURL | İçe ve dışa aktarma | ✅ (yapıştırma ve "Copy as", §I) |

### E. Ortamlar ve değişkenler
| Özellik | Durum |
|---|---|
| Kapsamlar: global → koleksiyon → ortam → yerel ve script. Sonraki kapsam öncekini ezer. | ✅ motor |
| `{{değişken}}` çözümleme. İç içe değişkenler çalışır, döngüye girmez, eksikler raporlanır. | ✅ |
| Ortam seçici (kenar çubuğunda, koleksiyon başına hatırlanır) | ✅ |
| Ortam editörü (Ctrl+E, "Değişkenler" penceresi): ortak değişkenler ve ortamlar tek yerde, ortam ekleme, yeniden adlandırma, silme (çöp kutusuna), gizli değerler (••••, göz düğmesiyle görünür). Kaydet'e basılana kadar diske hiçbir şey yazılmaz. | ✅ · ⬜ başlangıç ve güncel değer ayrımı |
| **Gizli değerler:** `*.local.toml` (git'e girmez; `.gitignore` satırı kendiliğinden eklenir, ✅) ve işletim sistemi anahtarlığı (Keychain / Credential Manager / Secret Service) | ✅ `.local.toml` · ✅ anahtarlık (Ayarlar › Ortam gizli değerleri; `.local.toml` yalnızca anahtarlık kaydının kimliğini tutar, CLI de okur; seçenek değişince açık ortamlar taşınır; şifreli koleksiyonlarda gizli değerler kasada kalır) |
| **Dinamik değişkenler:** `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` ve 23 tane daha. Her kullanımda yeni değer; aynı adla tanımlanan değişken önce gelir. Listesi pencerede, tıklayınca kopyalanır. | ✅ |
| Prod ortamı uyarısı: prod seçiliyken pencere kenarında kırmızı ince bir çizgi | ✅ |

### F. Scriptler ve testler
| Özellik | Durum |
|---|---|
| **JS motoru:** QuickJS (`rquickjs`). Küçüktür (yaklaşık 1 MB), hızlı açılır, her istek kendi izole ortamında çalışır. | ✅ `apix-script`: her olay (istek öncesi / test) yeni bir QuickJS çalışma zamanında; 10 sn süre ve 256 MB bellek sınırı; `pm.*` API'sinin çoğu `prelude.js` içinde, veri JSON olarak girip çıkar |
| **Pre-request script:** istek gönderilmeden önce çalışır. Değişken atayabilir, header ekleyebilir, imza hesaplayabilir. | ✅ `pm.request` (yöntem, URL, sorgu, başlıklar, ham ve urlencoded gövde değiştirilebilir), `pm.environment` / `pm.collectionVariables` / `pm.globals` (= proje değişkenleri) / `pm.variables`, `replaceIn`, `pm.info`, `pm.execution.setNextRequest` / `skipRequest`; sıra: koleksiyon (`_collection.prerequest.js`) → klasörler (`_folder.prerequest.js`) → istek; hata isteği durdurur. Script'in atadıkları diske yazılmaz: uygulamada açık kaldıkça, CLI'da koşu boyunca geçerlidir; çiplerde "Bir script atadı" görünür |
| **Test script:** `pm.test`, `pm.expect` (Chai tarzı), `pm.response.json()`, `pm.environment.set` ... | ✅ `pm.test` (+ `skip`), `pm.expect` (equal, eql, deep, include, property, lengthOf, above/below, a/an, match, oneOf, keys, members, jsonSchema …), `pm.response.to.have.status/header/jsonBody/jsonSchema`, `pm.response.to.be.ok/notFound/…`, eski `tests["…"]`, `responseBody`, `responseCode`, `postman.setEnvironmentVariable`; `console.*`, `atob`/`btoa` |
| `pm.sendRequest` ile script içinden istek gönderme (örneğin token alma) | ✅ geri çağırma ve `await`; scriptler async fonksiyon olarak çalışır; isteği çağıran taraf gönderir (CLI kendi çalışma zamanıyla, uygulama kendi istemcisi ve çerezleriyle); bekleme süresi 10 sn sınıra sayılmaz |
| Hazır kütüphaneler: `crypto-js`, `lodash`, `moment`, `uuid`, `ajv` (JSON Schema) | ✅ API scriptlerinin kullandığı kısımlar Apix için yazıldı (`libs.js`, indirme yok): crypto-js (MD5, SHA-1/224/256/384/512 ve HMAC'leri Rust'ta; Hex/Base64/Base64url/Utf8/Latin1), uuid, lodash (~70 fonksiyon, `_` olarak da), moment (ayrıştırma, format, add/subtract, diff, karşılaştırmalar, utc), ajv, tv4, chai; JSON Schema denetimi genişletildi (additionalProperties, allOf/anyOf/oneOf/not, format, minItems …) · ⬜ crypto-js AES/PBKDF2, cheerio, xml2Json |
| **Kodsuz testler:** `status == 200`, `body.users.length > 0`, `time < 300ms` gibi tablo hâlinde tanımlanan testler | ✅ istek dosyasında `[[assert]]` (hedef: status, time, size, body, body.<yol>, header.<ad>; işleç: == != > >= < <= contains, not contains, exists, not exists, matches, is); Scripts sekmesinde "Doğrulamalar" tablosu; test scriptlerinden sonra `pm.test` olarak çalışır |
| Yanıttan değişken çıkarma: "bu alanı `token` değişkenine kaydet" (sağ tık ile) | ✅ yanıt durum satırında **{x}** düğmesi: hedef + değişken adı → seçili ortama (yoksa koleksiyona) yazılır; "Her gönderimde güncelle" Testler scriptine `pm.environment.set(…)` satırı ekler |
| Test sonuçları sekmesi (✓ / ✗ ve hata mesajı) | ✅ yanıt panelinde "Testler 3/4" sekmesi; istek tarafında Scripts sekmesi (İstek öncesi / Testler, JavaScript renklendirme, kirli işareti, Ctrl+S ile `<ad>.prerequest.js` / `<ad>.test.js` olarak kaydedilir) |
| Script konsolu: `console.log` çıktıları ve ağ günlüğü (Postman Console karşılığı) | ✅ `console.*` çıktıları (Konsol sekmesi, CLI'da `│` ile) · ⬜ ağ günlüğü |

### G. Collection Runner ve CLI (Newman karşılığı)
| Özellik | Durum |
|---|---|
| `apix send <dosya>` | ✅ |
| `apix run <klasör>`: hata olursa sıfırdan farklı çıkış kodu döner | ✅ script'leri de çalıştırır: test sonuçları (✓/✗), konsol, `setNextRequest` (sıra değiştirme, `null` ile durdurma); testi olan istek testlere, testi olmayan durum koduna göre geçer/kalır |
| `--env`, `--var anahtar=değer` | ✅ |
| Runner arayüzü: sıra seçimi, tekrar sayısı, istekler arası bekleme, durdur ve devam et | ✅ Runner penceresi (Ctrl+Shift+R; kenar çubuğunda koleksiyon/klasör › Çalıştır…): istek seçimi ve ↑↓ ile sıra, ortam, tekrar, bekleme, veri dosyası, ilk hatada dur, duraklat/devam et/durdur; pencere kapansa da koşu sürer; sonuçlar testleri ve konsoluyla |
| **Veri odaklı test:** CSV veya JSON dosyasındaki her satır için koleksiyonu çalıştırma | ✅ Runner ve `apix run --data`; sütunlar `{{değişken}}` ve `pm.iterationData` olur; CSV'de tırnak, satır sonu, BOM ve `;` ayraç okunur |
| Raporlar: JUnit XML (CI için), JSON, HTML | ✅ Runner'da "Raporu kaydet" ve `apix run --junit/--json/--html`; HTML tek dosya, açık/koyu tema |
| `--bail` (ilk hatada dur), `--parallel N`, `--timeout` | ✅ `--bail`, `--iterations`, `--delay`, `--timeout`, `-k`, `--proxy`, klasör koşusu · ⬜ `--parallel N` (sonraya) |
| GitHub Actions örneği ve `apix` için resmi Docker imajı | ✅ depo bir GitHub Action (`action.yml`: `uses: 11R5/Apix@…`, `apix`'i derleyip önbelleğe alır, `path`/`environment`/`data`/`junit`/`html` … girdileri), ortam matrisli örnek (`examples/ci/github-actions.yml`), `Dockerfile` ve sürüm etiketinde ghcr.io'ya yayımlayan iş akışı · ⬜ Docker imajı Linux'ta henüz derlenip denenmedi |
| **CI'da gizli değerler** | ✅ `APIX_VAR_<AD>` ortam değişkenleri ve `--env-file` (dotenv); adlar koleksiyonun tanımladığı ya da isteklerinin kullandığı değişkenle büyük küçük harf gözetmeden eşleşir; sıra: ortam < `--env-file` < `APIX_VAR_*` < `--var`; hangi adların nereden geldiği stderr'e yazılır (değerler asla) |

### H. Diğer protokoller
| Protokol | Ayrıntı | Durum |
|---|---|---|
| **GraphQL** | Şema introspection, otomatik tamamlama, değişkenler, şema gezgini | ⬜ L |
| **WebSocket** | Bağlan ve bağlantıyı kes, mesaj gönder, mesajları zaman damgasıyla listele, kayıtlı mesaj şablonları | ✅ `ws://`/`wss://` adresi isteği WebSocket yapar; el sıkışmada parametre, başlık ve auth (OAuth 2.0 dahil); Mesajlar sekmesi (Ctrl+Enter gönderir, `{{değişkenler}}` doldurulur, bağlantı yoksa önce bağlanır), dosyada `[[message]]` şablonları; ↑/↓ zaman damgalı günlük, kapanış kodu ve nedeni; sekme arkadayken de sürer; `apix send` (`--message`, `--listen`), `apix run` atlar · ⬜ ikili mesaj gönderme, ping aralığı, alt protokol seçimi, TLS doğrulamasını kapatma |
| **Server-Sent Events** | Canlı olay akışı | ✅ `text/event-stream` yanıtında olaylar geldikçe (zaman, tür, id, veri); Durdur/Esc gelenleri tutar; biten akış Biçimli sekmesinde liste; `apix send` olayları geldikçe yazar; motor `send_with` ile gövdeyi parça parça bildirir · ⬜ olay türüne göre süzme, kopan bağlantıda `retry` ile yeniden bağlanma |
| **gRPC** | Unary ve streaming çağrılar, `.proto` dosyası veya sunucu yansıması (reflection), metadata | ⬜ L · kullanıcı isteği (not 17): M4 içe aktarıcılarından sonra öne alınabilir |
| **Socket.IO** | | ⬜ M |
| **MQTT** | Abone ol ve yayınla | ⬜ M |
| **SOAP** | WSDL'den şablon üretme, XML zarf | ⬜ M |

### I. Geliştirici araçları
| Özellik | Durum |
|---|---|
| **Kabuk komutu olarak kopyala** (not 8): sağ tık ve URL çubuğundaki "Kopyala ▾" menüsü. Değişkenler çözülmüş veya `{{değişken}}` olarak bırakılabilir. Gizli değerler maskelenip maskelenmeyeceği sorulur. Seçenekler: **bash/zsh/sh** için `curl` (tek tırnak kaçışlı) · **Windows cmd** için `curl.exe` (`^` satır devamı) · **PowerShell** için `Invoke-RestMethod` ve `Invoke-WebRequest` ile `curl.exe` sürümü · **fish** · **HTTPie** · **wget** | ✅ bash, cmd, PowerShell (gerçek kabuklarda httpbin ile doğrulandı) · ✅ fish (`curl`, fish tırnaklarıyla), HTTPie (`http`, 3.0+ için `--raw`), wget (GNU; multipart yapamadığı için not düşer) — birim testleriyle doğrulandı, bu makinede gerçek araçlarla çalıştırılmadı |
| **Kabuk komutundan içe aktar:** URL çubuğuna yapıştırılan `curl` (bash veya cmd biçimi) ve PowerShell `Invoke-RestMethod` komutları isteğe dönüşür | ✅ curl (bash ve cmd biçimi, tarayıcıların "Copy as cURL" çıktısı dahil) · ⬜ PowerShell |
| **Kod üretici:** fetch, axios, Python requests, Go, **Rust reqwest**, C#, Java, PHP, Swift, Kotlin, Node | ✅ 10 dil (fetch Node'u da kapsar); istek ayarları koda yazılır, yazılamayan ayar `Not in this snippet` yorumuyla belirtilir; fetch, C#, Java ve Rust çıktıları yerel bir yankı sunucusunda motorla birebir karşılaştırıldı; CLI `apix code --as <dil>` |
| Çerez yöneticisi: alan adına göre listeleme, silme, tümünü boşaltma; kavanoz `cookies.json` içinde kalıcı, istek başına kapatılabilir (İstek › Çerezler…) | ✅ listeleme, silme, kalıcılık · ⬜ elle ekleme/düzenleme |
| **Yakalama proxy'si:** tarayıcı veya telefon trafiğini kaydedip koleksiyona dönüştürme | ⬜ L |
| Sertifika yöneticisi | ⬜ S |
| Ağ konsolu: gerçekte gönderilen ham isteği ve gelen yanıtı gösterir | ⬜ M |

### J. Simülasyon, dokümantasyon ve izleme
| Özellik | Durum |
|---|---|
| **Mock sunucu:** kayıtlı örneklerden veya OpenAPI'den yerel sahte sunucu (`apix mock`) | ⬜ L |
| **Dokümantasyon üretme:** koleksiyondan statik HTML doküman sitesi (`apix docs`) | ⬜ M |
| **Monitörler:** zamanlanmış çalıştırma ve hata bildirimi. Yerel zamanlayıcı veya CI ile, bulut gerekmez. | ⬜ M |
| **Flows:** istekleri görsel düğümlerle birbirine bağlama | ⬜ L |
| **API tasarımı:** OpenAPI dosyasını düzenleme ve doğrulama | ⬜ L |

### K. Apix'e özel özellikler (Postman'de olmayanlar)
| Özellik | Açıklama |
|---|---|
| **Yük testi** | `apix bench` ve arayüzde grafik. Binlerce eşzamanlı istek, p50/p95/p99 gecikmeleri. |
| **Yanıt karşılaştırma** | İki çalıştırmayı veya iki ortamı yan yana gösterir. Hazır `Diff` bileşeni kullanılacak. |
| **Zaman yolculuğu geçmişi** | Her istek ve yanıt saklanır. Yanıtın dün ve bugünkü hâli karşılaştırılabilir. |
| **MCP sunucusu** | `apix mcp` ile yapay zeka ajanları koleksiyonlarını listeleyip çalıştırabilir. |
| **AI yardımcısı** | Test yazma ve hata açıklama, kendi API anahtarınla. Varsayılan olarak kapalı. |
| **WASM eklentileri** | Özel auth tipleri, içe aktarıcılar ve görselleştiriciler eklenebilir. |
| **Performans göstergesi** | FPS, bellek ve kare süresi. Geliştirici modunda açılır. |

### L. İşbirliği (bulutsuz)
- ⬜ Git tabanlı çalışma: branch, diff ve commit uygulamanın içinden yapılır.
- ⬜ Uygulama içi çakışma (conflict) çözümü. TOML dosyaları satır satır birleştirilebildiği için bu kolaydır.
- ⬜ Bir isteği link olarak paylaşma: dosyadan veya panodan `apix://` bağlantısı.

### M. Uygulama geneli
| Özellik | Durum |
|---|---|
| Şeffaf ve blur'lu pencere, kendi başlık çubuğu | ✅ |
| **Uygulama simgesi** (not 4): kaynak `Logo/LogoFirst.jpg` (2048×2048). Windows için `.ico` (16–256 px, exe içine gömülü), macOS için `.icns` (16–1024 px, @2x dahil), Linux için hicolor PNG'leri ve `.desktop` dosyası. Ayrıca başlık çubuğundaki işaret, README ve GitHub sosyal önizlemesi. Ayrıntılar M3'te. | ✅ · ⬜ GitHub sosyal önizlemesi |
| Komut paleti (Ctrl/⌘+K, Ctrl/⌘+P) | ✅ |
| **Menü çubuğu** (başlık çubuğunda): File, Request, Environment, View, Help. Kısayollar menüde görünür. Kenar çubuğundaki "Open or create" menüsünün yerini aldı. Windows'ta başlık çubuğu sürükleme alanı olduğu hâlde tıklanabilir (menü düğmeleri sürükleme alanını örter). macOS'ta yerel menü çubuğu M9'da. | ✅ |
| Kısayol tablosu ve kısayolları özelleştirme | ⬜ M |
| **Ayarlar ekranı** (not 13, Ctrl+,): arayüz dili, tema, şifreli koleksiyonların otomatik kilit süresi; her değişiklik anında kaydedilir (`settings.toml`, kullanıcı ayar klasöründe). Sonra: cam ayarı, font, proxy, güncelleme. | ✅ dil, tema, otomatik kilit · ⬜ diğerleri |
| **Arayüz dili** (not 13): İngilizce, Türkçe, Arapça, Almanca, Fransızca, Rusça, Japonca, Çince (basitleştirilmiş), İspanyolca. Ayarlardan anında değişir, yeniden başlatma gerekmez. Arapça için sağdan sola düzen (kenar çubuğu, sekmeler ve metin hizası aynalanır). Japonca, Çince ve Arapça için yazı tipi yedekleri. Her dil tek bir dosya (`locales/<kod>.yml`, `rust-i18n`; gpui-kit'in kendi metinleri de aynı sistemle çevrilir). Yeni dil eklemek: bir dosya ve dil listesine bir satır. Eksik çeviri İngilizceye düşer; CI eksik anahtarları raporlar. Çeviriler ilk etapta makine çevirisi + gözden geçirme; anadili konuşan katkıcılar düzeltebilir. | ✅ 9 dil, yaklaşık 414 anahtar, anında geçiş, sistem dilinden ilk seçim, eksik anahtar ve yer tutucu testi, Arapça için sağdan sola düzen (kenar çubuğu sağda, yanıt paneli solda, pencere düğmeleri solda; menüler, sekmeler, form satırları, diyaloglar ve durum çubuğu sağdan başlar; kod, URL ve anahtar–değer tabloları soldan sağa kalır; dil değişince anında aynalanır), sağdan sola paragrafların satır kaydırması (gpui yapamadığı için `bidi::para` satırları okuma sırasıyla kendisi böler) · ⬜ motor hata mesajlarının çevirisi, gpui-kit'in kendi metinleri (yalnız Çince var), anadili konuşanların gözden geçirmesi |
| Otomatik güncelleme | ⬜ M |
| Erişilebilirlik: klavyeyle tam kullanım, yüksek kontrast | ⬜ M |
| Çökme raporu (isteğe bağlı, varsayılan kapalı) | ⬜ S |

---

### N. Dokümantasyon ve tanıtım (not 14, 15, 16)
Dokümantasyon bu iş planından ayrıdır. Her dilin kendi klasörü vardır ve her biri aynı belge setini içerir:

```
Docs/
├── languages.toml           desteklenen diller (kod, ad, yön, kaynak sürüm)
├── en/                      kaynak dil (diğerleri bundan çevrilir)
│   ├── user-guide.md        kullanım kılavuzu, ekran görüntüleriyle
│   ├── architecture.md      mimari
│   ├── technical.md         teknik altyapı: crate'ler, bağımlılıklar, derleme, performans bütçesi
│   ├── workflows.md         iş akışları: istek gönderme, koleksiyon, ortam, şifreleme, içe aktarma, CI
│   └── images/              bu dildeki arayüzün ekran görüntüleri
├── tr/  ar/  de/  fr/  ru/  ja/  zh-CN/  es/   aynı yapı
└── IS-PLANI.md, ROADMAP.md  (plan; dokümantasyonun parçası değil)
```

| Özellik | Ayrıntı | Durum |
|---|---|---|
| **Dil başına belge seti** | Kullanım kılavuzu, mimari, teknik altyapı, iş akışları; 9 dilde ayrı dosyalar | ✅ yapı, İngilizce, Türkçe (M4) · ⬜ diğer 7 dil (M9) |
| **Dil sürümleri** | Her çeviri hangi İngilizce sürümden çevrildiğini başında yazar; İngilizce değişince eskiyen çeviriler CI'da listelenir | ✅ `cargo run -p docs-check` eskiyen ve eksik çevirileri listeler; yapı testi `cargo test` içinde |
| **Yeni dil ekleme** | `languages.toml`'a bir satır, yeni klasör, arayüz için `locales/<kod>.yml`. Kod değişikliği gerekmez. | ✅ |
| **Görselli kullanım kılavuzu** (not 15) | Her dilde, o dilin arayüzünden alınmış ekran görüntüleri. Görüntüler bir betikle otomatik alınır (uygulama örnek projeyle, ilgili dilde açılır), böylece arayüz değişince kılavuz bir komutla yenilenir. | ✅ İngilizce ve Türkçe, 7 ekran görüntüsü (elle alındı) · ⬜ betikle otomatik çekim ve diğer diller (M9) |
| **README: neden Apix** (not 16) | Postman, Insomnia, Bruno, Hoppscotch ve HTTPie ile karşılaştırma: yerel GPU arayüzü (Electron yok), dosya tabanlı koleksiyonlar, hesap zorunluluğu yok, şifreli koleksiyonlar, ağ denetçisi, CLI ile birebir aynı motor. Sadece doğrulanabilir iddialar. | ✅ (Postman, Insomnia, Bruno, Hoppscotch; HTTPie notu) |
| **Performans ölçümleri** (not 16) | Açılış süresi, boşta bellek, ikili boyutu, 100 MB JSON açma ve kaydırma; ölçüm betiği repoda, sonuçlar README'de ve her sürümde güncellenir. Rakip uygulamalar aynı makinede, aynı betikle ölçülür. | ✅ Apix: `scripts/measure.ps1` (24 Ekim: ikili 38.9 MB, ilk kare 278 ms, boşta 84 MB, 100 MB JSON 306 ms) · ⬜ karşılaştırmalı (M9) |

## 3. Postman → Apix karşılık tablosu
| Postman | Apix | Aşama |
|---|---|---|
| Request builder | URL çubuğu + sekmeler | M1 |
| Collections | Klasör ve TOML dosyaları + ağaç görünümü | M2 |
| Import / Export | Postman v2.1, Insomnia, OpenAPI, HAR, cURL | M2–M4 |
| Environments / Variables | Katmanlı değişkenler + ortam seçici | M3 |
| Authorization | Tüm tipler | M3–M4 |
| Pre-request / Tests | QuickJS, `pm.*` uyumlu | M5 |
| Collection Runner / Newman | Runner arayüzü + `apix run` | ✅ M5 |
| Console | Ağ ve script konsolu | M5 |
| GraphQL / WebSocket / gRPC / MQTT / SOAP | `apix-proto` | M6 |
| Code snippets | Kabuk komutu olarak kopyala (bash, cmd, PowerShell) M3 · `apix-codegen` (10 dil) M4 ✅ | M3–M4 |
| Cookies / Certificates / Proxy | Yöneticiler + yakalama proxy'si | M4–M7 |
| Mock servers | `apix mock` (yerel) | M7 |
| Documentation | `apix docs` | M7 |
| Monitors | Yerel zamanlayıcı veya CI | M7 |
| Flows | Düğüm editörü | M8 |
| Visualizer | Tablo ve grafik görünümü | M7 |
| Workspaces / Team | Projeler (§C1) M3 · git tabanlı işbirliği M7 · Tailscale ekip modu 💭 | M3–M7 |
| — (Postman'da yok) | Şifreli koleksiyonlar (§C2) | M4 |
| Console / zamanlama | Ağ denetçisi ve harita (§B2) | M4–M7 |
| Postbot (AI) | İsteğe bağlı AI yardımcısı + MCP | M8 |

---

## 4. Aşamalar ve sıralı iş listesi

Her aşamanın sonunda kullanılabilir bir sürüm çıkar.

### M0 — Temel ✅ (tamamlandı)
- [x] Cargo workspace ve hız/boyut için ayarlanmış release profili
- [x] `apix-core`: istek modeli, değişkenler, TOML formatı (9 test)
- [x] `apix-http`: reqwest + rustls, TTFB ve toplam süre (2 test)
- [x] `apix` CLI: `send` ve `run`, `--env` ve `--var` ile
- [x] Masaüstü kabuğu: blur'lu pencere, başlık çubuğu, pencere düğmeleri

### M1 — Arayüzden gerçek istek ✅ (tamamlandı)
1. [x] **Tasarım token'ları:** §1.3'teki paleti `theme.rs`'e taşımak, Obsidian ve Cyber temaları (S)
2. [x] **Fontları gömmek:** Inter 4.1 ve JetBrains Mono 2.304 (OFL 1.1), toplam 2.2 MB (S)
3. [x] **tokio ↔ GPUI köprüsü:** istekler arka planda çalışır, sonuç arayüze gelir, iptal edilebilir (M)
4. [x] **URL çubuğu:** düzenlenebilir alan, metot seçici, SEND düğmesi, Ctrl+Enter (M)
5. [x] **Yanıt paneli:** durum, süre, boyut, Pretty ve Raw görünümleri, Headers sekmesi (M)
6. [x] **JSON görüntüleyici:** renklendirme ve katlama. 512 KB üstü yanıtlar için satır tabanlı sanal görüntüleyici var. 100 MB JSON (9.6 milyon satır) release derlemede yaklaşık 0.33 saniyede ekrana geliyor. Tekerlek, kaydırma çubuğu ve klavye (↑ ↓ PgUp PgDn Ctrl+Home/End) ile geziliyor. Bellek 100 MB yanıtla yaklaşık 407 MB. (L)
7. [x] **Params ve Headers tabloları:** aç/kapa kutucuğu, sayı rozetleri (M)
8. [x] **Body editörü:** JSON, Text ve XML seçenekleri (M)

**Bitti sayılması için:** jsonplaceholder'a arayüzden GET ve POST gönderilip yanıt renkli olarak görülebilmeli.

### M2 — Koleksiyonlar ve Postman içe aktarma ⭐ ✅ (tamamlandı; son iki madde M3'te bitti)
1. [x] Klasör açma (Ctrl+O). Apix olmayan bir klasör seçilirse orada yeni koleksiyon oluşturmayı önerir. Klasör pencereye sürüklenince de açılır. "En son açılanlar" listesi M3'te eklendi. (S)
2. [x] Kenar çubuğu ağacı: gerçek dosyalardan, iç içe ve açılıp kapanan klasörler, metot rozetleri (M)
3. [x] İstek seçme, düzenleme ve kaydetme (Ctrl+S). Kaydedilmemiş değişiklik işareti var. Başka isteğe geçerken "Kaydet / Kaydetme / Vazgeç" sorusu çıkar. (M)
4. [x] Yeni istek (Ctrl+N) ve yeni klasör, yeniden adlandırma, çoğaltma, çöp kutusuna silme. Sağ tık menüsünden yapılır. Scriptler istekle birlikte taşınır. (M)
5. [x] Sürükle-bırak ile taşıma (M3'te yapıldı). Elle sıralama M4'te yapıldı. (M)
6. [x] Sekmeler (M3'te yapıldı) (M)
7. [x] **Postman v2.1/v2.0 koleksiyon içe aktarma** ve içe aktarma raporu (L). Kenar çubuğu düğmesi, sürükle-bırak ve CLI ile yapılabilir.
8. [x] **Postman environment içe aktarma** (S). `secret` değerler git'e girmeyen `*.local.toml` dosyasına yazılır.
9. [x] `apix import <dosya> [--out klasör] [--into koleksiyon]` CLI komutu, dosya türünü kendisi algılar (S)
10. [x] Dosya izleme: diskteki değişiklikler (git pull, başka editör) yarım saniye içinde yansır. Açık istek kaydedilmemiş değişiklik içermiyorsa editör de güncellenir. (M)

**Bitti sayılması için:** gerçek bir Postman koleksiyonu içe aktarılıp arayüzden gezilebilmeli ve istekleri çalıştırılabilmeli.

### M3 — Simge, projeler, ortamlar, kopyalama, auth ✅ (9 maddenin hepsi bitti; küçük kalanlar aşağıda)
1. [x] **Uygulama simgesi** (not 4) (M). `tools/icongen` aracı logodan PNG, ICO, ICNS ve Linux hicolor dosyalarını üretir. Simge exe'ye gömülü, başlık çubuğunda da görünüyor.
   - `Logo/LogoFirst.jpg` kırpılır: alttaki yansıma atılır, zemin squircle dışında saydam yapılır.
   - 16–32 px için sadeleştirilmiş bir sürüm hazırlanır (sadece kuş ve X). Küçük boyutta ince çizgiler kaybolur.
   - Windows için `.ico` exe'ye gömülür, macOS için `.icns`, Linux için hicolor PNG'leri ve `.desktop` dosyası.
   - Mümkünse saydam zeminli PNG veya vektör (SVG) bir ana dosya edinilir. JPG zemini sabit olduğu için en temiz sonuç bundan alınır.
2. [x] **Ortam seçici ve ortam editörü:** Seçici ✅ (kenar çubuğunda; Windows'ta başlık çubuğu sürükleme alanı olduğu için oraya konmadı). Koleksiyon başına hatırlanıyor, `.local.toml` gizli değerleri dahil. Ortam editörü ✅ (Ctrl+E). Dinamik değişkenler ✅ (`{{$guid}}` … 28 tane). OS anahtarlığı ✅ (M4'te). (M)
3. [x] **Projeler (§C1):** proje modeli (`apix-project.toml`), Yeni proje / Yeni koleksiyon / Aç, son açılanlar, proje menüsü, proje genelinde ortamlar (proje → koleksiyon → proje ortamı → koleksiyon ortamı), "Proje → Ortam → Koleksiyonlar" gruplaması, koleksiyonu ortamlara sabitleme, prod çizgisi. Renkler, koleksiyonu projeye taşıma ve prod onay penceresi M4'te yapıldı. (L)
4. [x] **Kabuk komutu olarak kopyala** (§I): bash `curl`, cmd `curl.exe`, PowerShell `Invoke-RestMethod`; URL çubuğundaki kopyala menüsü, sağ tık ve `apix shell` CLI komutu. Yapıştırılan `curl` komutu isteği doldurur. fish, HTTPie ve wget M4'te eklendi. (M)
5. [x] **Metot seçerek yeni istek** (§C): kenar çubuğundaki "+" menüsü, klasör ve koleksiyon menülerinde "New request ▸". POST, PUT ve PATCH JSON gövdeyle açılır. URL çubuğunda Alt+1…7 metodu değiştirir. (S)
6. [x] **Değişken çipleri** (§A2): URL çubuğunda renkli çipler, üstüne gelince değer, kapsam ve gizlilik bilgisi. Params, Headers ve form gövdesi alanları M4'te eklendi; ham gövde editörü kaldı. (M)
7. [x] **Auth sekmesi:** Inherit, No auth, Bearer, Basic, API key. Klasör, koleksiyon ve proje auth'u (sağ tık → Auth…, proje menüsü), istekler en yakın üst öğeden miras alır; masaüstü, CLI ve "Copy as" aynı kuralı kullanır. Postman içe aktarma auth'u olduğu yerde bırakır. OAuth 2.0 M4'e taşındı. (L)
8. [x] **Komut paleti** (Ctrl+K veya Ctrl+P): tüm istekler ve komutlar içinde bulanık arama ("crus" → "Create user"). Komutlar: gönder, kaydet, metotla yeni istek, klasör, koleksiyon, proje, aç, içe aktar, değişkenler, ortam seçimi, kopyala, auth, gruplama, son açılanlar. (M)
9. [x] **M2'den kalanlar:** sekmeler (her sekme taslağını, yanıtını ve süren isteğini saklar; Ctrl+W, Ctrl+Tab, orta tık), sürükle-bırak ile istek ve klasör taşıma (koleksiyonlar arası dahil). Elle sıralama M4'te yapıldı. (M)

**M3'ten küçük kalanlar:** hepsi M4'te yapıldı (aşağıda "M3'ten kalan küçük işler"). Yalnız ham gövde editöründe değişken çipi kaldı.

### M4 — Günlük kullanıma hazır
- **Şifreli koleksiyonlar (§C2)** (not 2): oluşturma, şifreleme ve şifreyi kaldırma, kilit açma ve otomatik kilit, kurtarma anahtarı, CLI desteği (L). Motor ✅ (`apix-vault`), uygulama ✅ (kilit açma, kilitleme, 15 dk otomatik kilit, şifreleme ve şifreyi kaldırma, parola değiştirme, yeni şifreli koleksiyon), CLI ✅, parolayı işletim sistemi anahtarlığında hatırlama ✅ (`apix_vault::keychain`, `keyring` crate'i; kayıt koleksiyon klasörünün mutlak yoluyla adlanır, yanlış çıkan kayıtlı parola silinip yeniden sorulur, parola değişince güncellenir, şifre kaldırılınca silinir).
- **Ağ denetçisi (§B2)** (not 9): yerel ağ bilgisi, DNS, TCP, TLS ve sertifika zinciri, zaman şelalesi, yönlendirmeler (L) Network sekmesi ve `apix send --network` ✅; TLS sürümü/şifre takımı, tam zincir ve TCP/TLS ayrı süreler M7'de (not 12).
- **Gövde tipleri (§A4)** ✅: düzenlenebilir urlencoded form, multipart (metin ve dosya alanları), binary ve GraphQL (renkli sorgu, JSON değişkenler). Motor, uygulama, CLI, "Copy as" (curl, cmd, PowerShell), curl yapıştırma (`-F`, `--form-string`, `--data-binary @dosya`) ve Postman içe aktarma hepsini taşır. Göreli dosya yolları isteğin klasörüne göre çözülür.
- **Auth tipleri (§A5)** ✅: Digest, OAuth 1.0, OAuth 2.0 (client credentials, password, authorization code + PKCE) ve AWS SigV4. İmzalar `apix_core::sign` içinde (AWS test paketi, RFC 7616, RFC 7636 ve OAuth 1.0 örnek değerleriyle sınanır); motor, CLI, "Copy as" (curl `--digest`, `--aws-sigv4`; PowerShell için kopyalarken imzalanır), curl yapıştırma ve Postman içe aktarma hepsini taşır. Gerçek sunucularda denendi: httpbin Digest (MD5, SHA-256), Postman Echo OAuth 1.0, yerel OAuth 2.0 sunucusu (`cargo run -p apix-http --example oauth_server`).
- **Ayarlar ekranı ve arayüz dili (§M, not 13)** (L) ✅: çeviri altyapısı (`rust-i18n`, tüm metinler anahtarlı), 9 dil, Arapça için sağdan sola, CJK ve Arapça yazı tipi yedekleri, ayarlardan anında dil değişimi. M7'den öne alındı: her yeni ekran yeni metin getirdiği için altyapı ne kadar erken gelirse sonradan o kadar az iş çıkar.
- **Dokümantasyon yapısı ve README (§N, not 14, 16)** (M) ✅: `Docs/<dil>/` yapısı, `languages.toml`, İngilizce ve Türkçe belgeler (kullanım kılavuzu, mimari, teknik altyapı, iş akışları), README'de "neden Apix" karşılaştırması ve Apix'in tekrarlanabilir performans ölçümleri.
- **İstek ayarları ve çerez yöneticisi (§A6)** (M) ✅: `.apix` dosyasında `[settings]` (zaman aşımı, yönlendirmeler ve üst sınır, TLS doğrulama, proxy, PEM istemci sertifikası + anahtar, çerez gönderimi); alan alan klasör → koleksiyon → proje kalıtımı; Ayarlar sekmesi (geçerli değerler satırıyla) ve klasör/koleksiyon/proje ayar diyalogları; motor her gönderimde ayarları uygular; tek çerez kavanozu `cookies.json` içinde kalıcı ve İstek › Çerezler… diyaloğundan yönetilir; CLI bayrakları (`--timeout`, `--insecure`, `--proxy`), "Copy as" curl/PowerShell bayrakları, curl yapıştırma ve Postman `protocolProfileBehavior` içe aktarma. Yerel sunucuya karşı testlerle doğrulandı.
- **Kod üretici ve geçmiş** (M) ✅: `apix_codegen::to_code` ile 10 dil (fetch, axios, Python requests, Go, Rust reqwest, C#, Java, PHP, Swift, Kotlin) ve kabukları da kapsayan ortak `Format`; "Copy as" menüleri, kenar çubuğunda "Kod olarak kopyala", komut paleti, `apix code`. Geçmiş: `apix_core::history` (ayar klasöründe `history/`), Ctrl+H penceresi; kayıt kendi sekmesine kaydedilmemiş değişiklik olarak ve yanıtıyla geri gelir.
- **Postman tamamlama (§D)** (M) ✅: otomatik tanıma, toplu dışa aktarımı (klasör/zip) projeye çevirme, globals → proje değişkenleri, Postman v2.1'e dışa aktarma (`apix_import::postman_dump`, `postman_export`).
- **İçerik tipleri (§A4, §B, not 17)** (M) ✅: HTML gövde tipi; XML ve HTML renklendirme (gövde ve yanıt); XML ve HTML yanıtları girintileme; yanıt türünü tanıma; resim önizleme; Hex sekmesi; yanıt gövdesini dosyaya kaydetme.
- **M3'ten kalan küçük işler** (L) ✅: fish/HTTPie/wget olarak kopyalama (uygulama ve `apix shell --as fish|httpie|wget`); prod ortamında DELETE/PUT/PATCH öncesi onay (Ayarlar'dan kapatılabilir); açık sekmeleri proje/koleksiyon başına hatırlama; sürükleyerek elle sıralama (istek, klasör ve projedeki koleksiyonlar; `order` listeleri); koleksiyonu projeye taşıma (menü ve klasör bırakma); projedeki koleksiyonlara renk; Params, Headers ve form/multipart alanlarında değişken çipleri. Yolda bulunan hata: sürükleyerek taşınan isteğin sekme yolu sonuna ayraç ekleniyor ve kaydetme başarısız oluyordu; düzeltildi.
- **Ortam gizli değerleri anahtarlıkta** (M) ✅: Ayarlar › Ortam gizli değerleri (`.local.toml` dosyası / sistem anahtarlığı). Anahtarlık seçilince `<ad>.local.toml` yalnızca `keychain = "<kimlik>"` satırını tutar, değerler `Apix` servisinde `env:<kimlik>` kaydına gider (Windows'un kayıt başına 2560 bayt sınırı için uzun metin parçalara bölünür). Kimlik dosyayla taşındığı için ortamı ya da klasörü yeniden adlandırmak kaydı bozmaz. Okuma ayardan bağımsızdır: CLI de bu işareti görüp anahtarlıktan okur. Seçenek değişince açık koleksiyon ve projedeki ortamlar taşınır, diğerleri ilk kaydedişte taşınır; ortam silinince kayıt da silinir. Windows Credential Manager aynı süreçten eşzamanlı yazma/silmede silinen kayıtları geri getirebildiği için anahtarlık çağrıları sıraya sokulur.
- **İçe aktarıcılar (§D)** (L) ✅: Insomnia v4, OpenAPI 3 / Swagger 2 (JSON ve YAML), HAR; hepsi içerikten tanınır ve Postman'la aynı yoldan (`apix_import::bundle::Found`) koleksiyon ya da proje olarak yazılır; klasör/zip taraması YAML ve `.har` dosyalarını da okur.

### M5 — Scriptler, testler, runner
Üç parçada yapılıyor:
1. [x] **Script motoru ve testler** (L): `apix-script` (QuickJS) ve `pm.*` uyumluluğu (değişken kapsamları, `pm.request`, `pm.response`, `pm.test`, Chai tarzı `pm.expect`, eski Postman API'si, konsol); `apix_core::scripts` script dosyalarını bulur (koleksiyon → klasör → istek) ve script'lerin atadığı değişkenleri tutar; `apix send` / `apix run` script çalıştırır, test sonuçlarını yazar, `setNextRequest` izler; uygulamada Scripts sekmesi, Tests ve Console sekmeleri. Postman'dan gelen scriptler artık çalışıyor.
2. [x] **Script'ten istek ve kütüphaneler** (M): `pm.sendRequest` (geri çağırma ve `await`), `require('crypto-js' | 'uuid' | 'lodash' | 'moment' | 'ajv' | 'tv4' | 'chai')` (Apix için yazılmış alt kümeler), kodsuz doğrulamalar (`[[assert]]`), yanıttan değişken kaydetme (**{x}**).
3. [x] **Runner** (M): yeni `apix-runner` crate'i CLI ile uygulamanın ortak koşu döngüsü (sıra, tekrar, veri satırları, bekleme, `bail`, `setNextRequest`, duraklat/durdur, raporlar); Runner penceresi (Ctrl+Shift+R, Görünüm menüsü, komut paleti, koleksiyon/klasör › Çalıştır…); CSV/JSON veri dosyalı koşular; JUnit XML / JSON / HTML raporlar; `apix run` için `--data`, `--iterations`, `--delay`, `--bail`, `--junit`, `--json`, `--html`, `--timeout`/`-k`/`--proxy` ve klasör ya da tek istek koşusu; `examples/ci/github-actions.yml`. Koşu artık kenar çubuğu sırasını izliyor (elle sıralama dahil). `--parallel` ve Docker imajı sonraya kaldı.

### M5 sonrası — Tema paketi (23 Ekim'de kararlaştırıldı) ✅
1. [x] Temayı yeniden başlatmadan değiştirme (görünümler paleti kopyalamak yerine ortak bir yerden okur); diğerlerinin temeli.
2. [x] Vurgu rengi seçimi (camgöbeği yerine mor, yeşil, turuncu …).
3. [x] Açık tema ve "sistemi izle" (Windows/macOS koyu-açık moduna göre); kod renklendirmesi de uyarlanır. M7'deki "açık tema" maddesi buraya alındı.
4. [x] Dosyadan özel tema (`<config>/Apix/themes/*.toml`), ekipçe paylaşılabilir.
- [x] Ekler: saydamlık kaydırıcısı (kullanıcı isteği, 24 Ekim: "Apple'daki Liquid gibi" ayarlanabilir şeffaflık; önce aç/kapa vardı ama gpui-kit'in kök arka planı ve %92 opak tema arka planı yüzünden pencere hiç şeffaf görünmüyordu), hazır temalar (Dracula, Nord, Solarized Dark/Light, Gün ışığı).
- Yapılanlar (24 Ekim): görünümler paleti kopyalamak yerine `theme::current()` ile okur, `theme::set` her şeyi hemen yeniden boyar; Ayarlar'da "Tema" bölümü (görünüm: koyu/açık/sistemi izle, koyu tema, açık tema, vurgu rengi: temanın kendi + 8 renk, saydamlık kaydırıcısı, "Klasörü aç" ile tema dosyaları ve örnek dosya); açık temada kod renklendirmesi gpui-kit'in açık temasına geçer, metot/koleksiyon renkleri koyulaşır; okunamayan tema dosyası nedeniyle gösterilir. "Sistemi izle" Windows'ta modu değiştirerek denenmedi (sistem ayarı).
- ⬜ Kalan: yazı boyutu ve satır yoğunluğu; kod editörü renklerinin temadan gelmesi (şimdilik gpui-kit'in koyu/açık renkleri).

**CLI ve CI/CD (24 Ekim'de konuşuldu):** kullanıcı CI'da gizli değerleri, hazır GitHub Action'ı ve Docker imajını seçti; bunlar yapıldı (§G). Bekleyenler: yalnızca PR'da değişen istekleri koşan `--changed`, koşu özetini PR'a yorum yazan rapor, `apix diff`, zamanlanmış izleme koşuları.

**Sıra (24 Ekim'de kararlaştırıldı):** kalan işler önem sırasına göre aşağıdaki aşamalara dağıtıldı: CI temeli → M6 → M7 → M8 → M9 → proje bitince Tailscale ve Docker imajı denemesi.

### CI temeli (M6'dan önce, 24 Ekim)
- [x] Deponun kendi CI'ı (`.github/workflows/ci.yml`): her push ve PR'da biçim ve `docs-check`; motor ve CLI için clippy (`-D warnings`) ve testler Windows, macOS ve Linux'ta; masaüstü uygulaması için aynısı üç sistemde (Linux pencere, yazı tipi ve anahtarlık kütüphanelerini kurar).
- [x] Action'ın kendini sınaması: `uses: ./` ile `examples/jsonplaceholder`, Linux ve Windows'ta; raporların yazıldığı denetlenir. Action önbelleği artık kaynakların özetiyle anahtarlanır (`` eski `apix`'i kullanmaz).
- [x] GitLab CI ve Azure Pipelines örnekleri (`examples/ci/`).
- [x] Linux ve macOS: ilk koşuda (24 Ekim) CLI ve masaüstü uygulaması üç sistemde de hatasız derlendi, clippy ve testler geçti; düzeltme gerekmedi. 10 işin hepsi yeşil, en uzunu Windows masaüstü (~24 dk).

### M6 — Protokoller
1. [x] **WebSocket ve SSE** (24 Ekim): motor `send_with` (yanıt parça parça), `apix_http::sse` (SSE ayrıştırıcı), `apix_http::ws` (tokio-tungstenite, aynı rustls); uygulamada canlı olay listesi, WebSocket Mesajlar sekmesi ve günlüğü; CLI'da olaylar ve mesajlar geldikçe; örnek sunucular `sse_server` ve `ws_server`. Ayrıntı §H.
2. [ ] GraphQL introspection ve otomatik tamamlama
3. [ ] gRPC, Socket.IO, MQTT, SOAP

### M7 — İleri özellikler
- **Rota ve ağ haritası (§B2)** (not 9): traceroute, çevrimdışı IP konum veritabanı ile dünya haritası
- **TLS ayrıntıları (§B2, not 12):** kendi bağlayıcımızla (hyper-util + rustls) TLS sürümü, şifre takımı, ALPN, tam sertifika zinciri, TCP ve TLS ayrı süreler, sıkıştırılmış boyut
- Mock sunucu, dokümantasyon üretme, monitörler, visualizer, yakalama proxy'si
- Git entegrasyonu, otomatik güncelleme (ayarlar ekranı ve diller M4'e, açık tema M5 sonrası tema paketine taşındı)
- **Günlük kullanım eksikleri (24 Ekim'de buraya alındı):** yanıtta Ctrl+F arama, JSONPath/jq ile süzme, Params ↔ URL senkronu, `/users/:id` yol değişkenleri, header otomatik tamamlama, kenar çubuğunda arama, istek ve yanıtı örnek olarak kaydetme.
- **Git'e duyarlı CLI:** kenar çubuğunda değişen dosyalar ve dal adı ile birlikte `apix run --changed`, `apix diff`, koşu özetini PR'a yorum yazan rapor.
- **İzleme:** zamanlanmış koşular (monitörler), `apix run --parallel`.
- **Tema kalanları:** kod editörü renklerinin temadan gelmesi, yazı boyutu ve satır yoğunluğu.

### M8 — Apix'e özel
- Yük testi, yanıt karşılaştırma, zaman yolculuğu, MCP, AI, WASM eklentileri, Flows

### M9 — Yayın
- GitHub Actions: Windows, macOS ve Linux'ta test ve derleme (CI temeli ile yapıldı), sürüm etiketinde derleme
- **Hazır ikili dosyalar** (GitHub Releases); GitHub Action derlemek yerine onları indirir, saniyeler içinde çalışır.
- **Docker imajı:** Linux'ta derleyip denemek ve ghcr.io'ya yayımlamak (kullanıcı notu: "proje bitince derleyip bakalım").
- **Marketplace:** GitHub Marketplace iş akışı dosyası olan depoyu kabul etmediği için ayrı, küçük bir `apix-action` deposu.
- **Performans hedefleri:** açılış 150 ms ve boşta 60 MB bütçesine inmek (şu an 278 ms ve 84 MB).
- Kurulum paketleri: `.msi` ve winget, `.dmg` ve Homebrew, AppImage, Flatpak ve `.deb`
- İkili boyutunu izlemek: üst sınır 100 MB (not 11). Şu an release 38.9 MB; içinde 2.2 MB font, simgeler ve tree-sitter var. CJK ve Arapça yazı tipleri eklenince büyür; sınır hız ve akıcılık için tanındı, boyut için ödün verilmez.
- **Tüm dillerde dokümantasyon (§N, not 14, 15):** 9 dilde kullanım kılavuzu, mimari, teknik altyapı, iş akışları; her dilin kendi arayüzünden otomatik alınmış ekran görüntüleriyle görselli kılavuz; dokümantasyon sitesinde dil seçici
- **Karşılaştırmalı performans (not 16):** Apix ile Postman, Insomnia, Bruno ve Hoppscotch aynı makinede, aynı betikle ölçülür; sonuçlar README'de
- **İmzalı sürümler:** Windows kod imzası (Akıllı Uygulama Denetimi engellemesin diye) ve macOS notarization
- Kurulum paketlerinde M3'te hazırlanan simgelerin kullanılması, web sitesi, tanıtım (r/rust, Hacker News)

---

## 5. Kalite kuralları (her aşamada geçerli)
- Her özelliğin `apix-core`'da birim testi olur. Arayüz mantığı mümkün olduğunca core'da kalır.
- `cargo clippy` uyarısız olmalı, `cargo fmt` uygulanmış olmalı.
- Performans bütçesi her sürümde ölçülür: açılış süresi, boşta bellek, ikili boyutu, 100 MB JSON testi.
- Arayüz değişiklikleri ekran görüntüsüyle doğrulanır.
- Her push ve PR üç işletim sisteminde test edilir (`.github/workflows/ci.yml`).

---

## 6. Karar bekleyen konular
| # | Konu | Seçenekler | Önerim |
|---|---|---|---|
| 1 | ~~İstek dosyası uzantısı~~ | ✅ **Karar verildi: `.apix`** (içerik TOML). `.gitattributes` ile GitHub TOML olarak renklendiriyor. | |
| 2 | Varsayılan tema | Obsidian (sade) veya Cyber (mockup) | Obsidian varsayılan olsun, Cyber tek tıkla seçilebilsin. |
| 3 | Postman script uyumluluğu ne kadar olsun | Tam `pm.*` veya kendi API'miz | `pm.*` ile uyumlu ve `apix.*` takma adı. Geçişi kolaylaştırır. |
| 4 | ~~Arayüz dili~~ | ✅ **Karar verildi (not 13):** 9 dil (EN, TR, AR, DE, FR, RU, JA, ZH, ES), M4'te. Kod ve kod yorumları İngilizce kalır; kullanıcı belgeleri her dilde ayrı (§N), kaynak dil İngilizce. | |
| 5 | Sürüm numarası | `0.x` ile başla | M2 sonunda `0.1.0` ilk açık sürüm. |
| 6 | İşletim sistemleri için klasör yapısı (not 5) | (a) Windows, macOS ve Linux için ayrı UI klasörleri · (b) tek masaüstü crate'i | **(b) Tek crate.** GPUI ve gpui-kit üç sistemi aynı kodla destekliyor; Zed de böyle çalışıyor. Ortak kısımlar zaten ayrı crate'lerde: `apix-core`, `apix-http`, `apix-import`. Sisteme özel parçalar `crates/apix-desktop/src/platform/{windows,macos,linux}.rs` altına girer: pencere düğmeleri ve başlık çubuğu, blur/vibrancy, macOS menü çubuğu, dosya ilişkilendirme, anahtarlık. Kurulum dosyaları `packaging/{windows,macos,linux}/` altına girer. Bir sistemin gerçekten ayrı bir arayüze ihtiyacı çıkarsa o zaman ayrılır. |
| 7 | Şifreli koleksiyonun diskteki biçimi | (a) Tek bir şifreli dosya (kasa) · (b) Her dosya ayrı şifreli, dosya adları anlamsız | **(b).** Git hangi isteğin değiştiğini görmeye devam eder (içeriği değil), birleştirme çakışmaları küçük kalır. (a) daha basit ama her değişiklikte tüm dosya değişir. |
| 8 | Ortam ile koleksiyon ilişkisi (not 6) | Ortam = değişken seti (Postman gibi) · Ortam = koleksiyonları gruplayan klasör | **İkisi birden:** ortamlar değişken setidir, ama bir koleksiyon belirli ortamlara bağlanabilir. Kenar çubuğu istenirse "Proje → Ortam → Koleksiyonlar" olarak gruplanır. |

---

## 7. 💭 Sonra karar verilecek fikirler
Bu fikirler planın dışında tutuluyor. Ana geliştirme (M9'a kadar) bitince tekrar ele alınacak.

| Fikir | Not | Ne zaman hatırlatılacak |
|---|---|---|
| **Tailscale ile ekip modu** (not 3) | Bulut kullanmadan, ekibin tailnet'i üzerinden projeleri ve koleksiyonları paylaşmak. Örnek: bir makinedeki proje diğerlerine canlı açılır, değişiklikler eşlenir. Seçenekler: Tailscale yerel API'si veya `tsnet`, ya da sadece git + Tailscale üzerinden paylaşılan bir klasör. Önce git tabanlı işbirliği (§L) tamamlanacak, sonra ihtiyaç değerlendirilecek. | M9 bitince |
