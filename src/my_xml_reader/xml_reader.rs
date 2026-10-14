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
        let paths = x_path.split('/');

        let mut result: Option<XmlTagInfo<'t>> = None;

        for node_name in paths {
            loop {
                {
                    let node = self.read_next_tag()?;

                    if node.is_none() {
                        break;
                    }

                    let node = node.unwrap();

                    match node.tag_type {
                        XmlTagType::Open => {
                            if node_name == node.name {
                                result = Some(node);
                                break;
                            }
                        }
                        XmlTagType::OpenClose => {
                            if node_name == node.name {
                                result = Some(node);
                                break;
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
        }

        return Ok(result);
    }

    pub fn find_any_of_these_nodes_inside_parent(
        &mut self,
        parent_tag: &XmlTagInfo<'t>,
        node_names: &[&str],
    ) -> Result<Option<XmlTagInfo<'t>>, String> {
        loop {
            {
                let node = self.read_next_tag()?;

                if node.is_none() {
                    return Ok(None);
                }

                let node = node.unwrap();

                match node.tag_type {
                    XmlTagType::Open => {
                        for node_name in node_names {
                            if *node_name == node.name {
                                return Ok(Some(node));
                            }
                        }
                    }
                    XmlTagType::OpenClose => {
                        for node_name in node_names {
                            if *node_name == node.name {
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
    }

    pub fn get_unread_slice(&self) -> &'t str {
        std::str::from_utf8(&self.xml[self.current_pos..]).unwrap()
    }

    pub fn find_the_open_node(&mut self, x_path: &str) -> Result<Option<XmlTagInfo<'t>>, String> {
        let mut segments = x_path.split('/').peekable();
        let mut current_parent: Option<XmlTagInfo<'t>> = None;

        while let Some(segment) = segments.next() {
            let is_last = segments.peek().is_none();

            let found = if let Some(parent) = current_parent.as_ref() {
                self.find_the_node_inside_parent(parent, segment)?
            } else {
                // No parent yet: scan forward until we find the desired tag.
                loop {
                    let node = self.read_next_tag()?;

                    if node.is_none() {
                        break None;
                    }

                    let node = node.unwrap();

                    match node.tag_type {
                        XmlTagType::Open | XmlTagType::OpenClose if node.name == segment => {
                            break Some(node)
                        }
                        _ => {}
                    }
                }
            };

            let Some(found_node) = found else {
                return Ok(None);
            };

            if is_last {
                return Ok(Some(found_node));
            }

            if matches!(found_node.tag_type, XmlTagType::OpenClose) {
                // Self-closing node cannot contain deeper segments
                return Ok(None);
            }

            current_parent = Some(found_node);
        }

        Ok(None)
    }

    fn move_to_the_next_open_tag_pos(&mut self) -> Result<bool, String> {
        if self.current_pos == self.xml.len() {
            return Ok(false);
        }

        if self.xml[self.current_pos] == b'<' {
            return Ok(true);
        }

        let next_pos = super::xml_utils::find_next_token(&self.xml, b'<', self.current_pos);

        match next_pos {
            Some(pos) => {
                self.current_pos = pos;
                Ok(true)
            }
            None => Err(format!(
                "Can not find the next open tag positin from pos {}",
                self.current_pos
            )),
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

        return Ok(Some(tag_info));
    }

    fn find_corelated_closed_node(
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

            self.current_pos = next_node.end_pos + 1;

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

        let close_node = self.find_corelated_closed_node(open_node.level, &open_node.name)?;

        let reuslt = MyXmlNode {
            xml: self.xml,
            open_node,
            close_node: Some(close_node),
        };

        return Ok(reuslt);
    }

    fn scan_for_the_next_tag(&mut self) -> Result<Option<XmlTagInfo<'t>>, String> {
        if !self.move_to_the_next_open_tag_pos()? {
            return Ok(None);
        }

        let start_pos = self.current_pos;
        let end_of_open_tag_pos = super::xml_utils::find_next_token(
            self.xml,
            super::xml_utils::CLOSE_TAG_TOKEN,
            self.current_pos,
        );

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
            result.level -= 1;
        }

        Ok(Some(result))
    }

    pub fn get_level(&self) -> usize {
        return self.opened_nodes.len();
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

        assert_eq!(true, found.is_none());
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

        assert_eq!(true, el_item.is_none());
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

        assert_eq!(true, found_s1);
        assert_eq!(true, found_s2);
        assert_eq!("</R>", reader.get_unread_slice())
    }
}
