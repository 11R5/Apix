<!-- translated from en/user-guide.md sha256:d10fcc437b8be1c7 -->

# Apix kullanım kılavuzu

Apix bir masaüstü API istemcisidir: HTTP istekleri oluşturur, gönderir ve
yanıtları okursunuz. İstekler bir klasörde küçük metin dosyaları olarak
kaydedilir; böylece bir koleksiyon kodunuzla birlikte git'te durabilir. Bu
kılavuz uygulamayı bölüm bölüm tanıtır; [iş akışları](workflows.md) sık yapılan
işleri adım adım anlatır, `apix` komut satırı ise aynı istekleri bir terminalde
veya CI'da çalıştırır.

![Apix penceresi](images/main.png)

## İçindekiler

- [Koleksiyon açma](#koleksiyon-açma)
- [Pencere](#pencere)
- [Bir istek gönderme](#bir-istek-gönderme)
- [Parametreler ve başlıklar](#parametreler-ve-başlıklar)
- [Gövde](#gövde)
- [Yetkilendirme](#yetkilendirme)
- [İstek ayarları](#istek-ayarları)
- [Çerezler](#çerezler)
- [Değişkenler ve ortamlar](#değişkenler-ve-ortamlar)
- [Yanıt](#yanıt)
- [Ağ sekmesi](#ağ-sekmesi)
- [Sekmeler](#sekmeler)
- [Kenar çubuğunda istekleri düzenleme](#kenar-çubuğunda-istekleri-düzenleme)
- [Projeler](#projeler)
- [Şifreli koleksiyonlar](#şifreli-koleksiyonlar)
- [Komut olarak kopyala ve curl yapıştırma](#komut-olarak-kopyala-ve-curl-yapıştırma)
- [Postman'dan içe aktarma](#postmandan-içe-aktarma)
- [Komut paleti](#komut-paleti)
- [Ayarlar ve dil](#ayarlar-ve-dil)
- [Klavye kısayolları](#klavye-kısayolları)

## Koleksiyon açma

**Koleksiyon**, bir `apix.toml` dosyası ile isteklerinizi (`.apix` dosyaları)
barındıran, isteğe bağlı olarak alt klasörler de içerebilen bir klasördür. Bir
koleksiyonu **Dosya › Aç…** (Ctrl+O) ile açın ya da **Dosya › Yeni koleksiyon…**
ile yeni bir tane başlatın. Henüz koleksiyon olmayan bir klasör seçerseniz Apix
onu koleksiyona dönüştürmeyi önerir.

Son açılan koleksiyonlar **Dosya › Son açılanlar** altındadır. Uygulamayı bir
klasörle başlatmak (`apix-desktop path/to/collection`) o klasörü doğrudan açar.

## Pencere

- **Kenar çubuğu (sol):** koleksiyonun klasörleri ve istekleri; diskteki
  dosyalardan okunur ve dosyalar değiştikçe güncel tutulur. Koleksiyon adının
  altındaki seçici etkin ortamı belirler.
- **Sekmeler (üst):** açtığınız her istek bir sekme alır.
- **İstek bölmesi:** metot, URL ve Parametreler, Başlıklar, Yetkilendirme,
  Gövde ve Ayarlar sekmeleri.
- **Yanıt bölmesi (sağ):** durum, süre, boyut ve yanıt gövdesi.
- **Durum çubuğu (alt):** koleksiyon, açık isteğin dosyası ve en kullanışlı
  kısayollar.
- **Menü çubuğu:** Dosya, İstek, Ortam, Görünüm ve Yardım menüleri tüm
  komutları içerir.

## Bir istek gönderme

1. URL çubuğunun solundan metodu seçin (ya da GET, POST, PUT, PATCH, DELETE,
   HEAD, OPTIONS için Alt+1…7 tuşlarına basın).
2. URL'yi yazın. `{{variables}}` her yerde kullanılabilir (bkz.
   [Değişkenler](#değişkenler-ve-ortamlar)).
3. **Gönder**'e basın veya Ctrl+Enter kullanın. Esc, hâlâ çalışan bir isteği
   iptal eder.

**Yeni istek** (Ctrl+N) bir GET isteği açar; kenar çubuğundaki **+** menüsü tüm
metotları sunar, POST, PUT ve PATCH ise JSON gövdesiyle başlar. Ctrl+S ile
kaydedin. Sekmedeki ve kenar çubuğundaki nokta kaydedilmemiş değişiklikleri
gösterir.

## Parametreler ve başlıklar

**Parametreler** URL'nin sorgu dizesine eklenir, **Başlıklar** olduğu gibi
gönderilir. Bir satırı dosyada tutup göndermemek için işaretini kaldırın.
Anahtarı olmayan satırlar yok sayılır. Kendiniz bir tane eklemediğiniz sürece
`Content-Type` gövde türünden belirlenir.

## Gövde

Gövde sekmesi şunları sunar:

| Tür | Gönderilen |
|---|---|
| Yok | Gövde yok. |
| JSON, Metin, XML | Metin, yazıldığı gibi; söz dizimi vurgulamasıyla. |
| Form | `application/x-www-form-urlencoded` anahtar–değer satırları. |
| Multipart | `multipart/form-data`. Her alan **Metin** veya **Dosya** olur; klasör düğmesi bir veya birden fazla dosya seçer. |
| İkili | Tek bir dosyanın baytları; bir başlık aksini belirtmedikçe `application/octet-stream` olarak. Apix dosya boyutunu ya da “Dosya bulunamadı” uyarısını gösterir. |
| GraphQL | Bir sorgu düzenleyicisi ve bir JSON değişken düzenleyicisi; `{"query", "variables"}` JSON'u olarak gönderilir. |

![Multipart gövde](images/body.png)

Koleksiyon içindeki dosya yolları istek dosyasına göre göreli kaydedilir;
böylece koleksiyon başka bir makinede de çalışmaya devam eder.

## Yetkilendirme

Yetkilendirme sekmesi, isteğin kimliğini nasıl kanıtlayacağını belirler:

| Tür | Notlar |
|---|---|
| Üst öğeden devral | Varsayılan: yetkilendirme ayarlayan en yakın klasörün, koleksiyonun veya projenin yetkilendirmesini kullanır. Sekme bunun ne olduğunu ve nereden geldiğini gösterir. |
| Yetkilendirme yok | Bir üst öğe yetkilendirme ayarlasa bile hiçbir şey gönderilmez. |
| Bearer token | `Authorization: Bearer <token>`. |
| Basic auth | Kullanıcı adı ve parola. |
| API anahtarı | Seçtiğiniz bir başlık veya sorgu parametresi. |
| Digest auth | Apix, sunucunun Digest sınamasını (MD5 veya SHA-256) yanıtlar. |
| OAuth 1.0 | HMAC-SHA1, HMAC-SHA256 veya PLAINTEXT; başlıkta veya sorgu dizesinde; her gönderimde yeni bir nonce ve zaman damgası. |
| OAuth 2.0 | İstemci kimlik bilgileri, parola veya PKCE ile yetkilendirme kodu. Apix token'ı kendisi alır ve bearer token olarak gönderir. |
| AWS Signature | Erişim anahtarı, gizli anahtar, isteğe bağlı oturum token'ı, bölge ve servis ile AWS Signature Version 4. |

![Yetkilendirme sekmesinde OAuth 2.0](images/auth.png)

**OAuth 2.0 token'ları.** İlk gönderim bir token alır; sonraki gönderimler onu
yeniden kullanır ve süresi dolduğunda refresh token ile yeniler. **Yeni token al**
hemen bir token alır, **Unut** onu atar. Token'lar yalnızca bellekte tutulur ve
asla diske yazılmaz. Yetkilendirme kodu akışında Apix oturum açma sayfasını
tarayıcınızda açar ve yanıtı `http://127.0.0.1` üzerinde alır (boş bir port ya
da istemci için kaydettiğiniz **Yönlendirme URL'si**).

**Paylaşılan yetkilendirme.** Yetkilendirmeyi içindeki her şey için bir kez
ayarlamak üzere bir klasöre veya koleksiyona sağ tıklayıp **Yetkilendirme…**
seçin; **Ortam › Proje yetkilendirmesi…** aynısını tüm bir proje için yapar.
Yetkilendirmesi *Üst öğeden devral* olan istekler bunu kullanır.

## İstek ayarları

Ayarlar sekmesi isteğin nasıl yol alacağını belirler:

| Ayar | Varsayılan |
|---|---|
| Zaman aşımı (saniye) | yok — istek bitene ya da siz iptal edene kadar sürer; `0` da zaman aşımı yok demektir |
| Yönlendirmeleri izle | açık |
| En çok yönlendirme | 10 |
| TLS sertifikasını doğrula | açık — yalnızca güvendiğiniz test sunucuları için kapatın |
| Proxy | ortamdan (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`); *Kapalı* bunları da yok sayar; bir URL proxy'yi açıkça belirtir |
| İstemci sertifikası | yok — PEM dosyası; anahtar aynı dosyada ya da ayrı bir dosyada |
| Çerez gönder | açık |

Boş bırakılan (ya da *Miras*ta kalan) her alan, onu ayarlayan en yakın
klasörden, koleksiyondan veya projeden, en sonunda da yukarıdaki
varsayılanlardan değer alır — yetkilendirmeyle aynı zincir. Bir klasöre veya
koleksiyona sağ tıklayıp **İstek ayarları…** seçerek içindeki her şey için bir
kez ayarlayın; **Ortam › Proje istek ayarları…** aynısını tüm bir proje için
yapar. Formun altındaki satır isteğin sonunda neyi kullanacağını gösterir.
Proxy ve sertifika alanlarında `{{değişkenler}}` çalışır; göreli sertifika
yolları, onları ayarlayan dosyaya göre çözülür.

İstek dosyasında (ve `folder.toml`, `apix.toml`, `apix-project.toml`
dosyalarında) ayarlar bir `[settings]` tablosudur:

```toml
[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""          # "" = proxy yok, ortam ayarlasa bile
client_cert = "certs/client.pem"
```

**Komut olarak kopyala** karşılık gelen curl bayraklarını yazar (`-m`,
`-L --max-redirs`, `-k`, `-x`, `--cert`, `--key`); bu bayrakları taşıyan bir
curl komutu yapıştırıldığında sekme dolar. `apix send` komutunda dosyayı ezen
`--timeout`, `--insecure` ve `--proxy` bayrakları vardır. Postman'ın istek
başına seçenekleri (`protocolProfileBehavior`) içe aktarılır.

## Çerezler

Apix tek bir çerez kavanozu tutar: `Set-Cookie` yanıtları kendiliğinden
saklanır ve aynı siteye sonraki isteklerle gönderilir; yeniden başlatmalarda da
korunur (uygulama ayarlarının yanındaki `cookies.json`; oturum çerezleri
uygulamayla birlikte silinir). **İstek › Çerezler…** hepsini listeler, tek tek
siler ya da kavanozu boşaltır. Ayarlar sekmesindeki **Çerez gönder**, kavanozu
bir istek, klasör, koleksiyon veya proje için kapatır. CLI her çalıştırmada boş
bir kavanozla başlar.

## Değişkenler ve ortamlar

Bir URL'ye, başlığa, gövdeye veya yetkilendirme alanına `{{name}}` yazın; Apix
gönderirken değeri yerine koyar. URL çubuğunda her değişken renkli bir etikettir:
değerini ve nereden geldiğini görmek için üzerine gelin; kırmızı bir etiket hiçbir
yerde tanımlı değildir.

Değerler katmanlar hâlinde gelir, sonraki öncekini geçersiz kılar:

1. **Proje** değişkenleri (bir projenin tüm koleksiyonlarınca paylaşılır)
2. **Koleksiyon** değişkenleri (`apix.toml`)
3. Etkin **ortam** (`environments/<name>.toml`)

Bunları düzenlemek için **Ortam › Değişkenler ve ortamlar…** (Ctrl+E) menüsünü
açın. Token ve parolalar için **Gizli** kutusunu işaretleyin: gizli değerler
`environments/<name>.local.toml` dosyasında saklanır; Apix bu dosyayı
`.gitignore`'a ekler, böylece asla commit'lenmezler.

![Değişkenler iletişim kutusu](images/variables.png)

**Dinamik değişkenler** her kullanımda yeni bir değer alır: `{{$guid}}`,
`{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomEmail}}` ve
daha fazlası; Değişkenler iletişim kutusunun **Dinamik değişkenler** sayfası
hepsini örnek bir değerle listeler.

## Yanıt

Yanıt bölmesi durumu, ilk bayta kadar geçen süreyi ve toplam süreyi, ayrıca
boyutu gösterir. **Biçimli** JSON'u biçimlendirir (blokları oklarla katlayın),
**Ham** gövdeyi alındığı gibi gösterir, **Başlıklar** yanıt başlıklarını
listeler. Çok büyük gövdeler (onlarca megabayt) yalnızca görünen satırları çizen
hızlı bir görüntüleyicide açılır.

## Ağ sekmesi

**Ağ** sekmesi, sürenin nereye gittiğini ve bağlantının nasıl olduğunu açıklar:

- DNS, bağlanma, bekleme ve indirme olarak bölünmüş bir zamanlama çubuğu
- DNS yanıtları, sunucu ve yerel adres, ağ arayüzü (VPN kullanılıyorsa bir
  ipucuyla), ağ geçidi, DNS sunucuları ve proxy
- her yönlendirme
- sunucu sertifikası: veren, geçerlilik ve kalan gün sayısı

![Ağ sekmesi](images/network.png)

## Sekmeler

Her sekme kaydedilmemiş düzenlemelerini, son yanıtını ve hatta hâlâ çalışan bir
isteği korur. Ctrl+Tab ve Ctrl+Shift+Tab sekmeler arasında geçiş yapar, Ctrl+W
veya orta tıklama bir sekmeyi kapatır; Apix kaydedilmemiş değişiklikleri
kapatmadan önce sorar.

## Kenar çubuğunda istekleri düzenleme

İstek ve klasör oluşturmak, yeniden adlandırmak, çoğaltmak veya silmek için
kenar çubuğuna sağ tıklayın; silinen dosyalar sistemin çöp kutusuna gider. Bir
isteği veya klasörü taşımak için başka bir klasörün üzerine sürükleyin.

## Projeler

**Proje**, birbirine ait birkaç koleksiyonu bir araya getirir ve ortamları ve
yetkilendirmeyi aralarında paylaştırır. **Dosya › Yeni proje…** ile bir tane
oluşturun. Bir projede **Ortam › Kenar çubuğunu grupla** kenar çubuğunu
Proje › Ortam › Koleksiyonlar düzeninde gösterebilir.

## Şifreli koleksiyonlar

Şifreli bir koleksiyon her isteği, ortamı ve hatta dosya adlarını diskte mühürlü
tutar (Argon2id ile paroladan türetilen anahtarla XChaCha20-Poly1305). Git
yalnızca hangi şifreli blob'un değiştiğini görür.

- **Dosya › Bu koleksiyonu şifrele…** ya da bir koleksiyona sağ tıklayıp ›
  **Şifrele…** mevcut bir koleksiyonu yerinde şifreler.
- **Dosya › Yeni şifreli koleksiyon…** bir proje içinde yeni bir tane oluşturur.
- Bir **kurtarma anahtarı** yalnızca bir kez gösterilir. Güvenli bir yerde
  saklayın: parola kaybolursa koleksiyonu bu anahtar açar.
- Koleksiyonu açarken parola sorulur. **Dosya › Şifreli koleksiyonları kilitle**
  seçtiğinizde, Apix'ten çıktığınızda veya Ayarlar'da belirlenen süre (varsayılan
  15 dakika) boyunca boşta bıraktığınızda yeniden kilitlenir.

## Komut olarak kopyala ve curl yapıştırma

**Gönder** düğmesinin yanındaki kopyala düğmesi (veya **İstek › Farklı
kopyala**) isteği bash için `curl`, Windows komut istemi için `curl.exe` ya da
bir PowerShell `Invoke-RestMethod` çağrısı olarak, değişkenler doldurulmuş
şekilde kopyalar. Her gönderimde imzalanan istekler (OAuth 1.0, PowerShell için
AWS) kopyaladığınız anda imzalanır.

URL alanına bir `curl` komutu yapıştırın (örneğin tarayıcınızın "Copy as cURL"
seçeneğinden); Apix isteğin tamamını doldurur: metot, URL, sorgu, başlıklar,
yetkilendirme ve gövde.

## Postman'dan içe aktarma

**Dosya › Postman'dan içe aktar…** menüsünü kullanın ya da bir Postman dışa
aktarımını pencereye bırakın. Koleksiyonlar (v2.0 ve v2.1) ve ortamlar
desteklenir. Klasörler, istekler, gövdeler, yetkilendirme ve değişkenler
aktarılır; Apix'in henüz devralamadığı her şey bir raporda listelenir.
Pre-request ve test betikleri isteklerinin yanında `.js` dosyaları olarak
saklanır, ortamlardaki gizli değerler ise git'in yok saydığı `.local.toml`
dosyasına gider.

## Komut paleti

Ctrl+K (veya Ctrl+P) komut paletini açar: bir istek adının veya komutun birkaç
harfini yazıp Enter'a basın. Komutlar her dilde İngilizce adlarıyla da
bulunabilir.

![Komut paleti](images/palette.png)

## Ayarlar ve dil

**Dosya › Ayarlar…** (Ctrl+,):

- **Dil:** English, Türkçe, العربية, Deutsch, Français, Русский, 日本語,
  简体中文 veya Español. Değişiklik hemen uygulanır. İlk açılışta Apix sistem
  dilini izler. Arapçada pencere aynalanır: kenar çubuğu sağdadır, menüler,
  sekmeler ve formlar sağdan başlar; URL'ler, kod ve anahtar–değer tabloları
  soldan sağa kalır.
- **Tema:** Obsidyen veya Siber cam; Apix'in bir sonraki açılışında uygulanır.
- **Şifreli koleksiyonları kilitle:** 5, 15 veya 30 dakika sonra, 1 saat sonra
  ya da hiçbir zaman.

![Ayarlar](images/settings.png)

Ayarlar, sisteminizin yapılandırma dizinindeki Apix klasöründe `settings.toml`
dosyasına kaydedilir (Windows'ta `%APPDATA%\Apix`, macOS'ta
`~/Library/Application Support/Apix`, Linux'ta `~/.config/Apix`).

## Klavye kısayolları

| Tuşlar | İşlem |
|---|---|
| Ctrl+Enter | Gönder |
| Esc | İsteği iptal et |
| Ctrl+S | Kaydet |
| Ctrl+N | Yeni GET isteği |
| Ctrl+O | Klasör, proje veya koleksiyon aç |
| Ctrl+K veya Ctrl+P | Komut paleti |
| Ctrl+E | Değişkenler ve ortamlar |
| Ctrl+, | Ayarlar |
| Alt+1…7 | Metodu değiştir |
| Ctrl+W | Sekmeyi kapat |
| Ctrl+Tab / Ctrl+Shift+Tab | Sonraki / önceki sekme |

macOS'ta Ctrl yerine ⌘ kullanın (Ctrl+Tab, Ctrl olarak kalır).
