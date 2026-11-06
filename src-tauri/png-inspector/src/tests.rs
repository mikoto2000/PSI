use super::*;
use std::io::Cursor;

#[test]
fn accepts_png_signature_and_consumes_eight_bytes() {
    let mut input = Cursor::new([SIGNATURE.as_slice(), b"rest"].concat());
    assert_eq!(validate_signature(&mut input), Ok(()));
    assert_eq!(input.position(), 8);
}

#[test]
fn rejects_wrong_or_incomplete_signature() {
    assert_eq!(
        validate_signature(&mut &b"NOTAPNG!"[..]),
        Err("Invalid PNG signature".into())
    );
    assert_eq!(
        validate_signature(&mut &SIGNATURE[..7]),
        Err("Incomplete PNG signature".into())
    );
}
