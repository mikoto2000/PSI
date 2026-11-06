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
    if name != "tEXt" {
        return Err(format!("Unsupported text chunk: {name}"));
    }
    let (keyword, body) = keyword(data)?;
    if body.contains(&0) {
        return Err("Text contains NUL".into());
    }
    Ok(Text {
        keyword,
        text: latin1(body),
        language_tag: None,
        translated_keyword: None,
        compressed: false,
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
