//! The label model, and the renderer that turns a label into the dots the printer burns.
//!
//! A [`Label`] is a piece of label stock plus what is on it. [`render`] draws it into a [`Rendered`]
//! label, which gives both the preview (what the label will look like) and the [`Raster`] to send to
//! the printer. They come from the same dots, so the preview is exactly what is printed.

use std::str::FromStr;

use image::{imageops, GrayImage, Luma};
use imageproc::drawing::{draw_text_mut, text_size};
use rusttype::{point, Font, Scale};

use crate::{
    fonts::{has_glyph, FontSpec, Fonts},
    picture::{Picture, Tone},
    protocol::{Raster, MAX_ROWS},
    stock::{mm_to_dots, LabelStock, TapePlan, PRINT_WIDTH_DOTS},
    D30Error, D30Scale,
};

/// How the lines of a block of text line up with each other and with the label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

impl FromStr for Align {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "left" | "l" => Ok(Align::Left),
            "center" | "centre" | "c" | "middle" => Ok(Align::Center),
            "right" | "r" => Ok(Align::Right),
            other => Err(format!(
                "unknown alignment `{other}`: use left, center or right"
            )),
        }
    }
}

/// How a block of text is divided into lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lines {
    /// Where the text has a new line, and nowhere else.
    #[default]
    AsTyped,
    /// Divide the words over this many lines, so that the widest line is as narrow as it can be,
    /// which lets the text be as large as it can be. New lines in the text count as spaces. A word
    /// is never broken, so a text of three words has three lines at the most.
    Split(usize),
}

/// A block of text of one or more lines.
#[derive(Clone, Debug)]
pub struct TextElement {
    pub text: String,
    pub font: FontSpec,
    /// A size in dots, or [`D30Scale::Auto`] to make the text as large as fits the label.
    pub size: D30Scale,
    pub align: Align,
    /// The space left free around the text when it is sized automatically, and the distance from
    /// the edge of the label for left and right aligned text, in dots.
    pub margin: f32,
    /// Where the lines break.
    pub lines: Lines,
}

impl TextElement {
    /// Centered text in the built-in font that fills the label, like `d30-cli print-text`.
    pub fn new(text: impl Into<String>) -> Self {
        TextElement {
            text: text.into(),
            font: FontSpec::default(),
            size: D30Scale::Auto { minus: 0.0 },
            align: Align::Center,
            margin: 15.0,
            lines: Lines::AsTyped,
        }
    }
}

/// Which end of the label a picture is at, with the text in between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Left,
    Right,
}

impl FromStr for Side {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "left" | "l" => Ok(Side::Left),
            "right" | "r" => Ok(Side::Right),
            other => Err(format!("unknown side `{other}`: use left or right")),
        }
    }
}

/// A picture on a label. It is put beside the text, at one end of the label, and takes as much of
/// the label as it needs: the text gets what is left. Without any text the picture is centered.
/// On a label of a set length that has text, the pictures together take at most 60% of its length,
/// so that the text always has some room.
#[derive(Clone, Debug)]
pub struct PictureElement {
    pub picture: Picture,
    pub side: Side,
    /// How the grays of the picture become dots.
    pub tone: Tone,
    /// Swap black and white.
    pub invert: bool,
    /// How high it is, in dots, or `None` for as high as the label allows, less the margin. It keeps
    /// its shape either way.
    pub height: Option<f32>,
    /// The space left free above and below the picture and between it and the edge of the label, in
    /// dots.
    pub margin: f32,
}

impl PictureElement {
    /// A picture at the left end of the label, as high as it can be, in black and white.
    pub fn new(picture: Picture) -> Self {
        PictureElement {
            picture,
            side: Side::Left,
            tone: Tone::default(),
            invert: false,
            height: None,
            margin: 15.0,
        }
    }
}

/// One thing on a label.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Element {
    Text(TextElement),
    Picture(PictureElement),
}

/// A label: a kind of label stock and what is on it.
#[derive(Clone, Debug)]
pub struct Label {
    pub stock: LabelStock,
    /// How long to make the label, in millimetres. Only continuous tape has a length to choose;
    /// `None` cuts it to fit what is on it.
    pub length_mm: Option<f32>,
    pub elements: Vec<Element>,
}

impl Label {
    pub fn new(stock: LabelStock) -> Self {
        Label {
            stock,
            length_mm: None,
            elements: Vec::new(),
        }
    }

    /// A label with one block of text.
    pub fn text(stock: LabelStock, text: impl Into<String>) -> Self {
        let mut label = Label::new(stock);
        label.elements.push(Element::Text(TextElement::new(text)));
        label
    }
}

/// A label drawn into dots, the way it reads: as long as the label and as high as the print head.
#[derive(Clone, Debug)]
pub struct Rendered {
    canvas: GrayImage,
}

impl Rendered {
    /// The dots, with 255 where ink goes and 0 elsewhere (anything brighter than 127 is printed).
    pub fn canvas(&self) -> &GrayImage {
        &self.canvas
    }

    /// How long the label is, in dots.
    pub fn length_dots(&self) -> u32 {
        self.canvas.width()
    }

    /// What the printed label will look like: black dots on white. It is the very bitmap that is
    /// sent to the printer, not an anti-aliased picture of it.
    pub fn preview(&self) -> GrayImage {
        GrayImage::from_fn(self.canvas.width(), self.canvas.height(), |x, y| {
            if self.canvas.get_pixel(x, y)[0] > 127 {
                Luma([0])
            } else {
                Luma([255])
            }
        })
    }

    /// The dots in the form the printer takes them: the label turned a quarter turn, because the
    /// tape comes out of the printer sideways to how the label reads.
    pub fn raster(&self) -> Result<Raster, D30Error> {
        Raster::from_gray(&imageops::rotate270(&self.canvas))
    }

    /// The rows to send for one copy of the label on the tape as `plan` lays it out: the label
    /// without the rows at its front that cannot be printed, with the blank rows before and after
    /// it. See [`LabelStock::tape_plan`].
    pub fn raster_for(&self, plan: &TapePlan) -> Result<Raster, D30Error> {
        self.raster()?
            .without_first_rows(plan.skip_rows)?
            .with_lead_rows(plan.lead_rows)?
            .with_blank_rows(plan.feed_rows)
    }
}

/// Draws `label`, using `fonts` to find its fonts.
///
/// The elements lie in a row along the label: the pictures at the left, then the text, then the
/// pictures at the right. The pictures take the room they need, which is how high the label allows
/// times their shape, and the text is made as large as fits what is left. On uncut tape that is cut
/// to fit its content, the label is as long as all of that together.
pub fn render(label: &Label, fonts: &Fonts) -> Result<Rendered, D30Error> {
    let height = PRINT_WIDTH_DOTS;
    let fixed_length = fixed_length(label)?;
    // How far from each end of the label the content has to stay: where the label's front cannot be
    // printed, that part, and the same at the other end so that the content is in the middle.
    let keep_clear = label.stock.dead_front_rows() as f32;

    let texts: Vec<&TextElement> = label
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    let mut pictures: Vec<SizedPicture> = label
        .elements
        .iter()
        .filter_map(|element| match element {
            Element::Picture(picture) => Some(SizedPicture::new(
                picture,
                height,
                fixed_length,
                keep_clear,
            )),
            _ => None,
        })
        .collect();

    // Pictures without any text to speak of stand alone, in the middle.
    let alone = !pictures.is_empty() && texts.iter().all(|text| text.text.trim().is_empty());
    // On a label of a set length, the text keeps a share of it, however large the pictures are.
    if let (false, Some(length)) = (alone, fixed_length) {
        limit_pictures(&mut pictures, length);
    }
    let (left, right): (Vec<&SizedPicture>, Vec<&SizedPicture>) = if alone {
        (Vec::new(), Vec::new())
    } else {
        pictures
            .iter()
            .partition(|picture| picture.element.side == Side::Left)
    };
    let left_extent: u64 = left.iter().map(|picture| picture.extent()).sum();
    let right_extent: u64 = right.iter().map(|picture| picture.extent()).sum();

    // What is left of a label that has a length of its own, for the text.
    let text_room = fixed_length
        .map(|length| (length as f32 - left_extent as f32 - right_extent as f32).max(1.0));
    // A text is kept clear of the ends of the label by its margin. Where that is less than has to
    // be kept clear, its room is made smaller by the difference (at an end with a picture, the
    // picture's margin has done that already).
    let insets = |text: &TextElement| {
        let more = (keep_clear - text.margin.max(0.0)).max(0.0);
        (
            if left.is_empty() { more } else { 0.0 },
            if right.is_empty() { more } else { 0.0 },
        )
    };
    let mut blocks = Vec::new();
    if !alone {
        for text in &texts {
            let (inset_left, inset_right) = insets(text);
            let room = text_room.map(|room| (room - inset_left - inset_right).max(1.0));
            blocks.push((
                *text,
                inset_left,
                inset_right,
                lay_out(text, fonts, room, height as f32)?,
            ));
        }
    }

    // Pictures that stand alone are a row, with a margin before each but the first.
    let row_width: u64 = pictures
        .iter()
        .map(|picture| u64::from(picture.width))
        .sum::<u64>()
        + pictures
            .iter()
            .skip(1)
            .map(|picture| u64::from(picture.margin))
            .sum::<u64>();
    let length: u64 = match fixed_length {
        Some(length) => u64::from(length),
        // Uncut tape without a chosen length is cut to fit what is on it.
        None if alone => row_width + 2 * u64::from(pictures[0].margin),
        None => {
            let text = blocks
                .iter()
                .map(|(text, _, _, block)| (block.width as f32 + 2.0 * text.margin).ceil() as u64)
                .max()
                .unwrap_or(1)
                .max(1);
            left_extent + right_extent + text
        }
    };
    if length > MAX_ROWS as u64 {
        return Err(invalid(format!(
            "the label would be {length} dots ({:.0} mm) long, but the printer takes at most {MAX_ROWS} dots",
            length as f32 / crate::stock::DOTS_PER_MM
        )));
    }

    let mut canvas = GrayImage::new(length as u32, height);
    if alone {
        let mut left_edge = (length as i64 - row_width as i64) / 2;
        for (index, picture) in pictures.iter().enumerate() {
            if index > 0 {
                left_edge += i64::from(picture.margin);
            }
            draw_picture(&mut canvas, picture, left_edge);
            left_edge += i64::from(picture.width);
        }
    } else {
        let text_width = (length as f32 - left_extent as f32 - right_extent as f32).max(1.0);
        for (text, inset_left, inset_right, block) in &blocks {
            draw(
                &mut canvas,
                text,
                block,
                left_extent as f32 + inset_left,
                (text_width - inset_left - inset_right).max(1.0),
            );
        }
        let mut left_edge = 0;
        for picture in &left {
            left_edge += i64::from(picture.margin);
            draw_picture(&mut canvas, picture, left_edge);
            left_edge += i64::from(picture.width);
        }
        // The pictures at the right are in the order they are given, the last one at the very end.
        let mut right_edge = length as i64;
        for picture in right.iter().rev() {
            right_edge -= i64::from(picture.margin) + i64::from(picture.width);
            draw_picture(&mut canvas, picture, right_edge);
        }
    }
    Ok(Rendered { canvas })
}

/// A picture with the size it gets on the label worked out.
struct SizedPicture<'a> {
    element: &'a PictureElement,
    width: u32,
    height: u32,
    /// The space on the outer side of it, between it and the edge of the label or the picture
    /// before it.
    margin: u32,
}

impl<'a> SizedPicture<'a> {
    /// `band` is how high the label is, `fixed_length` how long, if it has a length of its own.
    /// `keep_clear` is how far from the ends of the label the picture has to stay at least, which
    /// can be more than its margin (the margin above and below the picture is its own).
    fn new(
        element: &'a PictureElement,
        band: u32,
        fixed_length: Option<u32>,
        keep_clear: f32,
    ) -> Self {
        // (`max` and `clamp` leave a NaN out, which is how a size that is not a number ends up as
        // the smallest there is.)
        let margin = element.margin.max(0.0);
        let band = band as f32;
        let wanted = match element.height {
            Some(height) => height.clamp(1.0, band),
            None => (band - 2.0 * margin).max(1.0),
        };
        let shape = element.picture.width() as f32 / element.picture.height() as f32;
        let mut height = wanted.round().max(1.0);
        let mut width = (height * shape).round().max(1.0);
        // On a label of fixed length the picture is never wider than the label (less its margin, or
        // less what has to be kept clear at both ends if that is more).
        if let Some(length) = fixed_length {
            let room = (length as f32 - margin.max(2.0 * keep_clear))
                .floor()
                .max(1.0);
            if width > room {
                width = room;
                height = (width / shape).round().max(1.0);
            }
        }
        SizedPicture {
            element,
            width: width as u32,
            height: height as u32,
            margin: margin.max(keep_clear).round() as u32,
        }
    }

    /// How much of the length of the label the picture takes: itself and its margin.
    fn extent(&self) -> u64 {
        u64::from(self.margin) + u64::from(self.width)
    }
}

/// How much of the length of a label of a set length pictures may take together, when there is
/// text on it too. The text needs the rest; without it a wide picture would leave it no room at
/// all, and it would shrink to nothing.
const PICTURES_SHARE: f32 = 0.6;

/// Makes pictures smaller, all by the same factor, if together with their margins they take more
/// than their share of a label that is `length` dots long.
fn limit_pictures(pictures: &mut [SizedPicture], length: u32) {
    let budget = (length as f32 * PICTURES_SHARE).floor();
    let margins: f32 = pictures.iter().map(|picture| picture.margin as f32).sum();
    let widths: f32 = pictures.iter().map(|picture| picture.width as f32).sum();
    if margins + widths <= budget {
        return;
    }
    let factor = ((budget - margins).max(1.0) / widths).min(1.0);
    for picture in pictures {
        picture.width = ((picture.width as f32 * factor).floor() as u32).max(1);
        picture.height = ((picture.height as f32 * factor).round() as u32).max(1);
    }
}

/// Burns a picture into the label, centered up and down, with its left edge at `left` (which can
/// be outside the label: what falls outside it is not drawn).
fn draw_picture(canvas: &mut GrayImage, picture: &SizedPicture, left: i64) {
    let element = picture.element;
    let dots = element
        .picture
        .dots(picture.width, picture.height, element.tone, element.invert);
    let top = (i64::from(canvas.height()) - i64::from(picture.height)) / 2;
    for (x, y, dot) in dots.enumerate_pixels() {
        if dot.0[0] > 127 {
            let (x, y) = (left + i64::from(x), top + i64::from(y));
            if (0..i64::from(canvas.width())).contains(&x)
                && (0..i64::from(canvas.height())).contains(&y)
            {
                canvas.put_pixel(x as u32, y as u32, Luma([255]));
            }
        }
    }
}

/// The length of the label in dots if it is fixed, or `None` if it should fit its content.
fn fixed_length(label: &Label) -> Result<Option<u32>, D30Error> {
    match (label.stock.length_dots(), label.length_mm) {
        (Some(dots), None) => Ok(Some(dots)),
        (Some(_), Some(_)) => Err(invalid(format!(
            "the length of {} is fixed. Only continuous tape has a length to choose",
            label.stock.name
        ))),
        (None, Some(mm)) => {
            let dots = mm_to_dots(mm);
            if mm.is_nan() || dots == 0 {
                return Err(invalid(format!("a label {mm} mm long is too short")));
            }
            Ok(Some(dots))
        }
        (None, None) => Ok(None),
    }
}

fn invalid(reason: String) -> D30Error {
    D30Error::InvalidLabel { reason }
}

/// The size, in dots, that text is measured at to decide how large to make it and where to break
/// its lines. It is big, so that the rounding of the sizes of glyphs hardly matters.
const MEASURING_SIZE: f32 = 100.0;

/// The font of a block of text, and the fonts that draw what it has no glyph for (emoji, mostly).
///
/// A line is cut into runs, each in the font that has its glyphs, set one after the other with
/// their baselines level. A line that the font of the label can draw all by itself is one run in
/// that font, which comes out exactly as if there were no other fonts.
#[derive(Clone)]
struct Face {
    primary: Font<'static>,
    fallbacks: Vec<Font<'static>>,
}

impl Face {
    /// The face to draw `text` with. The fallback fonts only come along if a character needs one.
    fn new(primary: Font<'static>, fonts: &Fonts, text: &str) -> Face {
        let fallbacks = if text.chars().any(|c| !has_glyph(&primary, c)) {
            fonts.fallbacks().to_vec()
        } else {
            Vec::new()
        };
        Face { primary, fallbacks }
    }

    /// 0 for the font of the label, or 1 and up for a fallback: whichever draws `c`. Where none does,
    /// the font of the label, which draws the box that fonts draw for what they lack.
    fn font_index(&self, c: char) -> usize {
        if has_glyph(&self.primary, c) {
            return 0;
        }
        self.fallbacks
            .iter()
            .position(|font| has_glyph(font, c))
            .map_or(0, |position| position + 1)
    }

    fn font(&self, index: usize) -> &Font<'static> {
        match index {
            0 => &self.primary,
            _ => &self.fallbacks[index - 1],
        }
    }

    /// The text cut into runs: which font, and the part of the text.
    fn runs<'t>(&self, text: &'t str) -> Vec<(usize, &'t str)> {
        let mut runs = Vec::new();
        let mut start = 0;
        let mut current = None;
        for (position, c) in text.char_indices() {
            let index = self.font_index(c);
            match current {
                Some(font) if font == index => {}
                Some(font) => {
                    runs.push((font, &text[start..position]));
                    start = position;
                    current = Some(index);
                }
                None => current = Some(index),
            }
        }
        if let Some(font) = current {
            runs.push((font, &text[start..]));
        }
        runs
    }

    fn v_metrics(&self, scale: Scale) -> rusttype::VMetrics {
        self.primary.v_metrics(scale)
    }

    /// How far down a run in this font is set, so that its baseline is the line's.
    fn drop(&self, font: &Font<'static>, scale: Scale) -> i32 {
        (self.primary.v_metrics(scale).ascent - font.v_metrics(scale).ascent).round() as i32
    }

    /// The width and height of the ink of a line.
    fn text_size(&self, scale: Scale, text: &str) -> (i32, i32) {
        if self.fallbacks.is_empty() {
            return text_size(scale, &self.primary, text);
        }
        let (mut pen, mut width, mut height) = (0.0_f32, 0, 0);
        for (index, run) in self.runs(text) {
            let font = self.font(index);
            let (run_width, run_height) = text_size(scale, font, run);
            width = width.max((pen + run_width as f32).ceil() as i32);
            height = height.max(self.drop(font, scale) + run_height);
            pen += advance(font, scale, run);
        }
        (width, height)
    }

    /// How far the pen moves while the line is set: where the next glyph would start.
    fn advance(&self, scale: Scale, text: &str) -> f32 {
        if self.fallbacks.is_empty() {
            return advance(&self.primary, scale, text);
        }
        self.runs(text)
            .into_iter()
            .map(|(index, run)| advance(self.font(index), scale, run))
            .sum()
    }

    /// Draws a line with the top of its first line at `y`, starting at `x`.
    fn draw(&self, canvas: &mut GrayImage, x: i32, y: i32, scale: Scale, text: &str) {
        if self.fallbacks.is_empty() {
            draw_text_mut(canvas, Luma([255u8]), x, y, scale, &self.primary, text);
            return;
        }
        let mut pen = 0.0_f32;
        for (index, run) in self.runs(text) {
            let font = self.font(index);
            let at = (x + pen.round() as i32, y + self.drop(font, scale));
            draw_text_mut(canvas, Luma([255u8]), at.0, at.1, scale, font, run);
            pen += advance(font, scale, run);
        }
    }
}

/// Characters that only say how to show the one before them, and have nothing to show of their
/// own: the selectors that ask for the emoji or the text form, the joiner of emoji that make a
/// family, the skin tones, and the tags of flags. Fonts draw boxes for them, and they are left out.
fn is_ignorable(c: char) -> bool {
    matches!(
        c,
        '\u{200d}' | '\u{fe0e}' | '\u{fe0f}' | '\u{1f3fb}'..='\u{1f3ff}' | '\u{e0020}'..='\u{e007f}'
    )
}

/// A block of text with its font and size decided and measured.
struct Block {
    face: Face,
    scale: Scale,
    lines: Vec<String>,
    line_widths: Vec<i32>,
    /// How far apart the tops of two consecutive lines are.
    advance: i32,
    width: i32,
    height: i32,
}

fn lay_out(
    element: &TextElement,
    fonts: &Fonts,
    box_width: Option<f32>,
    box_height: f32,
) -> Result<Block, D30Error> {
    let text: String = element.text.chars().filter(|&c| !is_ignorable(c)).collect();
    let face = Face::new(fonts.load(&element.font)?, fonts, &text);
    let lines = match element.lines {
        Lines::AsTyped => typed_lines(&text),
        Lines::Split(count) => split_into_lines(&face, &text, count),
    };

    let size = match element.size {
        D30Scale::Value(size) => size,
        D30Scale::Auto { minus } => {
            let (width, height, _) = measure(&face, Scale::uniform(MEASURING_SIZE), &lines);
            if width == 0 || height == 0 {
                1.0
            } else {
                let by_height = (box_height - 2.0 * element.margin) / height as f32;
                let by_width =
                    box_width.map(|box_width| (box_width - 2.0 * element.margin) / width as f32);
                let fit = by_width.map_or(by_height, |by_width| by_width.min(by_height));
                MEASURING_SIZE * fit - minus
            }
        }
    };
    // A size of nothing or less would draw nothing, or garbage.
    let scale = Scale::uniform(size.max(1.0));

    let (width, height, advance) = measure(&face, scale, &lines);
    let line_widths = lines
        .iter()
        .map(|line| face.text_size(scale, line).0)
        .collect();
    Ok(Block {
        face,
        scale,
        lines,
        line_widths,
        advance,
        width,
        height,
    })
}

/// The width and height of the ink of a block of lines, and how far apart the lines are.
fn measure(face: &Face, scale: Scale, lines: &[String]) -> (i32, i32, i32) {
    let metrics = face.v_metrics(scale);
    let advance = (metrics.ascent - metrics.descent + metrics.line_gap).ceil() as i32;
    let width = lines
        .iter()
        .map(|line| face.text_size(scale, line).0)
        .max()
        .unwrap_or(0);
    let last_line_height = lines.last().map_or(0, |line| face.text_size(scale, line).1);
    let height = advance * (lines.len() as i32 - 1) + last_line_height;
    (width, height, advance)
}

/// The lines of a text as it was typed. A newline at the very end is nearly always an accident, so
/// it does not make a blank line.
fn typed_lines(text: &str) -> Vec<String> {
    text.trim_end_matches(['\n', '\r'])
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

/// Divides the words of `text` over `count` lines (fewer if there are not enough words), with the
/// breaks where the widest line comes out narrowest.
fn split_into_lines(face: &Face, text: &str, count: usize) -> Vec<String> {
    // A no-break space belongs to its word: it is there to keep the words around it together.
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{202f}'))
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return vec![String::new()];
    }
    let count = count.clamp(1, words.len());
    if count == 1 {
        return vec![words.join(" ")];
    }

    let spans = WordSpans::new(face, &words);
    let starts = best_breaks(words.len(), count, |first, last| spans.width(first, last));
    (0..count)
        .map(|line| {
            let first = starts[line];
            let last = starts.get(line + 1).copied().unwrap_or(words.len());
            words[first..last].join(" ")
        })
        .collect()
}

/// Where the words of a text lie when they are all set in one line, measured to find out how wide
/// a line of some of them would be. Setting each word on its own is enough for that (and fast, so
/// a long text does not slow the window down): a line is the words and the spaces between them,
/// the last word counted as far as its ink goes.
struct WordSpans {
    /// Where each word starts, and where the ink of each word ends.
    start: Vec<f32>,
    end: Vec<f32>,
}

impl WordSpans {
    fn new(face: &Face, words: &[&str]) -> Self {
        let scale = Scale::uniform(MEASURING_SIZE);
        let space = face.advance(scale, " ");
        let mut start = Vec::with_capacity(words.len());
        let mut end = Vec::with_capacity(words.len());
        let mut pen = 0.0;
        for word in words {
            start.push(pen);
            end.push(pen + face.text_size(scale, word).0 as f32);
            pen += face.advance(scale, word) + space;
        }
        WordSpans { start, end }
    }

    /// How wide the words `first..last` are when they are set in one line, in whole dots.
    fn width(&self, first: usize, last: usize) -> i64 {
        (self.end[last - 1] - self.start[first]).max(0.0).ceil() as i64
    }
}

/// How far the pen moves while `text` is set: where the next glyph would start.
fn advance(font: &Font, scale: Scale, text: &str) -> f32 {
    font.layout(text, scale, point(0.0, 0.0))
        .last()
        .map_or(0.0, |glyph| {
            glyph.position().x + glyph.unpositioned().h_metrics().advance_width
        })
}

/// Divides `words` words over `lines` lines (at least one word in each, so `lines <= words`) so
/// that the widest line is as narrow as can be. Of the ways to do that it takes one with lines as
/// even as it finds. `width(first, last)` is how wide the words `first..last` are in one line.
/// Returns the first word of each line.
fn best_breaks(words: usize, lines: usize, width: impl Fn(usize, usize) -> i64) -> Vec<usize> {
    assert!(
        1 <= lines && lines <= words,
        "{lines} lines for {words} words"
    );

    /// The best way found to set some words in some lines.
    #[derive(Clone, Copy)]
    struct Best {
        widest: i64,
        /// The sum of the squares of the widths of the lines: the smaller, the more even.
        squares: i64,
        /// Where the last line starts.
        last_start: usize,
    }

    // best[l][j] is for the first j words in l lines. The widest line of each is the smallest
    // there is, because the widest line of a better way to set the words before the last line
    // is never wider. Which of several ways with the same widest line is kept is a matter of how
    // even they are, judged only so far.
    let mut best: Vec<Vec<Option<Best>>> = vec![vec![None; words + 1]; lines + 1];
    for (end, one_line) in best[1].iter_mut().enumerate().skip(1) {
        let w = width(0, end);
        *one_line = Some(Best {
            widest: w,
            squares: w * w,
            last_start: 0,
        });
    }
    for l in 2..=lines {
        for end in l..=words {
            for start in (l - 1)..end {
                let Some(before) = best[l - 1][start] else {
                    continue;
                };
                let w = width(start, end);
                let candidate = Best {
                    widest: before.widest.max(w),
                    squares: before.squares + w * w,
                    last_start: start,
                };
                let better = match best[l][end] {
                    None => true,
                    Some(current) => {
                        (candidate.widest, candidate.squares) < (current.widest, current.squares)
                    }
                };
                if better {
                    best[l][end] = Some(candidate);
                }
            }
        }
    }

    let mut starts = vec![0; lines];
    let mut end = words;
    for l in (1..=lines).rev() {
        let entry = best[l][end].expect("l lines can be had from any l or more words");
        starts[l - 1] = entry.last_start;
        end = entry.last_start;
    }
    starts
}

/// Draws a block of text into the part of the label that starts `room_left` dots from its left
/// end and is `room_width` dots wide. That is all of the label unless there are pictures.
fn draw(
    canvas: &mut GrayImage,
    element: &TextElement,
    block: &Block,
    room_left: f32,
    room_width: f32,
) {
    let box_height = canvas.height() as f32;

    let left = match element.align {
        Align::Left => (room_left + element.margin) as i32,
        Align::Center => (room_left + (room_width - block.width as f32) / 2.0) as i32,
        Align::Right => (room_left + room_width - element.margin - block.width as f32) as i32,
    };
    let top = ((box_height - block.height as f32) / 2.0) as i32;

    for (index, line) in block.lines.iter().enumerate() {
        let spare = block.width - block.line_widths[index];
        let indent = match element.align {
            Align::Left => 0,
            Align::Center => (spare as f32 / 2.0) as i32,
            Align::Right => spare,
        };
        block.face.draw(
            canvas,
            left + indent,
            top + index as i32 * block.advance,
            block.scale,
            line,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generate_image, pack_image};

    fn stock(id: &str) -> LabelStock {
        LabelStock::parse(id).unwrap()
    }

    /// The block of text of a label made with `Label::text`.
    fn text_mut(label: &mut Label) -> &mut TextElement {
        match &mut label.elements[0] {
            Element::Text(text) => text,
            _ => panic!("the label should start with a block of text"),
        }
    }

    fn render_text(id: &str, text: &str) -> Rendered {
        render(&Label::text(stock(id), text), &Fonts::embedded()).unwrap()
    }

    /// The columns (from, to inclusive) that have any ink.
    fn ink_columns(rendered: &Rendered) -> Option<(u32, u32)> {
        let canvas = rendered.canvas();
        let inked = |x: u32| (0..canvas.height()).any(|y| canvas.get_pixel(x, y)[0] > 127);
        let first = (0..canvas.width()).find(|&x| inked(x))?;
        let last = (0..canvas.width()).rev().find(|&x| inked(x))?;
        Some((first, last))
    }

    fn ink_rows(rendered: &Rendered) -> Option<(u32, u32)> {
        let canvas = rendered.canvas();
        let inked = |y: u32| (0..canvas.width()).any(|x| canvas.get_pixel(x, y)[0] > 127);
        let first = (0..canvas.height()).find(|&y| inked(y))?;
        let last = (0..canvas.height()).rev().find(|&y| inked(y))?;
        Some((first, last))
    }

    #[test]
    fn single_line_text_prints_exactly_what_the_old_renderer_did() {
        let fonts = Fonts::embedded();
        let texts = [
            "Hello",
            "Kitchen",
            "W",
            "i",
            "MMMMMMMMMMMM",
            "Äpfel & Öl",
            "g y p",
            "12:30",
        ];
        let sizes = [
            D30Scale::Auto { minus: 0.0 },
            D30Scale::Auto { minus: 3.0 },
            D30Scale::Value(40.0),
            D30Scale::Value(75.5),
        ];
        for text in texts {
            for size in sizes {
                for margin in [15.0, 4.0] {
                    let old = pack_image(&generate_image(text, margin, size).unwrap());

                    let mut label = Label::text(stock("12x40"), text);
                    let element = text_mut(&mut label);
                    element.size = size;
                    element.margin = margin;
                    let new = render(&label, &fonts).unwrap().raster().unwrap();

                    assert_eq!(new.bytes(), &old[..], "{text:?} {size:?} margin {margin}");
                }
            }
        }
    }

    #[test]
    fn a_12x40_label_is_320_by_96_dots_and_96_wide_for_the_printer() {
        let rendered = render_text("12x40", "Hello");
        assert_eq!(rendered.canvas().dimensions(), (320, 96));
        let raster = rendered.raster().unwrap();
        assert_eq!((raster.width_bytes(), raster.rows()), (12, 320));
    }

    #[test]
    fn other_precut_sizes_have_their_own_length() {
        assert_eq!(render_text("12x30", "Hi").length_dots(), 240);
        assert_eq!(render_text("14x50", "Hi").length_dots(), 400);
        // The print head is 12 mm wide whatever the width of the label.
        assert_eq!(render_text("15x50", "Hi").canvas().height(), 96);
    }

    #[test]
    fn two_lines_are_each_smaller_than_one_line_would_be() {
        let one = render_text("12x40", "Hello");
        let two = render_text("12x40", "Hello\nWorld");
        let (one_top, one_bottom) = ink_rows(&one).unwrap();
        let (two_top, two_bottom) = ink_rows(&two).unwrap();
        // Two lines still have to fit the label, so the text is smaller...
        assert!(two_bottom - two_top < 2 * (one_bottom - one_top));
        assert!(
            ink_columns(&two).unwrap().1 - ink_columns(&two).unwrap().0
                < ink_columns(&one).unwrap().1 - ink_columns(&one).unwrap().0
        );
        // ...and it stays inside the label.
        assert!(two_bottom < 96);
    }

    #[test]
    fn a_trailing_newline_does_not_make_a_blank_line() {
        let plain = render_text("12x40", "Hello");
        let newline = render_text("12x40", "Hello\n");
        assert_eq!(plain.canvas().as_raw(), newline.canvas().as_raw());
    }

    #[test]
    fn left_and_right_alignment_anchor_the_text_to_the_margin() {
        let render_aligned = |align| {
            let mut label = Label::text(stock("12x40"), "Hi");
            let element = text_mut(&mut label);
            element.align = align;
            element.size = D30Scale::Value(40.0);
            element.margin = 10.0;
            render(&label, &Fonts::embedded()).unwrap()
        };
        let (left_first, _) = ink_columns(&render_aligned(Align::Left)).unwrap();
        let (_, right_last) = ink_columns(&render_aligned(Align::Right)).unwrap();
        let (center_first, center_last) = ink_columns(&render_aligned(Align::Center)).unwrap();

        assert!(
            (10..=14).contains(&left_first),
            "left text starts at {left_first}"
        );
        assert!(
            (305..=310).contains(&right_last),
            "right text ends at {right_last}"
        );
        let slack_left = center_first;
        let slack_right = 319 - center_last;
        assert!(
            slack_left.abs_diff(slack_right) <= 4,
            "{slack_left} vs {slack_right}"
        );
    }

    #[test]
    fn the_lines_of_a_block_line_up_as_asked() {
        let render_aligned = |align| {
            let mut label = Label::text(stock("12x40"), "A very long line\nHi");
            let element = text_mut(&mut label);
            element.align = align;
            element.size = D30Scale::Value(26.0);
            render(&label, &Fonts::embedded()).unwrap()
        };
        // Ink of the second line only: the rows below the middle of the label.
        let second_line_columns = |rendered: &Rendered| {
            let canvas = rendered.canvas();
            let cols: Vec<u32> = (0..canvas.width())
                .filter(|&x| (48..96).any(|y| canvas.get_pixel(x, y)[0] > 127))
                .collect();
            (*cols.first().unwrap(), *cols.last().unwrap())
        };
        let (left_first, _) = second_line_columns(&render_aligned(Align::Left));
        let (_, right_last) = second_line_columns(&render_aligned(Align::Right));
        let (center_first, center_last) = second_line_columns(&render_aligned(Align::Center));
        let (block_first, block_last) = ink_columns(&render_aligned(Align::Center)).unwrap();
        assert!(
            left_first <= block_first + 6,
            "{left_first} vs {block_first}"
        );
        assert!(right_last + 6 >= block_last, "{right_last} vs {block_last}");
        let middle = (center_first + center_last) / 2;
        let block_middle = (block_first + block_last) / 2;
        assert!(
            middle.abs_diff(block_middle) <= 6,
            "{middle} vs {block_middle}"
        );
    }

    #[test]
    fn continuous_tape_is_cut_to_fit_the_text() {
        let short = render_text("12-continuous", "Hi");
        let long = render_text("12-continuous", "Hello, tape!");
        assert_eq!(short.canvas().height(), 96);
        assert!(long.length_dots() > short.length_dots());
        // The text fills the width of the tape, and the label is the text plus a margin each side.
        let (top, bottom) = ink_rows(&long).unwrap();
        assert!(
            bottom - top > 50,
            "the text should use most of the 96 dots, used {}",
            bottom - top
        );
        let (first, last) = ink_columns(&long).unwrap();
        assert!(first >= 10 && long.length_dots() - last >= 10);
    }

    #[test]
    fn continuous_tape_can_be_given_a_length() {
        let mut label = Label::text(stock("12-continuous"), "Hi");
        label.length_mm = Some(25.0);
        let rendered = render(&label, &Fonts::embedded()).unwrap();
        assert_eq!(rendered.length_dots(), 200);
    }

    #[test]
    fn precut_labels_do_not_take_a_length() {
        let mut label = Label::text(stock("12x40"), "Hi");
        label.length_mm = Some(25.0);
        let error = render(&label, &Fonts::embedded()).err().unwrap();
        assert!(matches!(error, D30Error::InvalidLabel { .. }));
    }

    #[test]
    fn labels_longer_than_the_printer_takes_are_rejected() {
        let mut label = Label::text(stock("12-continuous"), "Hi");
        label.length_mm = Some(100_000.0);
        assert!(matches!(
            render(&label, &Fonts::embedded()).err().unwrap(),
            D30Error::InvalidLabel { .. }
        ));
        label.length_mm = Some(0.0);
        assert!(render(&label, &Fonts::embedded()).is_err());
    }

    #[test]
    fn blank_text_makes_a_blank_label_and_does_not_panic() {
        for text in ["", " ", "\n\n"] {
            for id in ["12x40", "12-continuous"] {
                let rendered = render_text(id, text);
                assert!(
                    rendered.canvas().pixels().all(|pixel| pixel[0] == 0),
                    "{text:?} {id}"
                );
                assert!(rendered.raster().is_ok());
            }
        }
    }

    #[test]
    fn the_preview_is_the_bitmap_that_is_printed() {
        let rendered = render_text("12x40", "Hello");
        let preview = rendered.preview();
        assert!(preview
            .pixels()
            .all(|pixel| pixel[0] == 0 || pixel[0] == 255));
        // Every dot of the preview that is black is a printed dot, and vice versa.
        let printed = rendered
            .canvas()
            .pixels()
            .filter(|pixel| pixel[0] > 127)
            .count();
        let black = preview.pixels().filter(|pixel| pixel[0] == 0).count();
        assert_eq!(printed, black);
        assert!(printed > 0);
    }

    // ---- splitting a text over lines ------------------------------------------------------------

    /// The lines that `Lines::Split(count)` makes of `text` in the built-in font.
    fn split(text: &str, count: usize) -> Vec<String> {
        let fonts = Fonts::embedded();
        let font = fonts.load(&FontSpec::default()).unwrap();
        split_into_lines(&Face::new(font, &fonts, text), text, count)
    }

    fn render_with(id: &str, text: &str, lines: Lines) -> Rendered {
        let mut label = Label::text(stock(id), text);
        let element = text_mut(&mut label);
        element.lines = lines;
        render(&label, &Fonts::embedded()).unwrap()
    }

    /// Every way of dividing `words` over `count` lines, as lines of text.
    fn all_divisions(words: &[&str], count: usize) -> Vec<Vec<String>> {
        if count == 1 {
            return vec![vec![words.join(" ")]];
        }
        let mut all = Vec::new();
        for first_line in 1..=(words.len() + 1 - count) {
            for rest in all_divisions(&words[first_line..], count - 1) {
                let mut lines = vec![words[..first_line].join(" ")];
                lines.extend(rest);
                all.push(lines);
            }
        }
        all
    }

    /// How wide the widest of `lines` really is at the size that decides how large text is made.
    fn widest(font: &Font, lines: &[String]) -> i32 {
        let scale = Scale::uniform(MEASURING_SIZE);
        lines
            .iter()
            .map(|line| text_size(scale, font, line).0)
            .max()
            .unwrap()
    }

    #[test]
    fn splitting_a_text_is_the_same_as_typing_the_breaks() {
        assert_eq!(split("Hello world", 2), ["Hello", "world"]);
        let split_text = render_with("12x40", "Hello world", Lines::Split(2));
        let typed = render_with("12x40", "Hello\nworld", Lines::AsTyped);
        assert_eq!(split_text.canvas().as_raw(), typed.canvas().as_raw());

        // One line is the whole text, and the breaks that were typed are only spaces now.
        assert_eq!(split("Hello\nbig\nworld", 1), ["Hello big world"]);
        assert_eq!(split("Hello\nbig\nworld", 2), split("Hello big world", 2));
        let one = render_with("12x40", "Hello\nworld", Lines::Split(1));
        let joined = render_with("12x40", "Hello world", Lines::AsTyped);
        assert_eq!(one.canvas().as_raw(), joined.canvas().as_raw());
    }

    #[test]
    fn a_split_keeps_every_word_in_order_and_uses_as_many_lines_as_it_can() {
        let text = "Kitchen pantry flour and sugar jars";
        let words: Vec<&str> = text.split(' ').collect();
        for count in 1..=9 {
            let lines = split(text, count);
            assert_eq!(lines.len(), count.min(words.len()), "{count} lines");
            assert_eq!(lines.join(" "), text, "{count} lines");
            assert!(lines.iter().all(|line| !line.is_empty()));
        }
    }

    #[test]
    fn the_breaks_make_the_widest_line_as_narrow_as_they_can() {
        let font = Fonts::embedded().load(&FontSpec::default()).unwrap();
        let texts = [
            "Kitchen pantry flour and sugar jars",
            "Do not open until Christmas",
            "i W i W i W i W",
            "Extraordinarily long word and a few small ones",
            "A B C D E F G H I J",
            "Mm mm Mm mm iiiiii Mm",
        ];
        for text in texts {
            let words: Vec<&str> = text.split(' ').collect();
            for count in 2..=4 {
                let chosen = widest(&font, &split(text, count));
                let best = all_divisions(&words, count)
                    .iter()
                    .map(|lines| widest(&font, lines))
                    .min()
                    .unwrap();
                // The breaks are chosen from word widths worked out one at a time, which can
                // differ from the real widths by a few dots at this size.
                assert!(
                    chosen <= best + 6,
                    "{text:?} in {count} lines: the widest line is {chosen} dots, it could be {best}"
                );
            }
        }
    }

    /// A simple generator of numbers, so that the test does not need a library for them.
    fn lcg(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *seed >> 33
    }

    /// How wide the widest line is at the very best, found by trying every way to divide the words
    /// from `first` on over `lines` lines.
    fn narrowest_widest_line(
        first: usize,
        words: usize,
        lines: usize,
        width: &dyn Fn(usize, usize) -> i64,
    ) -> i64 {
        if lines == 1 {
            return width(first, words);
        }
        // The first line takes the words up to `cut`, which leaves a word for each line after it.
        (first + 1..=words + 1 - lines)
            .map(|cut| width(first, cut).max(narrowest_widest_line(cut, words, lines - 1, width)))
            .min()
            .unwrap()
    }

    #[test]
    fn the_best_breaks_are_found_for_any_widths() {
        let mut seed = 7;
        for _ in 0..300 {
            let words = 1 + (lcg(&mut seed) % 9) as usize;
            let widths: Vec<i64> = (0..words)
                .map(|_| 1 + (lcg(&mut seed) % 60) as i64)
                .collect();
            let space = (lcg(&mut seed) % 12) as i64;
            let width = |first: usize, last: usize| {
                widths[first..last].iter().sum::<i64>() + space * (last - first - 1) as i64
            };
            for lines in 1..=words.min(4) {
                let starts = best_breaks(words, lines, width);
                assert_eq!(starts.len(), lines);
                assert_eq!(starts[0], 0);
                assert!(starts.windows(2).all(|pair| pair[0] < pair[1]));
                let ends = starts.iter().skip(1).copied().chain([words]);
                let chosen = starts
                    .iter()
                    .copied()
                    .zip(ends)
                    .map(|(first, last)| width(first, last))
                    .max()
                    .unwrap();

                let best = narrowest_widest_line(0, words, lines, &width);
                assert_eq!(chosen, best, "{widths:?} space {space} in {lines} lines");
            }
        }
    }

    #[test]
    fn of_equally_narrow_choices_the_most_even_one_is_taken() {
        let width = |first: usize, last: usize| 10 * (last - first) as i64;
        assert_eq!(best_breaks(4, 2, width), [0, 2]);
        assert_eq!(best_breaks(6, 3, width), [0, 2, 4]);
        // The widest line cannot be narrower than 7, and [5 1 | 1 5] and [5 | 1 1 5] differ in it.
        let widths = [5i64, 1, 1, 5];
        let width = |first: usize, last: usize| widths[first..last].iter().sum::<i64>();
        assert_eq!(best_breaks(4, 2, width), [0, 2]);
    }

    #[test]
    fn splitting_blank_text_or_too_few_words_does_not_fail() {
        assert_eq!(split("", 2), [""]);
        assert_eq!(split("  \n ", 3), [""]);
        assert_eq!(split("Hello", 2), ["Hello"]);
        assert_eq!(split("Hello world", 3), ["Hello", "world"]);
        // A count of nothing is taken as one line.
        assert_eq!(split("Hello world", 0), ["Hello world"]);
        for text in ["", " ", "\n\n"] {
            let rendered = render_with("12x40", text, Lines::Split(2));
            assert!(rendered.canvas().pixels().all(|pixel| pixel[0] == 0));
        }
    }

    #[test]
    fn a_no_break_space_keeps_its_words_together() {
        assert_eq!(
            split("10\u{a0}mm tape", 2),
            ["10\u{a0}mm", "tape"],
            "the words around a no-break space stay on one line"
        );
    }

    #[test]
    fn a_long_text_is_larger_in_two_lines_than_in_one() {
        let text = "Kitchen pantry cupboard flour";
        let (one_top, one_bottom) = ink_rows(&render_with("12x40", text, Lines::Split(1))).unwrap();
        let (two_top, two_bottom) = ink_rows(&render_with("12x40", text, Lines::Split(2))).unwrap();
        assert!(
            two_bottom - two_top > 2 * (one_bottom - one_top),
            "one line: {} rows of ink, two lines: {}",
            one_bottom - one_top,
            two_bottom - two_top
        );
        let (three_top, three_bottom) =
            ink_rows(&render_with("12x40", text, Lines::Split(3))).unwrap();
        assert!(three_bottom < 96 && three_top > 0, "it fits the label");
    }

    #[test]
    fn splitting_a_long_text_makes_a_shorter_label_on_uncut_tape() {
        let text = "Kitchen pantry cupboard flour and sugar";
        let one = render_with("15-continuous", text, Lines::Split(1)).length_dots();
        let two = render_with("15-continuous", text, Lines::Split(2)).length_dots();
        let three = render_with("15-continuous", text, Lines::Split(3)).length_dots();
        assert!(
            two < one && three < two,
            "{one} dots, {two} dots, {three} dots"
        );
    }

    #[test]
    fn a_very_long_text_is_split_quickly() {
        let text = "word ".repeat(3000);
        let started = std::time::Instant::now();
        let lines = split(&text, 3);
        assert_eq!(lines.len(), 3);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
    }

    // ---- pictures -------------------------------------------------------------------------------

    /// A picture that is black all over.
    fn black(width: u32, height: u32) -> Picture {
        let image = GrayImage::from_pixel(width, height, Luma([0]));
        Picture::new(&image::DynamicImage::ImageLuma8(image)).unwrap()
    }

    /// A picture whose left half is black and whose right half is white.
    fn black_and_white(width: u32, height: u32) -> Picture {
        let image = GrayImage::from_fn(width, height, |x, _| {
            Luma([if x < width / 2 { 0 } else { 255 }])
        });
        Picture::new(&image::DynamicImage::ImageLuma8(image)).unwrap()
    }

    fn picture_element(picture: Picture, side: Side, margin: f32) -> PictureElement {
        let mut element = PictureElement::new(picture);
        element.side = side;
        element.margin = margin;
        element
    }

    /// A label with only these pictures on it.
    fn pictures_only(id: &str, pictures: Vec<PictureElement>) -> Label {
        let mut label = Label::new(stock(id));
        label
            .elements
            .extend(pictures.into_iter().map(Element::Picture));
        label
    }

    /// A label with some text, and then these pictures.
    fn text_and_pictures(id: &str, text: &str, pictures: Vec<PictureElement>) -> Label {
        let mut label = Label::text(stock(id), text);
        label
            .elements
            .extend(pictures.into_iter().map(Element::Picture));
        label
    }

    fn draw_label(label: &Label) -> Rendered {
        render(label, &Fonts::embedded()).unwrap()
    }

    /// The columns of the label from `from` on that have any ink, first and last.
    fn ink_columns_from(rendered: &Rendered, from: u32) -> Option<(u32, u32)> {
        let canvas = rendered.canvas();
        let inked = |x: u32| (0..canvas.height()).any(|y| canvas.get_pixel(x, y)[0] > 127);
        let first = (from..canvas.width()).find(|&x| inked(x))?;
        let last = (from..canvas.width()).rev().find(|&x| inked(x))?;
        Some((first, last))
    }

    fn ink_in(rendered: &Rendered, columns: std::ops::RangeInclusive<u32>, row: u32) -> bool {
        columns
            .clone()
            .all(|x| rendered.canvas().get_pixel(x, row)[0] > 127)
    }

    #[test]
    fn a_picture_alone_is_as_high_as_the_label_allows_and_in_the_middle() {
        let label = pictures_only(
            "12x40",
            vec![picture_element(black(40, 20), Side::Left, 15.0)],
        );
        let rendered = draw_label(&label);
        assert_eq!(rendered.canvas().dimensions(), (320, 96));
        // 96 dots less 15 above and 15 below is 66 high, and twice that wide: 132, in the middle.
        assert_eq!(ink_rows(&rendered), Some((15, 80)));
        assert_eq!(ink_columns(&rendered), Some((94, 225)));
        let ink = rendered.canvas().pixels().filter(|p| p[0] > 127).count();
        assert_eq!(ink, 66 * 132);
    }

    #[test]
    fn a_picture_alone_on_uncut_tape_makes_the_label_as_long_as_it_and_its_margins() {
        let label = pictures_only(
            "12-continuous",
            vec![picture_element(black(40, 20), Side::Right, 15.0)],
        );
        let rendered = draw_label(&label);
        assert_eq!(rendered.length_dots(), 132 + 2 * 15);
        assert_eq!(ink_columns(&rendered), Some((15, 146)));
    }

    #[test]
    fn text_that_is_only_blanks_leaves_the_picture_alone() {
        let picture = picture_element(black(30, 30), Side::Left, 15.0);
        let alone = draw_label(&pictures_only("12x40", vec![picture.clone()]));
        let with_blank = draw_label(&text_and_pictures("12x40", " \n ", vec![picture]));
        assert_eq!(alone.canvas().as_raw(), with_blank.canvas().as_raw());
    }

    #[test]
    fn a_picture_sits_beside_the_text_and_the_text_gets_what_is_left() {
        let at = |side| {
            let picture = picture_element(black(20, 20), side, 15.0);
            draw_label(&text_and_pictures("12x40", "Hi", vec![picture]))
        };

        // The picture is 66 dots square. At the left it takes the columns from 15 to 80, and the
        // text is in the middle of the rest.
        let left = at(Side::Left);
        assert!(ink_in(&left, 15..=80, 15) && ink_in(&left, 15..=80, 80));
        assert_eq!(ink_columns_from(&left, 0).unwrap().0, 15);
        let (first, last) = ink_columns_from(&left, 81).unwrap();
        let middle = (first + last) / 2;
        assert!(
            middle.abs_diff((81 + 320) / 2) <= 6,
            "the text is {first} to {last}"
        );

        // At the right the picture is mirrored: columns 239 to 304, and the text is before it.
        let right = at(Side::Right);
        assert!(ink_in(&right, 239..=304, 15) && ink_in(&right, 239..=304, 80));
        assert_eq!(ink_columns_from(&right, 0).unwrap().1, 304);
        let canvas = right.canvas();
        let text: Vec<u32> = (0..239)
            .filter(|&x| (0..96).any(|y| canvas.get_pixel(x, y)[0] > 127))
            .collect();
        let middle = (text[0] + text[text.len() - 1]) / 2;
        assert!(middle.abs_diff(239 / 2) <= 6, "the text is {text:?}");
    }

    #[test]
    fn a_wide_picture_leaves_less_room_and_the_text_gets_smaller() {
        let text_height = |picture: Option<PictureElement>| {
            let pictures = picture.into_iter().collect();
            let rendered = draw_label(&text_and_pictures("12x40", "Hello", pictures));
            // Only the ink to the right of the picture (it ends at column 212), where the text is.
            let canvas = rendered.canvas();
            let rows: Vec<u32> = (0..96)
                .filter(|&y| (220..320).any(|x| canvas.get_pixel(x, y)[0] > 127))
                .collect();
            rows.last().unwrap() - rows.first().unwrap()
        };
        let alone = text_height(None);
        let beside_wide = text_height(Some(picture_element(black(300, 100), Side::Left, 15.0)));
        assert!(
            beside_wide < alone,
            "the text is {alone} dots high alone, {beside_wide} beside a wide picture"
        );
    }

    #[test]
    fn uncut_tape_gets_as_much_longer_as_the_room_the_picture_takes() {
        let length = |pictures: Vec<PictureElement>| {
            draw_label(&text_and_pictures("15-continuous", "Hello tape", pictures)).length_dots()
        };
        let plain = length(vec![]);
        // A square picture 66 dots across, with 15 of margin before it.
        let one = length(vec![picture_element(black(20, 20), Side::Left, 15.0)]);
        assert_eq!(one, plain + 15 + 66);
        let right = length(vec![picture_element(black(20, 20), Side::Right, 15.0)]);
        assert_eq!(right, one);
        let both = length(vec![
            picture_element(black(20, 20), Side::Left, 15.0),
            picture_element(black(20, 20), Side::Right, 15.0),
        ]);
        assert_eq!(both, plain + 2 * (15 + 66));
    }

    #[test]
    fn a_picture_can_be_made_lower_than_the_label_and_stays_in_the_middle() {
        let mut element = picture_element(black(20, 20), Side::Left, 15.0);
        element.height = Some(30.0);
        let rendered = draw_label(&pictures_only("12x40", vec![element]));
        // 30 dots high, in the middle of 96, so from row 33 to row 62. As wide as high.
        assert_eq!(ink_rows(&rendered), Some((33, 62)));
        let (first, last) = ink_columns(&rendered).unwrap();
        assert_eq!(last - first + 1, 30);
        // Asking for more than the label has gives the whole height.
        let mut element = picture_element(black(20, 20), Side::Left, 0.0);
        element.height = Some(500.0);
        let rendered = draw_label(&pictures_only("12x40", vec![element]));
        assert_eq!(ink_rows(&rendered), Some((0, 95)));
    }

    #[test]
    fn the_tone_and_invert_decide_which_dots_are_burned() {
        let draw = |invert: bool, tone: Tone| {
            let mut element = picture_element(black_and_white(20, 20), Side::Left, 15.0);
            element.invert = invert;
            element.tone = tone;
            draw_label(&pictures_only("12x40", vec![element]))
        };
        // The 66 dots of the picture lie from column 127 to 192: its black half is on the left,
        // so up to column 159, and its white half on the right. (Where exactly the edge between
        // them falls is up to the scaling, give or take a dot.)
        let normal = draw(false, Tone::default());
        let (first, last) = ink_columns(&normal).unwrap();
        assert_eq!(first, 127);
        assert!((158..=160).contains(&last), "burned up to column {last}");
        assert!(ink_in(&normal, 127..=157, 48));
        // Swapped, it is the other half that is burned.
        let swapped = draw(true, Tone::default());
        let (first, last) = ink_columns(&swapped).unwrap();
        assert_eq!(last, 192);
        assert!((159..=161).contains(&first), "burned from column {first}");
        assert!(ink_in(&swapped, 162..=192, 48));
        // A picture of only black and white comes out the same dithered.
        let dithered = draw(false, Tone::Dither);
        let (first, last) = ink_columns(&dithered).unwrap();
        assert_eq!(first, 127);
        assert!((158..=161).contains(&last), "burned up to column {last}");
    }

    #[test]
    fn pictures_on_one_side_follow_each_other_with_their_margins() {
        // With 10 dots of margin a square picture is 76 across (96 less 10 above and below), and
        // with 20 it is 56.
        let a = picture_element(black(10, 10), Side::Left, 10.0);
        let b = picture_element(black(10, 10), Side::Left, 20.0);
        let rendered = draw_label(&text_and_pictures("12x40", "Hi", vec![a, b]));
        // The first is from column 10 to 85. The second has 20 dots of margin before it, so it is
        // from 106 to 161, and the text is beyond that.
        assert!(ink_in(&rendered, 10..=85, 48));
        assert!((0..10).all(|x| (0..96).all(|y| rendered.canvas().get_pixel(x, y)[0] == 0)));
        assert!((86..106).all(|x| (0..96).all(|y| rendered.canvas().get_pixel(x, y)[0] == 0)));
        assert!(ink_in(&rendered, 106..=161, 48));
    }

    #[test]
    fn a_wide_picture_cannot_squeeze_the_text_off_a_label_of_a_set_length() {
        // Five times as wide as high: at full height it would fill the whole label.
        let banner = picture_element(black(300, 60), Side::Left, 15.0);
        let rendered = draw_label(&text_and_pictures("12x40", "Hi", vec![banner.clone()]));
        // The picture keeps to 60% of the 320 dots, its margin included: 192 dots. The text has
        // the rest, and is not small.
        let (first, last) = ink_columns(&rendered).unwrap();
        assert_eq!(first, 15);
        let text_columns = ink_columns_from(&rendered, 195).expect("the text is on the label");
        assert!(text_columns.1 > 250, "the text is at {text_columns:?}");
        let canvas = rendered.canvas();
        let rows: Vec<u32> = (0..96)
            .filter(|&y| (195..320).any(|x| canvas.get_pixel(x, y)[0] > 127))
            .collect();
        assert!(
            rows.last().unwrap() - rows.first().unwrap() > 30,
            "the text is only {} dots high",
            rows.last().unwrap() - rows.first().unwrap()
        );
        assert!(last > 250);

        // Uncut tape that is cut to fit has no such limit: it is as long as it needs to be.
        let length = |pictures: Vec<PictureElement>| {
            draw_label(&text_and_pictures("15-continuous", "Hi", pictures)).length_dots()
        };
        assert_eq!(length(vec![banner]), length(vec![]) + 15 + 330);
    }

    #[test]
    fn a_picture_never_sticks_out_of_a_label_that_has_a_length() {
        // A very wide picture on a precut label of 320 dots, with 15 dots of margin.
        let wide = picture_element(black(1000, 10), Side::Left, 15.0);
        let rendered = draw_label(&text_and_pictures("12x40", "Hi", vec![wide.clone()]));
        let (first, last) = ink_columns(&rendered).unwrap();
        assert!(first >= 15 && last <= 319, "{first} to {last}");

        // Several that together are wider than the label do not panic either.
        let rendered = draw_label(&pictures_only(
            "12x40",
            vec![wide.clone(), wide.clone(), wide],
        ));
        assert_eq!(rendered.canvas().dimensions(), (320, 96));
    }

    #[test]
    fn sizes_that_make_no_sense_are_not_a_problem() {
        for height in [Some(f32::NAN), Some(-4.0), Some(0.0), Some(1e9), None] {
            for margin in [f32::NAN, -10.0, 0.0, 47.0, 1000.0] {
                let mut element = picture_element(black(30, 20), Side::Left, margin);
                element.height = height;
                for id in ["12x40", "15-continuous"] {
                    for text in ["Hi", ""] {
                        let label = text_and_pictures(id, text, vec![element.clone()]);
                        let rendered = render(&label, &Fonts::embedded());
                        assert!(rendered.is_ok(), "{height:?} {margin} {id} {text:?}");
                        assert_eq!(rendered.unwrap().canvas().height(), 96);
                    }
                }
            }
        }
    }

    #[test]
    fn a_picture_that_would_make_the_label_longer_than_the_printer_takes_is_refused() {
        let label = pictures_only(
            "12-continuous",
            vec![picture_element(black(100_000, 1), Side::Left, 15.0)],
        );
        let error = render(&label, &Fonts::embedded()).err().unwrap();
        assert!(matches!(error, D30Error::InvalidLabel { .. }), "{error}");
    }

    #[test]
    fn the_preview_of_a_label_with_a_picture_is_the_bitmap_that_is_printed() {
        let rendered = draw_label(&text_and_pictures(
            "12x40",
            "Hi",
            vec![picture_element(black_and_white(20, 20), Side::Left, 15.0)],
        ));
        let printed = rendered.canvas().pixels().filter(|p| p[0] > 127).count();
        let black = rendered.preview().pixels().filter(|p| p[0] == 0).count();
        assert_eq!(printed, black);
        assert!(rendered.raster().is_ok());
    }

    #[test]
    fn sides_can_be_typed_loosely() {
        assert_eq!("left".parse::<Side>(), Ok(Side::Left));
        assert_eq!(" Right ".parse::<Side>(), Ok(Side::Right));
        assert_eq!("r".parse::<Side>(), Ok(Side::Right));
        assert!("middle".parse::<Side>().is_err());
    }

    // ---- emoji and other characters that the font of the label lacks ------------------------------

    /// The embedded font, which lacks the emoji, and what draws them when it does.
    fn dejavu() -> Font<'static> {
        Fonts::embedded().load(&FontSpec::default()).unwrap()
    }

    /// An apple: an emoji from Unicode 6, which the built-in emoji font has and DejaVu Sans lacks.
    const APPLE: char = '\u{1f34e}';
    /// A character that no font here has.
    const NOWHERE: char = '\u{10fffd}';

    fn ink(rendered: &Rendered) -> usize {
        rendered
            .canvas()
            .pixels()
            .filter(|pixel| pixel[0] > 127)
            .count()
    }

    #[test]
    fn the_test_characters_are_what_they_are_said_to_be() {
        let fonts = Fonts::embedded();
        let dejavu = dejavu();
        assert!(!has_glyph(&dejavu, APPLE), "DejaVu Sans lacks the apple");
        assert!(
            fonts.fallback_draws(APPLE),
            "the built-in emoji font has it"
        );
        assert!(!has_glyph(&dejavu, NOWHERE) && !fonts.fallback_draws(NOWHERE));
        assert!(has_glyph(&dejavu, 'A'));
    }

    #[test]
    fn an_emoji_is_drawn_by_the_fallback_font_and_not_as_a_box() {
        let apple = render_text("12x40", &APPLE.to_string());
        let nowhere = render_text("12x40", &NOWHERE.to_string());
        // Both draw something (the second, the box that DejaVu Sans draws for what it lacks), but
        // not the same thing.
        assert!(ink(&apple) > 0 && ink(&nowhere) > 0);
        assert_ne!(apple.canvas().as_raw(), nowhere.canvas().as_raw());
        // The apple is a solid shape and the box an outline.
        assert!(
            ink(&apple) > 2 * ink(&nowhere),
            "{} against {}",
            ink(&apple),
            ink(&nowhere)
        );
    }

    #[test]
    fn text_with_an_emoji_in_it_keeps_its_letters_and_adds_the_emoji() {
        // Left aligned and at a set size, so that the letters are where they were without the emoji.
        let left_aligned = |text: &str| {
            let mut label = Label::text(stock("15-continuous"), text);
            let element = text_mut(&mut label);
            element.size = D30Scale::Value(40.0);
            element.align = Align::Left;
            render(&label, &Fonts::embedded()).unwrap()
        };
        let plain = left_aligned("Apples");
        let with_apple = left_aligned(&format!("Apples {APPLE}"));
        assert_eq!(
            ink_columns(&plain).unwrap().0,
            ink_columns(&with_apple).unwrap().0
        );
        assert!(ink_columns(&with_apple).unwrap().1 > ink_columns(&plain).unwrap().1 + 20);
        assert!(ink(&with_apple) > ink(&plain));
        assert!(with_apple.length_dots() > plain.length_dots());
    }

    #[test]
    fn text_without_anything_missing_never_uses_the_fallback_fonts() {
        let fonts = Fonts::embedded();
        let face = Face::new(dejavu(), &fonts, "Hello, tape! 12:30");
        assert!(face.fallbacks.is_empty());
        let face = Face::new(dejavu(), &fonts, &format!("Hello {APPLE}"));
        assert!(!face.fallbacks.is_empty());
        // A line is cut into the pieces that each font draws.
        let line = format!("ab{APPLE}{APPLE}cd");
        let runs = face.runs(&line);
        let apples = format!("{APPLE}{APPLE}");
        assert_eq!(
            runs.iter().map(|(_, text)| *text).collect::<Vec<_>>(),
            ["ab", apples.as_str(), "cd"]
        );
        assert_eq!(runs[0].0, 0);
        assert_ne!(runs[1].0, 0);
        assert_eq!(runs[2].0, 0);
        assert!(face.runs("").is_empty());
    }

    #[test]
    fn what_only_asks_for_the_emoji_form_is_not_drawn() {
        let plain = render_text("12x40", &APPLE.to_string());
        for extra in ["\u{fe0f}", "\u{fe0e}", "\u{200d}", "\u{1f3fd}", "\u{e0067}"] {
            let with_extra = render_text("12x40", &format!("{APPLE}{extra}"));
            assert_eq!(
                plain.canvas().as_raw(),
                with_extra.canvas().as_raw(),
                "U+{:04X} should not be drawn",
                extra.chars().next().unwrap() as u32
            );
        }
    }

    #[test]
    fn emoji_are_measured_so_that_the_text_fits_the_label() {
        for text in [
            APPLE.to_string(),
            format!("{APPLE}{APPLE}{APPLE}"),
            format!("Apple {APPLE}"),
            format!("{APPLE} pie\n{APPLE} jam"),
        ] {
            let rendered = render_text("12x40", &text);
            let (top, bottom) = ink_rows(&rendered).unwrap();
            let (_, last) = ink_columns(&rendered).unwrap();
            assert!(bottom < 96, "{text:?}: ink down to row {bottom}");
            assert!(last < 320, "{text:?}: ink as far as column {last}");
            assert!(
                (bottom - top) > 20,
                "{text:?}: the text is only {} dots high",
                bottom - top
            );
        }
    }

    #[test]
    fn text_with_emoji_can_be_split_over_lines() {
        let text = format!("{APPLE} apple pie {APPLE} and cream");
        for count in 1..=3 {
            let lines = split(&text, count);
            assert_eq!(lines.len(), count);
            assert_eq!(lines.join(" "), text);
        }
        let rendered = render_with("12x40", &text, Lines::Split(2));
        assert!(ink(&rendered) > 0);
    }

    #[test]
    fn a_character_that_no_font_has_is_drawn_as_the_box_and_nothing_fails() {
        for text in [
            NOWHERE.to_string(),
            format!("a{NOWHERE}b"),
            format!("{NOWHERE}{APPLE}{NOWHERE}"),
        ] {
            let rendered = render_text("12x40", &text);
            assert!(ink(&rendered) > 0, "{text:?}");
        }
    }

    #[test]
    fn a_chosen_font_that_lacks_the_emoji_gets_the_fallback_too() {
        // A font file is another font than the embedded one.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/DejaVuSans.ttf");
        let mut label = Label::text(stock("12x40"), format!("Hi {APPLE}"));
        text_mut(&mut label).font = FontSpec::parse(path);
        let rendered = render(&label, &Fonts::embedded()).unwrap();
        let plain = {
            let mut label = Label::text(stock("12x40"), "Hi ");
            text_mut(&mut label).font = FontSpec::parse(path);
            render(&label, &Fonts::embedded()).unwrap()
        };
        assert!(ink(&rendered) > ink(&plain));
    }

    #[test]
    fn an_unknown_font_is_an_error_not_a_fallback() {
        let mut label = Label::text(stock("12x40"), "Hi");
        let element = text_mut(&mut label);
        element.font = FontSpec::family("No Such Font");
        assert!(matches!(
            render(&label, &Fonts::embedded()).err().unwrap(),
            D30Error::FontNotFound { .. }
        ));
    }

    // ---- labels whose front cannot be printed (the 14 x 30 mm labels) ---------------------------

    /// How far from each end of the 14 x 30 mm label (242 dots) the content has to stay.
    const KEEP_CLEAR: u32 = 24;

    #[test]
    fn text_stays_clear_of_both_ends_of_a_label_whose_front_cannot_be_printed() {
        assert_eq!(stock("14x30").dead_front_rows(), KEEP_CLEAR as usize);
        for text in ["Hello", "Flour and sugar and more", "W"] {
            for margin in [0.0, 15.0] {
                let mut label = Label::text(stock("14x30"), text);
                text_mut(&mut label).margin = margin;
                let (first, last) = ink_columns(&draw_label(&label)).unwrap();
                assert!(first >= KEEP_CLEAR, "{text:?} margin {margin}: from {first}");
                assert!(
                    last < 242 - KEEP_CLEAR,
                    "{text:?} margin {margin}: to {last}"
                );
            }
        }
    }

    #[test]
    fn a_wider_margin_than_what_has_to_be_kept_clear_still_counts() {
        let mut label = Label::text(stock("14x30"), "Hello");
        text_mut(&mut label).margin = 40.0;
        let (first, last) = ink_columns(&draw_label(&label)).unwrap();
        assert!(first >= 40 - 1 && last < 242 - 40 + 1, "{first} {last}");
    }

    #[test]
    fn the_part_that_cannot_be_printed_costs_length_but_not_height() {
        // A word that the height of the label limits, not its length, comes out the same size on
        // a label that has such a part and on one that has not.
        let tall = |id: &str| ink_rows(&render_text(id, "H")).unwrap();
        assert_eq!(tall("14x30"), tall("14x31"));
        // And text that is limited by the length is smaller than on a label without the part.
        let long = "Pepper and salt and rice";
        let on = |id: &str| ink_columns(&render_text(id, long)).unwrap();
        let (first, last) = on("14x30");
        let (plain_first, plain_last) = on("14x31");
        assert!(last - first < plain_last - plain_first);
    }

    #[test]
    fn a_picture_stays_clear_of_the_ends_of_such_a_label() {
        for margin in [0.0, 15.0] {
            let alone = pictures_only(
                "14x30",
                vec![picture_element(black(400, 40), Side::Left, margin)],
            );
            let (first, last) = ink_columns(&draw_label(&alone)).unwrap();
            assert!(first >= KEEP_CLEAR && last < 242 - KEEP_CLEAR, "{first} {last}");

            let beside_text = text_and_pictures(
                "14x30",
                "Hi",
                vec![
                    picture_element(black(20, 20), Side::Left, margin),
                    picture_element(black(20, 20), Side::Right, margin),
                ],
            );
            let (first, last) = ink_columns(&draw_label(&beside_text)).unwrap();
            assert!(first >= KEEP_CLEAR && last < 242 - KEEP_CLEAR, "{first} {last}");
        }
    }

    #[test]
    fn the_rows_of_a_counted_label_are_one_label_and_its_gap_long_and_leave_out_its_front() {
        let stock = stock("14x30");
        let plan = stock.tape_plan(None, None, None);
        let rendered = render_text("14x30", "Hello");
        let all = rendered.raster().unwrap();
        let raster = rendered.raster_for(&plan).unwrap();

        // 30.2 mm of label and 8 mm of gap, 8 rows to the millimetre.
        assert_eq!(all.rows(), 242);
        assert_eq!(raster.rows(), 306);
        // The label's rows start 24 rows in, and the rest of the 306 are blank.
        let row = all.width_bytes();
        assert_eq!(&raster.bytes()[..218 * row], &all.bytes()[24 * row..]);
        assert!(raster.bytes()[218 * row..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn labels_that_are_left_to_the_printer_are_sent_whole() {
        let rendered = render_text("12x40", "Hello");
        let plan = stock("12x40").tape_plan(None, None, None);
        assert_eq!(rendered.raster_for(&plan).unwrap(), rendered.raster().unwrap());
    }
}
