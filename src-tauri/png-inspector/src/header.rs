use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub color_name: &'static str,
    pub compression_method: u8,
    pub filter_method: u8,
    pub interlace_method: u8,
}

impl Header {
    pub(crate) fn decode(data: &[u8]) -> Result<Self, String> {
        let [
            w0,
            w1,
            w2,
            w3,
            h0,
            h1,
            h2,
            h3,
            bit_depth,
            color_type,
            compression_method,
            filter_method,
            interlace_method,
        ] = *data
        else {
            return Err("IHDR must contain 13 bytes".into());
        };
        let width = u32::from_be_bytes([w0, w1, w2, w3]);
        let height = u32::from_be_bytes([h0, h1, h2, h3]);
        if !(1..=i32::MAX as u32).contains(&width) || !(1..=i32::MAX as u32).contains(&height) {
            return Err("IHDR dimensions must be in 1..=2147483647".into());
        }
        let (color_name, depths): (_, &[_]) = match color_type {
            0 => ("Greyscale", &[1, 2, 4, 8, 16]),
            2 => ("Truecolor", &[8, 16]),
            3 => ("Indexed-color", &[1, 2, 4, 8]),
            4 => ("Greyscale with alpha", &[8, 16]),
            6 => ("Truecolor with alpha", &[8, 16]),
            _ => return Err("Invalid IHDR color type".into()),
        };
        if !depths.contains(&bit_depth) {
            return Err("Invalid IHDR bit depth for this color type".into());
        }
        if compression_method != 0 || filter_method != 0 || interlace_method > 1 {
            return Err("Invalid IHDR compression, filter or interlace method".into());
        }
        Ok(Self {
            width,
            height,
            bit_depth,
            color_type,
            color_name,
            compression_method,
            filter_method,
            interlace_method,
        })
    }
}
