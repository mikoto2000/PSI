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
