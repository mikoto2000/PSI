use crate::{
    Chunk, Header, Text, decode_text,
    metadata::{inflate, keyword, zlib_stream},
    read_error,
};
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};

const MAX_DETAIL_BYTES: u32 = 4 * 1024 * 1024;

#[derive(Debug, Default, Serialize)]
pub struct Detail {
    pub fields: Vec<Field>,
    pub text: Option<Text>,
}

#[derive(Debug, Serialize)]
pub struct Field {
    pub label: &'static str,
    pub value: String,
}

pub fn read_detail(reader: &mut (impl Read + Seek), chunk: &Chunk) -> Result<Detail, String> {
    let maximum = match chunk.name.as_str() {
        "IHDR" => 13,
        "PLTE" => 768,
        "pHYs" => 9,
        "gAMA" => 4,
        "sRGB" => 1,
        "cHRM" => 32,
        "tIME" => 7,
        "tEXt" | "zTXt" | "iTXt" | "iCCP" | "eXIf" => MAX_DETAIL_BYTES,
        _ => return Ok(Detail::default()),
    };
    if chunk.length > maximum {
        return Err(format!(
            "{} detail exceeds the {maximum}-byte limit",
            chunk.name
        ));
    }
    chunk
        .offset
        .checked_add(12 + u64::from(chunk.length))
        .ok_or("Invalid chunk offset")?;
    reader
        .seek(SeekFrom::Start(chunk.offset))
        .map_err(read_error)?;
    let mut header = [0; 8];
    reader.read_exact(&mut header).map_err(read_error)?;
    let changed = "PNG changed after inspection. Reopen the file.";
    if header[..4] != chunk.length.to_be_bytes() || &header[4..] != chunk.name.as_bytes() {
        return Err(changed.into());
    }
    let mut data = vec![0; chunk.length as usize];
    reader.read_exact(&mut data).map_err(read_error)?;
    let mut crc = [0; 4];
    reader.read_exact(&mut crc).map_err(read_error)?;
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&header[4..]);
    hasher.update(&data);
    if hasher.finalize() != chunk.computed_crc || u32::from_be_bytes(crc) != chunk.stored_crc {
        return Err(changed.into());
    }

    let mut detail = Detail::default();
    if matches!(chunk.name.as_str(), "tEXt" | "zTXt" | "iTXt") {
        detail.text = Some(decode_text(&chunk.name, &data)?);
    } else {
        detail.fields = fields(&chunk.name, &data)?;
    }
    Ok(detail)
}

fn fields(name: &str, data: &[u8]) -> Result<Vec<Field>, String> {
    let output = match (name, data) {
        ("IHDR", _) => {
            let h = Header::decode(data)?;
            vec![
                field("Width", h.width),
                field("Height", h.height),
                field("Bit Depth", h.bit_depth),
                field("Color Type", format!("{} ({})", h.color_name, h.color_type)),
                field("Compression Method", "Deflate (0)"),
                field("Filter Method", "Adaptive filtering (0)"),
                field(
                    "Interlace Method",
                    if h.interlace_method == 0 {
                        "None (0)"
                    } else {
                        "Adam7 (1)"
                    },
                ),
            ]
        }
        ("PLTE", _) if !data.is_empty() && data.len().is_multiple_of(3) => {
            vec![
                field("Entries", data.len() / 3),
                field(
                    "RGB Palette",
                    data.as_chunks::<3>()
                        .0
                        .iter()
                        .map(|rgb| format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
            ]
        }
        ("pHYs", [x0, x1, x2, x3, y0, y1, y2, y3, unit @ (0 | 1)]) => {
            let x = u32::from_be_bytes([*x0, *x1, *x2, *x3]);
            let y = u32::from_be_bytes([*y0, *y1, *y2, *y3]);
            let mut fields = vec![
                field("Pixels per unit (X)", x),
                field("Pixels per unit (Y)", y),
                field(
                    "Unit",
                    if *unit == 1 {
                        "Metre (1)"
                    } else {
                        "Unknown (0)"
                    },
                ),
            ];
            if *unit == 1 {
                fields.push(field(
                    "DPI (X × Y)",
                    format!(
                        "{:.2} × {:.2}",
                        f64::from(x) * 0.0254,
                        f64::from(y) * 0.0254
                    ),
                ));
            }
            fields
        }
        ("gAMA", [_, _, _, _]) if be32(data) != 0 => {
            vec![field("Image Gamma", scaled(data))]
        }
        ("sRGB", [intent @ 0..=3]) => {
            let label = [
                "Perceptual",
                "Relative colorimetric",
                "Saturation",
                "Absolute colorimetric",
            ][*intent as usize];
            vec![field("Rendering Intent", format!("{label} ({intent})"))]
        }
        ("cHRM", _) if data.len() == 32 => [
            "White Point (x, y)",
            "Red (x, y)",
            "Green (x, y)",
            "Blue (x, y)",
        ]
        .into_iter()
        .zip(data.as_chunks::<8>().0)
        .map(|(label, xy)| field(label, format!("{}, {}", scaled(&xy[..4]), scaled(&xy[4..]))))
        .collect(),
        (
            "tIME",
            [
                hi,
                lo,
                month @ 1..=12,
                day,
                hour @ 0..=23,
                min @ 0..=59,
                sec @ 0..=60,
            ],
        ) => {
            let year = u16::from_be_bytes([*hi, *lo]);
            let leap =
                year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
            let days = [
                31,
                28 + u8::from(leap),
                31,
                30,
                31,
                30,
                31,
                31,
                30,
                31,
                30,
                31,
            ];
            if *day == 0 || *day > days[usize::from(*month - 1)] {
                return Err("Invalid tIME day".into());
            }
            vec![field(
                "UTC",
                format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}"),
            )]
        }
        ("iCCP", _) => icc_fields(data)?,
        ("eXIf", _) => exif_fields(data)?,
        _ => return Err(format!("Invalid {name} data length or field value")),
    };
    Ok(output)
}

fn icc_fields(data: &[u8]) -> Result<Vec<Field>, String> {
    let (profile_name, body) = keyword(data)?;
    let profile = inflate(zlib_stream("iCCP", body)?)?;
    if profile.len() < 132
        || &profile[36..40] != b"acsp"
        || be32(&profile[..4]) as usize != profile.len()
    {
        return Err("Invalid ICC profile header or size".into());
    }
    let signature = |start| {
        String::from_utf8_lossy(&profile[start..start + 4])
            .trim()
            .to_owned()
    };
    Ok(vec![
        field("Profile Name", profile_name),
        field("Compression Method", "Deflate (0)"),
        field("Expanded Size", format!("{} bytes", profile.len())),
        field(
            "Profile Version",
            format!("{}.{}.{}", profile[8], profile[9] >> 4, profile[9] & 15),
        ),
        field("Device Class", signature(12)),
        field("Color Space", signature(16)),
        field("Connection Space", signature(20)),
    ])
}

fn exif_fields(data: &[u8]) -> Result<Vec<Field>, String> {
    let (order, little_endian) = match data.get(..4) {
        Some(b"II\x2a\0") => ("Little-endian (II)", true),
        Some(b"MM\0\x2a") => ("Big-endian (MM)", false),
        _ => return Err("Invalid Exif TIFF header".into()),
    };
    let bytes: [u8; 4] = data
        .get(4..8)
        .ok_or("Missing Exif IFD offset")?
        .try_into()
        .unwrap();
    let offset = if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    };
    let directory = data
        .get(offset as usize..)
        .filter(|_| offset >= 8)
        .ok_or("Invalid Exif IFD offset")?;
    let count_bytes: [u8; 2] = directory
        .get(..2)
        .ok_or("Truncated Exif directory")?
        .try_into()
        .unwrap();
    let count = if little_endian {
        u16::from_le_bytes(count_bytes)
    } else {
        u16::from_be_bytes(count_bytes)
    };
    if directory.len() < 6 + usize::from(count) * 12 {
        return Err("Truncated Exif directory".into());
    }
    Ok(vec![
        field("Byte Order", order),
        field("First IFD Offset", offset),
        field("First IFD Entries", count),
    ])
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}

fn scaled(bytes: &[u8]) -> String {
    format!("{:.5}", f64::from(be32(bytes)) / 100_000.0)
}

fn field(label: &'static str, value: impl ToString) -> Field {
    Field {
        label,
        value: value.to_string(),
    }
}
