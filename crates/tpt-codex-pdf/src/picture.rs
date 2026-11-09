// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pictures for PDF export, read in-house: PNG and JPEG files, turned into the form a PDF holds
//! an image in.
//!
//! A JPEG is embedded as the file has it. The PDF's own JPEG filter decodes it, so only its size
//! and component count are read here. A PNG that is 8 or 16 bits a channel, with no transparency
//! and no interlacing, is embedded with its compressed rows, and the PDF undoes the PNG filters
//! (predictor 15). Any other PNG (a palette, transparency, low bit depths) is decoded here: its
//! filters are undone, and its colour is written as 8-bit RGB with a separate alpha channel.
//!
//! Inflating and deflating use `flate2`, which the workspace already uses. The PNG filters, the
//! palette, the transparency and the JPEG header are read here.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

/// The largest side of a picture, in pixels.
const MAX_SIDE: u32 = 10_000;
/// The most pixels a picture may have.
const MAX_PIXELS: u64 = 40_000_000;
/// The most bytes inflating may produce for one picture.
const MAX_INFLATED: u64 = 200 * 1024 * 1024;

/// A picture, in the form a PDF image object holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The colour components: 1 (grey) or 3 (RGB).
    pub colors: u8,
    /// Bits per component: 8 or 16.
    pub bits: u8,
    /// The image's samples.
    pub data: ImageData,
    /// The alpha channel, as zlib-compressed 8-bit grey samples, when the picture has one.
    pub alpha: Option<Vec<u8>>,
    /// The picture's width in its file, before any resampling.
    pub natural_width: u32,
    /// The picture's height in its file, before any resampling.
    pub natural_height: u32,
}

/// How the samples of an image are stored in the PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
    /// JPEG bytes, as the file has them. The PDF's DCT filter decodes them.
    Jpeg(Vec<u8>),
    /// zlib-compressed PNG rows: each row starts with its PNG filter type, which the PDF's
    /// predictor 15 undoes.
    PngRows(Vec<u8>),
    /// zlib-compressed samples, with no predictor.
    Samples(Vec<u8>),
}

/// Reads a PNG or JPEG file, as [`decode`] does. A PNG larger than `max_width` by `max_height`
/// pixels is resampled to fit within them, keeping its aspect ratio. A JPEG is not resampled.
pub fn decode_within(bytes: &[u8], max_width: u32, max_height: u32) -> Option<Image> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        decode_png(bytes, (max_width, max_height))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        jpeg(bytes)
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

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *bytes.get(at)?,
        *bytes.get(at + 1)?,
        *bytes.get(at + 2)?,
        *bytes.get(at + 3)?,
    ]))
}

/// The size and components of a JPEG, from the first frame header. The file is embedded as it is.
fn jpeg(bytes: &[u8]) -> Option<Image> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        match marker {
            // Fill bytes, and markers with no length.
            0xFF => {
                at += 1;
                continue;
            }
            0x01 | 0xD0..=0xD7 => {
                at += 2;
                continue;
            }
            // End of image, or the scan before any frame header: not a picture.
            0xD9 | 0xDA => return None,
            _ => {}
        }
        let length = usize::from(be16(bytes, at + 2)?);
        // Start of frame, other than the markers that look like one but are not.
        let is_frame = (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_frame {
            let precision = *bytes.get(at + 4)?;
            let height = u32::from(be16(bytes, at + 5)?);
            let width = u32::from(be16(bytes, at + 7)?);
            let colors = *bytes.get(at + 9)?;
            if precision != 8 || !(colors == 1 || colors == 3) {
                return None;
            }
            within_limits(width, height)?;
            return Some(Image {
                width,
                height,
                colors,
                bits: 8,
                data: ImageData::Jpeg(bytes.to_vec()),
                alpha: None,
                natural_width: width,
                natural_height: height,
            });
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
}

/// The fields of a PNG's IHDR chunk.
struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color: u8,
    interlace: u8,
}

/// Channels per pixel for each PNG colour type.
fn channels(color: u8) -> Option<usize> {
    match color {
        0 | 3 => Some(1),
        2 => Some(3),
        4 => Some(2),
        6 => Some(4),
        _ => None,
    }
}

/// Whether a PNG colour type allows a bit depth.
fn depth_allowed(color: u8, depth: u8) -> bool {
    match color {
        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
        2 | 4 | 6 => matches!(depth, 8 | 16),
        3 => matches!(depth, 1 | 2 | 4 | 8),
        _ => false,
    }
}

fn decode_png(bytes: &[u8], max: (u32, u32)) -> Option<Image> {
    let mut header: Option<Header> = None;
    let mut palette: Vec<u8> = Vec::new();
    let mut transparency: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut at = 8;
    loop {
        let length = be32(bytes, at)? as usize;
        let kind = bytes.get(at + 4..at + 8)?;
        let data = bytes.get(at + 8..at.checked_add(8)?.checked_add(length)?)?;
        match kind {
            b"IHDR" => {
                if data.len() != 13 || header.is_some() {
                    return None;
                }
                // Compression and filter methods are the only ones PNG defines.
                if data[10] != 0 || data[11] != 0 {
                    return None;
                }
                header = Some(Header {
                    width: be32(data, 0)?,
                    height: be32(data, 4)?,
                    depth: data[8],
                    color: data[9],
                    interlace: data[12],
                });
            }
            b"PLTE" => palette = data.to_vec(),
            b"tRNS" => transparency = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        at = at.checked_add(12)?.checked_add(length)?;
        if at >= bytes.len() {
            return None;
        }
    }
    let header = header?;
    within_limits(header.width, header.height)?;
    // Interlaced pictures are not read; the placeholder stands in for them.
    if header.interlace != 0 || !depth_allowed(header.color, header.depth) || idat.is_empty() {
        return None;
    }
    let channel_count = channels(header.color)?;
    let has_alpha_channel = matches!(header.color, 4 | 6);
    let has_transparency = !transparency.is_empty();

    // The PDF can take the rows as they are: no transparency, no palette, 8 or 16 bits.
    let fits = header.width <= max.0 && header.height <= max.1;
    if matches!(header.color, 0 | 2) && matches!(header.depth, 8 | 16) && !has_transparency && fits
    {
        return Some(Image {
            width: header.width,
            height: header.height,
            colors: channel_count as u8,
            bits: header.depth,
            data: ImageData::PngRows(idat),
            alpha: None,
            natural_width: header.width,
            natural_height: header.height,
        });
    }

    let row_bits = channel_count * usize::from(header.depth);
    let row_bytes = (header.width as usize * row_bits).div_ceil(8);
    let bytes_per_pixel = (row_bits / 8).max(1);
    let raw = inflate(&idat)?;
    let rows = unfilter(&raw, row_bytes, bytes_per_pixel, header.height as usize)?;

    let width = header.width as usize;
    let pixels = width * header.height as usize;
    let mut rgb = Vec::with_capacity(pixels * 3);
    let mut alpha = Vec::with_capacity(pixels);
    for y in 0..header.height as usize {
        let row = &rows[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..width {
            let at = |c: usize| sample(row, x * channel_count + c, header.depth);
            let (r, g, b, a) = match header.color {
                0 => {
                    let v = at(0);
                    let a = key_alpha(&transparency, &[v], header.depth);
                    (v, v, v, a)
                }
                2 => {
                    let (r, g, b) = (at(0), at(1), at(2));
                    let a = key_alpha(&transparency, &[r, g, b], header.depth);
                    (r, g, b, a)
                }
                3 => {
                    let index = usize::from(at(0));
                    let colour = |c: usize| palette.get(index * 3 + c).copied().unwrap_or(0);
                    let a = transparency.get(index).copied().unwrap_or(255);
                    rgb.extend_from_slice(&[colour(0), colour(1), colour(2)]);
                    alpha.push(a);
                    continue;
                }
                4 => {
                    let v = at(0);
                    (v, v, v, at(1))
                }
                _ => (at(0), at(1), at(2), at(3)),
            };
            rgb.extend_from_slice(&[
                to8(r, header.depth),
                to8(g, header.depth),
                to8(b, header.depth),
            ]);
            alpha.push(to8(a, header.depth));
        }
    }
    let has_alpha = has_alpha_channel || has_transparency;
    // A picture larger than the cap is resampled to fit it, by averaging the pixels it covers.
    let (out_width, out_height) = fit(header.width, header.height, max);
    let (rgb, alpha) = if (out_width, out_height) == (header.width, header.height) {
        (rgb, alpha)
    } else {
        let (w, h) = (header.width as usize, header.height as usize);
        let (ow, oh) = (out_width as usize, out_height as usize);
        (
            box_resample(&rgb, w, h, 3, ow, oh),
            box_resample(&alpha, w, h, 1, ow, oh),
        )
    };
    Some(Image {
        width: out_width,
        height: out_height,
        colors: 3,
        bits: 8,
        data: ImageData::Samples(deflate(&rgb)?),
        alpha: has_alpha.then(|| deflate(&alpha)).flatten(),
        natural_width: header.width,
        natural_height: header.height,
    })
}

/// The size a picture of `width` by `height` pixels is given to fit within `max`, keeping its
/// aspect ratio. A picture that already fits keeps its size.
fn fit(width: u32, height: u32, max: (u32, u32)) -> (u32, u32) {
    if width <= max.0 && height <= max.1 {
        return (width, height);
    }
    let scale = (f64::from(max.0) / f64::from(width)).min(f64::from(max.1) / f64::from(height));
    let w = ((f64::from(width) * scale).floor() as u32).clamp(1, max.0.max(1));
    let h = ((f64::from(height) * scale).floor() as u32).clamp(1, max.1.max(1));
    (w, h)
}

/// Resamples `src`, `channels` bytes a pixel, from `w` by `h` pixels to `nw` by `nh`, by averaging
/// the source pixels each output pixel covers. Every source pixel is counted in one output pixel
/// or more, and each output pixel covers at least one.
fn box_resample(src: &[u8], w: usize, h: usize, channels: usize, nw: usize, nh: usize) -> Vec<u8> {
    let mut out = vec![0u8; nw * nh * channels];
    for y in 0..nh {
        let (y0, y1) = (y * h / nh, ((y + 1) * h / nh).max(y * h / nh + 1));
        for x in 0..nw {
            let (x0, x1) = (x * w / nw, ((x + 1) * w / nw).max(x * w / nw + 1));
            let count = ((y1 - y0) * (x1 - x0)) as u64;
            for c in 0..channels {
                let mut sum = 0u64;
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        sum += u64::from(src[(sy * w + sx) * channels + c]);
                    }
                }
                out[(y * nw + x) * channels + c] = (sum / count) as u8;
            }
        }
    }
    out
}

/// The alpha of a pixel from the colour key of a grey or RGB picture: transparent when every
/// component equals the key.
fn key_alpha(transparency: &[u8], values: &[u16], depth: u8) -> u16 {
    if transparency.is_empty() {
        return max_value(depth);
    }
    let key: Vec<u16> = transparency
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    if key.len() == values.len() && key.iter().zip(values).all(|(k, v)| k == v) {
        0
    } else {
        max_value(depth)
    }
}

fn max_value(depth: u8) -> u16 {
    ((1u32 << depth) - 1) as u16
}

/// The sample at `index` in a row, for a bit depth, as stored (not scaled).
fn sample(row: &[u8], index: usize, depth: u8) -> u16 {
    match depth {
        16 => u16::from_be_bytes([row[index * 2], row[index * 2 + 1]]),
        8 => u16::from(row[index]),
        _ => {
            let per_byte = 8 / usize::from(depth);
            let byte = row[index / per_byte];
            let shift = 8 - usize::from(depth) * (index % per_byte + 1);
            u16::from((byte >> shift) & max_value(depth) as u8)
        }
    }
}

/// A stored sample, scaled to 8 bits.
fn to8(value: u16, depth: u8) -> u8 {
    match depth {
        16 => (value >> 8) as u8,
        8 => value as u8,
        _ => (u32::from(value) * 255 / max_value(depth) as u32) as u8,
    }
}

/// Undoes the PNG filters of `raw` (each row with its filter type first), giving the rows.
fn unfilter(
    raw: &[u8],
    row_bytes: usize,
    bytes_per_pixel: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let stride = row_bytes + 1;
    if raw.len() < stride * height {
        return None;
    }
    let mut out = vec![0u8; row_bytes * height];
    for y in 0..height {
        let filter = raw[y * stride];
        let line = &raw[y * stride + 1..(y + 1) * stride];
        for i in 0..row_bytes {
            let left = if i >= bytes_per_pixel {
                out[y * row_bytes + i - bytes_per_pixel]
            } else {
                0
            };
            let (up, up_left) = if y > 0 {
                let above = (y - 1) * row_bytes;
                let up_left = if i >= bytes_per_pixel {
                    out[above + i - bytes_per_pixel]
                } else {
                    0
                };
                (out[above + i], up_left)
            } else {
                (0, 0)
            };
            let predicted = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => paeth(left, up, up_left),
                _ => return None,
            };
            out[y * row_bytes + i] = line[i].wrapping_add(predicted);
        }
    }
    Some(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Inflates zlib data, up to a size limit.
fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_INFLATED)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Deflates bytes as zlib data.
fn deflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).ok()?;
    encoder.finish().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Reads a file with no size cap.
    fn decode(bytes: &[u8]) -> Option<Image> {
        decode_within(bytes, u32::MAX, u32::MAX)
    }

    const JPEG: &[u8] = include_bytes!("../../../fixtures/pdf/sample.jpg");
    const RGB_PNG: &[u8] = include_bytes!("../../../fixtures/pdf/rgb.png");
    const GREY16_PNG: &[u8] = include_bytes!("../../../fixtures/pdf/grey16.png");
    const RGBA_PNG: &[u8] = include_bytes!("../../../fixtures/pdf/shapes.png");
    const PALETTE_PNG: &[u8] = include_bytes!("../../../fixtures/pdf/palette.png");
    const WIDE_PNG: &[u8] = include_bytes!("../../../fixtures/pdf/wide.png");

    fn samples(data: &ImageData) -> Vec<u8> {
        match data {
            ImageData::Samples(z) => inflate(z).unwrap(),
            other => panic!("not decoded samples: {other:?}"),
        }
    }

    #[test]
    fn a_jpeg_is_embedded_with_its_size_and_components() {
        let image = decode(JPEG).unwrap();
        assert_eq!(
            (image.width, image.height, image.colors, image.bits),
            (16, 16, 3, 8)
        );
        assert_eq!(image.data, ImageData::Jpeg(JPEG.to_vec()));
        assert!(image.alpha.is_none());
    }

    #[test]
    fn an_opaque_8_bit_png_is_embedded_as_its_rows() {
        let image = decode(RGB_PNG).unwrap();
        assert_eq!(
            (image.width, image.height, image.colors, image.bits),
            (4, 3, 3, 8)
        );
        assert!(matches!(image.data, ImageData::PngRows(_)));
    }

    #[test]
    fn a_16_bit_grey_png_is_embedded_at_16_bits() {
        let image = decode(GREY16_PNG).unwrap();
        assert_eq!((image.colors, image.bits), (1, 16));
        assert!(matches!(image.data, ImageData::PngRows(_)));
    }

    #[test]
    fn a_png_with_transparency_is_decoded_to_rgb_and_an_alpha_channel() {
        let image = decode(RGBA_PNG).unwrap();
        assert_eq!(
            (image.width, image.height, image.colors, image.bits),
            (120, 60, 3, 8)
        );
        let rgb = samples(&image.data);
        // The left half is opaque red; the right half is translucent blue.
        assert_eq!(&rgb[..3], &[200, 30, 30]);
        let alpha = inflate(image.alpha.as_deref().unwrap()).unwrap();
        assert_eq!(alpha[0], 255);
        assert!(
            (120..=135).contains(&alpha[60]),
            "translucent: {}",
            alpha[60]
        );
    }

    #[test]
    fn a_palette_png_is_decoded_through_its_palette() {
        let image = decode(PALETTE_PNG).unwrap();
        assert_eq!(image.colors, 3);
        let rgb = samples(&image.data);
        assert_eq!(rgb.len(), (image.width * image.height * 3) as usize);
        assert!(rgb.iter().any(|&b| b != 0), "the palette colours are there");
    }

    #[test]
    fn other_bytes_and_damaged_pictures_are_not_read() {
        assert!(decode(b"GIF89a").is_none());
        assert!(decode(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0]).is_none());
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0]).is_none());
        // A truncated file is not decoded past where it ends.
        assert!(decode(&RGBA_PNG[..RGBA_PNG.len() / 2]).is_none());
    }

    #[test]
    fn a_picture_too_large_is_refused_before_its_pixels_are_read() {
        assert!(within_limits(MAX_SIDE + 1, 1).is_none());
        assert!(within_limits(MAX_SIDE, MAX_SIDE).is_none());
        assert!(within_limits(4000, 4000).is_some());
    }

    #[test]
    fn a_picture_within_the_cap_keeps_its_pixels() {
        assert_eq!(fit(100, 50, (400, 400)), (100, 50));
        let image = decode_within(RGB_PNG, 4, 3).unwrap();
        assert_eq!((image.width, image.height), (4, 3));
    }

    #[test]
    fn a_picture_over_the_cap_is_resampled_to_fit_and_keeps_its_natural_size() {
        let image = decode_within(WIDE_PNG, 400, 400).unwrap();
        assert_eq!((image.width, image.height), (400, 100));
        assert_eq!((image.natural_width, image.natural_height), (1600, 400));
        assert_eq!(samples(&image.data).len(), 400 * 100 * 3);
    }

    #[test]
    fn fit_keeps_the_aspect_ratio_of_a_wide_or_a_tall_picture() {
        assert_eq!(fit(1600, 400, (400, 400)), (400, 100));
        assert_eq!(fit(300, 1200, (400, 400)), (100, 400));
    }

    #[test]
    fn resampling_averages_the_pixels_each_output_pixel_covers() {
        // Four pixels, two black and two white, down to one: grey halfway.
        assert_eq!(box_resample(&[0, 255, 0, 255], 2, 2, 1, 1, 1), vec![127]);
        // Two channels stay separate: red, green, blue, and alpha are averaged one at a time.
        assert_eq!(box_resample(&[10, 0, 30, 0], 2, 1, 2, 1, 1), vec![20, 0]);
    }

    #[test]
    fn each_png_filter_undoes_to_the_original_row() {
        // Two rows of three bytes, one pixel of one byte each: filtered with every type.
        let original = [10u8, 20, 30, 40, 50, 60];
        // Row 0 with Sub: each byte is the difference from the byte to its left.
        let sub = [1u8, 10, 10, 10];
        // Row 1 with Up: each byte is the difference from the byte above.
        let up = [2u8, 30, 30, 30];
        let raw: Vec<u8> = sub.iter().chain(up.iter()).copied().collect();
        let rows = unfilter(&raw, 3, 1, 2).unwrap();
        assert_eq!(rows, original);
    }
}
