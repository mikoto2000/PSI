use crate::{
    Chunk, scan,
    scan::read_error,
    text::{MAX_EXPANDED, inflate_up_to, split_field},
};
use std::io::{Cursor, Read, Seek};

const MAX_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PREVIEW_PIXELS: u64 = 12_000_000;
const MAX_PREVIEW_SIDE: u32 = 16_384;

pub fn read_preview(reader: &mut (impl Read + Seek)) -> Result<Vec<u8>, String> {
    reader.rewind().map_err(read_error)?;
    let mut bytes = Vec::new();
    reader
        .take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return Err("PNG exceeds the 8 MiB preview size limit".into());
    }
    // Validate the exact bytes passed to the WebView, even if the open file has changed.
    let report = scan(&mut Cursor::new(&bytes))?;
    let header = report.header.ok_or("Missing or invalid IHDR")?;
    if u64::from(header.width) * u64::from(header.height) > MAX_PREVIEW_PIXELS
        || header.width > MAX_PREVIEW_SIDE
        || header.height > MAX_PREVIEW_SIDE
    {
        return Err("Image dimensions exceed the preview pixel limit".into());
    }
    if report
        .chunks
        .iter()
        .any(|c| matches!(c.name.as_str(), "acTL" | "fcTL" | "fdAT"))
    {
        return Err("APNG preview is unavailable".into());
    }
    if !report.issues.is_empty() || report.chunks.iter().filter(|c| c.name == "IHDR").count() != 1 {
        return Err("Incomplete or invalid PNG structure".into());
    }
    check_compressed_metadata(&bytes, &report.chunks)?;
    Ok(bytes)
}

fn check_compressed_metadata(bytes: &[u8], chunks: &[Chunk]) -> Result<(), String> {
    let mut remaining = 8 * 1024 * 1024;
    for chunk in chunks {
        let start = chunk.offset as usize + 8;
        let data = &bytes[start..start + chunk.length as usize];
        let stream = metadata_stream(&chunk.name, data)
            .map_err(|error| format!("Invalid {} preview metadata: {error}", chunk.name))?;
        if let Some(stream) = stream {
            let expanded = inflate_up_to(stream, remaining.min(MAX_EXPANDED)).map_err(|error| {
                format!("Could not load {} preview metadata: {error}", chunk.name)
            })?;
            remaining -= expanded.len();
        }
    }
    Ok(())
}

fn metadata_stream<'a>(name: &str, data: &'a [u8]) -> Result<Option<&'a [u8]>, String> {
    match name {
        "zTXt" | "iCCP" => {
            let (_, body) = split_field(data, "Keyword or profile name")?;
            match body {
                [0, stream @ ..] => Ok(Some(stream)),
                _ => Err("Invalid compression method".into()),
            }
        }
        "iTXt" => {
            let (_, body) = split_field(data, "Keyword")?;
            match body {
                [0, _, ..] => Ok(None),
                [1, 0, fields @ ..] => {
                    let (_, rest) = split_field(fields, "Language Tag")?;
                    let (_, stream) = split_field(rest, "Translated Keyword")?;
                    Ok(Some(stream))
                }
                _ => Err("Invalid compression fields".into()),
            }
        }
        _ => Ok(None),
    }
}
