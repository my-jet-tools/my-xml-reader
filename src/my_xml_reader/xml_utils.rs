pub const OPEN_TAG_TOKEN: u8 = b'<';
pub const CLOSE_TAG_TOKEN: u8 = b'>';
pub const OPEN_HEADER_TOKEN: &[u8] = "<?".as_bytes();
pub const CLOSE_HEADER_TOKEN: &[u8] = "?>".as_bytes();

const OPEN_COMMENT_TOKEN: &[u8] = "<!--".as_bytes();
const CLOSE_COMMENT_TOKEN: &[u8] = "-->".as_bytes();
const OPEN_CDATA_TOKEN: &[u8] = "<![CDATA[".as_bytes();
const CLOSE_CDATA_TOKEN: &[u8] = "]]>".as_bytes();
const OPEN_DECLARATION_TOKEN: &[u8] = "<!".as_bytes();

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

const XML_ESC: [(&str, char); 5] = [
    ("&quot;", '"'),
    ("&apos;", '\''),
    ("&lt;", '<'),
    ("&gt;", '>'),
    ("&amp;", '&'),
];

pub fn has_special_symbol(xml_string: &str) -> bool {
    xml_string.contains("&#") || XML_ESC.iter().any(|(key, _)| xml_string.contains(key))
}

pub fn decode_xml_string(xml_string: &str) -> String {
    let mut result = String::with_capacity(xml_string.len());
    push_decoded(&mut result, xml_string);
    result
}

// Decodes the content of a node: escapes are decoded, CDATA sections are taken as they are,
// comments and processing instructions are skipped. Nested tags stay in the value
pub fn decode_xml_value(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut rest = content;

    while let Some(pos) = rest.find('<') {
        push_decoded(&mut result, &rest[..pos]);
        rest = &rest[pos..];

        match find_end_of_special_markup(rest.as_bytes(), 0) {
            Ok(Some(end_pos)) => {
                if rest.as_bytes().starts_with(OPEN_CDATA_TOKEN) {
                    let cdata_end = end_pos + 1 - CLOSE_CDATA_TOKEN.len();
                    result.push_str(&rest[OPEN_CDATA_TOKEN.len()..cdata_end]);
                }

                rest = &rest[end_pos + 1..];
            }
            _ => {
                result.push('<');
                rest = &rest[1..];
            }
        }
    }

    push_decoded(&mut result, rest);

    result
}

// Single pass: the '&' produced by "&amp;" must not start a new escape ("&amp;lt;" is "&lt;", not "<")
fn push_decoded(result: &mut String, text: &str) {
    let mut rest = text;

    while let Some(pos) = rest.find('&') {
        result.push_str(&rest[..pos]);
        rest = &rest[pos..];

        match decode_escape(rest) {
            Some((symbol, len)) => {
                result.push(symbol);
                rest = &rest[len..];
            }
            None => {
                result.push('&');
                rest = &rest[1..];
            }
        }
    }

    result.push_str(rest);
}

// Decodes the escape (&amp;) or the character reference (&#10; &#xA;) src starts with.
// Returns the symbol and the length of the escape
fn decode_escape(src: &str) -> Option<(char, usize)> {
    for (key, value) in XML_ESC.iter() {
        if src.starts_with(key) {
            return Some((*value, key.len()));
        }
    }

    let body = src.strip_prefix("&#")?;
    let end = body.find(';')?;
    let number = &body[..end];

    let code = match number.strip_prefix('x') {
        Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u32::from_str_radix(hex, 16).ok()?
        }
        None if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => {
            number.parse().ok()?
        }
        _ => return None,
    };

    Some((char::from_u32(code)?, "&#".len() + end + 1))
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

// If a comment, CDATA section, processing instruction or DOCTYPE starts at start_pos -
// returns the position of its last symbol. Returns None if it is an element tag
pub fn find_end_of_special_markup(xml: &[u8], start_pos: usize) -> Result<Option<usize>, String> {
    let src = &xml[start_pos..];

    let (open_token, close_token, markup_name) = if src.starts_with(OPEN_COMMENT_TOKEN) {
        (OPEN_COMMENT_TOKEN, CLOSE_COMMENT_TOKEN, "comment")
    } else if src.starts_with(OPEN_CDATA_TOKEN) {
        (OPEN_CDATA_TOKEN, CLOSE_CDATA_TOKEN, "CDATA section")
    } else if src.starts_with(OPEN_HEADER_TOKEN) {
        (
            OPEN_HEADER_TOKEN,
            CLOSE_HEADER_TOKEN,
            "processing instruction",
        )
    } else if src.starts_with(OPEN_DECLARATION_TOKEN) {
        return match find_end_of_declaration(xml, start_pos) {
            Some(end_pos) => Ok(Some(end_pos)),
            None => Err(format!(
                "Can not find the end of the declaration started at pos {}",
                start_pos
            )),
        };
    } else {
        return Ok(None);
    };

    match find_next_token_ext(xml, close_token, start_pos + open_token.len()) {
        Some(pos) => Ok(Some(pos + close_token.len() - 1)),
        None => Err(format!(
            "Can not find the end of the {} started at pos {}",
            markup_name, start_pos
        )),
    }
}

// <!DOCTYPE ...> can have an internal subset in [...] with '>' inside
fn find_end_of_declaration(xml: &[u8], start_pos: usize) -> Option<usize> {
    let mut opened_quote: Option<u8> = None;
    let mut opened_brackets: usize = 0;

    for (pos, b) in xml
        .iter()
        .enumerate()
        .skip(start_pos + OPEN_DECLARATION_TOKEN.len())
    {
        let b = *b;

        match opened_quote {
            Some(quote) => {
                if b == quote {
                    opened_quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => opened_quote = Some(b),
                b'[' => opened_brackets += 1,
                b']' => opened_brackets = opened_brackets.saturating_sub(1),
                CLOSE_TAG_TOKEN if opened_brackets == 0 => return Some(pos),
                _ => {}
            },
        }
    }

    None
}

// Finds the '>' which closes the element tag started at start_pos. '>' inside quoted attribute values is skipped
pub fn find_end_of_tag(xml: &[u8], start_pos: usize) -> Option<usize> {
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
        assert_eq!("&#10;", decode_xml_string("&amp;#10;"));
    }

    #[test]
    fn test_decode_character_references() {
        assert_eq!("a\nb", decode_xml_string("a&#10;b"));
        assert_eq!("A€", decode_xml_string("&#x41;&#x20AC;"));
        assert!(has_special_symbol("a&#10;b"));
    }

    #[test]
    fn test_decode_xml_string_keeps_unknown_escapes() {
        let src = "&nbsp; & &#; &#x; &#xZZ; &#+10; &#1114112; &#10 &";
        assert_eq!(src, decode_xml_string(src));
    }

    #[test]
    fn test_decode_xml_value() {
        let src = "a &amp; b<!-- c --> &#x41;<?pi x?><![CDATA[&amp; <i> ]]>";
        assert_eq!("a & b A&amp; <i> ", decode_xml_value(src));
    }

    #[test]
    fn test_decode_xml_value_with_nested_tags() {
        assert_eq!("<b>1 & 2</b>", decode_xml_value("<b>1 &amp; 2</b>"));
        assert_eq!("<!-- not closed", decode_xml_value("<!-- not closed"));
    }

    #[test]
    fn test_find_end_of_special_markup() {
        for markup in [
            "<!-- it's > a comment -->",
            "<!---->",
            "<![CDATA[ <A> ]]>",
            "<![CDATA[]]>",
            "<?pi value='>'?>",
            "<!DOCTYPE R>",
            "<!DOCTYPE R [<!ELEMENT R (A)> <!ATTLIST R a CDATA \"]>\">]>",
        ] {
            let xml = format!("{}<A/>", markup);

            assert_eq!(
                Some(markup.len() - 1),
                find_end_of_special_markup(xml.as_bytes(), 0).unwrap(),
                "{}",
                markup
            );
        }

        assert_eq!(None, find_end_of_special_markup(b"<A/>", 0).unwrap());
        assert_eq!(None, find_end_of_special_markup(b"</A>", 0).unwrap());

        assert!(find_end_of_special_markup(b"<!-- not closed", 0).is_err());
        assert!(find_end_of_special_markup(b"<![CDATA[ not closed", 0).is_err());
        assert!(find_end_of_special_markup(b"<?pi not closed", 0).is_err());
        assert!(find_end_of_special_markup(b"<!DOCTYPE R [ <A>", 0).is_err());
    }

    #[test]
    fn test_find_end_of_tag() {
        let xml = br#"<A b="x>y" c='/>'>"#;
        assert_eq!(Some(xml.len() - 1), find_end_of_tag(xml, 0));

        let xml = br#"<A b="x>"#;
        assert_eq!(None, find_end_of_tag(xml, 0));
    }
}
