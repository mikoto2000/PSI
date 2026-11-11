pub(super) struct InternationalText<'a> {
    pub(super) language: &'a [u8],
    pub(super) translated: &'a [u8],
    pub(super) body: &'a [u8],
    pub(super) compressed: bool,
}

pub(super) fn international_fields(data: &[u8]) -> Result<InternationalText<'_>, String> {
    let [flag, method, fields @ ..] = data else {
        return Err("Missing iTXt compression fields".into());
    };
    let compressed = match flag {
        0 => false, // PNG decoders ignore the method when the flag is zero.
        1 if *method == 0 => true,
        1 => return Err("Invalid iTXt compression method".into()),
        _ => return Err("Invalid iTXt compression flag".into()),
    };
    let (language, rest) = split_field(fields, "Language Tag")?;
    let (translated, body) = split_field(rest, "Translated Keyword")?;
    Ok(InternationalText {
        language,
        translated,
        body,
        compressed,
    })
}

pub(super) fn zlib_stream<'a>(name: &str, data: &'a [u8]) -> Result<&'a [u8], String> {
    match data {
        [0, stream @ ..] => Ok(stream),
        _ => Err(format!("Invalid or missing {name} compression method")),
    }
}

pub(super) fn keyword(data: &[u8]) -> Result<(String, &[u8]), String> {
    let (key, rest) = split_field(data, "Keyword")?;
    if !(1..=79).contains(&key.len())
        || key.first() == Some(&b' ')
        || key.last() == Some(&b' ')
        || key.windows(2).any(|pair| pair == b"  ")
        || !key.iter().all(|b| matches!(b, 0x20..=0x7e | 0xa1..=0xff))
    {
        return Err(
            "Invalid Keyword (expected 1–79 printable Latin-1 bytes and single internal spaces)"
                .into(),
        );
    }
    Ok((latin1(key), rest))
}

fn split_field<'a>(data: &'a [u8], field: &str) -> Result<(&'a [u8], &'a [u8]), String> {
    data.iter()
        .position(|b| *b == 0)
        .map(|index| (&data[..index], &data[index + 1..]))
        .ok_or_else(|| format!("Missing {field} separator"))
}

pub(super) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

pub(super) const MAX_EXPANDED: usize = 4 * 1024 * 1024;

pub(super) fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    inflate_up_to(data, MAX_EXPANDED)
}

pub(super) fn inflate_up_to(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    use flate2::{Decompress, FlushDecompress, Status};
    let mut decoder = Decompress::new(true);
    let mut output = Vec::new();
    let mut block = [0; 8192];
    loop {
        let consumed = decoder.total_in() as usize;
        let produced = decoder.total_out() as usize;
        let status = decoder
            .decompress(&data[consumed..], &mut block, FlushDecompress::None)
            .map_err(|_| "Invalid zlib stream")?;
        let end = decoder.total_out() as usize;
        if end > limit {
            return Err(format!(
                "zlib output exceeds the {limit}-byte expansion limit"
            ));
        }
        output.extend_from_slice(&block[..end - produced]);
        if status == Status::StreamEnd {
            if decoder.total_in() as usize != data.len() {
                return Err("Trailing bytes after zlib stream".into());
            }
            return Ok(output);
        }
        if consumed == decoder.total_in() as usize && produced == end {
            return Err("Truncated zlib stream".into());
        }
    }
}
