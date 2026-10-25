#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordHash {
    Legacy(u16),
    Sha512 {
        hash: String,
        salt: String,
        spin: u32,
    },
}

impl PasswordHash {
    pub fn of(password: &str) -> Option<Self> {
        (!password.is_empty()).then(|| Self::Legacy(legacy_hash(password)))
    }

    pub fn matches(&self, password: &str) -> bool {
        match self {
            Self::Legacy(hash) => legacy_hash(password) == *hash,
            Self::Sha512 { hash, salt, spin } => {
                let Some(salt) = base64_decode(salt) else {
                    return false;
                };
                let text: Vec<u8> = password.encode_utf16().flat_map(u16::to_le_bytes).collect();
                let mut digest = sha512(&[salt, text].concat());
                for round in 0..*spin {
                    digest = sha512(&[digest.to_vec(), round.to_le_bytes().to_vec()].concat());
                }
                base64_encode(&digest) == *hash
            }
        }
    }
}

pub fn verify(hash: Option<&PasswordHash>, password: &str) -> bool {
    match hash {
        None => true,
        Some(hash) => hash.matches(password),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub select_locked: bool,
    pub select_unlocked: bool,
    pub format_cells: bool,
    pub format_columns: bool,
    pub format_rows: bool,
    pub insert_columns: bool,
    pub insert_rows: bool,
    pub insert_links: bool,
    pub delete_columns: bool,
    pub delete_rows: bool,
    pub sort: bool,
    pub autofilter: bool,
    pub edit_objects: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            select_locked: true,
            select_unlocked: true,
            format_cells: false,
            format_columns: false,
            format_rows: false,
            insert_columns: false,
            insert_rows: false,
            insert_links: false,
            delete_columns: false,
            delete_rows: false,
            sort: false,
            autofilter: false,
            edit_objects: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Protection {
    pub password: Option<PasswordHash>,
    pub allow: Permissions,
}

pub fn legacy_hash(password: &str) -> u16 {
    if password.is_empty() {
        return 0;
    }
    let mut hash: u16 = 0;
    for byte in password.as_bytes().iter().rev() {
        hash = ((hash >> 14) & 0x01) | ((hash << 1) & 0x7FFF);
        hash ^= u16::from(*byte);
    }
    hash = ((hash >> 14) & 0x01) | ((hash << 1) & 0x7FFF);
    hash ^= password.len() as u16;
    hash ^ 0xCE4B
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(BASE64[((n >> (18 - 6 * index)) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
    {
        let value = BASE64.iter().position(|b| *b == c)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

const K: [u64; 80] = [
    0x428a2f98d728ae22,
    0x7137449123ef65cd,
    0xb5c0fbcfec4d3b2f,
    0xe9b5dba58189dbbc,
    0x3956c25bf348b538,
    0x59f111f1b605d019,
    0x923f82a4af194f9b,
    0xab1c5ed5da6d8118,
    0xd807aa98a3030242,
    0x12835b0145706fbe,
    0x243185be4ee4b28c,
    0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f,
    0x80deb1fe3b1696b1,
    0x9bdc06a725c71235,
    0xc19bf174cf692694,
    0xe49b69c19ef14ad2,
    0xefbe4786384f25e3,
    0x0fc19dc68b8cd5b5,
    0x240ca1cc77ac9c65,
    0x2de92c6f592b0275,
    0x4a7484aa6ea6e483,
    0x5cb0a9dcbd41fbd4,
    0x76f988da831153b5,
    0x983e5152ee66dfab,
    0xa831c66d2db43210,
    0xb00327c898fb213f,
    0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2,
    0xd5a79147930aa725,
    0x06ca6351e003826f,
    0x142929670a0e6e70,
    0x27b70a8546d22ffc,
    0x2e1b21385c26c926,
    0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df,
    0x650a73548baf63de,
    0x766a0abb3c77b2a8,
    0x81c2c92e47edaee6,
    0x92722c851482353b,
    0xa2bfe8a14cf10364,
    0xa81a664bbc423001,
    0xc24b8b70d0f89791,
    0xc76c51a30654be30,
    0xd192e819d6ef5218,
    0xd69906245565a910,
    0xf40e35855771202a,
    0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8,
    0x1e376c085141ab53,
    0x2748774cdf8eeb99,
    0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63,
    0x4ed8aa4ae3418acb,
    0x5b9cca4f7763e373,
    0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc,
    0x78a5636f43172f60,
    0x84c87814a1f0ab72,
    0x8cc702081a6439ec,
    0x90befffa23631e28,
    0xa4506cebde82bde9,
    0xbef9a3f7b2c67915,
    0xc67178f2e372532b,
    0xca273eceea26619c,
    0xd186b8c721c0c207,
    0xeada7dd6cde0eb1e,
    0xf57d4f7fee6ed178,
    0x06f067aa72176fba,
    0x0a637dc5a2c898a6,
    0x113f9804bef90dae,
    0x1b710b35131c471b,
    0x28db77f523047d84,
    0x32caab7b40c72493,
    0x3c9ebe0a15c9bebc,
    0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6,
    0x597f299cfc657e2a,
    0x5fcb6fab3ad6faec,
    0x6c44198c4a475817,
];

pub fn sha512(data: &[u8]) -> [u8; 64] {
    let mut h: [u64; 8] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 128 != 112 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u128) * 8).to_be_bytes());
    for block in message.chunks(128) {
        let mut w = [0u64; 80];
        for (index, word) in block.chunks(8).enumerate() {
            w[index] = u64::from_be_bytes(word.try_into().expect("eight bytes"));
        }
        for t in 16..80 {
            let s0 = w[t - 15].rotate_right(1) ^ w[t - 15].rotate_right(8) ^ (w[t - 15] >> 7);
            let s1 = w[t - 2].rotate_right(19) ^ w[t - 2].rotate_right(61) ^ (w[t - 2] >> 6);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for t in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = [0u8; 64];
    for (index, value) in h.iter().enumerate() {
        out[index * 8..index * 8 + 8].copy_from_slice(&value.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_legacy_passwords() {
        assert_eq!(legacy_hash("password"), 0x83AF);
        assert_eq!(legacy_hash(""), 0);
        let hash = PasswordHash::of("gizli").unwrap();
        assert!(hash.matches("gizli"));
        assert!(!hash.matches("Gizli"));
        assert!(verify(None, "herhangi"));
        assert_eq!(PasswordHash::of(""), None);
    }

    #[test]
    fn computes_sha512_and_base64() {
        let hex: String = sha512(b"abc").iter().map(|b| format!("{b:02x}")).collect();
        assert!(hex.starts_with("ddaf35a193617aba"));
        assert!(hex.ends_with("2a9ac94fa54ca49f"));
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_decode("TWE=").unwrap(), b"Ma");
    }

    #[test]
    fn verifies_modern_hashes() {
        let salt = base64_encode(b"0123456789abcdef");
        let text: Vec<u8> = "şifre".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let mut digest = sha512(&[b"0123456789abcdef".to_vec(), text].concat());
        for round in 0..3u32 {
            digest = sha512(&[digest.to_vec(), round.to_le_bytes().to_vec()].concat());
        }
        let hash = PasswordHash::Sha512 {
            hash: base64_encode(&digest),
            salt,
            spin: 3,
        };
        assert!(hash.matches("şifre"));
        assert!(!hash.matches("sifre"));
    }
}
