//! Transcript bytes must never become dotool commands after a line break.
pub fn encode(text: &str) -> Result<String, String> {
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
    {
        return Err("transcript contains unsupported control characters".to_string());
    }
    let mut output = String::new();
    let mut data = String::new();
    let mut chars = text.chars().peekable();
    let flush = |output: &mut String, data: &mut String| {
        if !data.is_empty() {
            output.push_str("type ");
            output.push_str(data);
            output.push('\n');
            data.clear();
        }
    };
    while let Some(c) = chars.next() {
        let key = match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                Some("Return")
            }
            '\n' => Some("Return"),
            '\t' => Some("Tab"),
            _ => None,
        };
        if let Some(key) = key {
            flush(&mut output, &mut data);
            output.push_str("key ");
            output.push_str(key);
            output.push('\n');
        } else {
            data.push(c);
            // Stay below bufio.Scanner's 64 KiB limit without splitting UTF-8.
            if data.len() >= 4096 {
                flush(&mut output, &mut data);
            }
        }
    }
    flush(&mut output, &mut data);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::encode;
    #[test]
    fn transcript_lines_are_always_data() {
        assert_eq!(encode("hello\nkey Escape\r\ntype world\r\t!").unwrap(),
            "type hello\nkey Return\ntype key Escape\nkey Return\ntype type world\nkey Return\nkey Tab\ntype !\n");
        assert_eq!(encode("\n\n").unwrap(), "key Return\nkey Return\n");
        assert!(encode("hello\0key Escape").is_err());
        assert!(encode("hello\u{1b}key Escape").is_err());
    }
    #[test]
    fn long_unicode_input_cannot_escape_scanner_lines() {
        let text = "\u{0915}".repeat(20000);
        let protocol = encode(&text).unwrap();
        let recovered: String = protocol
            .lines()
            .map(|line| {
                assert!(line.len() < 8192);
                line.strip_prefix("type ").unwrap()
            })
            .collect();
        assert_eq!(recovered, text);
    }
    #[test]
    fn preserves_indentation_and_chunk_boundary_spaces() {
        // Upstream cutWord removes exactly one ASCII delimiter after `type`.
        let text = format!("  key Escape{}  ", "é ".repeat(3000));
        let protocol = encode(&text).unwrap();
        let recovered: String = protocol
            .lines()
            .map(|line| line.strip_prefix("type ").unwrap())
            .collect();
        assert_eq!(recovered, text);
        assert_eq!(encode("   ").unwrap(), "type    \n");
        assert_eq!(
            encode("  key Escape\r\n\ttext").unwrap(),
            "type   key Escape\nkey Return\nkey Tab\ntype text\n"
        );
    }
}
