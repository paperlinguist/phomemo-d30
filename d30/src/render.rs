//! The label model, and the renderer that turns a label into the dots the printer burns.
//!
//! A [`Label`] is a piece of label stock plus what is on it. [`render`] draws it into a [`Rendered`]
//! label, which gives both the preview (what the label will look like) and the [`Raster`] to send to
//! the printer. They come from the same dots, so the preview is exactly what is printed.

use std::str::FromStr;

use image::{imageops, GrayImage, Luma};
use imageproc::drawing::{draw_text_mut, text_size};
use rusttype::{Font, Scale};

use crate::{
    fonts::{FontSpec, Fonts},
    protocol::{Raster, MAX_ROWS},
    stock::{mm_to_dots, LabelStock, PRINT_WIDTH_DOTS},
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
        }
    }
}

/// One thing on a label.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Element {
    Text(TextElement),
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
}

/// Draws `label`, using `fonts` to find its fonts.
pub fn render(label: &Label, fonts: &Fonts) -> Result<Rendered, D30Error> {
    let height = PRINT_WIDTH_DOTS;
    let fixed_length = fixed_length(label)?;

    let mut blocks = Vec::new();
    for element in &label.elements {
        match element {
            Element::Text(text) => {
                let block = lay_out(
                    text,
                    fonts,
                    fixed_length.map(|dots| dots as f32),
                    height as f32,
                )?;
                blocks.push((text, block));
            }
        }
    }

    // Uncut tape without a chosen length is cut to fit what is on it.
    let length = fixed_length.unwrap_or_else(|| {
        blocks
            .iter()
            .map(|(text, block)| (block.width as f32 + 2.0 * text.margin).ceil() as u32)
            .max()
            .unwrap_or(1)
            .max(1)
    });
    if length as usize > MAX_ROWS {
        return Err(invalid(format!(
            "the label would be {length} dots ({:.0} mm) long, but the printer takes at most {MAX_ROWS} dots",
            length as f32 / crate::stock::DOTS_PER_MM
        )));
    }

    let mut canvas = GrayImage::new(length, height);
    for (text, block) in &blocks {
        draw(&mut canvas, text, block);
    }
    Ok(Rendered { canvas })
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

/// A block of text with its font and size decided and measured.
struct Block {
    font: Font<'static>,
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
    let font = fonts.load(&element.font)?;
    // A newline at the very end is nearly always an accident, so it does not make a blank line.
    let lines: Vec<String> = element
        .text
        .trim_end_matches(['\n', '\r'])
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect();

    let size = match element.size {
        D30Scale::Value(size) => size,
        D30Scale::Auto { minus } => {
            let (width, height, _) = measure(&font, Scale::uniform(100.0), &lines);
            if width == 0 || height == 0 {
                1.0
            } else {
                let by_height = (box_height - 2.0 * element.margin) / height as f32;
                let by_width =
                    box_width.map(|box_width| (box_width - 2.0 * element.margin) / width as f32);
                let fit = by_width.map_or(by_height, |by_width| by_width.min(by_height));
                100.0 * fit - minus
            }
        }
    };
    // A size of nothing or less would draw nothing, or garbage.
    let scale = Scale::uniform(size.max(1.0));

    let (width, height, advance) = measure(&font, scale, &lines);
    let line_widths = lines
        .iter()
        .map(|line| text_size(scale, &font, line).0)
        .collect();
    Ok(Block {
        font,
        scale,
        lines,
        line_widths,
        advance,
        width,
        height,
    })
}

/// The width and height of the ink of a block of lines, and how far apart the lines are.
fn measure(font: &Font, scale: Scale, lines: &[String]) -> (i32, i32, i32) {
    let metrics = font.v_metrics(scale);
    let advance = (metrics.ascent - metrics.descent + metrics.line_gap).ceil() as i32;
    let width = lines
        .iter()
        .map(|line| text_size(scale, font, line).0)
        .max()
        .unwrap_or(0);
    let last_line_height = lines
        .last()
        .map_or(0, |line| text_size(scale, font, line).1);
    let height = advance * (lines.len() as i32 - 1) + last_line_height;
    (width, height, advance)
}

fn draw(canvas: &mut GrayImage, element: &TextElement, block: &Block) {
    let box_width = canvas.width() as f32;
    let box_height = canvas.height() as f32;

    let left = match element.align {
        Align::Left => element.margin as i32,
        Align::Center => ((box_width - block.width as f32) / 2.0) as i32,
        Align::Right => (box_width - element.margin - block.width as f32) as i32,
    };
    let top = ((box_height - block.height as f32) / 2.0) as i32;

    for (index, line) in block.lines.iter().enumerate() {
        let spare = block.width - block.line_widths[index];
        let indent = match element.align {
            Align::Left => 0,
            Align::Center => (spare as f32 / 2.0) as i32,
            Align::Right => spare,
        };
        draw_text_mut(
            canvas,
            Luma([255u8]),
            left + indent,
            top + index as i32 * block.advance,
            block.scale,
            &block.font,
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
                    let Element::Text(element) = &mut label.elements[0];
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
            let Element::Text(element) = &mut label.elements[0];
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
            let Element::Text(element) = &mut label.elements[0];
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

    #[test]
    fn an_unknown_font_is_an_error_not_a_fallback() {
        let mut label = Label::text(stock("12x40"), "Hi");
        let Element::Text(element) = &mut label.elements[0];
        element.font = FontSpec::family("No Such Font");
        assert!(matches!(
            render(&label, &Fonts::embedded()).err().unwrap(),
            D30Error::FontNotFound { .. }
        ));
    }
}
