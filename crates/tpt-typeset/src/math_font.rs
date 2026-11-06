// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layout constants read from a font's MATH table.
//!
//! The bundled font is STIX Two Math 2.13 under the SIL Open Font License (see
//! `assets/README.md`). Its MATH table gives the axis height, rule thickness, shifts and gaps
//! that [`MathConstants`] holds. Values are converted from font units to fractions of the size.

use crate::math_layout::MathConstants;

/// STIX Two Math Regular, the font whose MATH table sets formula layout.
pub const STIX_TWO_MATH: &[u8] = include_bytes!("../assets/STIXTwoMath-Regular.otf");

/// Reads [`MathConstants`] from the MATH table of the font in `data`. Returns `None` when the
/// font cannot be parsed or has no MATH constants, so the caller can use the defaults.
pub fn math_constants(data: &[u8]) -> Option<MathConstants> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let constants = face.tables().math?.constants?;
    let units = f64::from(face.units_per_em().max(1));
    let em = |value: i16| f64::from(value) / units;
    Some(MathConstants {
        script_scale: f64::from(constants.script_percent_scale_down()) / 100.0,
        axis_height: em(constants.axis_height().value),
        fraction_rule: em(constants.fraction_rule_thickness().value),
        numerator_gap: em(constants.fraction_numerator_gap_min().value),
        denominator_gap: em(constants.fraction_denominator_gap_min().value),
        numerator_shift_up: Some(em(constants.fraction_numerator_shift_up().value)),
        denominator_shift_down: Some(em(constants.fraction_denominator_shift_down().value)),
        superscript_shift_up: em(constants.superscript_shift_up().value),
        subscript_shift_down: em(constants.subscript_shift_down().value),
        radical_rule: em(constants.radical_rule_thickness().value),
        radical_gap: em(constants.radical_vertical_gap().value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stix_two_math_has_constants() {
        let k = math_constants(STIX_TWO_MATH).expect("MATH table");
        // The axis sits a little above the baseline, and the shifts are positive.
        assert!(k.axis_height > 0.0 && k.axis_height < 0.5, "{k:?}");
        assert!(k.superscript_shift_up > 0.0, "{k:?}");
        assert!(k.subscript_shift_down > 0.0, "{k:?}");
        assert!(k.script_scale > 0.0 && k.script_scale < 1.0, "{k:?}");
        assert!(k.fraction_rule > 0.0, "{k:?}");
    }

    #[test]
    fn a_font_without_a_math_table_gives_none() {
        assert_eq!(math_constants(b"not a font"), None);
    }
}
