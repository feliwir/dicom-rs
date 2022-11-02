//! Support for JPG image decoding.

use dicom_encoding::adapters::{
    decode_error, DecodeResult, EncodeOptions, EncodeResult, PixelDataObject, PixelDataReader,
    PixelDataWriter,
};
use dicom_encoding::snafu::prelude::*;
use jpeg_decoder::Decoder;
use jpeg_encoder::ColorType;
use std::io::Cursor;

/// Pixel data adapter for JPEG-based transfer syntaxes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JpegAdapter;

impl PixelDataReader for JpegAdapter {
    /// Decode DICOM image data with jpeg encoding.
    fn decode(&self, src: &dyn PixelDataObject, dst: &mut Vec<u8>) -> DecodeResult<()> {
        let cols = src
            .cols()
            .context(decode_error::MissingAttributeSnafu { name: "Columns" })?;
        let rows = src
            .rows()
            .context(decode_error::MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel =
            src.samples_per_pixel()
                .context(decode_error::MissingAttributeSnafu {
                    name: "SamplesPerPixel",
                })?;
        let bits_allocated = src
            .bits_allocated()
            .context(decode_error::MissingAttributeSnafu {
                name: "BitsAllocated",
            })?;

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;
        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = bytes_per_sample as usize * cols as usize * rows as usize;
        let base_offset = dst.len();
        dst.resize(
            base_offset + (samples_per_pixel as usize * stride) * nr_frames,
            0,
        );

        // Embedded jpegs can span multiple fragments
        // Hence we collect all fragments into single vector
        // and then just iterate a cursor for each frame
        let fragments: Vec<u8> = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?
            .fragments
            .into_iter()
            .flatten()
            .collect();

        let fragments_len = fragments.len() as u64;
        let mut cursor = Cursor::new(fragments);
        let mut dst_offset = base_offset;

        loop {
            let mut decoder = Decoder::new(&mut cursor);
            let decoded = decoder
                .decode()
                .map_err(|e| Box::new(e) as Box<_>)
                .whatever_context("JPEG decoder failure")?;

            let decoded_len = decoded.len();
            dst[dst_offset..(dst_offset + decoded_len)].copy_from_slice(&decoded);
            dst_offset += decoded_len;

            // dicom fields always have to have an even length and fill this space with padding
            // if uneven we have to move one position further to consume this padding
            if cursor.position() % 2 > 0 {
                cursor.set_position(cursor.position() + 1);
            }

            if cursor.position() >= fragments_len {
                break;
            }
        }

        Ok(())
    }

    /// Decode DICOM image data with jpeg encoding.
    fn decode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        dst: &mut Vec<u8>,
    ) -> DecodeResult<()> {
        let cols = src
            .cols()
            .context(decode_error::MissingAttributeSnafu { name: "Columns" })?;
        let rows = src
            .rows()
            .context(decode_error::MissingAttributeSnafu { name: "Rows" })?;
        let samples_per_pixel =
            src.samples_per_pixel()
                .context(decode_error::MissingAttributeSnafu {
                    name: "SamplesPerPixel",
                })?;
        let bits_allocated = src
            .bits_allocated()
            .context(decode_error::MissingAttributeSnafu {
                name: "BitsAllocated",
            })?;

        if bits_allocated != 8 && bits_allocated != 16 {
            whatever!("BitsAllocated other than 8 or 16 is not supported");
        }

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;

        ensure!(
            nr_frames > frame as usize,
            decode_error::FrameRangeOutOfBoundsSnafu
        );

        let bytes_per_sample = bits_allocated / 8;

        // `stride` it the total number of bytes for each sample plane
        let stride: usize = bytes_per_sample as usize * cols as usize * rows as usize;
        let base_offset = dst.len();
        dst.resize(base_offset + (samples_per_pixel as usize * stride), 0);

        // Embedded jpegs can span multiple fragments
        // Hence we collect all fragments into single vector
        // and then just iterate a cursor for each frame
        let raw_pixeldata = src
            .raw_pixel_data()
            .whatever_context("Expected to have raw pixel data available")?;
        let fragment = raw_pixeldata
            .fragments
            .get(frame as usize)
            .whatever_context("Missing fragment for the frame requested")?;

        let fragment_len = fragment.len() as u64;
        let mut cursor = Cursor::new(fragment);
        let mut dst_offset = base_offset;

        loop {
            let mut decoder = Decoder::new(&mut cursor);
            let decoded = decoder
                .decode()
                .map_err(|e| Box::new(e) as Box<_>)
                .whatever_context("JPEG decoder failure")?;

            let decoded_len = decoded.len();
            dst[dst_offset..(dst_offset + decoded_len)].copy_from_slice(&decoded);
            dst_offset += decoded_len;

            // dicom fields always have to have an even length and fill this space with padding
            // if uneven we have to move one position further to consume this padding
            if cursor.position() % 2 > 0 {
                cursor.set_position(cursor.position() + 1);
            }

            if cursor.position() >= fragment_len {
                break;
            }
        }

        Ok(())
    }
}

impl PixelDataWriter for JpegAdapter {
    fn encode_frame(
        &self,
        src: &dyn PixelDataObject,
        frame: u32,
        options: EncodeOptions,
        dst: &mut Vec<u8>,
    ) -> EncodeResult<Vec<dicom_core::ops::AttributeOp>> {
        let cols = src.cols().unwrap();
        let rows = src.rows().unwrap();
        let samples_per_pixel = src.samples_per_pixel().unwrap();
        let bits_allocated = src.bits_allocated().unwrap();

        let nr_frames = src.number_of_frames().unwrap_or(1) as usize;

        let quality = options.quality.unwrap_or(85);

        let frame_size = (
            cols * rows * samples_per_pixel * (bits_allocated / 8)
        ) as usize;

        let color_type = match samples_per_pixel {
            1 => ColorType::Luma,
            3 => ColorType::Rgb,
            4 => ColorType::Rgba,
            _ => todo!("Implement error handling for wrong samples per pixel")
        };

        // record dst length before encoding to know full jpeg size
        let len_before = dst.len();

        // Encode the data
        let frame_uncompressed = src.fragment(frame as usize).unwrap();
        let encoder = jpeg_encoder::Encoder::new(&mut *dst, quality);
        encoder
            .encode(&frame_uncompressed, cols, rows, color_type)
            .unwrap();

        let compressed_frame_size = dst.len() - len_before;
        
        // TODO
        Ok(vec![])
    }
}
