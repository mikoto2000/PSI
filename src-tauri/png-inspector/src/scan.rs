use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};

pub const MAX_CHUNKS: usize = 20_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub name: String,
    pub length: u32,
    pub offset: u64,
    pub stored_crc: u32,
    pub computed_crc: u32,
    pub issues: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub chunks: Vec<Chunk>,
    pub issues: Vec<String>,
}

pub fn scan(reader: &mut (impl Read + Seek)) -> Result<Report, String> {
    reader.rewind().map_err(read_error)?;
    super::validate_signature(reader)?;
    let size = reader.seek(SeekFrom::End(0)).map_err(read_error)?;
    reader.seek(SeekFrom::Start(8)).map_err(read_error)?;

    let mut report = Report::default();
    let mut order = Order::default();
    let mut offset = 8_u64;
    let mut buffer = [0; 65_536];
    while offset < size {
        if report.chunks.len() == MAX_CHUNKS {
            report.issues.push(format!(
                "Inspection stopped at the {MAX_CHUNKS} chunk limit"
            ));
            return Ok(report);
        }
        if size - offset < 8 {
            report
                .issues
                .push(format!("Truncated chunk header at byte {offset}"));
            break;
        }
        let mut header = [0; 8];
        reader.read_exact(&mut header).map_err(read_error)?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap());
        let code: [u8; 4] = header[4..].try_into().unwrap();
        let name: String = code
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() {
                    char::from(b)
                } else {
                    '\u{fffd}'
                }
            })
            .collect();
        if length > i32::MAX as u32 {
            report.issues.push(format!(
                "{name} at byte {offset} exceeds the 31-bit chunk length limit"
            ));
            break;
        }
        let span = 12 + u64::from(length);
        if span > size - offset {
            report
                .issues
                .push(format!("Truncated {name} data or CRC at byte {offset}"));
            break;
        }

        let mut issues = order.check(&code, length, report.chunks.is_empty());
        if !code.iter().all(u8::is_ascii_alphabetic) {
            issues.push("Invalid chunk name (expected four ASCII letters)".into());
        } else if code[2].is_ascii_lowercase() {
            issues.push("Reserved chunk-name bit is set".into());
        }
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&code);
        let mut remaining = length as usize;
        while remaining != 0 {
            let count = remaining.min(buffer.len());
            reader
                .read_exact(&mut buffer[..count])
                .map_err(read_error)?;
            hasher.update(&buffer[..count]);
            remaining -= count;
        }
        let mut crc = [0; 4];
        reader.read_exact(&mut crc).map_err(read_error)?;
        report.chunks.push(Chunk {
            name,
            length,
            offset,
            stored_crc: u32::from_be_bytes(crc),
            computed_crc: hasher.finalize(),
            issues,
        });
        offset += span;
        if code == *b"IEND" {
            if offset != size {
                report
                    .issues
                    .push(format!("{} trailing bytes after IEND", size - offset));
            }
            break;
        }
    }
    for (present, name) in [
        (order.ihdr, "IHDR"),
        (order.idat, "IDAT"),
        (order.iend, "IEND"),
    ] {
        if !present {
            report.issues.push(format!("Missing {name}"));
        }
    }
    Ok(report)
}

#[derive(Default)]
struct Order {
    ihdr: bool,
    plte: bool,
    idat: bool,
    after_idat: bool,
    iend: bool,
}

impl Order {
    fn check(&mut self, code: &[u8; 4], length: u32, first: bool) -> Vec<String> {
        let mut issues = Vec::new();
        if first && code != b"IHDR" {
            issues.push("IHDR must be the first chunk".into());
        }
        match code {
            b"IHDR" => {
                if !first {
                    issues.push("IHDR is out of order or duplicated".into());
                }
                self.ihdr = true;
            }
            b"PLTE" => {
                if self.idat || self.plte {
                    issues.push("PLTE is out of order or duplicated".into());
                }
                self.plte = true;
            }
            b"IDAT" => {
                if self.after_idat {
                    issues.push("IDAT chunks must be consecutive".into());
                }
                self.idat = true;
            }
            b"IEND" => {
                if length != 0 {
                    issues.push("IEND must be empty".into());
                }
                self.iend = true;
            }
            _ => {}
        }
        self.after_idat |= self.idat && code != b"IDAT";
        issues
    }
}

pub(crate) fn read_error(error: std::io::Error) -> String {
    format!("Could not read PNG: {error}")
}
