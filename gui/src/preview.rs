//! Drawing the label the way it will be printed.

use d30::{
    render::Rendered,
    stock::{LabelStock, DOTS_PER_MM},
};
use eframe::egui::{
    self, pos2, vec2, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions,
};

/// A drawn label, and its picture on the screen.
pub struct Preview {
    pub rendered: Rendered,
    pub stock: LabelStock,
    texture: TextureHandle,
}

impl Preview {
    pub fn new(ctx: &egui::Context, rendered: Rendered, stock: LabelStock) -> Self {
        // The very bitmap that will be sent to the printer: black dots on white.
        let picture = rendered.preview();
        let size = [picture.width() as usize, picture.height() as usize];
        let texture = ctx.load_texture(
            "label",
            ColorImage::from_gray(size, picture.as_raw()),
            TextureOptions::NEAREST,
        );
        Preview {
            rendered,
            stock,
            texture,
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
        let band = Rect::from_min_size(
            rect.min + vec2(0.0, edge * scale),
            vec2(size.x, dots_high * scale),
        );
        painter.image(
            self.texture.id(),
            band,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_gray(110)),
            StrokeKind::Inside,
        );
    }
}
