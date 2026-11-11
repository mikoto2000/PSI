use crate::metadata::{
    InternationalText, inflate, international_fields, keyword, latin1, zlib_stream,
};
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
            expanded = inflate(zlib_stream(name, body)?)?;
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
    let InternationalText {
        language,
        translated,
        body,
        compressed,
    } = international_fields(data)?;
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
