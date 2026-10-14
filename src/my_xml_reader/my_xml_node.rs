use super::XmlTagInfo;

#[derive(Debug, Clone)]
pub struct MyXmlNode<'t> {
    pub xml: &'t [u8],
    pub open_node: XmlTagInfo<'t>,
    pub close_node: Option<XmlTagInfo<'t>>,
}

impl<'t> MyXmlNode<'t> {
    pub fn get_node_name(&self) -> &'t str {
        self.open_node.name
    }

    pub fn get_inner_content(&self) -> Option<&'t [u8]> {
        self.close_node
            .as_ref()
            .map(|close_node| &self.xml[self.open_node.end_pos + 1..close_node.start_pos])
    }

    /// Inner content with decoded escapes and CDATA sections, without comments.
    /// None for a self-closing node.
    pub fn get_value(&self) -> Option<String> {
        let inner_content = self.get_inner_content()?;

        let value = std::str::from_utf8(inner_content).unwrap();

        Some(super::xml_utils::decode_xml_value(value))
    }

    pub fn get_xml(&self) -> &'t str {
        let xml = match &self.close_node {
            Some(close_node) => &self.xml[self.open_node.start_pos..close_node.end_pos + 1],
            None => &self.xml[self.open_node.start_pos..self.open_node.end_pos + 1],
        };

        std::str::from_utf8(xml).unwrap()
    }
}
