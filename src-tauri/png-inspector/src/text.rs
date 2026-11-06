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
    if name == "iTXt" {
        return international(keyword, body);
    }
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

fn international(keyword: String, data: &[u8]) -> Result<Text, String> {
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
    let language = utf8(language, "Language Tag")?;
    if !language.is_empty() {
        language_tags::LanguageTag::parse(language)
            .map_err(|_| "Invalid Language Tag (BCP 47)")?
            .validate()
            .map_err(|_| "Invalid Language Tag (BCP 47)")?;
    }
    let translated = utf8(translated, "Translated Keyword")?.to_owned();
    let expanded;
    let body = if compressed {
        expanded = inflate(body)?;
        &expanded
    } else {
        body
    };
    if body.contains(&0) {
        return Err("Text contains NUL".into());
    }
    Ok(Text {
        keyword,
        text: utf8(body, "Text")?.to_owned(),
        language_tag: Some(language.to_owned()),
        translated_keyword: Some(translated),
        compressed,
        encoding: "UTF-8",
    })
}

fn utf8<'a>(data: &'a [u8], field: &str) -> Result<&'a str, String> {
    std::str::from_utf8(data).map_err(|_| format!("{field} is not valid UTF-8"))
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
