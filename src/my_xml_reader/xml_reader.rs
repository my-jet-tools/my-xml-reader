use super::XmlTagType;
use super::{MyXmlNode, XmlTagInfo};

#[derive(Debug)]
pub struct OpenedNode {
    pub name: String,
    pub start_pos: usize,
    pub end_pos: usize,
}

pub struct MyXmlReader<'t> {
    pub xml: &'t [u8],
    current_pos: usize,
    pub opened_nodes: Vec<OpenedNode>,
}

impl<'t> MyXmlReader<'t> {
    pub fn from_slice(xml: &'t [u8]) -> Result<Self, String> {
        let current_pos = super::xml_utils::init_pos_start(xml)?;
        Ok(Self {
            xml,
            current_pos,
            opened_nodes: vec![],
        })
    }

    pub fn find_the_node_inside_parent(
        &mut self,
        parent_tag: &XmlTagInfo<'t>,
        x_path: &str,
    ) -> Result<Option<XmlTagInfo<'t>>, String> {
        self.find_any_of_these_nodes_inside_parent(parent_tag, &[x_path])
    }

    pub fn find_any_of_these_nodes_inside_parent(
        &mut self,
        parent_tag: &XmlTagInfo<'t>,
        node_names: &[&str],
    ) -> Result<Option<XmlTagInfo<'t>>, String> {
        if matches!(parent_tag.tag_type, XmlTagType::OpenClose) {
            // Self-closing node has no children
            return Ok(None);
        }

        loop {
            let node = self.read_next_tag()?;

            if node.is_none() {
                return Ok(None);
            }

            let node = node.unwrap();

            match node.tag_type {
                XmlTagType::Open | XmlTagType::OpenClose => {
                    for x_path in node_names {
                        if self.is_path_matched(&node, x_path, parent_tag.level + 1) {
                            return Ok(Some(node));
                        }
                    }
                }
                XmlTagType::Close => {
                    if parent_tag.level == node.level && parent_tag.name == node.name {
                        return Ok(None);
                    }
                }
            }
        }
    }

    pub fn get_unread_slice(&self) -> &'t str {
        std::str::from_utf8(&self.xml[self.current_pos..]).unwrap()
    }

    pub fn find_the_open_node(&mut self, x_path: &str) -> Result<Option<XmlTagInfo<'t>>, String> {
        loop {
            let node = self.read_next_tag()?;

            if node.is_none() {
                return Ok(None);
            }

            let node = node.unwrap();

            match node.tag_type {
                XmlTagType::Open | XmlTagType::OpenClose => {
                    if self.is_path_matched(&node, x_path, 0) {
                        return Ok(Some(node));
                    }
                }
                XmlTagType::Close => {}
            }
        }
    }

    // The last segment of x_path is the node itself, every previous segment must be
    // the direct parent of the next one. The first segment can be at any level >= min_level
    fn is_path_matched(&self, node: &XmlTagInfo<'t>, x_path: &str, min_level: usize) -> bool {
        let mut segments = x_path.rsplit('/');

        if node.level < min_level || segments.next() != Some(node.name) {
            return false;
        }

        let mut level = node.level;

        for segment in segments {
            if level == min_level {
                return false;
            }

            level -= 1;

            if self.opened_nodes[level].name != segment {
                return false;
            }
        }

        true
    }

    fn move_to_the_next_open_tag_pos(&mut self) -> Result<bool, String> {
        if self.current_pos == self.xml.len() {
            return Ok(false);
        }

        if self.xml[self.current_pos] == b'<' {
            return Ok(true);
        }

        let next_pos = super::xml_utils::find_next_token(self.xml, b'<', self.current_pos);

        match next_pos {
            Some(pos) => {
                self.current_pos = pos;
                Ok(true)
            }
            None => {
                // Whitespaces after the last tag (e.g. a trailing new line) are not an error
                let rest = &self.xml[self.current_pos..];

                if rest.iter().all(|b| super::xml_utils::is_xml_whitespace(*b)) {
                    return Ok(false);
                }

                Err(format!(
                    "Can not find the next open tag position from pos {}",
                    self.current_pos
                ))
            }
        }
    }

    pub fn read_next_tag(&mut self) -> Result<Option<XmlTagInfo<'t>>, String> {
        let tag_info = self.scan_for_the_next_tag()?;

        if tag_info.is_none() {
            return Ok(None);
        }

        let tag_info = tag_info.unwrap();

        self.current_pos = tag_info.end_pos + 1;

        match tag_info.tag_type {
            XmlTagType::Open => {
                self.opened_nodes.push(OpenedNode {
                    name: tag_info.name.to_string(),
                    start_pos: tag_info.start_pos,
                    end_pos: tag_info.end_pos,
                });
            }

            XmlTagType::Close => {
                let last_opened_tag = self.opened_nodes.last();

                if last_opened_tag.is_none() {
                    return Err(format!(
                        "Attempt to close tag with name {}. There are no opened tags",
                        tag_info.name
                    ));
                }

                let last_opened_tag = last_opened_tag.unwrap();

                if last_opened_tag.name != tag_info.name {
                    return Err(format!(
                            "Attempt to close tag with name </{}>. But last opened tag has the name <{}>",
                            tag_info.name, last_opened_tag.name
                        ));
                } else {
                    self.opened_nodes.pop();
                }
            }

            XmlTagType::OpenClose => {}
        }

        Ok(Some(tag_info))
    }

    fn find_correlated_closed_node(
        &mut self,
        node_level: usize,
        node_name: &str,
    ) -> Result<XmlTagInfo<'t>, String> {
        loop {
            let next_node = self.read_next_tag()?;

            if next_node.is_none() {
                return Err(format!(
                    "Can not find the node to close with name: {} and level {}",
                    node_name, node_level
                ));
            }

            let next_node = next_node.unwrap();

            if matches!(next_node.tag_type, XmlTagType::Close)
                && next_node.level == node_level
                && next_node.name == node_name
            {
                return Ok(next_node);
            }
        }
    }

    pub fn read_the_whole_node(
        &mut self,
        open_node: XmlTagInfo<'t>,
    ) -> Result<MyXmlNode<'t>, String> {
        if matches!(open_node.tag_type, XmlTagType::OpenClose) {
            let result = MyXmlNode {
                xml: self.xml,
                open_node,
                close_node: None,
            };

            return Ok(result);
        }

        let close_node = self.find_correlated_closed_node(open_node.level, open_node.name)?;

        let result = MyXmlNode {
            xml: self.xml,
            open_node,
            close_node: Some(close_node),
        };

        Ok(result)
    }

    fn scan_for_the_next_tag(&mut self) -> Result<Option<XmlTagInfo<'t>>, String> {
        if !self.move_to_the_next_open_tag_pos()? {
            return Ok(None);
        }

        let start_pos = self.current_pos;
        let end_of_open_tag_pos = super::xml_utils::find_end_of_tag(self.xml, self.current_pos);

        if end_of_open_tag_pos.is_none() {
            return Err(format!(
                "Can not find the close. Start pos is {}",
                start_pos
            ));
        }

        let end_pos = end_of_open_tag_pos.unwrap();

        let raw = &self.xml[self.current_pos..end_pos + 1];

        let mut tag_type = XmlTagType::Open;

        if raw[raw.len() - 2] == b'/' {
            tag_type = XmlTagType::OpenClose;
        } else if raw[1] == b'/' {
            tag_type = XmlTagType::Close;
        }

        let name = std::str::from_utf8(super::xml_utils::extract_tag_name(raw)).unwrap();

        let mut result = XmlTagInfo {
            name,
            raw,
            tag_type,
            start_pos,
            end_pos,
            level: self.get_level(),
        };

        if matches!(result.tag_type, XmlTagType::Close) {
            // Close tag without opened tags is reported as an error by read_next_tag
            result.level = result.level.saturating_sub(1);
        }

        Ok(Some(result))
    }

    pub fn get_level(&self) -> usize {
        self.opened_nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_open_tag_after_header_node() {
        let xml_src = r#"<?xml version="1.0" encoding="utf-8"?><RootNode></RootNode>"#;

        let xml = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        assert_eq!("<RootNode></RootNode>", &xml_src[xml.current_pos..])
    }
    #[test]
    fn test_find_open_tag_with_no_header_node() {
        let xml_src = r#"<RootNode></RootNode>"#;

        let xml = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        assert_eq!("<RootNode></RootNode>", &xml_src[xml.current_pos..])
    }

    #[test]
    fn test_find_open_tags() {
        let xml_src = r#"<RootNode><SkipTag1/><SkipTag2></SkipTag2><SubRoot><Tag1>Val1</Tag1><Tag1>Val1</Tag1></SubRoot></RootNode>"#;

        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader.find_the_open_node("RootNode").unwrap().unwrap();

        assert_eq!("RootNode", found.name);
        assert_eq!(0, found.level);
        assert_eq!(1, reader.get_level());

        let found = reader.find_the_open_node("SubRoot").unwrap().unwrap();

        println!("{:?}", reader.opened_nodes);
        assert_eq!("SubRoot", found.name);
        assert_eq!(2, reader.get_level());
    }

    #[test]
    fn test_xpath_node_search() {
        let xml_src = r#"<RootNode><SkipTag1/><SkipTag2></SkipTag2><SubRoot><Tag1>Val1</Tag1><Tag1>Val1</Tag1></SubRoot></RootNode>"#;

        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader
            .find_the_open_node("RootNode/SubRoot")
            .unwrap()
            .unwrap();

        assert_eq!("SubRoot", found.name);
        assert_eq!(2, reader.get_level());
    }

    #[test]
    fn test_read_whole_node() {
        let xml_src = r#"<RootNode><SkipTag1/><SkipTag2></SkipTag2><SubRoot><Tag1>Val1</Tag1><Tag1>Val1</Tag1></SubRoot></RootNode>"#;

        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader
            .find_the_open_node("RootNode/SubRoot")
            .unwrap()
            .unwrap();

        assert_eq!("SubRoot", found.name);
        assert_eq!(1, found.level);
        assert_eq!(2, reader.get_level());

        let whole_node = reader.read_the_whole_node(found).unwrap();

        println!("{}", std::str::from_utf8(whole_node.xml).unwrap());
        assert_eq!("SubRoot", whole_node.get_node_name());
        assert_eq!(
            "<Tag1>Val1</Tag1><Tag1>Val1</Tag1>",
            std::str::from_utf8(whole_node.get_inner_content().unwrap()).unwrap()
        );
    }

    #[test]
    fn test_read_array() {
        let xml_src = r#"<RootNode><SkipTag1/><SkipTag2></SkipTag2><SubRoot><Tag1>Val1</Tag1><Tag1>Val2</Tag1></SubRoot></RootNode>"#;

        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let array_node = reader
            .find_the_open_node("RootNode/SubRoot")
            .unwrap()
            .unwrap();

        assert_eq!("SubRoot", array_node.name);
        assert_eq!(1, array_node.level);
        assert_eq!(2, reader.get_level());

        let found = reader
            .find_the_node_inside_parent(&array_node, "Tag1")
            .unwrap();
        let whole_node = reader.read_the_whole_node(found.unwrap()).unwrap();

        assert_eq!("Val1", whole_node.get_value().unwrap());

        assert_eq!("Tag1", whole_node.get_node_name());

        let found = reader
            .find_the_node_inside_parent(&array_node, "Tag1")
            .unwrap();
        let whole_node = reader.read_the_whole_node(found.unwrap()).unwrap();

        assert_eq!("Val2", whole_node.get_value().unwrap());
        assert_eq!("Tag1", whole_node.get_node_name());

        let found = reader
            .find_the_node_inside_parent(&array_node, "Tag1")
            .unwrap();

        assert!(found.is_none());
    }

    #[test]
    fn test_next_tag_after_reading_array() {
        let xml_src = r#"<R><S1><I>V1</I><I>V2</I></S1><S2>V2</S2></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let array_node = reader.find_the_open_node("R/S1").unwrap().unwrap();

        let el_item = reader
            .find_the_node_inside_parent(&array_node, "I")
            .unwrap();

        let whole_node = reader.read_the_whole_node(el_item.unwrap()).unwrap();

        assert_eq!("<I>V1</I>", whole_node.get_xml());

        let el_item = reader
            .find_the_node_inside_parent(&array_node, "I")
            .unwrap();
        let whole_node = reader.read_the_whole_node(el_item.unwrap()).unwrap();

        assert_eq!("<I>V2</I>", whole_node.get_xml());

        let el_item = reader
            .find_the_node_inside_parent(&array_node, "I")
            .unwrap();

        assert!(el_item.is_none());
        assert_eq!("<S2>V2</S2></R>", reader.get_unread_slice())
    }

    #[test]
    fn test_finding_any_of_nodes() {
        let xml_src = r#"<R><A><S1><I>V1</I><I>V2</I></S1><S2>V2</S2></A></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let array_node = reader.find_the_open_node("R/A").unwrap().unwrap();

        assert_eq!(
            "<S1><I>V1</I><I>V2</I></S1><S2>V2</S2></A></R>",
            reader.get_unread_slice()
        );

        let node_names = vec!["S1", "S2"];

        let mut found_s1 = false;
        let mut found_s2 = false;

        loop {
            let el_item = reader
                .find_any_of_these_nodes_inside_parent(&array_node, &node_names)
                .unwrap();

            if el_item.is_none() {
                break;
            }

            let el_item = el_item.unwrap();

            match el_item.name {
                "S1" => {
                    found_s1 = true;
                    reader.read_the_whole_node(el_item).unwrap();
                }
                "S2" => {
                    found_s2 = true;
                    reader.read_the_whole_node(el_item).unwrap();
                }
                _ => {}
            }
        }

        assert!(found_s1);
        assert!(found_s2);
        assert_eq!("</R>", reader.get_unread_slice())
    }

    #[test]
    fn test_trailing_whitespaces_are_not_an_error() {
        let xml_src = "<R><A/></R>\r\n";
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        assert!(reader.find_the_open_node("Missing").unwrap().is_none());
        assert!(reader.read_next_tag().unwrap().is_none());
    }

    #[test]
    fn test_trailing_text_is_an_error() {
        let xml_src = "<R><A/></R>garbage";
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        assert!(reader.find_the_open_node("Missing").is_err());
    }

    #[test]
    fn test_close_tag_without_opened_tags_is_an_error() {
        let xml_src = "<a></a></b>";
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        assert!(reader.read_next_tag().unwrap().is_some());
        assert!(reader.read_next_tag().unwrap().is_some());
        assert!(reader.read_next_tag().is_err());
    }

    #[test]
    fn test_close_tag_token_inside_attribute_value() {
        let xml_src = r#"<R><A f="a>b" g='/>'>Val</A></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader.find_the_open_node("R/A").unwrap().unwrap();

        assert_eq!(r#"<A f="a>b" g='/>'>"#, found.raw_as_string());
        assert!(matches!(found.tag_type, XmlTagType::Open));

        let whole_node = reader.read_the_whole_node(found).unwrap();
        assert_eq!("Val", whole_node.get_value().unwrap());
    }

    #[test]
    fn test_x_path_segments_are_direct_children() {
        let xml_src = r#"<R><A><B><C>Deep</C></B></A><C>Direct</C></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader.find_the_open_node("R/C").unwrap().unwrap();
        let whole_node = reader.read_the_whole_node(found).unwrap();

        assert_eq!("Direct", whole_node.get_value().unwrap());
    }

    #[test]
    fn test_single_name_is_found_at_any_level() {
        let xml_src = r#"<R><A><B><C>Deep</C></B></A><C>Direct</C></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let root = reader.find_the_open_node("R").unwrap().unwrap();
        let found = reader.find_the_node_inside_parent(&root, "C").unwrap();
        let whole_node = reader.read_the_whole_node(found.unwrap()).unwrap();

        assert_eq!("Deep", whole_node.get_value().unwrap());
    }

    #[test]
    fn test_x_path_is_found_in_the_second_branch() {
        let xml_src = r#"<R><A><X/></A><A><B>Val</B></A></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let found = reader.find_the_open_node("R/A/B").unwrap().unwrap();
        let whole_node = reader.read_the_whole_node(found).unwrap();

        assert_eq!("Val", whole_node.get_value().unwrap());
    }

    #[test]
    fn test_x_path_inside_parent() {
        let xml_src =
            r#"<R><P><A></A><B>Wrong</B><A><B>Right</B></A></P><A><B>Outside</B></A></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let parent = reader.find_the_open_node("R/P").unwrap().unwrap();

        let found = reader.find_the_node_inside_parent(&parent, "A/B").unwrap();
        let whole_node = reader.read_the_whole_node(found.unwrap()).unwrap();
        assert_eq!("Right", whole_node.get_value().unwrap());

        let found = reader.find_the_node_inside_parent(&parent, "A/B").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_nothing_is_found_inside_self_closing_parent() {
        let xml_src = r#"<R><P/><Q><X/></Q></R>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let parent = reader.find_the_open_node("R/P").unwrap().unwrap();

        let found = reader.find_the_node_inside_parent(&parent, "X").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_attributes_outlive_the_tag() {
        fn read_attributes<'t>(tag: XmlTagInfo<'t>) -> Vec<(&'t str, &'t str)> {
            tag.iterate_attributes().collect()
        }

        let xml_src = r#"<R a="1" b='2'/>"#;
        let mut reader = MyXmlReader::from_slice(xml_src.as_bytes()).unwrap();

        let tag = reader.read_next_tag().unwrap().unwrap();

        assert_eq!(vec![("a", "1"), ("b", "2")], read_attributes(tag));
    }
}
