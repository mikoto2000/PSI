use std::io::{ErrorKind, Read};

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn validate_signature(reader: &mut impl Read) -> Result<(), String> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == ErrorKind::UnexpectedEof {
            "Incomplete PNG signature".into()
        } else {
            format!("Could not read PNG signature: {error}")
        }
    })?;
    if &bytes != SIGNATURE {
        return Err("Invalid PNG signature".into());
    }
    Ok(())
}

mod header;
mod metadata;
mod scan;
mod text;
pub use header::Header;
pub use scan::{Chunk, Report, scan};
pub use text::{Text, decode_text};

#[cfg(test)]
mod tests;

mod detail;
pub use detail::{Detail, read_detail};

mod kind;
pub use kind::Attributes;

mod preview;
pub use preview::read_preview;

fn read_error(error: std::io::Error) -> String {
    format!("Could not read PNG: {error}")
}
