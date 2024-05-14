use std::fmt::{Display, Formatter};
use thiserror::Error;
use xml::attribute::OwnedAttribute;
use xml::common::{Position, TextPosition};
use xml::EventReader;
use xml::name::OwnedName;
use xml::namespace::Namespace;
use xml::reader::XmlEvent;
use crate::gen_scrape_from_file;
use crate::link_scraper::find_urls;

#[derive(Error, Debug)]
pub enum XmlScrapingError {
    #[error(transparent)]
    XmlReaderError(#[from] xml::reader::Error)
}

#[cfg(feature = "xlink")]
pub mod xlink;
pub mod svg;

#[derive(Debug, Clone)]
pub enum XmlLinkType {
    Attribute(OwnedAttribute),
    Comment,
    PlainText(ParentInformation),
    CData(ParentInformation),
    NameSpace(String),
}

#[derive(Debug, Clone)]
pub struct ParentInformation {
    pub parent_tag_name: Option<OwnedName>,
}

#[derive(Debug, Clone)]
pub struct XmlLink {
    pub url: String,
    pub location: TextPosition,
    pub kind: XmlLinkType
}

impl Display for XmlLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}

pub struct XmlStartElement<'a> {
    name: &'a OwnedName,
    attributes: &'a Vec<OwnedAttribute>,
    _namespace: &'a Namespace
}

#[derive(Debug, Clone)]
struct NamespaceOccurrence {
    namespace: String,
    namespace_uri: String,
    first_occurrence: TextPosition
}

impl PartialEq for NamespaceOccurrence {
    fn eq(&self, other: &Self) -> bool {
        self.namespace_uri == other.namespace_uri && self.namespace == other.namespace
    }
}

/// Scrapes all links from href-attributes regardless of their namespace or tag-name
pub fn scrape_from_href_tags(bytes: &[u8]) -> Result<Vec<XmlLink>, XmlScrapingError> {
    let mut collector: Vec<XmlLink> = vec![];

    let mut parser = EventReader::new(bytes);
    while let Ok(xml_event) = &parser.next() {
        match xml_event {
            XmlEvent::StartElement { name: _name, attributes, namespace: _namespace } => {
                let mut list: Vec<XmlLink> = scrape_from_xml_start_element_attributes(attributes, &parser)?
                    .into_iter()
                    .filter(|link| {
                    if let XmlLinkType::Attribute(att) = &link.kind {
                        if att.name.local_name == "href" {
                            return true
                        }
                    }
                    return false
                }).collect();
                collector.append(&mut list)
            }
            XmlEvent::EndDocument => break,
            _ => {}
        }
    }

    Ok(collector)
}

/// Scrapes links from any file with a xml-schema
pub fn scrape(bytes: &[u8]) -> Result<Vec<XmlLink>, XmlScrapingError> {
    let mut collector: Vec<XmlLink> = vec![];
    let mut namespaces: Vec<NamespaceOccurrence> = vec![];

    let mut current_parent: Option<OwnedName> = None;
    let mut parser = EventReader::new(bytes);
    while let Ok(xml_event) = &parser.next() {
        match xml_event {
            XmlEvent::StartElement { name, attributes, namespace } => {
                namespace.0.iter().for_each(|(ns_name, ns_ref)| {
                    let ns_occurence = NamespaceOccurrence {
                        namespace: ns_name.to_string(), namespace_uri: ns_ref.to_string(), first_occurrence: parser.position() };
                    if !&namespaces.contains(&ns_occurence) { namespaces.push(ns_occurence); }
                });
                current_parent = Some(name.clone());
                collector.append(&mut scrape_from_xml_start_element_attributes(&attributes, &parser)?)
            }
            XmlEvent::Comment(comment) => {
                collector.append(&mut find_urls(comment)
                    .iter()
                    .map(|link| XmlLink {
                        url: link.as_str().to_string(),
                        location: parser.position(),
                        kind: XmlLinkType::Comment,
                    })
                    .collect()
                )
            }
            XmlEvent::Characters(chars) => {
                collector.append(&mut find_urls(chars)
                    .iter()
                    .map(|link| XmlLink {
                        url: link.as_str().to_string(),
                        location: parser.position(),
                        kind: XmlLinkType::PlainText(ParentInformation {
                            parent_tag_name: current_parent.clone()
                        }),
                    })
                    .collect()
                )
            }
            XmlEvent::CData(chars) => {
                collector.append(&mut find_urls(chars)
                    .iter()
                    .map(|link| XmlLink {
                        url: link.as_str().to_string(),
                        location: parser.position(),
                        kind: XmlLinkType::CData(ParentInformation {
                            parent_tag_name: current_parent.clone()
                        }),
                    })
                    .collect()
                )
            }
            XmlEvent::EndDocument => {break}
            _ => {}
        }
    }

    namespaces.into_iter().for_each(|NamespaceOccurrence{namespace, namespace_uri, first_occurrence }| {
        if find_urls(&namespace_uri).len() == 0 { return }

        collector.push(XmlLink {
            url: namespace_uri,
            location: first_occurrence,
            kind: XmlLinkType::NameSpace(namespace)
        })
    });
    
    Ok(collector)
}
gen_scrape_from_file!(Result<Vec<XmlLink>, XmlScrapingError>);

fn scrape_from_xml_start_element_attributes(attributes: &Vec<OwnedAttribute>, parser: &EventReader<&[u8]>) -> Result<Vec<XmlLink>, XmlScrapingError> {
    let mut ret: Vec<XmlLink> = vec![];
    for attribute in attributes {
        let mut links = find_urls(&attribute.value)
            .iter().map(|link| XmlLink {
            url: link.as_str().to_string(),
            location: parser.position(),
            kind: XmlLinkType::Attribute(attribute.clone()),
        }).collect();

        ret.append(&mut links);
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_XLINK: &[u8] = include_bytes!("../../../test_files/xml/xlink_test.xml");

    #[test]
    fn scrape_hrefs_test() {
        let links = scrape_from_href_tags(TEST_XLINK).unwrap();
        println!("{:?}", links);
        assert_eq!(1, links.len())
    }

    #[test]
    fn scrape_all_test() {
        let links = scrape(TEST_XLINK).unwrap();
        println!("{:?}", links);
        assert_eq!(6, links.len())
    }
}
