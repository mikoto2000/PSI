use crate::{
    Header,
    kind::{Attributes, description},
    text::keyword,
};
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};

pub const MAX_CHUNKS: usize = 20_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub name: String,
    pub description: &'static str,
    pub attributes: Option<Attributes>,
    pub text_keyword: Option<String>,
    pub length: u32,
    pub offset: u64,
    pub stored_crc: u32,
    pub computed_crc: u32,
    pub issues: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub header: Option<Header>,
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

        let mut issues = order.check(
            &code,
            length,
            report.chunks.is_empty(),
            report.header.as_ref(),
        );
        let attributes = Attributes::from_code(&code);
        let description = description(&code);
        match attributes {
            None => issues.push("Invalid chunk name (expected four ASCII letters)".into()),
            Some(bits) => {
                if bits.reserved_bit_set {
                    issues.push("Reserved chunk-name bit is set".into());
                }
                if !bits.ancillary && description.is_none() {
                    issues.push("Unknown critical chunk".into());
                }
            }
        }
        let mut text_keyword = None;
        if code == *b"IHDR" && length != 13 {
            issues.push("IHDR must contain 13 bytes".into());
        }
        if length == 0 && matches!(&code, b"tEXt" | b"zTXt" | b"iTXt") {
            issues.push("Missing Keyword separator".into());
        }
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&code);
        let mut remaining = length as usize;
        while remaining != 0 {
            let count = remaining.min(buffer.len());
            reader
                .read_exact(&mut buffer[..count])
                .map_err(read_error)?;
            if code == *b"IHDR" && length == 13 {
                match Header::decode(&buffer[..count]) {
                    Ok(header) if report.chunks.is_empty() => report.header = Some(header),
                    Ok(_) => {}
                    Err(error) => issues.push(error),
                }
            }
            if remaining == length as usize && matches!(&code, b"tEXt" | b"zTXt" | b"iTXt") {
                match keyword(&buffer[..count.min(80)]) {
                    Ok((key, _)) => text_keyword = Some(key),
                    Err(error) => issues.push(error),
                }
            }
            hasher.update(&buffer[..count]);
            remaining -= count;
        }
        let mut crc = [0; 4];
        reader.read_exact(&mut crc).map_err(read_error)?;
        report.chunks.push(Chunk {
            name,
            description: description.unwrap_or("Unknown chunk"),
            attributes,
            text_keyword,
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
    seen: std::collections::HashSet<[u8; 4]>,
}

impl Order {
    fn check(
        &mut self,
        code: &[u8; 4],
        length: u32,
        first: bool,
        header: Option<&Header>,
    ) -> Vec<String> {
        let mut issues = Vec::new();
        if first && code != b"IHDR" {
            issues.push("IHDR must be the first chunk".into());
        }
        let before_palette = matches!(
            code,
            b"gAMA" | b"cHRM" | b"iCCP" | b"sRGB" | b"sBIT" | b"cICP" | b"mDCV" | b"cLLI"
        );
        let before_data = matches!(
            code,
            b"pHYs" | b"eXIf" | b"tRNS" | b"bKGD" | b"hIST" | b"acTL"
        );
        if (before_palette && self.plte) || ((before_palette || before_data) && self.idat) {
            issues.push("Chunk must appear before PLTE or IDAT as specified for this type".into());
        }
        if (before_palette || before_data || code == b"tIME") && !self.seen.insert(*code) {
            issues.push("Chunk may appear only once".into());
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
                if length == 0 || length > 768 || !length.is_multiple_of(3) {
                    issues.push("PLTE must contain 1–256 RGB entries".into());
                }
                if let Some(header) = header {
                    if matches!(header.color_type, 0 | 4) {
                        issues.push("PLTE is forbidden in greyscale images".into());
                    }
                    if header.color_type == 3 && length / 3 > 1 << header.bit_depth {
                        issues.push(
                            "PLTE has more entries than the indexed bit depth permits".into(),
                        );
                    }
                }
                self.plte = true;
            }
            b"IDAT" => {
                if self.after_idat {
                    issues.push("IDAT chunks must be consecutive".into());
                }
                if !self.idat && !self.plte && header.is_some_and(|h| h.color_type == 3) {
                    issues.push("Indexed-color PNG requires PLTE before IDAT".into());
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
