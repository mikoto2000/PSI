use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub keyword: String,
    pub text: String,
    pub language_tag: Option<String>,
    pub translated_keyword: Option<String>,
    pub compressed: bool,
    pub encoding: &'static str,
}

pub fn decode_text(name: &str, data: &[u8]) -> Result<Text, String> {
    let (keyword, body) = keyword(data)?;
    let compressed = name == "zTXt";
    let expanded;
    let body = match name {
        "tEXt" => body,
        "zTXt" => {
            let [0, stream @ ..] = body else {
                return Err("Invalid or missing zTXt compression method".into());
            };
            expanded = inflate(stream)?;
            &expanded
        }
        _ => return Err(format!("Unsupported text chunk: {name}")),
    };
    if body.contains(&0) {
        return Err("Text contains NUL".into());
    }
    Ok(Text {
        keyword,
        text: latin1(body),
        language_tag: None,
        translated_keyword: None,
        compressed,
        encoding: "Latin-1",
    })
}

fn keyword(data: &[u8]) -> Result<(String, &[u8]), String> {
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

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().copied().map(char::from).collect()
}

pub(crate) const MAX_EXPANDED: usize = 4 * 1024 * 1024;

pub(crate) fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
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
        if end > MAX_EXPANDED {
            return Err("zlib output exceeds the 4 MiB expansion limit".into());
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
