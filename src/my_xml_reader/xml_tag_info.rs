use crate::my_xml_reader::AttributesIterator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlTagType {
    Open = 0,
    Close = 1,
    OpenClose = 2,
}

#[derive(Debug, Clone)]
pub struct XmlTagInfo<'t> {
    pub name: &'t str,
    pub raw: &'t [u8],
    pub tag_type: XmlTagType,
    pub start_pos: usize,
    pub end_pos: usize,
    pub level: usize,
}

impl<'t> XmlTagInfo<'t> {
    pub fn raw_as_string(&self) -> &'t str {
        std::str::from_utf8(self.raw).unwrap()
    }

    pub fn iterate_attributes(&self) -> AttributesIterator<'t> {
        AttributesIterator::new(self.raw)
    }
}
