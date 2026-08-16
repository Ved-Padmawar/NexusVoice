use super::*;

#[test]
fn concat_text_blocks_joins_multiple_text_blocks() {
    let parsed = Response {
        content: vec![
            Block::Text {
                text: "Hello, ".to_string(),
            },
            Block::Text {
                text: "world.".to_string(),
            },
        ],
    };
    assert_eq!(concat_text_blocks(parsed), "Hello, world.");
}

#[test]
fn concat_text_blocks_skips_non_text_blocks() {
    let parsed = Response {
        content: vec![
            Block::Other,
            Block::Text {
                text: "kept".to_string(),
            },
        ],
    };
    assert_eq!(concat_text_blocks(parsed), "kept");
}

#[test]
fn concat_text_blocks_empty_content_is_empty_string() {
    let parsed = Response { content: vec![] };
    assert_eq!(concat_text_blocks(parsed), "");
}
