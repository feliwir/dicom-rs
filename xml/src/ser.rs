//! DICOM XML serialization module

use std::io::Write;

use dicom_core::dictionary::{DataDictionary, DataDictionaryEntry};
use dicom_core::header::Header;
use dicom_core::{DicomValue, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::StandardDataDictionary;
use dicom_object::mem::InMemElement;
use dicom_object::{DefaultDicomObject, InMemDicomObject};
use serde::{Serialize, Serializer};

use crate::DicomXml;
use crate::error::Result;
use crate::model::{
    DicomAttribute, Item, NativeDicomModel, PersonName, PersonNameComponents, TextValue,
};

/// Serialize a piece of DICOM data as a string of DICOM XML.
pub(crate) fn to_string<T>(data: T) -> Result<String>
where
    T: Serialize,
{
    Ok(quick_xml::se::to_string(&data)?)
}

/// Serialize a piece of DICOM data as a pretty-printed string of DICOM XML.
pub(crate) fn to_string_pretty<T>(data: T) -> Result<String>
where
    T: Serialize,
{
    let mut buffer = String::new();
    let mut ser = quick_xml::se::Serializer::new(&mut buffer);
    ser.indent(' ', 2);
    data.serialize(ser)?;
    Ok(buffer)
}

/// Serialize a piece of DICOM data to a byte writer.
pub(crate) fn to_writer<W, T>(writer: W, data: T) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    quick_xml::se::to_utf8_io_writer(writer, &data)?;
    Ok(())
}

impl<D> Serialize for DicomXml<&InMemDicomObject<D>> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NativeDicomModel::from(*self.inner()).serialize(serializer)
    }
}

impl<D> Serialize for DicomXml<InMemDicomObject<D>> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NativeDicomModel::from(self.inner()).serialize(serializer)
    }
}

impl<D> Serialize for DicomXml<&DefaultDicomObject<D>> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NativeDicomModel::from(*self.inner()).serialize(serializer)
    }
}

impl<D> Serialize for DicomXml<DefaultDicomObject<D>> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        NativeDicomModel::from(self.inner()).serialize(serializer)
    }
}

fn tag_to_hex(tag: Tag) -> String {
    format!("{:04X}{:04X}", tag.0, tag.1)
}

fn keyword_of(tag: Tag) -> Option<String> {
    StandardDataDictionary
        .by_tag(tag)
        .map(|entry| entry.alias().to_string())
}

/// Split a raw DICOM PN value into its (at most three) group representations,
/// each one split further into its (at most five) name components.
fn person_name_components(raw: &str) -> Option<PersonNameComponents> {
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.splitn(5, '^');
    let components = PersonNameComponents {
        family_name: parts.next().filter(|s| !s.is_empty()).map(String::from),
        given_name: parts.next().filter(|s| !s.is_empty()).map(String::from),
        middle_name: parts.next().filter(|s| !s.is_empty()).map(String::from),
        name_prefix: parts.next().filter(|s| !s.is_empty()).map(String::from),
        name_suffix: parts.next().filter(|s| !s.is_empty()).map(String::from),
    };
    if components.is_empty() {
        None
    } else {
        Some(components)
    }
}

fn build_person_names(value: &PrimitiveValue) -> Vec<PersonName> {
    value
        .to_multi_str()
        .iter()
        .enumerate()
        .map(|(i, raw)| {
            let mut groups = raw.splitn(3, '=');
            PersonName {
                number: i as u32 + 1,
                alphabetic: groups.next().and_then(person_name_components),
                ideographic: groups.next().and_then(person_name_components),
                phonetic: groups.next().and_then(person_name_components),
            }
        })
        .collect()
}

fn build_text_values(value: &PrimitiveValue) -> Vec<TextValue> {
    value
        .to_multi_str()
        .iter()
        .enumerate()
        .map(|(i, text)| TextValue {
            number: i as u32 + 1,
            text: text.clone(),
        })
        .collect()
}

fn build_tag_values(value: &PrimitiveValue) -> Vec<TextValue> {
    match value {
        PrimitiveValue::Tags(tags) => tags
            .iter()
            .enumerate()
            .map(|(i, tag)| TextValue {
                number: i as u32 + 1,
                text: tag_to_hex(*tag),
            })
            .collect(),
        _ => build_text_values(value),
    }
}

fn build_inline_binary(value: &PrimitiveValue) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&*value.to_bytes())
}

/// Build the XML representation of a single data element.
fn build_attribute<D>(e: &InMemElement<D>) -> Result<DicomAttribute> {
    let tag = e.tag();
    let vr = e.vr();
    let mut attribute = DicomAttribute {
        tag: tag_to_hex(tag),
        vr: vr.to_string().to_owned(),
        keyword: keyword_of(tag),
        ..Default::default()
    };

    match e.value() {
        DicomValue::Sequence(seq) => {
            attribute.items = seq
                .items()
                .iter()
                .enumerate()
                .map(|(i, item)| build_item(i as u32 + 1, item))
                .collect::<Result<_>>()?;
        }
        DicomValue::PixelSequence(_) => {
            tracing::warn!(
                "encapsulated pixel data is not supported in DICOM XML; skipping tag {}",
                tag
            );
        }
        DicomValue::Primitive(PrimitiveValue::Empty) => {
            // no-op: attribute has no content
        }
        DicomValue::Primitive(v) => match vr {
            VR::PN => {
                attribute.person_names = build_person_names(v);
            }
            VR::AT => {
                attribute.values = build_tag_values(v);
            }
            VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN => {
                attribute.inline_binary = Some(build_inline_binary(v));
            }
            VR::SQ => unreachable!("unexpected VR SQ in primitive value"),
            _ => {
                attribute.values = build_text_values(v);
            }
        },
    }

    Ok(attribute)
}

fn build_item<D>(number: u32, obj: &InMemDicomObject<D>) -> Result<Item> {
    let attributes = obj
        .into_iter()
        .map(build_attribute)
        .collect::<Result<_>>()?;
    Ok(Item { number, attributes })
}

impl<D> From<&InMemDicomObject<D>> for NativeDicomModel {
    fn from(value: &InMemDicomObject<D>) -> Self {
        let attributes = value
            .into_iter()
            .map(build_attribute)
            .collect::<Result<Vec<_>>>()
            // errors here can only stem from unreachable branches
            .expect("attribute construction should never fail");
        NativeDicomModel {
            xml_space: Some("preserve".to_string()),
            attributes,
        }
    }
}

impl<D> From<InMemDicomObject<D>> for NativeDicomModel {
    fn from(value: InMemDicomObject<D>) -> Self {
        NativeDicomModel::from(&value)
    }
}

impl<D> From<&DefaultDicomObject<D>> for NativeDicomModel {
    fn from(value: &DefaultDicomObject<D>) -> Self {
        let mut attributes: Vec<DicomAttribute> = value
            .meta()
            .to_element_iter()
            .filter_map(|e| {
                let DicomValue::Primitive(v) = e.value() else {
                    return None;
                };
                let tag = e.tag();
                let vr = e.vr();
                Some(DicomAttribute {
                    tag: tag_to_hex(tag),
                    vr: vr.to_string().to_owned(),
                    keyword: keyword_of(tag),
                    values: if matches!(
                        vr,
                        VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN
                    ) {
                        Vec::new()
                    } else {
                        build_text_values(v)
                    },
                    inline_binary: if matches!(
                        vr,
                        VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN
                    ) {
                        Some(build_inline_binary(v))
                    } else {
                        None
                    },
                    ..Default::default()
                })
            })
            .collect();

        let inner: &InMemDicomObject<D> = value;
        let mut model = NativeDicomModel::from(inner);
        attributes.append(&mut model.attributes);
        model.attributes = attributes;
        model
    }
}

impl<D> From<DefaultDicomObject<D>> for NativeDicomModel {
    fn from(value: DefaultDicomObject<D>) -> Self {
        NativeDicomModel::from(&value)
    }
}

#[cfg(test)]
mod tests {
    use dicom_core::{PrimitiveValue, Tag, VR, dicom_value};
    use dicom_dictionary_std::tags;
    use dicom_object::mem::InMemElement;

    use super::*;

    #[test]
    fn serialize_simple_data_elements() {
        let all_data = vec![
            InMemElement::new(
                Tag(0x0008, 0x0005),
                VR::CS,
                PrimitiveValue::from("ISO_IR 192"),
            ),
            InMemElement::new(
                Tag(0x0008, 0x0061),
                VR::CS,
                dicom_value!(Strs, ["CT", "PET"]),
            ),
            InMemElement::new(
                Tag(0x0008, 0x0090),
                VR::PN,
                PrimitiveValue::from("Bob^^^^Dr."),
            ),
            InMemElement::new(tags::PATIENT_AGE, VR::AS, PrimitiveValue::from("30Y")),
        ];

        let obj = InMemDicomObject::from_element_iter(all_data);
        let xml = crate::to_string(&obj).unwrap();

        assert!(xml.contains(r#"tag="00080005""#));
        assert!(xml.contains(r#"vr="CS""#));
        assert!(xml.contains("ISO_IR 192"));
        assert!(xml.contains("<Value number=\"1\">CT</Value>"));
        assert!(xml.contains("<Value number=\"2\">PET</Value>"));
        assert!(xml.contains("<FamilyName>Bob</FamilyName>"));
        assert!(xml.contains("<NameSuffix>Dr.</NameSuffix>"));
    }

    #[test]
    fn round_trip_sequence_and_binary() {
        let inner = InMemDicomObject::from_element_iter([InMemElement::new(
            Tag(0x0018, 0x9302),
            VR::CS,
            PrimitiveValue::from("STATIC"),
        )]);

        let obj = InMemDicomObject::from_element_iter([
            InMemElement::new(
                tags::SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
                VR::SQ,
                dicom_core::value::DataSetSequence::from(vec![inner]),
            ),
            InMemElement::new(
                Tag(0x0009, 0x1002),
                VR::UN,
                dicom_value!(U8, [0xcf, 0x4c, 0x7d, 0x73]),
            ),
            InMemElement::new(
                Tag(0x0020, 0x0032),
                VR::DS,
                dicom_value!(Strs, ["1.5", "2.5", "3.5"]),
            ),
        ]);

        let xml = crate::to_string(&obj).unwrap();
        let round_tripped: InMemDicomObject = crate::from_str(&xml).unwrap();

        // Note: whole-object equality is not used here because
        // sequences have an undefined length (`Length::UNDEFINED`),
        // and by design it never compares equal to itself.
        for elem in &obj {
            let other = round_tripped.element(elem.tag()).unwrap();
            assert_eq!(elem.vr(), other.vr());
            assert_eq!(elem.value(), other.value());
        }
    }

    #[test]
    fn write_full_file_to_xml() {
        let sc_rgb_rle = dicom_test_files::path("pydicom/SC_rgb_rle.dcm").unwrap();

        let obj = dicom_object::OpenFileOptions::new()
            .read_until(Tag(0x0010, 0))
            .open_file(sc_rgb_rle)
            .expect("Failed to open test file");

        let xml = crate::to_string(&obj).unwrap();

        assert!(xml.contains(r#"<NativeDicomModel xml:space="preserve">"#));
        assert!(xml.contains(r#"tag="00020010""#));
        assert!(xml.contains("1.2.840.10008.1.2.5"));
        assert!(xml.contains(r#"tag="00080090""#));
        assert!(xml.contains("<FamilyName>Moriarty</FamilyName>"));
    }
}
