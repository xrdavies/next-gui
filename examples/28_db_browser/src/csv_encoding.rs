//! File decoding before CSV parsing or any database mutation.

use encoding_rs::Encoding;

pub const NAMES: &[&str] = &[
    "UTF-8",
    "UTF-16LE",
    "UTF-16BE",
    "UTF-32LE",
    "UTF-32BE",
    "ISO-8859-1",
    "US-ASCII",
    "windows-1250",
    "windows-1251",
    "windows-1252",
    "windows-1253",
    "windows-1254",
    "windows-1255",
    "windows-1256",
    "windows-1257",
    "windows-1258",
    "windows-874",
    "Shift_JIS",
    "EUC-JP",
    "ISO-2022-JP",
    "GBK",
    "GB18030",
    "Big5",
    "EUC-KR",
    "KOI8-R",
    "KOI8-U",
    "IBM866",
    "macintosh",
    "x-mac-cyrillic",
    "ISO-8859-2",
    "ISO-8859-3",
    "ISO-8859-4",
    "ISO-8859-5",
    "ISO-8859-6",
    "ISO-8859-7",
    "ISO-8859-8",
    "ISO-8859-8-I",
    "ISO-8859-10",
    "ISO-8859-13",
    "ISO-8859-14",
    "ISO-8859-15",
    "ISO-8859-16",
];

// shortcut: Qt-only codecs outside this supported list require additional decoders for full codec parity.
enum Codec {
    Standard(&'static Encoding),
    Latin1,
    Ascii,
    Utf32(bool),
}

fn codec(label: &str) -> Result<Codec, String> {
    let label = label.trim();
    let normalized = label.to_ascii_lowercase().replace(['-', '_', ' '], "");
    Ok(match normalized.as_str() {
        "" => Codec::Standard(encoding_rs::UTF_8),
        "latin1" | "l1" | "iso88591" | "iso885911987" | "isoir100" | "ibm819" | "cp819"
        | "csisolatin1" => Codec::Latin1,
        "ascii" | "usascii" | "iso646us" | "ansix3.41968" => Codec::Ascii,
        "utf32" | "utf32le" => Codec::Utf32(true),
        "utf32be" => Codec::Utf32(false),
        _ => Codec::Standard(
            Encoding::for_label_no_replacement(label.as_bytes())
                .ok_or_else(|| format!("Unsupported CSV encoding: {label}"))?,
        ),
    })
}

pub fn decode(raw: &[u8], label: &str) -> Result<String, String> {
    let selected = codec(label)?;
    // QTextStream detects Unicode BOMs even when another codec was selected.
    let (selected, raw) = if raw.starts_with(&[0xff, 0xfe, 0, 0]) {
        (Codec::Utf32(true), &raw[4..])
    } else if raw.starts_with(&[0, 0, 0xfe, 0xff]) {
        (Codec::Utf32(false), &raw[4..])
    } else if let Some((encoding, len)) = Encoding::for_bom(raw) {
        (Codec::Standard(encoding), &raw[len..])
    } else {
        (selected, raw)
    };
    match selected {
        Codec::Latin1 => Ok(raw.iter().copied().map(char::from).collect()),
        Codec::Ascii if raw.is_ascii() => Ok(raw.iter().copied().map(char::from).collect()),
        Codec::Ascii => Err("CSV US-ASCII contains a non-ASCII byte".into()),
        Codec::Utf32(little) => {
            let name = if little { "UTF-32LE" } else { "UTF-32BE" };
            if raw.len() % 4 != 0 {
                return Err(format!("CSV {name} has an incomplete code unit"));
            }
            raw.chunks_exact(4)
                .map(|bytes| {
                    let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
                    let unit = if little {
                        u32::from_le_bytes(bytes)
                    } else {
                        u32::from_be_bytes(bytes)
                    };
                    char::from_u32(unit).ok_or_else(|| {
                        format!("CSV {name} contains an invalid Unicode scalar U+{unit:04X}")
                    })
                })
                .collect()
        }
        Codec::Standard(encoding) => encoding
            .decode_without_bom_handling_and_without_replacement(raw)
            .map(|text| text.into_owned())
            .ok_or_else(|| {
                format!(
                    "CSV {} contains invalid or incomplete byte sequences",
                    encoding.name()
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_code_pages_and_latin1_are_distinct_and_lossless() {
        for (label, bytes, text) in [
            ("Shift_JIS", &b"\x93\xfa\x96\x7b"[..], "日本"),
            ("GBK", &b"\xd6\xd0\xce\xc4"[..], "中文"),
            ("windows-1251", &b"\xcf\xf0\xe8\xe2\xe5\xf2"[..], "Привет"),
            ("windows-1252", &b"caf\xe9 \x80"[..], "café €"),
            ("ISO-8859-1", &b"caf\xe9 \x80"[..], "café \u{80}"),
            ("latin1", &b"\x80\xff"[..], "\u{80}ÿ"),
        ] {
            assert_eq!(decode(bytes, label).unwrap(), text, "{label}");
        }
        for name in NAMES {
            assert!(codec(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn unicode_boms_override_the_selected_encoding_and_endianness() {
        for label in ["UTF-8", "UTF-16", "ISO-8859-1", "Shift_JIS", "US-ASCII"] {
            for bytes in [
                vec![0xef, 0xbb, 0xbf, 0xf0, 0x9f, 0x98, 0x80],
                vec![0xff, 0xfe, 0x3d, 0xd8, 0, 0xde],
                vec![0xfe, 0xff, 0xd8, 0x3d, 0xde, 0],
                vec![0xff, 0xfe, 0, 0, 0, 0xf6, 1, 0],
                vec![0, 0, 0xfe, 0xff, 0, 1, 0xf6, 0],
            ] {
                assert_eq!(decode(&bytes, label).unwrap(), "😀", "{label}/{bytes:?}");
            }
        }
        assert_eq!(decode(&[0x3d, 0xd8, 0, 0xde], "UTF-16").unwrap(), "😀");
        assert_eq!(decode(&[0xd8, 0x3d, 0xde, 0], "UTF-16BE").unwrap(), "😀");
        assert_eq!(decode(&[0, 1, 0xf6, 0], "UTF-32BE").unwrap(), "😀");
    }

    #[test]
    fn invalid_sequences_and_unknown_codecs_reject_without_replacement() {
        for (label, bytes) in [
            ("UTF-8", vec![0xff]),
            ("UTF-16LE", vec![0]),
            ("UTF-16BE", vec![0xd8, 0]),
            ("UTF-32LE", vec![0, 0xd8, 0, 0]),
            ("UTF-32BE", vec![0, 0x11, 0, 0]),
            ("UTF-32", vec![65, 0]),
            ("Shift_JIS", vec![0x82]),
            ("GBK", vec![0x81]),
            ("US-ASCII", vec![0x80]),
            ("replacement", vec![]),
            ("made-up-codec", vec![0xef, 0xbb, 0xbf]),
        ] {
            assert!(decode(&bytes, label).is_err(), "{label}/{bytes:?}");
        }
        assert_eq!(decode(b"value", "").unwrap(), "value");
    }
}
