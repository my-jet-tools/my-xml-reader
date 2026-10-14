# my-xml-reader

Small, forward-only XML helper aimed at lightweight parsing in constrained contexts. It walks a byte slice, exposes tags as `XmlTagInfo`, and lets you pull full nodes as `MyXmlNode` without allocating an entire DOM.

## Features
- Iterate through tags with a tiny stateful `MyXmlReader`.
- Read whole nodes (including nested content) and extract inner text via `MyXmlNode`.
- XPath-lite helpers: `find_the_open_node`, `find_the_node_inside_parent`, and `find_any_of_these_nodes_inside_parent`.
- Attribute iteration with `AttributesIterator`, exposing key/value pairs without extra copies.
- Decoding of the common entities (`&quot;`, `&apos;`, `&lt;`, `&gt;`, `&amp;`) and character references (`&#10;`, `&#xA;`).
- Comments, processing instructions and `DOCTYPE` are skipped; CDATA sections are read as they are.
- Handles XML headers and UTF-8 BOM at the start of the buffer.

## Getting started
Add the crate to your `Cargo.toml` (use the path form if you're working in this repo):
```toml
[dependencies]
my-xml-reader = { path = "../my-xml-reader" }
```

## Quick example
```rust
use my_xml_reader::MyXmlReader;

fn main() -> Result<(), String> {
    let xml = r#"<?xml version="1.0"?>
        <!-- Generated -->
        <R>
          <A>
            <S1><I id="1">V1</I><I id="2">V2</I></S1>
            <S2><![CDATA[V3 & <more>]]></S2>
          </A>
        </R>"#;

    let mut reader = MyXmlReader::from_slice(xml.as_bytes())?;

    // Navigate by segments (very small XPath-like helper).
    let parent = reader.find_the_open_node("R/A")?.unwrap();

    // Pull child nodes by name, streaming forward.
    while let Some(node) = reader.find_any_of_these_nodes_inside_parent(&parent, &["S1", "S2"])? {
        match node.name {
            "S1" => {
                let full = reader.read_the_whole_node(node)?;
                assert_eq!(r#"<S1><I id="1">V1</I><I id="2">V2</I></S1>"#, full.get_xml());
            }
            "S2" => {
                let val = reader.read_the_whole_node(node)?.get_value().unwrap();
                assert_eq!("V3 & <more>", val);
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Paths
- `find_the_open_node("A/B/C")` scans forward for the first `C` whose parent is `B` and whose grandparent is `A`. Each `/` means a direct child; the first segment (`A`) can be at any depth.
- `find_the_node_inside_parent(&parent, "B/C")` does the same, but only inside `parent`. It returns `None` once the closing tag of `parent` is reached.
- A single name (`"C"`) matches a node at any depth: inside a parent it is the first `C` among all its descendants, not only among its children.
- `find_any_of_these_nodes_inside_parent` accepts the same paths and returns the first node matching any of them.

## Working with attributes
```rust
use my_xml_reader::MyXmlReader;

fn main() -> Result<(), String> {
    let xml = r#"<Item id="42" name='Widget' enabled = "true"/>"#;
    let mut reader = MyXmlReader::from_slice(xml.as_bytes())?;
    let tag = reader.read_next_tag()?.unwrap(); // first tag is <Item .../>

    for (k, v) in tag.iterate_attributes() {
        println!("{k} = {v}");
    }

    Ok(())
}
```
Values are returned exactly as they are in the source, so escapes such as `&amp;` are not decoded. Use `my_xml_reader::my_xml_reader::xml_utils::decode_xml_string` when you need the decoded value.

## Key types
All of them are exported from the crate root.
- `MyXmlReader<'t>`: streaming cursor over the input slice; maintains nesting with `opened_nodes`.
- `XmlTagInfo<'t>`: view of a single tag (`name`, `raw`, `tag_type`, `level`, positions); can iterate attributes.
- `MyXmlNode<'t>`: represents an open/close pair; provides `get_xml()`, `get_inner_content()`, and `get_value()` (decodes escapes and CDATA, drops comments).
- `AttributesIterator<'t>`: zero-copy attribute iterator over a tag's raw bytes; implements `Iterator`.
- `XmlTagType`: enum of `Open`, `Close`, `OpenClose`.

## Behavioral notes
- Forward-only: the reader consumes input as you call `read_next_tag`/find functions; it does not rewind.
- The input must be UTF-8 (`from_slice` returns an error otherwise). Convert other encodings, e.g. `windows-1251`, before parsing.
- Only a UTF-8 BOM and whitespaces are allowed before the XML header (or the first tag), and only whitespaces after the last tag or comment. Any other text there is an error.
- Basic XML coverage: no namespace handling (prefixes are part of the name), no validation, entities declared in `DOCTYPE` are not expanded. Input should be well-formed for best results.
- Errors are returned as `String`; no custom error type yet.

## Testing
Run the built-in suite (the examples in this README are compiled and run as doc tests):
```sh
cargo test
```
