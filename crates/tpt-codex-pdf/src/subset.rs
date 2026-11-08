// SPDX-License-Identifier: MIT OR Apache-2.0
//! Subsetting of the font embedded in a PDF: the glyphs the file uses, and the tables a reader needs.
//!
//! The bundled font is a variable font, so its file is large mostly for its variation data. The
//! outlines and the text layer use the default instance, so the subset drops the variation tables
//! and the layout tables that a PDF reader does not use. Glyph ids are kept: a glyph the file does
//! not use is an empty outline, so the ids in the text layer still mean the same glyphs.

use std::collections::BTreeSet;

/// The tables a PDF reader needs from a TrueType font, in the subset. Everything else is dropped.
const KEPT: [&[u8; 4]; 8] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"prep",
];

/// A subset of `font` (a TrueType file) with the outlines of `glyphs` and of the glyphs they are
/// built from. Other glyphs are empty. Errors if the file is not a TrueType font this can read.
pub fn subset_truetype(font: &[u8], glyphs: &BTreeSet<u32>) -> Result<Vec<u8>, String> {
    let tables = Tables::read(font)?;
    let head = tables.get(b"head")?;
    let format = i16_at(head, 50)?;
    let glyph_count = u16_at(tables.get(b"maxp")?, 4)? as u32;
    let glyf = tables.get(b"glyf")?;
    let offsets = glyph_offsets(tables.get(b"loca")?, format, glyph_count)?;

    let needed = needed_glyphs(glyf, &offsets, glyphs, glyph_count)?;

    // The outlines of the needed glyphs, each padded to four bytes, with the offsets of all glyphs.
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca: Vec<u8> = Vec::new();
    for gid in 0..glyph_count {
        new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        if needed.contains(&gid) {
            let (start, end) = (offsets[gid as usize], offsets[gid as usize + 1]);
            let bytes = glyf
                .get(start..end)
                .ok_or_else(|| format!("glyph {gid} is outside the glyph table"))?;
            new_glyf.extend_from_slice(bytes);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
    }
    new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());

    // The head table with the long offset format, which the new loca table uses.
    let mut new_head = head.to_vec();
    new_head[50..52].copy_from_slice(&1i16.to_be_bytes());
    new_head[8..12].copy_from_slice(&[0, 0, 0, 0]);

    let mut out: Vec<(&[u8; 4], Vec<u8>)> = Vec::new();
    for tag in KEPT {
        let data = match tag {
            b"head" => new_head.clone(),
            _ => tables.get(tag).map(<[u8]>::to_vec).unwrap_or_default(),
        };
        if !data.is_empty() {
            out.push((tag, data));
        }
    }
    out.push((b"glyf", new_glyf));
    out.push((b"loca", new_loca));
    Ok(assemble(font, out))
}

/// The tables of a TrueType file, by tag.
struct Tables<'a> {
    entries: Vec<([u8; 4], &'a [u8])>,
}

impl<'a> Tables<'a> {
    fn read(font: &'a [u8]) -> Result<Self, String> {
        let count = u16_at(font, 4)? as usize;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let record = 12 + 16 * i;
            let tag: [u8; 4] = font
                .get(record..record + 4)
                .ok_or("the table directory is cut short")?
                .try_into()
                .map_err(|_| "a table tag is cut short")?;
            let offset = u32_at(font, record + 8)? as usize;
            let length = u32_at(font, record + 12)? as usize;
            let data = font
                .get(offset..offset.checked_add(length).ok_or("a table overflows")?)
                .ok_or("a table is outside the file")?;
            entries.push((tag, data));
        }
        Ok(Self { entries })
    }

    fn get(&self, tag: &[u8; 4]) -> Result<&'a [u8], String> {
        self.entries
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, data)| *data)
            .ok_or_else(|| format!("the font has no {} table", String::from_utf8_lossy(tag)))
    }
}

/// The byte offset of each glyph in the glyph table, with one more for the end.
fn glyph_offsets(loca: &[u8], format: i16, glyph_count: u32) -> Result<Vec<usize>, String> {
    let mut offsets = Vec::with_capacity(glyph_count as usize + 1);
    for i in 0..=glyph_count as usize {
        let offset = match format {
            0 => 2 * u16_at(loca, 2 * i)? as usize,
            1 => u32_at(loca, 4 * i)? as usize,
            other => return Err(format!("unknown loca format {other}")),
        };
        offsets.push(offset);
    }
    Ok(offsets)
}

/// The glyphs to keep: those asked for, glyph 0, and the glyphs that the composite glyphs among
/// them are built from, which can be composites themselves.
fn needed_glyphs(
    glyf: &[u8],
    offsets: &[usize],
    asked: &BTreeSet<u32>,
    glyph_count: u32,
) -> Result<BTreeSet<u32>, String> {
    let mut needed = BTreeSet::new();
    let mut pending: Vec<u32> = asked.iter().copied().filter(|g| *g < glyph_count).collect();
    pending.push(0);
    while let Some(gid) = pending.pop() {
        if !needed.insert(gid) {
            continue;
        }
        let (start, end) = (offsets[gid as usize], offsets[gid as usize + 1]);
        if end <= start {
            continue;
        }
        let data = glyf
            .get(start..end)
            .ok_or_else(|| format!("glyph {gid} is outside the glyph table"))?;
        if i16_at(data, 0)? < 0 {
            pending.extend(component_glyphs(data)?);
        }
    }
    Ok(needed)
}

/// The glyph ids a composite glyph's components refer to.
fn component_glyphs(data: &[u8]) -> Result<Vec<u32>, String> {
    const ARG_WORDS: u16 = 0x0001;
    const MORE: u16 = 0x0020;
    const SCALE: u16 = 0x0008;
    const XY_SCALE: u16 = 0x0040;
    const TWO_BY_TWO: u16 = 0x0080;

    let mut out = Vec::new();
    let mut at = 10;
    loop {
        let flags = u16_at(data, at)?;
        out.push(u16_at(data, at + 2)? as u32);
        at += 4;
        at += if flags & ARG_WORDS != 0 { 4 } else { 2 };
        if flags & TWO_BY_TWO != 0 {
            at += 8;
        } else if flags & XY_SCALE != 0 {
            at += 4;
        } else if flags & SCALE != 0 {
            at += 2;
        }
        if flags & MORE == 0 {
            return Ok(out);
        }
    }
}

/// The font file for the tables in `out`, with a directory sorted by tag and checksums set.
fn assemble(font: &[u8], mut out: Vec<(&[u8; 4], Vec<u8>)>) -> Vec<u8> {
    out.sort_by(|a, b| a.0.cmp(b.0));
    let count = out.len() as u16;
    let mut search_range: u16 = 1;
    let mut entry_selector: u16 = 0;
    while search_range * 2 <= count {
        search_range *= 2;
        entry_selector += 1;
    }
    let search_range = search_range * 16;
    let range_shift = count * 16 - search_range;

    let scaler: [u8; 4] = font
        .get(0..4)
        .and_then(|b| b.try_into().ok())
        .unwrap_or([0, 1, 0, 0]);
    let mut file = Vec::new();
    file.extend_from_slice(&scaler);
    file.extend_from_slice(&count.to_be_bytes());
    file.extend_from_slice(&search_range.to_be_bytes());
    file.extend_from_slice(&entry_selector.to_be_bytes());
    file.extend_from_slice(&range_shift.to_be_bytes());

    let directory_end = 12 + 16 * out.len();
    let mut offset = directory_end;
    let mut records = Vec::new();
    let mut body = Vec::new();
    for (tag, data) in &out {
        let checksum = checksum(data);
        records.push((*tag, checksum, offset as u32, data.len() as u32));
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = directory_end + body.len();
    }
    for (tag, sum, at, length) in &records {
        file.extend_from_slice(*tag);
        file.extend_from_slice(&sum.to_be_bytes());
        file.extend_from_slice(&at.to_be_bytes());
        file.extend_from_slice(&length.to_be_bytes());
    }
    file.extend_from_slice(&body);

    // The head table's checksum adjustment makes the whole file sum to a fixed value.
    if let Some(index) = records.iter().position(|(tag, ..)| *tag == b"head") {
        let (_, _, head_at, head_len) = records[index];
        let head_at = head_at as usize;
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&file));
        file[head_at + 8..head_at + 12].copy_from_slice(&adjustment.to_be_bytes());
        // The head record's checksum covers the adjustment that was just written.
        let head_sum = checksum(&file[head_at..head_at + head_len as usize]);
        let record = 12 + 16 * index + 4;
        file[record..record + 4].copy_from_slice(&head_sum.to_be_bytes());
    }
    file
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| "the font is cut short".to_string())
}

fn i16_at(data: &[u8], at: usize) -> Result<i16, String> {
    u16_at(data, at).map(|v| v as i16)
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "the font is cut short".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_text::font::FontFace;
    use tpt_text::fx::Fx;

    const REGULAR: &[u8] = include_bytes!("../../tpt-codex-text/fonts/NotoSans-Regular.ttf");

    fn outline(face: &FontFace, glyph: u32) -> String {
        let size = Fx::from_int(i32::from(face.units_per_em()));
        format!("{:?}", face.glyph_outline(glyph, size))
    }

    #[test]
    fn the_subset_is_much_smaller_and_keeps_the_outlines_of_its_glyphs() {
        let original = FontFace::from_font_data(REGULAR.to_vec(), 0).unwrap();
        let wanted: BTreeSet<u32> = "Hello, world"
            .chars()
            .filter_map(|c| original.glyph_index(c))
            .collect();
        let subset = subset_truetype(REGULAR, &wanted).unwrap();
        assert!(
            subset.len() * 10 < REGULAR.len(),
            "{} bytes, from {}",
            subset.len(),
            REGULAR.len()
        );
        let parsed = FontFace::from_font_data(subset, 0).unwrap();
        for gid in &wanted {
            assert_eq!(
                outline(&parsed, *gid),
                outline(&original, *gid),
                "glyph {gid}"
            );
        }
    }

    #[test]
    fn a_glyph_that_is_not_asked_for_has_no_outline() {
        let original = FontFace::from_font_data(REGULAR.to_vec(), 0).unwrap();
        let h = original.glyph_index('H').unwrap();
        let w = original.glyph_index('w').unwrap();
        let subset = subset_truetype(REGULAR, &BTreeSet::from([h])).unwrap();
        let parsed = FontFace::from_font_data(subset, 0).unwrap();
        assert_eq!(outline(&parsed, h), outline(&original, h));
        assert_eq!(outline(&parsed, w), "[]");
    }
}
