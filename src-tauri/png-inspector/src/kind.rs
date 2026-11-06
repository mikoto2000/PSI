use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub ancillary: bool,
    pub private: bool,
    pub reserved_bit_set: bool,
    pub safe_to_copy: bool,
}

impl Attributes {
    pub(crate) fn from_code(code: &[u8; 4]) -> Option<Self> {
        code.iter().all(u8::is_ascii_alphabetic).then(|| Self {
            ancillary: code[0].is_ascii_lowercase(),
            private: code[1].is_ascii_lowercase(),
            reserved_bit_set: code[2].is_ascii_lowercase(),
            safe_to_copy: code[3].is_ascii_lowercase(),
        })
    }
}

pub(crate) fn description(code: &[u8; 4]) -> Option<&'static str> {
    Some(match code {
        b"IHDR" => "Image header",
        b"PLTE" => "Color palette",
        b"IDAT" => "Compressed image data",
        b"IEND" => "End of datastream",
        b"tEXt" => "Latin-1 text",
        b"zTXt" => "Compressed Latin-1 text",
        b"iTXt" => "International text",
        b"pHYs" => "Physical pixel dimensions",
        b"gAMA" => "Image gamma",
        b"sRGB" => "Standard RGB color space",
        b"cHRM" => "Chromaticities and white point",
        b"iCCP" => "Embedded ICC profile",
        b"tIME" => "Last image modification time",
        b"eXIf" => "Exif profile",
        b"tRNS" => "Transparency",
        b"sBIT" => "Significant bits",
        b"cICP" => "Video signal type identification",
        b"mDCV" => "Mastering display color volume",
        b"cLLI" => "Content light level",
        b"bKGD" => "Background color",
        b"hIST" => "Palette histogram",
        b"sPLT" => "Suggested palette",
        b"acTL" => "Animation control",
        b"fcTL" => "Animation frame control",
        b"fdAT" => "Animation frame data",
        _ => return None,
    })
}
