// SPDX-License-Identifier: MIT OR Apache-2.0
//! Box layout of a parsed formula.
//!
//! Each node becomes a box with a width, an ascent and a descent (both measured from the
//! baseline), and the glyphs and rules that draw it. Fractions and roots get rules. Scripts are
//! drawn raised or lowered from the baseline.
//!
//! Metrics come from a [`MathMetrics`]. This module does not look up fonts, so the same formula
//! lays out the same way wherever the metrics agree. Proportions and shifts come from a
//! [`MathConstants`]: a metrics source that has a font's MATH table returns its values, and one
//! that has none gets the fixed fallbacks in [`MathConstants::default`].

use alloc::vec::Vec;

use crate::math::MathNode;

/// Character metrics for math layout. All distances are in the same unit as the size.
pub trait MathMetrics {
    /// The advance width of `ch` at `size`.
    fn advance(&self, ch: char, size: f64) -> f64;
    /// Height of the tallest glyph above the baseline, at `size`.
    fn ascent(&self, size: f64) -> f64;
    /// Depth of the deepest glyph below the baseline, at `size`.
    fn descent(&self, size: f64) -> f64;
    /// Layout constants for this font, when it has them (a MATH table). `None` uses
    /// [`MathConstants::default`].
    fn math_constants(&self) -> Option<MathConstants> {
        None
    }
}

/// Layout constants for math, as fractions of the size, except where a field says otherwise.
///
/// A font's MATH table gives these in font units; [`crate::math_font::math_constants`] converts
/// them. The defaults are the fixed proportions the layout used before the table was read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathConstants {
    /// Script size as a fraction of the base size.
    pub script_scale: f64,
    /// Height of the axis (the fraction bar's centre) above the baseline.
    pub axis_height: f64,
    /// Thickness of fraction bars.
    pub fraction_rule: f64,
    /// Minimum gap between a numerator and the bar.
    pub numerator_gap: f64,
    /// Minimum gap between the bar and a denominator.
    pub denominator_gap: f64,
    /// Minimum numerator baseline shift above the baseline, when the font sets one.
    pub numerator_shift_up: Option<f64>,
    /// Minimum denominator baseline shift below the baseline, when the font sets one.
    pub denominator_shift_down: Option<f64>,
    /// Raise of a superscript.
    pub superscript_shift_up: f64,
    /// Drop of a subscript.
    pub subscript_shift_down: f64,
    /// Thickness of root overlines.
    pub radical_rule: f64,
    /// Gap between a root's argument and its overline.
    pub radical_gap: f64,
}

impl Default for MathConstants {
    fn default() -> Self {
        Self {
            script_scale: 0.7,
            axis_height: 0.25,
            fraction_rule: 0.04,
            numerator_gap: 0.1,
            denominator_gap: 0.1,
            numerator_shift_up: None,
            denominator_shift_down: None,
            superscript_shift_up: 0.4,
            subscript_shift_down: 0.2,
            radical_rule: 0.04,
            radical_gap: 0.1,
        }
    }
}

/// Something drawn by a math box. Positions are relative to the box's left edge and baseline,
/// with y growing upwards.
#[derive(Debug, Clone, PartialEq)]
pub enum MathItem {
    /// One character at a baseline position.
    Glyph {
        /// Left edge of the glyph.
        x: f64,
        /// Baseline height.
        y: f64,
        /// The character.
        ch: char,
        /// Font size of the glyph.
        size: f64,
    },
    /// A filled rectangle, for fraction bars and root overlines.
    Rule {
        /// Left edge.
        x: f64,
        /// Bottom edge.
        y: f64,
        /// Width.
        width: f64,
        /// Height (thickness).
        height: f64,
    },
}

/// The layout of a formula or part of one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MathBox {
    /// Total width.
    pub width: f64,
    /// Height above the baseline.
    pub ascent: f64,
    /// Depth below the baseline, positive.
    pub descent: f64,
    /// What to draw.
    pub items: Vec<MathItem>,
}

/// Space on each side of an operator, as a fraction of the size.
const OP_SPACE: f64 = 0.2;
/// Space after a script, as a fraction of the size.
const SCRIPT_SPACE: f64 = 0.05;

/// Lays out `nodes` at `size`, with the constants the metrics give (or the defaults).
pub fn layout_math(nodes: &[MathNode], metrics: &dyn MathMetrics, size: f64) -> MathBox {
    let k = metrics.math_constants().unwrap_or_default();
    hbox(nodes, metrics, &k, size)
}

/// Lays out a sequence side by side on one baseline.
fn hbox(nodes: &[MathNode], metrics: &dyn MathMetrics, k: &MathConstants, size: f64) -> MathBox {
    let mut out = MathBox::default();
    for node in nodes {
        let child = node_box(node, metrics, k, size);
        out.append(child, 0.0);
    }
    out
}

impl MathBox {
    /// Places `child` after this box, with `space` before it.
    fn append(&mut self, child: MathBox, space: f64) {
        let x0 = self.width + space;
        for mut item in child.items {
            shift_x(&mut item, x0);
            self.items.push(item);
        }
        self.width = x0 + child.width;
        self.ascent = self.ascent.max(child.ascent);
        self.descent = self.descent.max(child.descent);
    }
}

fn shift_x(item: &mut MathItem, dx: f64) {
    match item {
        MathItem::Glyph { x, .. } | MathItem::Rule { x, .. } => *x += dx,
    }
}

fn shift_y(item: &mut MathItem, dy: f64) {
    match item {
        MathItem::Glyph { y, .. } | MathItem::Rule { y, .. } => *y += dy,
    }
}

/// The box of one node.
fn node_box(node: &MathNode, metrics: &dyn MathMetrics, k: &MathConstants, size: f64) -> MathBox {
    match node {
        MathNode::Number(text) => glyphs(text.chars(), metrics, size),
        MathNode::Ident(c) => glyphs([*c].into_iter(), metrics, size),
        MathNode::Symbol { glyph, .. } => glyphs([*glyph].into_iter(), metrics, size),
        MathNode::Operator(c) => {
            let mut b = glyphs([*c].into_iter(), metrics, size);
            // Operators have space on both sides.
            let space = OP_SPACE * size;
            for item in &mut b.items {
                shift_x(item, space);
            }
            b.width += 2.0 * space;
            b
        }
        MathNode::Group(items) => hbox(items, metrics, k, size),
        MathNode::Frac { num, den } => fraction(num, den, metrics, k, size),
        MathNode::Sqrt(arg) => root(arg, metrics, k, size),
        MathNode::Script { base, sup, sub } => {
            script(base, sup.as_deref(), sub.as_deref(), metrics, k, size)
        }
    }
}

/// Characters in a row, on one baseline.
fn glyphs(chars: impl Iterator<Item = char>, metrics: &dyn MathMetrics, size: f64) -> MathBox {
    let mut out = MathBox {
        width: 0.0,
        ascent: metrics.ascent(size),
        descent: metrics.descent(size),
        items: Vec::new(),
    };
    for ch in chars {
        let advance = metrics.advance(ch, size);
        out.items.push(MathItem::Glyph {
            x: out.width,
            y: 0.0,
            ch,
            size,
        });
        out.width += advance;
    }
    out
}

fn fraction(
    num: &[MathNode],
    den: &[MathNode],
    metrics: &dyn MathMetrics,
    k: &MathConstants,
    size: f64,
) -> MathBox {
    let script = size * k.script_scale;
    let num = hbox(num, metrics, k, script);
    let den = hbox(den, metrics, k, script);
    let thick = k.fraction_rule * size;
    let width = num.width.max(den.width);
    let mut out = MathBox {
        width,
        ascent: 0.0,
        descent: 0.0,
        items: Vec::new(),
    };
    // Baselines of the parts: the numerator sits above the bar, the denominator below. The
    // font's shifts, when it has them, are minimums; the gaps keep the parts clear of the bar.
    let bar_bottom = k.axis_height * size - thick / 2.0;
    let bar_top = k.axis_height * size + thick / 2.0;
    let mut y_num = bar_top + k.numerator_gap * size + num.descent;
    if let Some(shift) = k.numerator_shift_up {
        y_num = y_num.max(shift * size);
    }
    let mut y_den = bar_bottom - k.denominator_gap * size - den.ascent;
    if let Some(shift) = k.denominator_shift_down {
        y_den = y_den.min(-shift * size);
    }
    out.ascent = y_num + num.ascent;
    out.descent = den.descent - y_den;

    let num_x = (width - num.width) / 2.0;
    let den_x = (width - den.width) / 2.0;
    for mut item in num.items {
        shift_x(&mut item, num_x);
        shift_y(&mut item, y_num);
        out.items.push(item);
    }
    for mut item in den.items {
        shift_x(&mut item, den_x);
        shift_y(&mut item, y_den);
        out.items.push(item);
    }
    out.items.push(MathItem::Rule {
        x: 0.0,
        y: bar_bottom,
        width,
        height: thick,
    });
    out
}

fn root(arg: &[MathNode], metrics: &dyn MathMetrics, k: &MathConstants, size: f64) -> MathBox {
    let arg = hbox(arg, metrics, k, size);
    let thick = k.radical_rule * size;
    let gap = k.radical_gap * size;
    let sign_advance = metrics.advance('√', size);
    let mut out = MathBox {
        width: 0.0,
        ascent: arg.ascent + gap + thick,
        descent: arg.descent,
        items: Vec::new(),
    };
    out.items.push(MathItem::Glyph {
        x: 0.0,
        y: 0.0,
        ch: '√',
        size,
    });
    out.width = sign_advance;
    // The argument sits after the sign, under the overline.
    out.append(arg, 0.0);
    let arg_width = out.width - sign_advance;
    out.items.push(MathItem::Rule {
        x: sign_advance,
        y: out.ascent - thick,
        width: arg_width,
        height: thick,
    });
    out
}

fn script(
    base: &MathNode,
    sup: Option<&[MathNode]>,
    sub: Option<&[MathNode]>,
    metrics: &dyn MathMetrics,
    k: &MathConstants,
    size: f64,
) -> MathBox {
    let base = node_box(base, metrics, k, size);
    let script_size = size * k.script_scale;
    let sup = sup.map(|n| hbox(n, metrics, k, script_size));
    let sub = sub.map(|n| hbox(n, metrics, k, script_size));
    let sup_y = k.superscript_shift_up * size;
    let sub_y = -k.subscript_shift_down * size;
    let scripts_width = sup
        .as_ref()
        .map_or(0.0, |b| b.width)
        .max(sub.as_ref().map_or(0.0, |b| b.width));
    let mut out = MathBox {
        width: 0.0,
        ascent: base.ascent,
        descent: base.descent,
        items: Vec::new(),
    };
    out.append(base, 0.0);
    if scripts_width > 0.0 {
        let x = out.width + SCRIPT_SPACE * size;
        if let Some(mut b) = sup {
            out.ascent = out.ascent.max(sup_y + b.ascent);
            for item in &mut b.items {
                shift_x(item, x);
                shift_y(item, sup_y);
            }
            out.items.extend(b.items);
        }
        if let Some(mut b) = sub {
            out.descent = out.descent.max(-sub_y + b.descent);
            for item in &mut b.items {
                shift_x(item, x);
                shift_y(item, sub_y);
            }
            out.items.extend(b.items);
        }
        out.width = x + scripts_width;
    }
    out
}
