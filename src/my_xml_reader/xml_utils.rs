pub const OPEN_TAG_TOKEN: u8 = b'<';
pub const CLOSE_TAG_TOKEN: u8 = b'>';
pub const OPEN_HEADER_TOKEN: &[u8] = "<?".as_bytes();
pub const CLOSE_HEADER_TOKEN: &[u8] = "?>".as_bytes();

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

const XML_ESC: [(&str, &str); 5] = [
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&amp;", "&"),
];

pub fn has_special_symbol(xml_string: &str) -> bool {
    XML_ESC.iter().any(|(key, _)| xml_string.contains(key))
}

pub fn decode_xml_string(xml_string: &str) -> String {
    // Single pass: the '&' produced by "&amp;" must not start a new escape ("&amp;lt;" is "&lt;", not "<")
    let mut result = String::with_capacity(xml_string.len());
    let mut rest = xml_string;

    while let Some(pos) = rest.find('&') {
        result.push_str(&rest[..pos]);
        rest = &rest[pos..];

        match XML_ESC.iter().find(|(key, _)| rest.starts_with(key)) {
            Some((key, value)) => {
                result.push_str(value);
                rest = &rest[key.len()..];
            }
            None => {
                result.push('&');
                rest = &rest[1..];
            }
        }
    }

    result.push_str(rest);

    result
}

pub fn is_xml_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

pub fn skip_whitespaces(xml: &[u8], start_pos: usize) -> usize {
    let mut pos = start_pos;

    while pos < xml.len() && is_xml_whitespace(xml[pos]) {
        pos += 1;
    }

    pos
}

fn skip_xml_header(xml: &[u8], start_pos: usize) -> Result<usize, String> {
    if !xml[start_pos..].starts_with(OPEN_HEADER_TOKEN) {
        return Ok(start_pos);
    }

    let close_header_pos =
        find_next_token_ext(xml, CLOSE_HEADER_TOKEN, start_pos + OPEN_HEADER_TOKEN.len());

    match close_header_pos {
        Some(end_header_pos) => {
            let pos = skip_whitespaces(xml, end_header_pos + CLOSE_HEADER_TOKEN.len());

            if xml.get(pos) == Some(&OPEN_TAG_TOKEN) {
                Ok(pos)
            } else {
                Err(format!(
                    "Can not find root TAG after header Node. Unexpected content at pos {}",
                    pos
                ))
            }
        }
        None => Err("Can not find close header Node".to_string()),
    }
}

pub fn init_pos_start(xml: &[u8]) -> Result<usize, String> {
    // Only a UTF-8 BOM and whitespaces are allowed before the first tag or the XML header
    let pos = if xml.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else {
        0
    };

    let pos = skip_whitespaces(xml, pos);

    if pos == xml.len() {
        return Err("Can not find first TAG or XML Header such as '<'".to_string());
    }

    if xml[pos] != OPEN_TAG_TOKEN {
        return Err(format!(
            "Unexpected content before the first TAG or XML Header at pos {}",
            pos
        ));
    }

    skip_xml_header(xml, pos)
}

pub fn find_next_token(xml: &[u8], token_to_find: u8, start_pos: usize) -> Option<usize> {
    let pos = xml
        .get(start_pos..)?
        .iter()
        .position(|b| *b == token_to_find)?;

    Some(start_pos + pos)
}

// Finds the '>' which closes the tag started at start_pos. '>' inside quoted attribute values is skipped
pub fn find_end_of_tag(xml: &[u8], start_pos: usize) -> Option<usize> {
    // Comments, CDATA, DOCTYPE and processing instructions are not elements - quotes mean nothing there
    if matches!(xml.get(start_pos + 1), Some(b'!') | Some(b'?')) {
        return find_next_token(xml, CLOSE_TAG_TOKEN, start_pos);
    }

    let mut opened_quote: Option<u8> = None;

    for (pos, b) in xml.iter().enumerate().skip(start_pos) {
        let b = *b;

        match opened_quote {
            Some(quote) => {
                if b == quote {
                    opened_quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => opened_quote = Some(b),
                CLOSE_TAG_TOKEN => return Some(pos),
                _ => {}
            },
        }
    }

    None
}

pub fn extract_tag_name(node_tag: &[u8]) -> &[u8] {
    let mut loop_start: usize = 1;

    if node_tag[1] == b'/' {
        loop_start = 2;
    }

    for i in loop_start..node_tag.len() - 1 {
        if node_tag[i] <= 32 || node_tag[i] == b'/' {
            return &node_tag[loop_start..i];
        }
    }

    &node_tag[loop_start..node_tag.len() - 1]
}

fn find_next_token_ext(xml: &[u8], token_to_find: &[u8], start_pos: usize) -> Option<usize> {
    let pos = xml
        .get(start_pos..)?
        .windows(token_to_find.len())
        .position(|window| window == token_to_find)?;

    Some(start_pos + pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_node_name() {
        let xml_src = r#"<RootNode/>"#;

        let node_name = extract_tag_name(xml_src.as_bytes());

        assert_eq!("RootNode", std::str::from_utf8(node_name).unwrap());
    }

    #[test]
    fn test_extract_node_name_ver2() {
        let xml_src = r#"<RootNode />"#;

        let node_name = extract_tag_name(xml_src.as_bytes());

        assert_eq!("RootNode", std::str::from_utf8(node_name).unwrap());
    }

    #[test]
    fn test_extract_node_name_ver3() {
        let xml_src = r#"<RootNode>"#;

        let node_name = extract_tag_name(xml_src.as_bytes());

        assert_eq!("RootNode", std::str::from_utf8(node_name).unwrap());
    }

    #[test]
    fn test_extract_node_name_ver4() {
        let xml_src = r#"</RootNode>"#;

        let node_name = extract_tag_name(xml_src.as_bytes());

        assert_eq!("RootNode", std::str::from_utf8(node_name).unwrap());
    }

    #[test]
    fn test_init_pos_start_with_header_and_leading_space() {
        let xml_src = "   <?xml version=\"1.0\"?><Root/>";

        let pos = init_pos_start(xml_src.as_bytes()).unwrap();

        assert_eq!("<Root/>", &xml_src[pos..]);
    }

    #[test]
    fn test_init_pos_start_with_header_and_bom() {
        let xml_src = "\u{feff}<?xml version=\"1.0\"?><Root></Root>";
        let bytes = xml_src.as_bytes();

        let pos = init_pos_start(bytes).unwrap();

        assert_eq!("<Root></Root>", std::str::from_utf8(&bytes[pos..]).unwrap());
    }

    #[test]
    fn test_init_pos_start_with_bom_and_no_header() {
        let xml_src = "\u{feff}\r\n<Root></Root>";
        let bytes = xml_src.as_bytes();

        let pos = init_pos_start(bytes).unwrap();

        assert_eq!("<Root></Root>", std::str::from_utf8(&bytes[pos..]).unwrap());
    }

    #[test]
    fn test_init_pos_start_without_header_and_leading_space() {
        let xml_src = "   <Root></Root>";
        let pos = init_pos_start(xml_src.as_bytes()).unwrap();

        assert_eq!("<Root></Root>", &xml_src[pos..]);
    }

    #[test]
    fn test_init_pos_start_with_header_and_gap_before_root() {
        let xml_src = "<?xml version=\"1.0\"?>   <Root></Root>";
        let pos = init_pos_start(xml_src.as_bytes()).unwrap();

        assert_eq!("<Root></Root>", &xml_src[pos..]);
    }

    #[test]
    fn test_init_pos_start_with_unexpected_content() {
        assert!(init_pos_start(b"").is_err());
        assert!(init_pos_start(b" \r\n").is_err());
        assert!(init_pos_start(b"garbage<Root/>").is_err());
        assert!(init_pos_start(b"<?xml version=\"1.0\"?>garbage<Root/>").is_err());
        assert!(init_pos_start(b"<?xml version=\"1.0\"").is_err());
        assert!(init_pos_start(b"<?xml version=\"1.0\"?>").is_err());
    }

    #[test]
    fn test_decode_xml_string() {
        assert_eq!(
            "a < b && c > \"d\" 'e'",
            decode_xml_string("a &lt; b &amp;&amp; c &gt; &quot;d&quot; &apos;e&apos;")
        );
    }

    #[test]
    fn test_decode_xml_string_does_not_decode_twice() {
        assert_eq!("&lt;", decode_xml_string("&amp;lt;"));
        assert_eq!("&amp;", decode_xml_string("&amp;amp;"));
        assert_eq!("&quot;", decode_xml_string("&amp;quot;"));
    }

    #[test]
    fn test_decode_xml_string_keeps_unknown_escapes() {
        assert_eq!("&nbsp; & &#10; &", decode_xml_string("&nbsp; & &#10; &"));
    }

    #[test]
    fn test_find_end_of_tag() {
        let xml = br#"<A b="x>y" c='/>'>"#;
        assert_eq!(Some(xml.len() - 1), find_end_of_tag(xml, 0));

        let xml = br#"<!-- it's a comment -->"#;
        assert_eq!(Some(xml.len() - 1), find_end_of_tag(xml, 0));

        let xml = br#"<A b="x>"#;
        assert_eq!(None, find_end_of_tag(xml, 0));
    }
}
