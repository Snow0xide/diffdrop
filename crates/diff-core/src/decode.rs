//! Turns raw file bytes into text, or reports that the file is binary.
//!
//! Order of attempts: byte order mark, UTF-16 without BOM (detected from the
//! pattern of zero bytes), binary (any other zero byte), UTF-8, Windows-1252.

use encoding_rs::{UTF_16BE, UTF_16LE, WINDOWS_1252};
use serde::Serialize;

/// How many leading bytes are inspected to tell text from binary.
const SNIFF_LEN: usize = 8000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    Text { text: String, encoding: Encoding },
    Binary,
}

pub fn decode(bytes: &[u8]) -> Decoded {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return match std::str::from_utf8(rest) {
            Ok(text) => text_of(text.to_owned(), Encoding::Utf8Bom),
            Err(_) => Decoded::Binary,
        };
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return decode_utf16(rest, Encoding::Utf16Le);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return decode_utf16(rest, Encoding::Utf16Be);
    }

    let sample = &bytes[..bytes.len().min(SNIFF_LEN)];
    if sample.contains(&0) {
        return match utf16_without_bom(sample) {
            Some(encoding) => decode_utf16(bytes, encoding),
            None => Decoded::Binary,
        };
    }

    match std::str::from_utf8(bytes) {
        Ok(text) => text_of(text.to_owned(), Encoding::Utf8),
        Err(_) => {
            // Windows-1252 maps every byte, so this never fails.
            let (text, _) = WINDOWS_1252.decode_without_bom_handling(bytes);
            text_of(text.into_owned(), Encoding::Windows1252)
        }
    }
}

fn text_of(text: String, encoding: Encoding) -> Decoded {
    Decoded::Text { text, encoding }
}

fn decode_utf16(bytes: &[u8], encoding: Encoding) -> Decoded {
    let codec = if encoding == Encoding::Utf16Le {
        UTF_16LE
    } else {
        UTF_16BE
    };
    let (text, had_errors) = codec.decode_without_bom_handling(bytes);
    // Decoded UTF-16 that still contains NULs is not a text file.
    if had_errors || text.contains('\0') {
        Decoded::Binary
    } else {
        text_of(text.into_owned(), encoding)
    }
}

/// Mostly-ASCII text stored as UTF-16 has a zero in every other byte: odd
/// positions for little endian, even positions for big endian.
fn utf16_without_bom(sample: &[u8]) -> Option<Encoding> {
    // An odd total length is caught later: decoding then reports an error.
    if sample.len() < 2 {
        return None;
    }
    let pairs = sample.len() / 2;
    let (mut even_zeros, mut odd_zeros) = (0usize, 0usize);
    for pair in sample.as_chunks::<2>().0 {
        even_zeros += usize::from(pair[0] == 0);
        odd_zeros += usize::from(pair[1] == 0);
    }
    let mostly = |zeros: usize| zeros * 10 >= pairs * 7;
    let rarely = |zeros: usize| zeros * 20 <= pairs;
    if mostly(odd_zeros) && rarely(even_zeros) {
        Some(Encoding::Utf16Le)
    } else if mostly(even_zeros) && rarely(odd_zeros) {
        Some(Encoding::Utf16Be)
    } else {
        None
    }
}
