use super::*;
use std::io::Cursor;

#[test]
fn accepts_png_signature_and_consumes_eight_bytes() {
    let mut input = Cursor::new([SIGNATURE.as_slice(), b"rest"].concat());
    assert_eq!(validate_signature(&mut input), Ok(()));
    assert_eq!(input.position(), 8);
}

#[test]
fn rejects_wrong_or_incomplete_signature() {
    assert_eq!(
        validate_signature(&mut &b"NOTAPNG!"[..]),
        Err("Invalid PNG signature".into())
    );
    assert_eq!(
        validate_signature(&mut &SIGNATURE[..7]),
        Err("Incomplete PNG signature".into())
    );
}

use crate::scan::MAX_CHUNKS;

pub(crate) const HEADER: &[u8] = &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0];

pub(crate) fn chunk(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
    bytes.extend(name);
    bytes.extend(data);
    bytes.extend(crc32fast::hash(&bytes[4..]).to_be_bytes());
    bytes
}

pub(crate) fn png(extras: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = SIGNATURE.to_vec();
    bytes.extend(chunk(b"IHDR", HEADER));
    for extra in extras {
        bytes.extend(extra);
    }
    bytes.extend(chunk(b"IDAT", &[]));
    bytes.extend(chunk(b"IEND", &[]));
    bytes
}

fn inspect(bytes: Vec<u8>) -> Report {
    scan(&mut Cursor::new(bytes)).unwrap()
}

#[test]
fn lists_chunks_in_order_with_offsets_and_checks_crc() {
    let mut bad = chunk(b"tEXt", b"k\0v");
    *bad.last_mut().unwrap() ^= 1;
    let report = inspect(png(&[bad]));
    assert_eq!(
        report
            .chunks
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["IHDR", "tEXt", "IDAT", "IEND"]
    );
    let [header, text, data, end] = report.chunks.as_slice() else {
        unreachable!()
    };
    assert_eq!(
        (header.offset, text.offset, data.offset, end.offset),
        (8, 33, 48, 60)
    );
    assert_eq!(end.length, 0);
    assert_eq!(end.stored_crc, 0xae426082);
    assert_eq!(end.computed_crc, end.stored_crc);
    assert_ne!(text.computed_crc, text.stored_crc);
    assert!(report.issues.is_empty());
}

#[test]
fn reports_truncated_headers_payloads_and_missing_end() {
    let mut bytes = png(&[]);
    bytes.truncate(bytes.len() - 10);
    let report = inspect(bytes);
    assert!(report.issues.iter().any(|m| m.contains("header")));
    assert!(report.issues.iter().any(|m| m.contains("Missing IEND")));
    let mut bytes = png(&[]);
    bytes.pop();
    let report = inspect(bytes);
    assert!(report.issues.iter().any(|m| m.contains("Truncated IEND")));
    assert_eq!(report.chunks.len(), 2);
}

#[test]
fn reports_invalid_names_order_missing_header_and_trailing_bytes() {
    let mut bytes = SIGNATURE.to_vec();
    for item in [
        chunk(b"IDAT", &[]),
        chunk(b"t!Xt", &[]),
        chunk(b"IDAT", &[]),
        chunk(b"IEND", &[0]),
    ] {
        bytes.extend(item);
    }
    bytes.extend(b"trailing");
    let report = inspect(bytes);
    assert!(report.issues.iter().any(|m| m.contains("Missing IHDR")));
    assert!(report.issues.iter().any(|m| m.contains("after IEND")));
    assert!(report.chunks[0].issues.iter().any(|m| m.contains("first")));
    assert!(report.chunks[1].issues.iter().any(|m| m.contains("name")));
    assert!(
        report.chunks[2]
            .issues
            .iter()
            .any(|m| m.contains("consecutive"))
    );
    assert!(report.chunks[3].issues.iter().any(|m| m.contains("empty")));
}

#[test]
fn rejects_out_of_range_length_and_bounds_chunk_count() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.extend(b"\xff\xff\xff\xffIDAT");
    assert!(inspect(bytes).issues.iter().any(|m| m.contains("31-bit")));
    let report = inspect(png(&vec![chunk(b"vpAg", &[]); MAX_CHUNKS]));
    assert_eq!(report.chunks.len(), MAX_CHUNKS);
    assert!(report.issues.iter().any(|m| m.contains("chunk limit")));
}

#[test]
fn reads_large_chunk_in_bounded_blocks() {
    struct Limited(Cursor<Vec<u8>>);
    impl Read for Limited {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            assert!(buf.len() <= 65_536);
            self.0.read(buf)
        }
    }
    impl std::io::Seek for Limited {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.0.seek(pos)
        }
    }
    let mut bytes = SIGNATURE.to_vec();
    bytes.extend(chunk(b"IHDR", HEADER));
    bytes.extend(chunk(b"IDAT", &vec![123; 300_000]));
    bytes.extend(chunk(b"IEND", &[]));
    let report = scan(&mut Limited(Cursor::new(bytes))).unwrap();
    assert_eq!(report.chunks[1].length, 300_000);
    assert_eq!(report.chunks[1].computed_crc, report.chunks[1].stored_crc);
}

#[test]
fn decodes_all_header_fields() {
    let header = Header::decode(HEADER).unwrap();
    assert_eq!((header.width, header.height, header.bit_depth), (1, 1, 8));
    assert_eq!(
        (header.color_type, header.color_name),
        (6, "Truecolor with alpha")
    );
    assert_eq!(
        (
            header.compression_method,
            header.filter_method,
            header.interlace_method
        ),
        (0, 0, 0)
    );
    let mut interlaced = HEADER.to_vec();
    interlaced[12] = 1;
    assert_eq!(Header::decode(&interlaced).unwrap().interlace_method, 1);
}

#[test]
fn rejects_illegal_header_fields_and_reports_them_during_scan() {
    assert!(Header::decode(&HEADER[..12]).is_err());
    for (offset, value) in [(0, 128), (3, 0), (8, 4), (9, 5), (10, 1), (11, 1), (12, 2)] {
        let mut data = HEADER.to_vec();
        data[offset] = value;
        assert!(Header::decode(&data).is_err(), "field {offset}");
    }
    let mut bytes = png(&[]);
    bytes[24] = 4;
    assert!(
        inspect(bytes).chunks[0]
            .issues
            .iter()
            .any(|m| m.contains("bit depth"))
    );
}

#[test]
fn decodes_latin1_text_and_accepts_an_empty_body() {
    let text = crate::text::decode_text("tEXt", b"Caf\xe9\0Gr\xfc\xdfe\n").unwrap();
    assert_eq!(
        (text.keyword.as_str(), text.text.as_str()),
        ("Café", "Grüße\n")
    );
    assert_eq!((text.encoding, text.compressed), ("Latin-1", false));
    assert!(
        crate::text::decode_text("tEXt", b"Title\0")
            .unwrap()
            .text
            .is_empty()
    );
}

#[test]
fn validates_latin1_keyword_and_body_separators() {
    use crate::text::decode_text;
    for data in [
        b"\0v".as_slice(),
        b" Key\0v",
        b"Key \0v",
        b"K  Y\0v",
        b"K\xa0Y\0v",
        b"K\x80Y\0v",
    ] {
        assert!(decode_text("tEXt", data).unwrap_err().contains("Keyword"));
    }
    let mut long = vec![b'k'; 80];
    long.push(0);
    assert!(decode_text("tEXt", &long).unwrap_err().contains("Keyword"));
    assert!(
        decode_text("tEXt", b"Title")
            .unwrap_err()
            .contains("separator")
    );
    assert!(
        decode_text("tEXt", b"Title\0a\0b")
            .unwrap_err()
            .contains("NUL")
    );
}

pub(crate) fn zlib(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn decodes_zlib_compressed_latin1() {
    let data = [b"Caf\xe9\0\0".as_slice(), &zlib(b"Cr\xe8me")].concat();
    let text = decode_text("zTXt", &data).unwrap();
    assert_eq!(
        (text.keyword.as_str(), text.text.as_str(), text.compressed),
        ("Café", "Crème", true)
    );
}

#[test]
fn rejects_bad_zlib_and_caps_decompression() {
    let decode = |stream: &[u8]| decode_text("zTXt", &[b"K\0\0".as_slice(), stream].concat());
    let good = zlib(b"content");
    let mut damaged = good.clone();
    *damaged.last_mut().unwrap() ^= 1;
    for stream in [
        b"broken".as_slice(),
        &good[..good.len() - 1],
        &damaged,
        &[good.as_slice(), b"trailing"].concat(),
    ] {
        assert!(decode(stream).unwrap_err().contains("zlib"));
    }
    assert!(
        decode_text("zTXt", b"K\0\x01bad")
            .unwrap_err()
            .contains("compression method")
    );
    assert!(
        decode_text("zTXt", b"K\0")
            .unwrap_err()
            .contains("compression method")
    );
    assert!(
        decode(&zlib(&vec![b'x'; 4 * 1024 * 1024 + 1]))
            .unwrap_err()
            .contains("limit")
    );
    assert_eq!(
        decode(&zlib(&vec![b'x'; 4 * 1024 * 1024]))
            .unwrap()
            .text
            .len(),
        4 * 1024 * 1024
    );
}

#[test]
fn decodes_international_metadata_with_optional_compression() {
    let text = decode_text("iTXt", "Title\0\0\x07ja-JP\0表題\0日本語の本文".as_bytes()).unwrap();
    assert_eq!(
        (
            text.language_tag.as_deref(),
            text.translated_keyword.as_deref(),
            text.text.as_str()
        ),
        (Some("ja-JP"), Some("表題"), "日本語の本文")
    );
    assert_eq!((text.encoding, text.compressed), ("UTF-8", false));
    let data = [
        b"Title\0\x01\0ja\0\0".as_slice(),
        &zlib("日本語".as_bytes()),
    ]
    .concat();
    let text = decode_text("iTXt", &data).unwrap();
    assert_eq!((text.text.as_str(), text.compressed), ("日本語", true));
    assert_eq!(decode_text("iTXt", b"Title\0\0\0\0\0").unwrap().text, "");
}

#[test]
fn rejects_invalid_international_metadata() {
    for (data, reason) in [
        (b"K\0\x02\0\0\0body".as_slice(), "compression flag"),
        (b"K\0\x01\x01\0\0body", "compression method"),
        (b"K\0\0", "compression fields"),
        (b"K\0\0\0ja", "Language Tag separator"),
        (b"K\0\0\0ja\0title", "Translated Keyword separator"),
        (b"K\0\0\0ja--JP\0\0body", "Language Tag"),
        (b"K\0\0\0ja\0\xff\0body", "UTF-8"),
        (b"K\0\0\0ja\0\0\xff", "UTF-8"),
        (b"K\0\0\0ja\0\0a\0b", "NUL"),
    ] {
        let error = decode_text("iTXt", data).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}

#[test]
fn interprets_ancillary_metadata_in_human_units() {
    let cases = [
        (
            *b"pHYs",
            [
                3780_u32.to_be_bytes().as_slice(),
                &3780_u32.to_be_bytes(),
                &[1],
            ]
            .concat(),
            ("DPI (X × Y)", "96.01 × 96.01"),
        ),
        (
            *b"gAMA",
            45455_u32.to_be_bytes().to_vec(),
            ("Image Gamma", "0.45455"),
        ),
        (
            *b"tIME",
            vec![7, 234, 11, 6, 8, 9, 60],
            ("UTC", "2026-11-06 08:09:60"),
        ),
        (*b"sRGB", vec![2], ("Rendering Intent", "Saturation (2)")),
        (
            *b"cHRM",
            [31270_u32.to_be_bytes().as_slice(), &[0; 28]].concat(),
            ("White Point (x, y)", "0.31270, 0.00000"),
        ),
        (*b"PLTE", vec![255, 128, 0], ("Entries", "1")),
        (
            *b"eXIf",
            b"II\x2a\0\x08\0\0\0\0\0\0\0\0\0".to_vec(),
            ("Byte Order", "Little-endian (II)"),
        ),
    ];
    for (name, bytes, (label, value)) in cases {
        let mut reader = Cursor::new(png(&[chunk(&name, &bytes)]));
        let report = scan(&mut reader).unwrap();
        let detail = read_detail(&mut reader, &report.chunks[1]).unwrap();
        assert!(
            detail
                .fields
                .iter()
                .any(|f| f.label == label && f.value == value),
            "{detail:?}"
        );
    }
    let mut profile = vec![0; 132];
    profile[..4].copy_from_slice(&132_u32.to_be_bytes());
    profile[16..20].copy_from_slice(b"RGB ");
    profile[36..40].copy_from_slice(b"acsp");
    let bytes = [b"Display\0\0".as_slice(), &zlib(&profile)].concat();
    let mut reader = Cursor::new(png(&[chunk(b"iCCP", &bytes)]));
    let report = scan(&mut reader).unwrap();
    let detail = read_detail(&mut reader, &report.chunks[1]).unwrap();
    assert!(
        detail
            .fields
            .iter()
            .any(|f| f.label == "Color Space" && f.value == "RGB")
    );
}

#[test]
fn rejects_malformed_or_oversized_detail_on_demand() {
    for (name, data) in [
        (*b"pHYs", vec![0; 8]),
        (*b"gAMA", vec![0; 4]),
        (*b"sRGB", vec![4]),
        (*b"tIME", vec![7, 234, 2, 29, 0, 0, 0]),
        (*b"eXIf", vec![0; 8]),
        (*b"tEXt", vec![0; 4 * 1024 * 1024 + 1]),
    ] {
        let mut reader = Cursor::new(png(&[chunk(&name, &data)]));
        let report = scan(&mut reader).unwrap();
        assert!(
            read_detail(&mut reader, &report.chunks[1]).is_err(),
            "{name:?}"
        );
    }
}

#[test]
fn preserves_all_text_chunks_and_duplicate_keywords() {
    let mut reader = Cursor::new(png(&[
        chunk(b"tEXt", b"Title\0one"),
        chunk(b"zTXt", &[b"Title\0\0".as_slice(), &zlib(b"two")].concat()),
        chunk(b"iTXt", "Title\0\0\0ja\0表題\0三".as_bytes()),
    ]));
    let report = scan(&mut reader).unwrap();
    for (index, expected) in ["one", "two", "三"].into_iter().enumerate() {
        let text = read_detail(&mut reader, &report.chunks[index + 1])
            .unwrap()
            .text
            .unwrap();
        assert_eq!(
            (text.keyword.as_str(), text.text.as_str()),
            ("Title", expected)
        );
    }
}

#[test]
fn identifies_unknown_chunk_properties_and_keeps_scanning() {
    let report = inspect(png(&[
        chunk(b"vpAg", &[]),
        chunk(b"AbCD", &[]),
        chunk(b"pUBX", &[]),
    ]));
    let ancillary = &report.chunks[1];
    assert_eq!(
        (
            ancillary.attributes.unwrap().ancillary,
            ancillary.attributes.unwrap().private,
            ancillary.attributes.unwrap().safe_to_copy
        ),
        (true, true, true)
    );
    assert!(ancillary.issues.is_empty());
    let critical = &report.chunks[2];
    assert_eq!(
        (
            critical.attributes.unwrap().ancillary,
            critical.attributes.unwrap().private,
            critical.attributes.unwrap().safe_to_copy
        ),
        (false, true, false)
    );
    assert!(
        critical
            .issues
            .iter()
            .any(|m| m.contains("Unknown critical"))
    );
    assert_eq!(report.chunks[3].description, "Unknown chunk");
    assert_eq!(report.chunks.last().unwrap().name, "IEND");
}

#[test]
fn reports_palette_and_metadata_order_rules() {
    let mut indexed = HEADER.to_vec();
    indexed[9] = 3;
    let mut bytes = SIGNATURE.to_vec();
    for item in [
        chunk(b"IHDR", &indexed),
        chunk(b"IDAT", &[]),
        chunk(b"PLTE", &[0, 0, 0]),
        chunk(b"gAMA", &45455_u32.to_be_bytes()),
        chunk(b"IEND", &[]),
    ] {
        bytes.extend(item);
    }
    let report = inspect(bytes);
    assert!(report.chunks[1].issues.iter().any(|m| m.contains("PLTE")));
    assert!(report.chunks[2].issues.iter().any(|m| m.contains("order")));
    assert!(report.chunks[3].issues.iter().any(|m| m.contains("before")));
}
