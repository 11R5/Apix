//! Postman-style dynamic variables: `{{$guid}}`, `{{$timestamp}}`,
//! `{{$randomEmail}}`, ... Each occurrence gets a fresh value. The values
//! are for test data only; the generator is not cryptographically secure.

use std::cell::Cell;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

/// Every supported name (without the `$`) and what it produces.
pub const DYNAMIC_VARIABLES: &[(&str, &str)] = &[
    ("guid", "A random UUID v4"),
    ("randomUUID", "A random UUID v4"),
    ("timestamp", "Current Unix time in seconds"),
    ("isoTimestamp", "Current time, ISO 8601 in UTC"),
    ("randomInt", "An integer from 0 to 1000"),
    ("randomBoolean", "true or false"),
    ("randomAlphaNumeric", "One letter or digit"),
    ("randomHexadecimal", "One hexadecimal digit"),
    ("randomPassword", "15 letters and digits"),
    ("randomFirstName", "A first name"),
    ("randomLastName", "A last name"),
    ("randomFullName", "A first and last name"),
    ("randomUserName", "A user name"),
    ("randomEmail", "An email address at example.com"),
    ("randomPhoneNumber", "A phone number like 555-123-4567"),
    ("randomCity", "A city"),
    ("randomCountry", "A country"),
    ("randomCountryCode", "A two-letter country code"),
    ("randomColor", "A colour name"),
    ("randomWord", "A word"),
    ("randomWords", "Three words"),
    ("randomDomainName", "A domain name"),
    ("randomUrl", "An https URL"),
    ("randomIP", "An IPv4 address"),
    ("randomIPV6", "An IPv6 address"),
    ("randomPrice", "A price from 0.00 to 1000.00"),
    ("randomDatePast", "A date in the last year, ISO 8601"),
    ("randomDateFuture", "A date in the next year, ISO 8601"),
];

const FIRST_NAMES: &[&str] = &[
    "Ada", "Alan", "Grace", "Linus", "Ayşe", "Mehmet", "Elif", "Can", "Maria", "John", "Yuki",
    "Omar", "Lena", "Diego", "Priya", "Noah",
];
const LAST_NAMES: &[&str] = &[
    "Lovelace", "Turing", "Hopper", "Torvalds", "Yılmaz", "Kaya", "Demir", "Smith", "García",
    "Tanaka", "Haddad", "Novak", "Rossi", "Patel", "Müller", "Kim",
];
const CITIES: &[&str] = &[
    "Istanbul", "Ankara", "Izmir", "Berlin", "Lisbon", "Tokyo", "Toronto", "Nairobi", "Lima",
    "Oslo", "Seoul", "Austin",
];
const COUNTRIES: &[(&str, &str)] = &[
    ("Türkiye", "TR"),
    ("Germany", "DE"),
    ("Portugal", "PT"),
    ("Japan", "JP"),
    ("Canada", "CA"),
    ("Kenya", "KE"),
    ("Peru", "PE"),
    ("Norway", "NO"),
    ("South Korea", "KR"),
    ("United States", "US"),
];
const COLORS: &[&str] = &[
    "red", "orange", "yellow", "green", "teal", "cyan", "blue", "indigo", "violet", "magenta",
    "black", "white",
];
const WORDS: &[&str] = &[
    "apex", "binary", "cache", "delta", "echo", "flux", "graph", "hash", "index", "json", "kernel",
    "lambda", "matrix", "node", "orbit", "packet", "query", "relay", "socket", "token",
];
const TLDS: &[&str] = &["com", "dev", "io", "net", "org"];

thread_local! {
    static STATE: Cell<u64> = Cell::new(seed());
}

/// Seeds from the process-random key of `RandomState` plus the clock.
fn seed() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    hasher.write_u128(nanos);
    hasher.finish() | 1
}

/// xorshift64*: fast, small, and plenty for test data.
fn next_u64() -> u64 {
    STATE.with(|state| {
        let mut x = state.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        state.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    })
}

fn below(n: u64) -> u64 {
    next_u64() % n
}

fn pick<T: Copy>(items: &[T]) -> T {
    items[below(items.len() as u64) as usize]
}

fn alphanumeric(len: usize) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    (0..len).map(|_| pick(CHARS) as char).collect()
}

fn uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&next_u64().to_le_bytes());
    bytes[8..].copy_from_slice(&next_u64().to_le_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // RFC 4122 variant
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Unix milliseconds → `2026-10-17T12:34:56.789Z` (UTC).
pub fn iso8601(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let ms = millis.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 → (year, month, day), from Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The value for `{{$name}}` (pass `name` without the `$`), or `None` if unknown.
pub fn generate(name: &str) -> Option<String> {
    const YEAR_MS: u64 = 365 * 86_400_000;
    let value = match name {
        "guid" | "randomUUID" => uuid_v4(),
        "timestamp" => (now_millis() / 1000).to_string(),
        "isoTimestamp" => iso8601(now_millis()),
        "randomInt" => below(1001).to_string(),
        "randomBoolean" => (below(2) == 1).to_string(),
        "randomAlphaNumeric" => alphanumeric(1),
        "randomHexadecimal" => format!("{:x}", below(16)),
        "randomPassword" => alphanumeric(15),
        "randomFirstName" => pick(FIRST_NAMES).to_owned(),
        "randomLastName" => pick(LAST_NAMES).to_owned(),
        "randomFullName" => format!("{} {}", pick(FIRST_NAMES), pick(LAST_NAMES)),
        "randomUserName" => format!("{}{}", ascii_lower(pick(FIRST_NAMES)), below(1000)),
        "randomEmail" => format!(
            "{}.{}{}@example.com",
            ascii_lower(pick(FIRST_NAMES)),
            ascii_lower(pick(LAST_NAMES)),
            below(100)
        ),
        "randomPhoneNumber" => format!("555-{:03}-{:04}", below(1000), below(10_000)),
        "randomCity" => pick(CITIES).to_owned(),
        "randomCountry" => pick(COUNTRIES).0.to_owned(),
        "randomCountryCode" => pick(COUNTRIES).1.to_owned(),
        "randomColor" => pick(COLORS).to_owned(),
        "randomWord" => pick(WORDS).to_owned(),
        "randomWords" => format!("{} {} {}", pick(WORDS), pick(WORDS), pick(WORDS)),
        "randomDomainName" => format!("{}-{}.{}", pick(WORDS), pick(WORDS), pick(TLDS)),
        "randomUrl" => format!("https://{}-{}.{}", pick(WORDS), pick(WORDS), pick(TLDS)),
        "randomIP" => format!(
            "{}.{}.{}.{}",
            1 + below(254),
            below(256),
            below(256),
            1 + below(254)
        ),
        "randomIPV6" => (0..8)
            .map(|_| format!("{:x}", below(0x1_0000)))
            .collect::<Vec<_>>()
            .join(":"),
        "randomPrice" => format!("{}.{:02}", below(1000), below(100)),
        "randomDatePast" => iso8601(now_millis() - below(YEAR_MS) as i64),
        "randomDateFuture" => iso8601(now_millis() + below(YEAR_MS) as i64),
        _ => return None,
    };
    Some(value)
}

/// Lowercase for names that go into emails and user names (`Ayşe` → `ayse`).
fn ascii_lower(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'ş' | 'Ş' => 's',
            'ı' | 'İ' => 'i',
            'ğ' | 'Ğ' => 'g',
            'ü' | 'Ü' => 'u',
            'ö' | 'Ö' => 'o',
            'ç' | 'Ç' => 'c',
            'í' => 'i',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_v4_and_unique() {
        let a = generate("guid").unwrap();
        let b = generate("randomUUID").unwrap();
        assert_ne!(a, b);
        for id in [&a, &b] {
            assert_eq!(id.len(), 36);
            assert_eq!(&id[14..15], "4", "{id}");
            assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"), "{id}");
        }
    }

    #[test]
    fn iso_dates_are_correct() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(iso8601(1_792_237_696_789), "2026-10-17T11:48:16.789Z");
        let now = generate("isoTimestamp").unwrap();
        assert!(now.starts_with("20") && now.ends_with('Z'), "{now}");
    }

    #[test]
    fn every_listed_name_generates_and_others_do_not() {
        for (name, _) in DYNAMIC_VARIABLES {
            let value = generate(name).unwrap_or_else(|| panic!("{name} not generated"));
            assert!(!value.is_empty(), "{name}");
        }
        assert!(generate("nope").is_none());
        let int: u32 = generate("randomInt").unwrap().parse().unwrap();
        assert!(int <= 1000);
        assert!(generate("randomEmail").unwrap().ends_with("@example.com"));
        assert!(generate("randomEmail").unwrap().is_ascii());
    }
}
