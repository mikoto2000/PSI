use png_inspector::{Detail, Field};
use serde::Serialize;
use std::io::{BufRead, BufReader, Cursor, ErrorKind, Read, Seek, SeekFrom};

const MAX_MARKERS: usize = 20_000;
const MAX_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Default, Serialize)]
pub struct Report {
    header: Option<Frame>,
    pub chunks: Vec<Marker>,
    issues: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    width: u16,
    height: u16,
    bit_depth: u8,
    color_name: String,
}

#[derive(Serialize)]
pub struct Marker {
    code: u8,
    name: String,
    description: &'static str,
    offset: u64,
    length: u16,
}

impl Marker {
    fn new(code: u8, offset: u64, length: u16) -> Self {
        let (name, description) = match code {
            0xC0 => ("SOF0", "Baseline DCT frame"),
            0xC1 => ("SOF1", "Extended sequential DCT frame"),
            0xC2 => ("SOF2", "Progressive DCT frame"),
            0xC4 => ("DHT", "Huffman tables"),
            0xCC => ("DAC", "Arithmetic coding conditioning"),
            0xD8 => ("SOI", "Start of image"),
            0xD9 => ("EOI", "End of image"),
            0xDA => ("SOS", "Start of scan"),
            0xDB => ("DQT", "Quantization tables"),
            0xDC => ("DNL", "Define number of lines"),
            0xDD => ("DRI", "Restart interval"),
            0xDE => ("DHP", "Hierarchical progression"),
            0xDF => ("EXP", "Expand reference components"),
            0xFE => ("COM", "Comment"),
            0x01 => ("TEM", "Temporary private use"),
            0xD0..=0xD7 => {
                return Self {
                    code,
                    name: format!("RST{}", code - 0xD0),
                    description: "Restart marker",
                    offset,
                    length,
                };
            }
            0xE0..=0xEF => {
                return Self {
                    code,
                    name: format!("APP{}", code - 0xE0),
                    description: "Application data",
                    offset,
                    length,
                };
            }
            _ if is_frame(code) => {
                return Self {
                    code,
                    name: format!("SOF{}", code - 0xC0),
                    description: "Start of frame",
                    offset,
                    length,
                };
            }
            _ => ("Unknown", "Reserved or unrecognized marker"),
        };
        Self {
            code,
            name: name.into(),
            description,
            offset,
            length,
        }
    }
}

fn is_frame(code: u8) -> bool {
    matches!(code, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
}

pub fn scan(reader: &mut (impl Read + Seek)) -> Result<Report, String> {
    reader.rewind().map_err(read_error)?;
    let mut reader = BufReader::new(reader);
    let mut signature = [0; 2];
    reader.read_exact(&mut signature).map_err(read_error)?;
    if signature != [0xFF, 0xD8] {
        return Err("Invalid JPEG signature".into());
    }

    let mut report = Report::default();
    report.chunks.push(Marker::new(0xD8, 0, 0));
    match scan_markers(&mut reader, &mut report) {
        Ok(()) => {}
        Err(error)
            if error.kind() == ErrorKind::UnexpectedEof
                || error.kind() == ErrorKind::InvalidData =>
        {
            report.issues.push(error.to_string());
        }
        Err(error) => return Err(read_error(error)),
    }
    if report.header.is_none() {
        report
            .issues
            .push("Missing or invalid JPEG frame header".into());
    }
    if !report.chunks.iter().any(|marker| marker.code == 0xDA) {
        report.issues.push("Missing JPEG scan".into());
    }
    Ok(report)
}

fn scan_markers(reader: &mut impl BufRead, report: &mut Report) -> std::io::Result<()> {
    let mut offset = 2;
    let mut in_scan = false;
    let mut data = Vec::new();
    while report.chunks.len() < MAX_MARKERS {
        let (start, code) = next_marker(reader, &mut offset, in_scan)?;
        let standalone = matches!(code, 0x01 | 0xD0..=0xD9);
        if !standalone {
            let mut length = [0; 2];
            reader.read_exact(&mut length)?;
            let length = u16::from_be_bytes(length).checked_sub(2).ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("Invalid JPEG segment length at byte {start}"),
                )
            })?;
            data.resize(usize::from(length), 0);
            reader.read_exact(&mut data)?;
            offset += 2 + u64::from(length);
            report.chunks.push(Marker::new(code, start, length));
            if is_frame(code) {
                match frame(&data) {
                    Some(header) if report.header.is_none() => report.header = Some(header),
                    Some(_) => report
                        .issues
                        .push(format!("Additional JPEG frame at byte {start}")),
                    None => report
                        .issues
                        .push(format!("Invalid JPEG frame at byte {start}")),
                }
            }
        } else {
            report.chunks.push(Marker::new(code, start, 0));
        }
        match code {
            0xD9 => {
                if !reader.fill_buf()?.is_empty() {
                    report.issues.push("Trailing data after EOI".into());
                }
                return Ok(());
            }
            0xD8 => report
                .issues
                .push(format!("Unexpected SOI at byte {start}")),
            0xD0..=0xD7 if !in_scan => report
                .issues
                .push(format!("Restart marker outside scan at byte {start}")),
            _ => {}
        }
        in_scan = code == 0xDA || (in_scan && matches!(code, 0x01 | 0xD0..=0xD7 | 0xDC));
    }
    report.issues.push(format!(
        "Inspection stopped at the {MAX_MARKERS} marker limit"
    ));
    Ok(())
}

// Only scan data can contain FF 00 escapes. FF fill bytes may precede a marker.
fn next_marker(
    reader: &mut impl BufRead,
    offset: &mut u64,
    in_scan: bool,
) -> std::io::Result<(u64, u8)> {
    let mut prefix = false;
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "Missing JPEG EOI marker",
            ));
        }
        let mut consumed = 0;
        for &byte in bytes {
            consumed += 1;
            match byte {
                0xFF => prefix = true,
                0 if prefix && in_scan => prefix = false,
                code if prefix && code != 0 => {
                    *offset += consumed as u64;
                    reader.consume(consumed);
                    return Ok((*offset - 2, code));
                }
                _ if in_scan => {}
                _ => {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Expected JPEG marker at byte {}",
                            *offset + consumed as u64 - 1
                        ),
                    ));
                }
            }
        }
        *offset += consumed as u64;
        reader.consume(consumed);
    }
}

fn frame(data: &[u8]) -> Option<Frame> {
    let [precision, y0, y1, x0, x1, components, rest @ ..] = data else {
        return None;
    };
    let width = u16::from_be_bytes([*x0, *x1]);
    let height = u16::from_be_bytes([*y0, *y1]);
    if width == 0
        || *precision == 0
        || *components == 0
        || rest.len() != usize::from(*components) * 3
    {
        return None;
    }
    Some(Frame {
        width,
        height,
        bit_depth: *precision,
        color_name: match components {
            1 => "Grayscale".into(),
            count => format!("{count} components"),
        },
    })
}

pub fn detail(reader: &mut (impl Read + Seek), marker: &Marker) -> Result<Detail, String> {
    let mut detail = Detail::default();
    detail.fields.push(Field {
        label: "Marker code",
        value: format!("FF {:02X}", marker.code),
    });
    if marker.length == 0 {
        return Ok(detail);
    }
    reader
        .seek(SeekFrom::Start(marker.offset))
        .map_err(read_error)?;
    let mut header = [0; 4];
    reader.read_exact(&mut header).map_err(read_error)?;
    if header[..2] != [0xFF, marker.code]
        || u16::from_be_bytes([header[2], header[3]]).checked_sub(2) != Some(marker.length)
    {
        return Err("JPEG changed after inspection. Reopen the file.".into());
    }
    let mut data = vec![0; usize::from(marker.length)];
    reader.read_exact(&mut data).map_err(read_error)?;
    let fields = match (marker.code, data.as_slice()) {
        (code, _) if is_frame(code) => frame(&data)
            .map(|frame| {
                vec![
                    Field {
                        label: "Width",
                        value: frame.width.to_string(),
                    },
                    Field {
                        label: "Height",
                        value: frame.height.to_string(),
                    },
                    Field {
                        label: "Sample precision",
                        value: frame.bit_depth.to_string(),
                    },
                    Field {
                        label: "Components",
                        value: data[5].to_string(),
                    },
                ]
            })
            .unwrap_or_default(),
        (0xE0, [b'J', b'F', b'I', b'F', 0, major, minor, ..]) => vec![Field {
            label: "JFIF version",
            value: format!("{major}.{minor:02}"),
        }],
        (0xE1, [b'E', b'x', b'i', b'f', 0, 0, ..]) => vec![Field {
            label: "Metadata",
            value: "Exif".into(),
        }],
        (0xE1, data) if data.starts_with(b"http://ns.adobe.com/xap/1.0/\0") => vec![Field {
            label: "Metadata",
            value: "XMP".into(),
        }],
        (0xE2, data) if data.starts_with(b"ICC_PROFILE\0") => vec![Field {
            label: "Metadata",
            value: "ICC profile".into(),
        }],
        (0xDD, [high, low]) => vec![Field {
            label: "Restart interval",
            value: u16::from_be_bytes([*high, *low]).to_string(),
        }],
        (0xFE, _) => vec![Field {
            label: "Comment",
            value: String::from_utf8_lossy(&data).into_owned(),
        }],
        _ => Vec::new(),
    };
    detail.fields.extend(fields);
    Ok(detail)
}

pub fn preview(reader: &mut (impl Read + Seek)) -> Result<Vec<u8>, String> {
    reader.rewind().map_err(read_error)?;
    let mut bytes = Vec::new();
    reader
        .take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err("JPEG exceeds the 8 MiB preview size limit".into());
    }
    let report = scan(&mut Cursor::new(&bytes))?;
    let header = report
        .header
        .ok_or("Missing or invalid JPEG frame header")?;
    if !report.issues.is_empty()
        || header.height == 0
        || header.bit_depth != 8
        || report.chunks.iter().any(|marker| {
            (is_frame(marker.code) && !matches!(marker.code, 0xC0..=0xC2))
                || matches!(marker.code, 0xDC | 0xDE)
        })
    {
        return Err("Incomplete or unsupported JPEG structure for preview".into());
    }
    if u64::from(header.width) * u64::from(header.height) > 12_000_000
        || header.width > 16_384
        || header.height > 16_384
    {
        return Err("Image dimensions exceed the preview pixel limit".into());
    }
    Ok(bytes)
}

fn read_error(error: std::io::Error) -> String {
    format!("Could not read JPEG: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A progressive marker stream with two scans, stuffing, fill, and a restart.
    const PROGRESSIVE: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xFE, 0, 4, b'o', b'k', 0xFF, 0xC2, 0, 11, 8, 0, 3, 0, 4, 1, 1, 0x11, 0,
        0xFF, 0xDA, 0, 8, 1, 1, 0, 0, 0, 0, 0xFF, 0, 0xD9, 0xFF, 0xD0, 0xAB, 0xFF, 0xFF, 0xDA, 0,
        8, 1, 1, 0, 1, 63, 0, 0xAB, 0xFF, 0xD9,
    ];

    #[test]
    fn scans_progressive_markers_and_reads_comment() {
        let mut reader = Cursor::new(PROGRESSIVE);
        let report = scan(&mut reader).unwrap();
        assert!(report.issues.is_empty());
        let header = report.header.unwrap();
        assert_eq!((header.width, header.height), (4, 3));
        let markers: Vec<_> = report
            .chunks
            .iter()
            .map(|m| (m.name.as_str(), m.offset))
            .collect();
        assert_eq!(
            markers,
            [
                ("SOI", 0),
                ("COM", 2),
                ("SOF2", 8),
                ("SOS", 21),
                ("RST0", 34),
                ("SOS", 38),
                ("EOI", 49)
            ]
        );
        assert_eq!(
            detail(&mut reader, &report.chunks[1]).unwrap().fields[1].value,
            "ok"
        );
        assert_eq!(preview(&mut reader).unwrap(), PROGRESSIVE);
    }

    #[test]
    fn keeps_partial_report_and_rejects_unsafe_preview() {
        let mut truncated = Cursor::new(&PROGRESSIVE[..PROGRESSIVE.len() - 1]);
        let report = scan(&mut truncated).unwrap();
        assert!(report.header.is_some());
        assert!(report.issues.iter().any(|issue| issue.contains("EOI")));
        assert!(preview(&mut truncated).is_err());

        let mut oversized = PROGRESSIVE.to_vec();
        oversized[15..17].copy_from_slice(&16_385_u16.to_be_bytes());
        assert!(
            preview(&mut Cursor::new(oversized))
                .unwrap_err()
                .contains("pixel limit")
        );
    }
}
