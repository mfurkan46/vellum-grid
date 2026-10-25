const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

const WINDOWS_1254_CONTROL_RANGE: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', '\u{8E}',
    '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
    '\u{9E}', 'Ÿ',
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Auto,
    Utf8,
    Windows1254,
    Utf16,
}

impl TextEncoding {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Utf8, Self::Windows1254, Self::Utf16];
}

const UTF16_BOM: [u8; 2] = [0xFF, 0xFE];

fn utf16(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&UTF16_BOM).unwrap_or(bytes);
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    String::from_utf16_lossy(&units)
}

pub fn decode_with(bytes: &[u8], encoding: TextEncoding) -> String {
    match encoding {
        TextEncoding::Auto if bytes.starts_with(&UTF16_BOM) => utf16(bytes),
        TextEncoding::Auto => decode(bytes),
        TextEncoding::Utf8 => {
            String::from_utf8_lossy(bytes.strip_prefix(&UTF8_BOM).unwrap_or(bytes)).into_owned()
        }
        TextEncoding::Windows1254 => bytes.iter().map(|&b| windows_1254(b)).collect(),
        TextEncoding::Utf16 => utf16(bytes),
    }
}

pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&UTF8_BOM).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| windows_1254(b)).collect(),
    }
}

fn windows_1254(byte: u8) -> char {
    match byte {
        0x80..=0x9F => WINDOWS_1254_CONTROL_RANGE[usize::from(byte - 0x80)],
        0xD0 => 'Ğ',
        0xDD => 'İ',
        0xDE => 'Ş',
        0xF0 => 'ğ',
        0xFD => 'ı',
        0xFE => 'ş',
        other => char::from(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_utf8_and_strips_bom() {
        assert_eq!(decode("çığ".as_bytes()), "çığ");
        assert_eq!(decode(b"\xEF\xBB\xBFa;b"), "a;b");
    }

    #[test]
    fn decodes_chosen_encodings() {
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("Şık;1".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode_with(&utf16, TextEncoding::Auto), "Şık;1");
        assert_eq!(decode_with(&utf16, TextEncoding::Utf16), "Şık;1");
        assert_eq!(decode_with(&[0xDE, 0xFD], TextEncoding::Windows1254), "Şı");
        assert_eq!(decode_with("ç".as_bytes(), TextEncoding::Utf8), "ç");
    }

    #[test]
    fn falls_back_to_windows_1254() {
        let bytes = [
            0xDD, 0x73, 0x74, 0x61, 0x6E, 0x62, 0x75, 0x6C, 0x20, 0xF0, 0xFD, 0xFE, 0xE7, 0xF6,
            0xFC, 0x80,
        ];
        assert_eq!(decode(&bytes), "İstanbul ğışçöü€");
        assert_eq!(decode(&[0xD0, 0xDE, 0xC7, 0xD6, 0xDC]), "ĞŞÇÖÜ");
    }
}
