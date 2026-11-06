use super::*;
use serde_json::{Value, to_value};

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name)
}

#[test]
fn serializes_png_fixtures_to_the_frontend_contract() {
    let session = Session::default();
    for (name, text_cases) in [
        ("plain.png", &[][..]),
        ("text.png", &[("tEXt", "Title", "Café")][..]),
        (
            "compressed-text.png",
            &[("zTXt", "Title", "Compressed text")][..],
        ),
        (
            "japanese.png",
            &[("iTXt", "Title", "日本語のメタデータ")][..],
        ),
        (
            "json.png",
            &[(
                "tEXt",
                "prompt",
                r#"{"id":9007199254740993,"nodes":[{"type":"example","enabled":true}]}"#,
            )][..],
        ),
        (
            "multiple-text.png",
            &[
                ("tEXt", "Title", "Café"),
                ("zTXt", "Title", "Compressed text"),
                ("iTXt", "Title", "日本語のメタデータ"),
                (
                    "tEXt",
                    "prompt",
                    r#"{"id":9007199254740993,"nodes":[{"type":"example","enabled":true}]}"#,
                ),
            ][..],
        ),
    ] {
        let info = session.open(&fixture(name)).unwrap();
        let id = info.id;
        let json = to_value(info).unwrap();
        assert_eq!(json["name"], name);
        assert_eq!(json["report"]["header"]["width"], 2);
        assert_eq!(json["report"]["header"]["bitDepth"], 8);
        let chunks = json["report"]["chunks"].as_array().unwrap();
        assert_eq!(chunks.len(), text_cases.len() + 3);
        assert_eq!(chunks[0]["offset"], 8);
        assert_eq!(chunks.last().unwrap()["name"], "IEND");
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk["storedCrc"] == chunk["computedCrc"])
        );
        for (index, &(kind, keyword, body)) in text_cases.iter().enumerate() {
            let chunk = &chunks[index + 1];
            assert_eq!(chunk["name"], kind);
            assert_eq!(chunk["textKeyword"], keyword);
            let detail = to_value(session.detail(id, index + 1).unwrap()).unwrap();
            assert_eq!(detail["text"]["keyword"], keyword);
            assert_eq!(detail["text"]["text"], body);
            assert_eq!(detail["text"]["compressed"], kind == "zTXt");
            if kind == "iTXt" {
                assert_eq!(detail["text"]["languageTag"], "ja");
                assert_eq!(detail["text"]["translatedKeyword"], "表題");
            }
        }
        assert_eq!(
            session.preview(id).unwrap(),
            fs::read(fixture(name)).unwrap()
        );
    }
}

#[test]
fn preserves_diagnostics_and_rejects_access_from_an_old_document() {
    let session = Session::default();
    let bad = session.open(&fixture("bad-crc.png")).unwrap();
    let truncated = session.open(&fixture("truncated.png")).unwrap();
    assert!(session.detail(bad.id, 1).is_err());
    assert!(session.preview(bad.id).is_err());
    assert!(session.preview(truncated.id).is_err());
    assert_eq!(
        session.detail(truncated.id, 1).unwrap().text.unwrap().text,
        "Café"
    );
    let bad = to_value(bad).unwrap();
    assert_ne!(
        bad["report"]["chunks"][1]["storedCrc"],
        bad["report"]["chunks"][1]["computedCrc"]
    );
    let partial = to_value(truncated).unwrap();
    assert!(
        partial["report"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|issue| issue.contains("Truncated IEND"))
    );
}

#[test]
fn navigates_siblings_in_filename_order_and_stops_at_ends() {
    let session = Session::default();
    let first = session.open(&fixture("bad-crc.png")).unwrap();
    assert!(
        session
            .adjacent(first.id, Direction::Previous)
            .unwrap()
            .is_none()
    );
    let second = session
        .adjacent(first.id, Direction::Next)
        .unwrap()
        .unwrap();
    assert_eq!(second.name, "compressed-text.png");
    assert!(session.adjacent(first.id, Direction::Next).is_err());
    let first_again = session
        .adjacent(second.id, Direction::Previous)
        .unwrap()
        .unwrap();
    assert_eq!(first_again.name, first.name);
    let last = session.open(&fixture("truncated.png")).unwrap();
    assert!(
        session
            .adjacent(last.id, Direction::Next)
            .unwrap()
            .is_none()
    );
}
