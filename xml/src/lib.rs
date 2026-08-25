#![warn(missing_docs)]
//! DICOM XML module
//!
//! This library provides serialization of DICOM data to XML
//! and deserialization of XML to DICOM data,
//! as per the Native DICOM Model described in
//! [DICOM standard part 19][1].
//!
//! [1]: https://dicom.nema.org/medical/dicom/current/output/html/part19.html#PS3.19
//!
//! The easiest path to serialization is in
//! using the functions readily available [`to_string`].
//! Alternatively, DICOM data can be enclosed by a [`DicomXml`] value,
//! which implements serialization and deserialization via [Serde](serde).
//!
//! # Example
//!
//! To serialize an object to DICOM XML:
//!
//! ```
//! # use dicom_core::{PrimitiveValue, VR};
//! # use dicom_object::mem::{InMemDicomObject, InMemElement};
//! # use dicom_dictionary_std::tags;
//! let obj = InMemDicomObject::from_element_iter([
//!     InMemElement::new(tags::SERIES_DATE, VR::DA, "20230610"),
//!     InMemElement::new(tags::INSTANCE_NUMBER, VR::IS, "5"),
//! ]);
//!
//! let xml = dicom_xml::to_string(&obj)?;
//! # let _ = xml;
//! # Result::<(), dicom_xml::Error>::Ok(())
//! ```
//!
//! To turn DICOM XML back into an in-memory object:
//!
//! ```
//! # use dicom_object::mem::InMemDicomObject;
//! let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
//! <NativeDicomModel xml:space="preserve">
//!   <DicomAttribute tag="00080021" vr="DA" keyword="SeriesDate">
//!     <Value number="1">20230610</Value>
//!   </DicomAttribute>
//!   <DicomAttribute tag="00200013" vr="IS" keyword="InstanceNumber">
//!     <Value number="1">5</Value>
//!   </DicomAttribute>
//! </NativeDicomModel>"#;
//! let obj: InMemDicomObject = dicom_xml::from_str(xml)?;
//! # Result::<(), dicom_xml::Error>::Ok(())
//! ```
//!
//! Use the [`DicomXml`] wrapper type
//! for greater control on how to serialize or deserialize data,
//! for example with a generic XML (de)serializer other than
//! the functions provided at the top of this crate.

mod de;
mod error;
pub mod model;
mod ser;

pub use crate::error::{Error, Result};
pub use crate::model::NativeDicomModel;

/// Serialize a piece of DICOM data as a string of DICOM XML.
pub fn to_string<T>(data: T) -> Result<String>
where
    DicomXml<T>: From<T> + serde::Serialize,
{
    ser::to_string(DicomXml::from(data))
}

/// Serialize a piece of DICOM data as a pretty-printed string of DICOM XML.
pub fn to_string_pretty<T>(data: T) -> Result<String>
where
    DicomXml<T>: From<T> + serde::Serialize,
{
    ser::to_string_pretty(DicomXml::from(data))
}

/// Serialize a piece of DICOM data to a byte writer.
pub fn to_writer<W, T>(writer: W, data: T) -> Result<()>
where
    DicomXml<T>: From<T> + serde::Serialize,
    W: std::io::Write,
{
    ser::to_writer(writer, DicomXml::from(data))
}

/// Deserialize a piece of DICOM data from a string of DICOM XML.
pub fn from_str<'a, T>(source: &'a str) -> Result<T>
where
    DicomXml<T>: serde::Deserialize<'a>,
{
    de::from_str::<DicomXml<T>>(source).map(DicomXml::into_inner)
}

/// Deserialize a piece of DICOM data from a byte reader.
pub fn from_reader<R, T>(reader: R) -> Result<T>
where
    R: std::io::Read,
    DicomXml<T>: serde::de::DeserializeOwned,
{
    de::from_reader::<R, DicomXml<T>>(reader).map(DicomXml::into_inner)
}

/// A wrapper type for DICOM XML serialization using [Serde](serde).
///
/// Serializing this type will yield XML data
/// following the Native DICOM Model,
/// as described in [DICOM PS3.19][1].
/// Deserialization from this type
/// will interpret the input data
/// as a standard Native DICOM Model XML document.
///
/// [1]: https://dicom.nema.org/medical/dicom/current/output/html/part19.html#PS3.19
///
/// # Serialization
///
/// Convert a DICOM data type such as a file or object
/// into a `DicomXml` value using [`From`] or [`Into`],
/// then use an XML serializer such as the one in [`quick_xml`]
/// to serialize it to the intended type.
/// A reference may be used as well,
/// so as to not consume the DICOM data.
///
/// `DicomXml` can serialize:
///
/// - [`InMemDicomObject`][1] as a Native DICOM Model data set;
/// - [`DefaultDicomObject`][2],
///   which will also include the attributes from the file meta group.
///
/// [1]: dicom_object::InMemDicomObject
/// [2]: dicom_object::DefaultDicomObject
///
/// # Deserialization
///
/// Specify the concrete DICOM data type to deserialize to,
/// place it as the type parameter `T` of `DicomXml<T>`,
/// then request to deserialize it.
///
/// `DicomXml` can deserialize [`InMemDicomObject`][1],
/// expecting a Native DICOM Model XML document.
#[derive(Debug, Clone, PartialEq)]
pub struct DicomXml<T>(T);

impl<T> DicomXml<T> {
    /// Unwrap the DICOM XML wrapper,
    /// returning the underlying value.
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Obtain a reference to the underlying value.
    pub fn inner(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for DicomXml<T> {
    fn from(value: T) -> Self {
        DicomXml(value)
    }
}
