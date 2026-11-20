//! Drawing the label the way it will be printed.

use d30::{
    render::Rendered,
    stock::{LabelStock, DOTS_PER_MM},
};
use eframe::egui::{
    self, pos2, vec2, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions,
};
use image::imageops;

/// A drawn label, and its picture on the screen.
pub struct Preview {
    pub rendered: Rendered,
    pub stock: LabelStock,
    /// The picture of the label in pieces that lie side by side. A graphics card only takes a
    /// texture up to some width (8192 dots is common), and a label on uncut tape can be longer.
    tiles: Vec<Tile>,
}

/// A piece of the picture of a label.
struct Tile {
    texture: TextureHandle,
    /// The dot this piece starts at, and how many dots wide it is.
    first: u32,
    width: u32,
}

/// The narrowest limit to go by. egui itself cannot work with a graphics card that takes less.
const SMALLEST_LIMIT: usize = 1024;

impl Preview {
    pub fn new(ctx: &egui::Context, rendered: Rendered, stock: LabelStock) -> Self {
        // The very bitmap that will be sent to the printer: black dots on white.
        let picture = rendered.preview();
        let height = picture.height();
        // A texture wider than this makes the graphics card refuse it, and that stops the program.
        let limit = ctx
            .input(|input| input.max_texture_side)
            .max(SMALLEST_LIMIT) as u32;
        let tiles = tile_spans(picture.width(), limit)
            .into_iter()
            .map(|(first, width)| {
                let piece = imageops::crop_imm(&picture, first, 0, width, height).to_image();
                let size = [width as usize, height as usize];
                Tile {
                    texture: ctx.load_texture(
                        "label",
                        ColorImage::from_gray(size, piece.as_raw()),
                        TextureOptions::NEAREST,
                    ),
                    first,
                    width,
                }
            })
            .collect();
        Preview {
            rendered,
            stock,
            tiles,
        }
    }

    /// The size of the label in words.
    pub fn size_text(&self) -> String {
        let dots = self.rendered.length_dots();
        let length_mm = dots as f32 / DOTS_PER_MM;
        format!(
            "{length_mm:.1} mm long, on tape {} mm wide  ({dots} × {} dots)",
            self.stock.width_mm,
            self.rendered.canvas().height()
        )
    }

    /// Draws the label, as large as fits the width that is left, with a dot never smaller than a
    /// pixel. Tape wider than the 12 mm print head has an edge on each side that nothing prints
    /// on; that is drawn as a shaded border.
    pub fn show(&self, ui: &mut egui::Ui) {
        let dots_wide = self.rendered.length_dots() as f32;
        let dots_high = self.rendered.canvas().height() as f32;
        let edge = self.stock.side_margin_mm() * DOTS_PER_MM;

        let scale = (ui.available_width() / dots_wide).clamp(1.0, 6.0);
        let size = vec2(dots_wide * scale, (dots_high + 2.0 * edge) * scale);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 0.0, Color32::from_rgb(214, 214, 206));
        let top = rect.min.y + edge * scale;
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        for tile in &self.tiles {
            let band = Rect::from_min_max(
                pos2(rect.min.x + tile.first as f32 * scale, top),
                pos2(
                    rect.min.x + (tile.first + tile.width) as f32 * scale,
                    top + dots_high * scale,
                ),
            );
            // A long label is mostly out of sight, in the scroll area.
            if painter.clip_rect().intersects(band) {
                painter.image(tile.texture.id(), band, whole, Color32::WHITE);
            }
        }
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_gray(110)),
            StrokeKind::Inside,
        );
    }
}

/// Cuts `total` dots into pieces of at most `limit` dots: the dot each piece starts at, and its
/// width. Together they cover all the dots, with nothing left out and nothing twice.
fn tile_spans(total: u32, limit: u32) -> Vec<(u32, u32)> {
    let limit = limit.max(1);
    let mut spans = Vec::new();
    let mut first = 0;
    while first < total {
        let width = (total - first).min(limit);
        spans.push((first, width));
        first += width;
    }
    spans
}

#[cfg(test)]
mod tests {
    use d30::{
        fonts::Fonts,
        render::{render, Label},
    };

    use super::*;

    #[test]
    fn the_pieces_cover_the_label_and_none_is_wider_than_the_limit() {
        for (total, limit) in [
            (1, 8),
            (8, 8),
            (9, 8),
            (100, 7),
            (8594, 8192),
            (16384, 8192),
            (16385, 8192),
            (5, 0),
        ] {
            let spans = tile_spans(total, limit);
            let mut next = 0;
            for &(first, width) in &spans {
                assert_eq!(first, next, "no gap or overlap for {total} in {limit}");
                assert!((1..=limit.max(1)).contains(&width), "{total} in {limit}");
                next += width;
            }
            assert_eq!(next, total, "all of {total} dots are covered");
        }
        assert!(tile_spans(0, 8).is_empty());
    }

    /// The window once stopped with "Dimension X value 8594 exceeds the limit of 8192": a label of
    /// uncut tape with a very long text, shown as one texture.
    #[test]
    fn a_label_longer_than_the_graphics_card_takes_is_shown_in_pieces() {
        let ctx = egui::Context::default();
        // Tell egui what the graphics card would take, the way eframe does when it starts. (1024 is
        // the least egui itself can work with.)
        let limit = 1024;
        let input = egui::RawInput {
            max_texture_side: Some(limit),
            ..Default::default()
        };
        ctx.run_ui(input, |_| {}).drop_without_applying_deltas();

        let stock = d30::stock::LabelStock::parse("15-continuous").unwrap();
        let label = Label::text(
            stock.clone(),
            "A very long text on uncut tape, far too long. ".repeat(4),
        );
        let rendered = render(&label, &Fonts::embedded()).unwrap();
        let dots = rendered.length_dots();
        assert!(
            dots as usize > 3 * limit,
            "the test needs a long label, got {dots} dots"
        );

        let preview = Preview::new(&ctx, rendered, stock);
        assert!(preview.tiles.len() >= 4);
        for tile in &preview.tiles {
            assert!(tile.texture.size()[0] <= limit, "a texture is too wide");
            assert_eq!(tile.texture.size()[1], 96);
        }
        let covered: u32 = preview.tiles.iter().map(|tile| tile.width).sum();
        assert_eq!(covered, dots);
    }
}
