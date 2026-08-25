//! DICOM XML deserialization module

use std::io::Read;
use std::str::FromStr;

use dicom_core::dictionary::DataDictionary;
use dicom_core::value::{C, DataSetSequence};
use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_object::InMemDicomObject;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::DicomXml;
use crate::error::{Error, Result};
use crate::model::{DicomAttribute, Item, NativeDicomModel, PersonName, PersonNameComponents};

/// Deserialize a value of type `T` from a string of DICOM XML.
pub(crate) fn from_str<'a, T>(source: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    Ok(quick_xml::de::from_str(source)?)
}

/// Deserialize a value of type `T` from a byte reader.
pub(crate) fn from_reader<R, T>(reader: R) -> Result<T>
where
    R: Read,
    T: DeserializeOwned,
{
    Ok(quick_xml::de::from_reader(std::io::BufReader::new(reader))?)
}

impl<'de, D> Deserialize<'de> for DicomXml<InMemDicomObject<D>>
where
    D: Default + Clone + DataDictionary,
{
    fn deserialize<De>(deserializer: De) -> std::result::Result<Self, De::Error>
    where
        De: serde::Deserializer<'de>,
    {
        let model = NativeDicomModel::deserialize(deserializer)?;
        let obj = model_to_object(&model).map_err(serde::de::Error::custom)?;
        Ok(DicomXml::from(obj))
    }
}

fn model_to_object<D>(model: &NativeDicomModel) -> Result<InMemDicomObject<D>>
where
    D: Default + Clone + DataDictionary,
{
    let mut obj = InMemDicomObject::<D>::new_empty_with_dict(D::default());
    for attr in &model.attributes {
        if let Some((tag, vr, value)) = build_element(attr)? {
            obj.put(DataElement::new(tag, vr, value));
        }
    }
    Ok(obj)
}

fn parse_tag(text: &str) -> Result<Tag> {
    Tag::from_str(text).map_err(|_| Error::InvalidTag {
        text: text.to_string(),
    })
}

fn parse_values<T>(tag: Tag, attr: &DicomAttribute) -> Result<C<T>>
where
    T: FromStr,
{
    attr.values
        .iter()
        .map(|v| {
            v.text.parse::<T>().map_err(|_| Error::InvalidValue {
                tag,
                text: v.text.clone(),
            })
        })
        .collect()
}

fn parse_texts(attr: &DicomAttribute) -> C<String> {
    attr.values.iter().map(|v| v.text.clone()).collect()
}

fn format_components(components: &Option<PersonNameComponents>) -> String {
    let Some(c) = components else {
        return String::new();
    };
    let parts = [
        c.family_name.as_deref().unwrap_or(""),
        c.given_name.as_deref().unwrap_or(""),
        c.middle_name.as_deref().unwrap_or(""),
        c.name_prefix.as_deref().unwrap_or(""),
        c.name_suffix.as_deref().unwrap_or(""),
    ];
    let mut end = parts.len();
    while end > 0 && parts[end - 1].is_empty() {
        end -= 1;
    }
    parts[..end].join("^")
}

fn format_person_name(pn: &PersonName) -> String {
    let groups = [
        format_components(&pn.alphabetic),
        format_components(&pn.ideographic),
        format_components(&pn.phonetic),
    ];
    let mut end = groups.len();
    while end > 0 && groups[end - 1].is_empty() {
        end -= 1;
    }
    groups[..end].join("=")
}

fn build_item<D>(item: &Item) -> Result<InMemDicomObject<D>>
where
    D: Default + Clone + DataDictionary,
{
    let mut obj = InMemDicomObject::<D>::new_empty_with_dict(D::default());
    for attr in &item.attributes {
        if let Some((tag, vr, value)) = build_element(attr)? {
            obj.put(DataElement::new(tag, vr, value));
        }
    }
    Ok(obj)
}

/// Build a `(Tag, VR, DicomValue)` triple from an XML attribute description,
/// or `None` if the attribute only references bulk data
/// (which is not supported for in-memory objects).
#[allow(clippy::type_complexity)]
fn build_element<D>(
    attr: &DicomAttribute,
) -> Result<
    Option<(
        Tag,
        VR,
        dicom_core::value::Value<InMemDicomObject<D>, dicom_core::value::InMemFragment>,
    )>,
>
where
    D: Default + Clone + DataDictionary,
{
    let tag = parse_tag(&attr.tag)?;
    let vr = VR::from_str(&attr.vr).map_err(|_| Error::InvalidVr {
        tag,
        text: attr.vr.clone(),
    })?;

    if attr.bulk_data.is_some() {
        tracing::warn!(
            "bulk data URI is not supported for in-memory objects; skipping tag {}",
            tag
        );
        return Ok(None);
    }

    let value = match vr {
        VR::SQ => {
            let items = attr
                .items
                .iter()
                .map(build_item::<D>)
                .collect::<Result<Vec<_>>>()?;
            dicom_core::value::Value::Sequence(DataSetSequence::from(items))
        }
        VR::PN => {
            let names: C<String> = attr.person_names.iter().map(format_person_name).collect();
            PrimitiveValue::Strs(names).into()
        }
        VR::AT => {
            let tags: C<Tag> = attr
                .values
                .iter()
                .map(|v| parse_tag(&v.text))
                .collect::<Result<_>>()?;
            PrimitiveValue::Tags(tags).into()
        }
        VR::SS => PrimitiveValue::I16(parse_values(tag, attr)?).into(),
        VR::US => PrimitiveValue::U16(parse_values(tag, attr)?).into(),
        VR::SL => PrimitiveValue::I32(parse_values(tag, attr)?).into(),
        VR::UL => PrimitiveValue::U32(parse_values(tag, attr)?).into(),
        VR::SV => PrimitiveValue::I64(parse_values(tag, attr)?).into(),
        VR::UV => PrimitiveValue::U64(parse_values(tag, attr)?).into(),
        VR::FL => PrimitiveValue::F32(parse_values(tag, attr)?).into(),
        VR::FD => PrimitiveValue::F64(parse_values(tag, attr)?).into(),
        VR::OB | VR::OD | VR::OF | VR::OL | VR::OV | VR::OW | VR::UN => {
            let Some(inline_binary) = &attr.inline_binary else {
                return Ok(Some((tag, vr, PrimitiveValue::Empty.into())));
            };
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD
                .decode(inline_binary)
                .map_err(|_| Error::InvalidBase64 { tag })?;
            PrimitiveValue::from(data).into()
        }
        // remaining VRs are always textual (AE, AS, CS, DA, DS, DT, IS, LO, LT,
        // SH, ST, TM, UC, UI, UR, UT)
        _ => PrimitiveValue::Strs(parse_texts(attr)).into(),
    };

    Ok(Some((tag, vr, value)))
}

#[cfg(test)]
mod tests {
    use dicom_core::{Tag, VR};
    use dicom_object::StandardDataDictionary;

    use super::*;

    #[test]
    fn can_parse_simple_data_sets() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<NativeDicomModel xml:space="preserve">
    <DicomAttribute tag="00080005" vr="CS" keyword="SpecificCharacterSet">
        <Value number="1">ISO_IR 192</Value>
    </DicomAttribute>
    <DicomAttribute tag="00100010" vr="PN" keyword="PatientName">
        <PersonName number="1">
            <Alphabetic>
                <FamilyName>Bob</FamilyName>
                <NameSuffix>Dr.</NameSuffix>
            </Alphabetic>
        </PersonName>
    </DicomAttribute>
</NativeDicomModel>"#;

        let obj: InMemDicomObject<StandardDataDictionary> = crate::from_str(xml).unwrap();

        let tag = Tag(0x0008, 0x0005);
        assert_eq!(
            obj.get(tag),
            Some(&DataElement::new(tag, VR::CS, "ISO_IR 192"))
        );

        let tag = Tag(0x0010, 0x0010);
        assert_eq!(
            obj.get(tag),
            Some(&DataElement::new(tag, VR::PN, "Bob^^^^Dr."))
        );
    }
}
