//! Error types for DICOM XML serialization and deserialization.

use std::fmt;

use dicom_core::Tag;

/// An error occurring while serializing or deserializing DICOM XML.
#[derive(Debug)]
pub enum Error {
    /// an underlying XML serialization error
    Serialize(quick_xml::se::SeError),
    /// an underlying XML deserialization error
    Deserialize(quick_xml::de::DeError),
    /// the attribute tag could not be parsed
    InvalidTag {
        /// the invalid tag text
        text: String,
    },
    /// the attribute value representation could not be parsed
    InvalidVr {
        /// the tag of the offending attribute
        tag: Tag,
        /// the invalid VR text
        text: String,
    },
    /// the inline binary data is not valid Base64
    InvalidBase64 {
        /// the tag of the offending attribute
        tag: Tag,
    },
    /// a numeric or textual value of the attribute could not be parsed
    InvalidValue {
        /// the tag of the offending attribute
        tag: Tag,
        /// the invalid text
        text: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Serialize(e) => write!(f, "failed to serialize DICOM XML: {e}"),
            Error::Deserialize(e) => write!(f, "failed to deserialize DICOM XML: {e}"),
            Error::InvalidTag { text } => write!(f, "invalid attribute tag `{text}`"),
            Error::InvalidVr { tag, text } => {
                write!(f, "invalid value representation `{text}` for tag {tag}")
            }
            Error::InvalidBase64 { tag } => {
                write!(f, "invalid inline binary data for tag {tag}")
            }
            Error::InvalidValue { tag, text } => {
                write!(f, "invalid value `{text}` for tag {tag}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Serialize(e) => Some(e),
            Error::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<quick_xml::se::SeError> for Error {
    fn from(e: quick_xml::se::SeError) -> Self {
        Error::Serialize(e)
    }
}

impl From<quick_xml::de::DeError> for Error {
    fn from(e: quick_xml::de::DeError) -> Self {
        Error::Deserialize(e)
    }
}

/// The result type used throughout this crate.
pub type Result<T> = std::result::Result<T, Error>;
