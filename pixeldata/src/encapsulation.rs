use dicom_core::value::{Value, C};
use dicom_core::DataDictionary;
use dicom_object::mem::InMemFragment;
use dicom_object::InMemDicomObject;

#[derive(Debug)]
pub struct EncapsulatedPixels {
    offset_table: C<u32>,
    current_offset: u32,
    fragments: C<Vec<u8>>,
    fragment_size: u32,
}

impl EncapsulatedPixels {
    /// Add a single frame to EncapsulatedPixels
    pub fn add_frame(&mut self, data: Vec<u8>) {
        for fragment in fragment_frame(data, self.fragment_size) {
            let size = fragment.len() as u32;
            self.offset_table.push(self.current_offset);
            self.fragments.push((*fragment).to_vec());
            self.current_offset += size;
        }
    }

    /// Creates an empty EncapsulatedPixels struct
    pub fn new(fragment_size: u32) -> Self {
        EncapsulatedPixels {
            offset_table: C::new(),
            current_offset: 0,
            fragments: C::new(),
            fragment_size,
        }
    }
}

impl<D> From<EncapsulatedPixels> for Value<InMemDicomObject<D>, InMemFragment>
where
    D: DataDictionary + Clone
{
    fn from(value: EncapsulatedPixels) -> Self {
        Value::PixelSequence {
            offset_table: value.offset_table,
            fragments: value.fragments
        }
    }
}

impl From<Vec<Vec<u8>>> for EncapsulatedPixels {
    /// Create EncapsulatedPixels from a list of fragments and calculate the bot
    fn from(fragments: Vec<Vec<u8>>) -> Self {
        let mut offset_table = C::with_capacity(fragments.len());
        let mut current_offset = 0u32;
        for fragment in &fragments {
            offset_table.push(current_offset);
            current_offset += fragment.len() as u32;
        }

        EncapsulatedPixels {
            offset_table,
            current_offset,
            fragments: fragments.into(),
            fragment_size: 0,
        }
    }
}

/// Create the fragments for a single frame. It returns a list with the fragments.
pub fn fragment_frame(data: Vec<u8>, fragment_size: u32) -> Vec<Vec<u8>> {
    let fragment_size: u32 = if fragment_size == 0 {
        data.len() as u32
    } else {
        fragment_size
    };

    let fragment_size = if fragment_size % 2 == 0 {
        fragment_size
    } else {
        fragment_size + 1
    };

    let number_of_fragments = (data.len() as f32 / fragment_size as f32).ceil() as u32;

    // Calculate the encapsulated size. If necessary pad the vector with zeroes so all the
    // chunks have the same fragment_size
    let mut data = data;
    let encapsulated_size = (fragment_size * number_of_fragments) as usize;
    if encapsulated_size > data.len() {
        data.resize(encapsulated_size, 0);
    }

    data.chunks_exact(fragment_size as usize)
        .map(|fragment| (*fragment).to_vec())
        .collect::<Vec<Vec<u8>>>()
}

/// Encapsulate the pixel data of the frames. If frames > 1 then fragments is ignored and set to 1.
/// If the calculated fragment size is less than 2 bytes, then it is set to 2 bytes
pub fn encapsulate(frames: Vec<Vec<u8>>, fragment_size: u32) -> EncapsulatedPixels {
    let number_of_fragments = if frames.len() > 1 {
        0
    } else {
        fragment_size
    };
    let mut encapsulated_data = EncapsulatedPixels::new(number_of_fragments);

    for frame in frames {
        encapsulated_data.add_frame(frame);
    }

    encapsulated_data
}

#[cfg(test)]
mod tests {
    use crate::encapsulation::{encapsulate, fragment_frame, EncapsulatedPixels};

    #[test]
    fn test_add_frame() {
        let mut enc = EncapsulatedPixels::new(1);
        assert_eq!(enc.offset_table.len(), 0);
        assert_eq!(enc.fragments.len(), 0);
        assert_eq!(enc.current_offset, 0);

        enc.add_frame(vec![10, 20, 30]);
        assert_eq!(enc.offset_table.len(), 1);
        assert_eq!(enc.fragments.len(), 1);
        assert_eq!(enc.current_offset, 4);

        enc.add_frame(vec![10, 20, 30, 50]);
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 2);
        assert_eq!(enc.current_offset, 8);
    }

    #[test]
    fn test_encapsulated_pixels() {
        let enc = encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 1);
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 2);

        let enc = encapsulate(vec![vec![20, 30, 40]], 2);
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 2);

        let enc = encapsulate(vec![vec![20, 30, 40], vec![50, 60, 70, 80]], 2);
        assert_eq!(enc.offset_table.len(), 2);
        assert_eq!(enc.fragments.len(), 2);
    }

    #[test]
    fn test_fragment_frame() {
        let fragment = fragment_frame(vec![150, 164, 200], 1);
        assert_eq!(fragment.len(), 1, "1 fragment should be present");
        assert_eq!(fragment[0].len(), 4, "The fragment size should be 4");
        assert_eq!(
            fragment[0],
            vec![150, 164, 200, 0],
            "The data should be 0 padded"
        );

        let fragment = fragment_frame(vec![150, 164, 200, 222], 1);
        assert_eq!(fragment.len(), 1, "1 fragment should be present");
        assert_eq!(fragment[0].len(), 4, "The fragment size should be 4");
        assert_eq!(
            fragment[0],
            vec![150, 164, 200, 222],
            "The data should be what was sent"
        );

        let fragment = fragment_frame(vec![150, 164, 200, 222], 2);
        assert_eq!(fragment.len(), 2, "2 fragments should be present");
        assert_eq!(fragment[0].len(), 2);
        assert_eq!(fragment[1].len(), 2);
        assert_eq!(fragment[0], vec![150, 164]);
        assert_eq!(fragment[1], vec![200, 222]);

        let fragment = fragment_frame(vec![150, 164, 200], 3);
        assert_eq!(
            fragment.len(),
            2,
            "2 fragments should be present as fragment_size < 2"
        );
        assert_eq!(fragment[0].len(), 2);
        assert_eq!(fragment[0], vec![150, 164]);
        assert_eq!(fragment[1].len(), 2);
        assert_eq!(fragment[1], vec![200, 0]);

        let fragment = fragment_frame(vec![150, 164, 200, 222], 3);
        assert_eq!(
            fragment.len(),
            2,
            "2 fragments should be present as fragment_size < 2"
        );
        assert_eq!(fragment[0].len(), 2);
        assert_eq!(fragment[0], vec![150, 164]);
        assert_eq!(fragment[1].len(), 2);
        assert_eq!(fragment[1], vec![200, 222]);
    }
}
