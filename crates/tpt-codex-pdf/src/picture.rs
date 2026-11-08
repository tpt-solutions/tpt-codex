// SPDX-License-Identifier: MIT OR Apache-2.0
//! Decoding of the pictures a document shows: PNG and JPEG, to straight-alpha RGBA pixels.
//!
//! A picture that cannot be decoded, or that is too large to decode safely, is left out: the
//! export draws its placeholder instead. CMYK JPEGs are left out too, since converting them to
//! RGB needs a colour profile this module does not have.

use std::io::Cursor;

/// The largest side of a picture, in pixels.
const MAX_SIDE: u32 = 10_000;
/// The most pixels a picture may have. The decoded pixels take four bytes each.
const MAX_PIXELS: u64 = 40_000_000;

/// A decoded picture: its size, and its pixels as straight-alpha RGBA, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Four bytes a pixel: red, green, blue and alpha, row by row.
    pub rgba: Vec<u8>,
}

/// Decodes a PNG or JPEG file. `None` for any other format, or for a picture that cannot be
/// decoded or is too large.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        decode_png(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg(bytes)
    } else {
        None
    }
}

fn within_limits(width: u32, height: u32) -> Option<()> {
    let ok = width > 0
        && height > 0
        && width <= MAX_SIDE
        && height <= MAX_SIDE
        && u64::from(width) * u64::from(height) <= MAX_PIXELS;
    ok.then_some(())
}

fn decode_png(bytes: &[u8]) -> Option<Decoded> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    // Palettes and low bit depths become eight bits a channel, so the colour types below are all.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    within_limits(width, height)?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let data = buffer.get(..frame.buffer_size())?;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Decoded {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

fn decode_jpeg(bytes: &[u8]) -> Option<Decoded> {
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    within_limits(width, height)?;
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        jpeg_decoder::PixelFormat::CMYK32 | jpeg_decoder::PixelFormat::L16 => return None,
    };
    Some(Decoded {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 PNG with four colours, written here with the same crate that reads it.
    fn png_2x2(color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, 2, 2);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(data).unwrap();
        }
        out
    }

    #[test]
    fn an_rgb_png_decodes_with_full_alpha() {
        let data = [255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9];
        let decoded = decode(&png_2x2(png::ColorType::Rgb, &data)).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(&decoded.rgba[..8], &[255, 0, 0, 255, 0, 255, 0, 255]);
        assert_eq!(decoded.rgba.len(), 16);
    }

    #[test]
    fn a_grey_alpha_png_keeps_its_alpha() {
        let data = [10, 128, 20, 0, 30, 255, 40, 64];
        let decoded = decode(&png_2x2(png::ColorType::GrayscaleAlpha, &data)).unwrap();
        assert_eq!(&decoded.rgba[..4], &[10, 10, 10, 128]);
        assert_eq!(&decoded.rgba[12..], &[40, 40, 40, 64]);
    }

    #[test]
    fn a_jpeg_decodes_to_its_size() {
        let bytes = include_bytes!("../../../fixtures/pdf/sample.jpg");
        let decoded = decode(bytes).unwrap();
        assert_eq!((decoded.width, decoded.height), (16, 16));
        assert_eq!(decoded.rgba.len(), 16 * 16 * 4);
    }

    #[test]
    fn other_bytes_and_damaged_pictures_are_not_decoded() {
        assert!(decode(b"GIF89a").is_none());
        assert!(decode(&[0x89, b'P', b'N', b'G', 0, 0]).is_none());
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0]).is_none());
    }

    #[test]
    fn a_picture_too_large_to_decode_is_refused_before_its_pixels_are_read() {
        assert!(within_limits(MAX_SIDE + 1, 1).is_none());
        assert!(within_limits(MAX_SIDE, MAX_SIDE).is_none());
        assert!(within_limits(4000, 4000).is_some());
    }
}
