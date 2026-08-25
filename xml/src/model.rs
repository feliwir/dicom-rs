//! Data types mirroring the Native DICOM Model XML structure,
//! as described in [DICOM PS3.19 Annex A][1].
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/html/part19.html#chapter_A

use serde::{Deserialize, Serialize};

/// The root element of a Native DICOM Model document.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename = "NativeDicomModel")]
pub struct NativeDicomModel {
    /// the `xml:space` attribute, usually set to `"preserve"`
    #[serde(
        rename = "@xml:space",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub xml_space: Option<String>,
    /// the data elements of the data set
    #[serde(rename = "DicomAttribute", default)]
    pub attributes: Vec<DicomAttribute>,
}

/// A single DICOM data element, identified by its tag.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DicomAttribute {
    /// the attribute tag, in the form `"GGGGEEEE"`
    #[serde(rename = "@tag")]
    pub tag: String,
    /// the value representation of the attribute
    #[serde(rename = "@vr")]
    pub vr: String,
    /// the keyword of the attribute, if known
    #[serde(rename = "@keyword", skip_serializing_if = "Option::is_none", default)]
    pub keyword: Option<String>,
    /// the private creator of the attribute, if it is a private attribute
    #[serde(
        rename = "@privateCreator",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub private_creator: Option<String>,
    /// textual values of the attribute
    #[serde(rename = "Value", default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<TextValue>,
    /// the values of the attribute, as person names
    #[serde(rename = "PersonName", default, skip_serializing_if = "Vec::is_empty")]
    pub person_names: Vec<PersonName>,
    /// items of a sequence attribute
    #[serde(rename = "Item", default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
    /// the value of the attribute, inlined as Base64
    #[serde(
        rename = "InlineBinary",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub inline_binary: Option<String>,
    /// a reference to the value of the attribute, retrievable from a URI
    #[serde(rename = "BulkData", skip_serializing_if = "Option::is_none", default)]
    pub bulk_data: Option<BulkData>,
}

/// A single textual value of a data element,
/// used for every value representation
/// other than `PN`, and the ones with binary content.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TextValue {
    /// the 1-based position of this value
    /// among the other values of the attribute
    #[serde(rename = "@number")]
    pub number: u32,
    /// the textual representation of the value
    #[serde(rename = "$text", default)]
    pub text: String,
}

/// An item of a sequence attribute (`VR` of `SQ`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Item {
    /// the 1-based position of this item
    /// among the other items of the sequence
    #[serde(rename = "@number")]
    pub number: u32,
    /// the data elements contained in this item
    #[serde(rename = "DicomAttribute", default)]
    pub attributes: Vec<DicomAttribute>,
}

/// A reference to bulk data retrievable from a URI.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BulkData {
    /// the URI where the value can be retrieved from
    #[serde(rename = "@uri")]
    pub uri: String,
}

/// A single person name value (`VR` of `PN`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PersonName {
    /// the 1-based position of this value
    /// among the other values of the attribute
    #[serde(rename = "@number")]
    pub number: u32,
    /// the alphabetic representation of the name
    #[serde(
        rename = "Alphabetic",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub alphabetic: Option<PersonNameComponents>,
    /// the ideographic representation of the name
    #[serde(
        rename = "Ideographic",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub ideographic: Option<PersonNameComponents>,
    /// the phonetic representation of the name
    #[serde(rename = "Phonetic", skip_serializing_if = "Option::is_none", default)]
    pub phonetic: Option<PersonNameComponents>,
}

/// The group of components of a single person name representation.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PersonNameComponents {
    /// the family name component
    #[serde(
        rename = "FamilyName",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub family_name: Option<String>,
    /// the given name component
    #[serde(rename = "GivenName", skip_serializing_if = "Option::is_none", default)]
    pub given_name: Option<String>,
    /// the middle name component
    #[serde(
        rename = "MiddleName",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub middle_name: Option<String>,
    /// the name prefix component
    #[serde(
        rename = "NamePrefix",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub name_prefix: Option<String>,
    /// the name suffix component
    #[serde(
        rename = "NameSuffix",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub name_suffix: Option<String>,
}

impl PersonNameComponents {
    /// Whether all components are absent.
    pub(crate) fn is_empty(&self) -> bool {
        self.family_name.is_none()
            && self.given_name.is_none()
            && self.middle_name.is_none()
            && self.name_prefix.is_none()
            && self.name_suffix.is_none()
    }
}
