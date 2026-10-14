use super::xml_utils::{is_xml_whitespace, skip_whitespaces};

pub struct AttributesIterator<'s> {
    data: &'s [u8],
    pos: usize,
}

impl<'s> AttributesIterator<'s> {
    pub fn new(data: &'s [u8]) -> Self {
        Self {
            data,
            pos: find_end_of_tag_name(data),
        }
    }

    pub fn get_next(&mut self) -> Option<(&'s str, &'s str)> {
        match read_attribute(self.data, self.pos) {
            Some((key, value, next_pos)) => {
                self.pos = next_pos;
                Some((key, value))
            }
            None => {
                // End of the tag or a malformed attribute - nothing to read anymore
                self.pos = self.data.len();
                None
            }
        }
    }
}

impl<'s> Iterator for AttributesIterator<'s> {
    type Item = (&'s str, &'s str);

    fn next(&mut self) -> Option<Self::Item> {
        self.get_next()
    }
}

fn find_end_of_tag_name(data: &[u8]) -> usize {
    for (i, b) in data.iter().enumerate() {
        let b = *b;

        if is_xml_whitespace(b) || b == b'/' || b == b'>' {
            return i;
        }
    }

    data.len()
}

// Reads key="value" or key='value' starting from pos.
// Returns the key, the value and the position right after the closing quote
fn read_attribute(data: &[u8], pos: usize) -> Option<(&str, &str, usize)> {
    let key_start = skip_whitespaces(data, pos);
    let mut key_end = key_start;

    while key_end < data.len() {
        let b = data[key_end];

        if is_xml_whitespace(b) || b == b'=' || b == b'/' || b == b'>' {
            break;
        }

        key_end += 1;
    }

    if key_end == key_start {
        return None;
    }

    let eq_pos = skip_whitespaces(data, key_end);

    if data.get(eq_pos) != Some(&b'=') {
        return None;
    }

    let quote_pos = skip_whitespaces(data, eq_pos + 1);
    let quote = *data.get(quote_pos)?;

    if quote != b'"' && quote != b'\'' {
        return None;
    }

    // There are no escapes with '\' in XML - the value ends with the first matching quote
    let value_start = quote_pos + 1;
    let value_end = value_start + data[value_start..].iter().position(|b| *b == quote)?;

    let key = std::str::from_utf8(&data[key_start..key_end]).ok()?;
    let value = std::str::from_utf8(&data[value_start..value_end]).ok()?;

    Some((key, value, value_end + 1))
}

#[cfg(test)]
mod tests {
    use crate::my_xml_reader::AttributesIterator;

    fn read_all(xml: &str) -> Vec<(&str, &str)> {
        AttributesIterator::new(xml.as_bytes()).collect()
    }

    #[test]
    fn test_tags_iterator_with_no_attrs() {
        let xml = "<test></test>";

        let mut attrs_iterator = AttributesIterator::new(xml.as_bytes());
        let next = attrs_iterator.get_next();
        assert!(next.is_none());
    }

    #[test]
    fn test_tags_iterator_with_no_attrs_case_2() {
        let xml = "<test/>";

        let mut attrs_iterator = AttributesIterator::new(xml.as_bytes());
        let next = attrs_iterator.get_next();
        assert!(next.is_none());
    }

    #[test]
    fn test_tags_iterator_with_no_attrs_case_3() {
        let xml = "<test />";

        let mut attrs_iterator = AttributesIterator::new(xml.as_bytes());
        let next = attrs_iterator.get_next();
        assert!(next.is_none());
    }

    #[test]
    fn test_tags_iterator_with_single_attribute() {
        let xml = "<test attr1=\"value1\" attr2=\"value2\"/>";

        let mut attrs_iterator = AttributesIterator::new(xml.as_bytes());
        let next = attrs_iterator.get_next().unwrap();

        assert_eq!(next.0, "attr1");
        assert_eq!(next.1, "value1");

        let next = attrs_iterator.get_next().unwrap();

        assert_eq!(next.0, "attr2");
        assert_eq!(next.1, "value2");

        assert!(attrs_iterator.get_next().is_none());
    }

    #[test]
    fn test_attributes_separated_by_tabs_and_new_lines() {
        let xml = "<test\tattr1=\"value1\"\r\n    attr2=\"value2\"\n/>";

        assert_eq!(
            vec![("attr1", "value1"), ("attr2", "value2")],
            read_all(xml)
        );
    }

    #[test]
    fn test_single_quoted_attributes() {
        let xml = r#"<test attr1='value1' attr2='say "hi"'>"#;

        assert_eq!(
            vec![("attr1", "value1"), ("attr2", "say \"hi\"")],
            read_all(xml)
        );
    }

    #[test]
    fn test_spaces_around_eq() {
        let xml = r#"<test attr1 = "value1" attr2= "value2" >"#;

        assert_eq!(
            vec![("attr1", "value1"), ("attr2", "value2")],
            read_all(xml)
        );
    }

    #[test]
    fn test_backslash_is_not_an_escape() {
        let xml = r#"<test path="C:\" attr2="value2"/>"#;

        assert_eq!(vec![("path", r"C:\"), ("attr2", "value2")], read_all(xml));
    }

    #[test]
    fn test_special_symbols_inside_value() {
        let xml = r#"<test attr1="a/>b" attr2="" attr3="&amp;"/>"#;

        assert_eq!(
            vec![("attr1", "a/>b"), ("attr2", ""), ("attr3", "&amp;")],
            read_all(xml)
        );
    }

    #[test]
    fn test_malformed_attribute_stops_iteration() {
        let xml = r#"<test attr1="value1" attr2 attr3="value3"/>"#;

        assert_eq!(vec![("attr1", "value1")], read_all(xml));

        let xml = r#"<test attr1="value1/>"#;

        assert!(read_all(xml).is_empty());
    }
}
