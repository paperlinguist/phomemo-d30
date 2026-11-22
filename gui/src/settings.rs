//! What the person has set up for the label, and the little that is remembered between runs.

use std::{fs, io, path::PathBuf, str::FromStr};

use d30::{
    fonts::{FontSource, FontSpec},
    picture::{Picture, Tone},
    render::{Align, Element, Label, Lines, PictureElement, Side, TextElement},
    stock::{LabelStock, TapePlan, DEFAULT_STOCK_ID},
    D30Scale,
};
use serde::{Deserialize, Serialize};

/// The most lines a text can be split into, as far as what is remembered goes. The window offers
/// fewer: a label this high has no room for more.
const MOST_LINES: u32 = 8;

/// Everything that can be changed about a label and how it is printed.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelSettings {
    pub text: String,
    /// Whether the lines break where the text does, or the program splits it over some lines.
    pub lines: Lines,
    /// The name of a kind of label stock from `d30::stock::STOCKS`, e.g. `12x40`.
    pub stock_id: String,
    /// A size typed by hand, e.g. `12x35`. If it is not empty, it is used instead of `stock_id`.
    pub custom_stock: String,
    /// For uncut tape: cut the label to fit its text, instead of making it `length_mm` long.
    pub fit_length: bool,
    pub length_mm: f32,
    /// An installed font by family name, or `None` for the one built into the program.
    pub font: Option<String>,
    pub bold: bool,
    pub italic: bool,
    /// Make the text as large as fits the label, instead of `size`.
    pub auto_size: bool,
    pub size: f32,
    pub align: Align,
    pub margin: f32,
    /// A picture on the label, if there is one.
    pub picture: Option<PictureSettings>,
    /// Blank rows to print before and after a label, or `None` for what suits the label stock.
    pub lead_in: Option<usize>,
    pub feed: Option<usize>,
    pub copies: u32,
}

impl Default for LabelSettings {
    fn default() -> Self {
        LabelSettings {
            text: "Hello".to_string(),
            lines: Lines::AsTyped,
            stock_id: DEFAULT_STOCK_ID.to_string(),
            custom_stock: String::new(),
            fit_length: true,
            length_mm: 40.0,
            font: None,
            bold: false,
            italic: false,
            auto_size: true,
            size: 48.0,
            align: Align::Center,
            margin: 15.0,
            picture: None,
            lead_in: None,
            feed: None,
            copies: 1,
        }
    }
}

impl LabelSettings {
    /// The label stock that is chosen: the size typed by hand if there is one, else the preset.
    pub fn stock(&self) -> Result<LabelStock, String> {
        let custom = self.custom_stock.trim();
        LabelStock::parse(if custom.is_empty() {
            &self.stock_id
        } else {
            custom
        })
    }

    /// The label to draw.
    pub fn label(&self) -> Result<Label, String> {
        let stock = self.stock()?;
        let mut text = TextElement::new(self.text.clone());
        text.font = FontSpec {
            source: match &self.font {
                Some(family) => FontSource::Family(family.clone()),
                None => FontSource::Embedded,
            },
            bold: self.bold,
            italic: self.italic,
        };
        text.size = if self.auto_size {
            D30Scale::Auto { minus: 0.0 }
        } else {
            D30Scale::Value(self.size)
        };
        text.align = self.align;
        text.margin = self.margin;
        text.lines = self.lines;

        let mut label = Label::new(stock);
        if label.stock.is_continuous() && !self.fit_length {
            label.length_mm = Some(self.length_mm);
        }
        label.elements.push(Element::Text(text));
        if let Some(picture) = &self.picture {
            label
                .elements
                .push(Element::Picture(picture.element(self.margin)));
        }
        Ok(label)
    }

    /// How the rows of the label go onto the tape: the blank rows before and after it are what
    /// was set, or what the stock needs.
    pub fn tape_plan(&self, stock: &LabelStock) -> TapePlan {
        stock.tape_plan(None, self.lead_in, self.feed)
    }
}

/// A picture on the label, and how it is turned into dots.
#[derive(Clone, Debug, PartialEq)]
pub struct PictureSettings {
    /// The file the picture was read from.
    pub path: PathBuf,
    pub picture: Picture,
    /// Which end of the label it is at.
    pub side: Side,
    /// Dots in patterns that follow the grays (for photographs), instead of black and white at
    /// the `level`.
    pub dither: bool,
    /// Darker than this (0 to 255) is black, when it is not dithered.
    pub level: u8,
    /// Swap black and white.
    pub invert: bool,
    /// As high as the label allows, instead of `height`.
    pub fit_height: bool,
    /// How high it is when it is not as high as the label allows, in dots.
    pub height: f32,
}

impl PictureSettings {
    /// A picture read from `path`, at the left end of the label, as high as it can be, in black
    /// and white.
    pub fn new(path: PathBuf, picture: Picture) -> Self {
        PictureSettings {
            path,
            picture,
            side: Side::Left,
            dither: false,
            level: 128,
            invert: false,
            fit_height: true,
            height: 48.0,
        }
    }

    /// The picture as an element of a label, with `margin` dots around it.
    fn element(&self, margin: f32) -> PictureElement {
        let mut element = PictureElement::new(self.picture.clone());
        element.side = self.side;
        element.tone = if self.dither {
            Tone::Dither
        } else {
            Tone::Threshold(self.level)
        };
        element.invert = self.invert;
        element.height = (!self.fit_height).then_some(self.height);
        element.margin = margin;
        element
    }
}

/// What is remembered of the picture on the label: where it came from and how it is set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedPicture {
    pub path: Option<String>,
    pub side: Option<String>,
    pub dither: Option<bool>,
    pub level: Option<u8>,
    pub invert: Option<bool>,
    pub fit_height: Option<bool>,
    pub height: Option<f32>,
}

impl SavedPicture {
    /// What to remember of a picture. A file whose name is not text cannot be remembered.
    fn new(picture: &PictureSettings) -> Option<Self> {
        Some(SavedPicture {
            path: Some(picture.path.to_str()?.to_string()),
            side: Some(side_name(picture.side).to_string()),
            dither: Some(picture.dither),
            level: Some(picture.level),
            invert: Some(picture.invert),
            fit_height: Some(picture.fit_height),
            height: Some(picture.height),
        })
    }

    /// The file to read the picture from again.
    pub fn path(&self) -> Option<PathBuf> {
        self.path.as_deref().map(PathBuf::from)
    }

    /// The picture's settings, from the defaults with what was remembered on top, for a picture
    /// that has been read again from `path`.
    pub fn settings(&self, path: PathBuf, picture: Picture) -> PictureSettings {
        let mut settings = PictureSettings::new(path, picture);
        if let Some(side) = self
            .side
            .as_deref()
            .and_then(|name| Side::from_str(name).ok())
        {
            settings.side = side;
        }
        settings.dither = self.dither.unwrap_or(settings.dither);
        settings.level = self.level.unwrap_or(settings.level);
        settings.invert = self.invert.unwrap_or(settings.invert);
        settings.fit_height = self.fit_height.unwrap_or(settings.fit_height);
        if let Some(height) = self.height.filter(|height| height.is_finite()) {
            settings.height = height.clamp(1.0, 96.0);
        }
        settings
    }
}

/// What is remembered from one run to the next.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    /// The address of the printer that was used last, as `AA:BB:CC:DD:EE:FF`.
    pub printer: Option<String>,
    pub text: Option<String>,
    /// How many lines the text is split into, or 0 for where it has new lines.
    pub lines: Option<u32>,
    pub stock: Option<String>,
    pub custom_stock: Option<String>,
    pub font: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub align: Option<String>,
    pub margin: Option<f32>,
    pub copies: Option<u32>,
    /// The picture on the label. It is the last field because it is a table in the file, and a
    /// table cannot be followed by plain values.
    pub picture: Option<SavedPicture>,
}

impl Saved {
    pub fn new(settings: &LabelSettings, printer: Option<String>) -> Self {
        Saved {
            printer,
            text: Some(settings.text.clone()),
            lines: Some(match settings.lines {
                Lines::AsTyped => 0,
                Lines::Split(count) => u32::try_from(count).unwrap_or(MOST_LINES),
            }),
            stock: Some(settings.stock_id.clone()),
            custom_stock: Some(settings.custom_stock.clone()),
            font: settings.font.clone(),
            bold: Some(settings.bold),
            italic: Some(settings.italic),
            align: Some(align_name(settings.align).to_string()),
            margin: Some(settings.margin),
            copies: Some(settings.copies),
            picture: settings.picture.as_ref().and_then(SavedPicture::new),
        }
    }

    /// Settings that start from the defaults and take what was remembered on top of them.
    pub fn settings(&self) -> LabelSettings {
        let mut settings = LabelSettings::default();
        if let Some(text) = &self.text {
            settings.text = text.clone();
        }
        if let Some(lines) = self.lines {
            settings.lines = match lines {
                0 => Lines::AsTyped,
                count => Lines::Split(count.min(MOST_LINES) as usize),
            };
        }
        if let Some(stock) = &self.stock {
            settings.stock_id = stock.clone();
        }
        if let Some(custom) = &self.custom_stock {
            settings.custom_stock = custom.clone();
        }
        settings.font = self.font.clone();
        settings.bold = self.bold.unwrap_or(settings.bold);
        settings.italic = self.italic.unwrap_or(settings.italic);
        if let Some(align) = self
            .align
            .as_deref()
            .and_then(|name| Align::from_str(name).ok())
        {
            settings.align = align;
        }
        if let Some(margin) = self.margin {
            settings.margin = margin;
        }
        if let Some(copies) = self.copies {
            settings.copies = copies.clamp(1, 99);
        }
        settings
    }

    fn path() -> io::Result<PathBuf> {
        d30::place_config_file("phomemo-gui-config.toml")
            .map_err(|error| io::Error::other(error.to_string()))
    }

    /// Reads what was remembered, or nothing if it cannot be read.
    pub fn load() -> Saved {
        Self::path()
            .and_then(fs::read_to_string)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        fs::write(Self::path()?, text)
    }
}

pub fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
    }
}

pub fn align_name(align: Align) -> &'static str {
    match align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_element(label: &Label) -> &TextElement {
        let Element::Text(text) = &label.elements[0] else {
            panic!("the label should have a block of text");
        };
        text
    }

    #[test]
    fn the_default_settings_make_the_same_label_as_the_command_line_does() {
        let label = LabelSettings::default().label().unwrap();
        assert_eq!(label.stock.id, "12x40");
        assert_eq!(label.length_mm, None);
        let text = text_element(&label);
        assert_eq!(text.margin, 15.0);
        assert_eq!(text.align, Align::Center);
        assert!(matches!(text.size, D30Scale::Auto { minus } if minus == 0.0));
        assert_eq!(text.font, FontSpec::default());
    }

    #[test]
    fn a_size_typed_by_hand_takes_the_place_of_the_preset() {
        let mut settings = LabelSettings {
            custom_stock: " 12x35 ".to_string(),
            ..Default::default()
        };
        assert_eq!(settings.stock().unwrap().id, "12x35");
        settings.custom_stock = "banana".to_string();
        assert!(settings.stock().is_err());
        assert!(settings.label().is_err());
        settings.custom_stock.clear();
        assert_eq!(settings.stock().unwrap().id, "12x40");
    }

    #[test]
    fn uncut_tape_is_cut_to_fit_or_given_a_length() {
        let mut settings = LabelSettings {
            stock_id: "15-continuous".to_string(),
            length_mm: 60.0,
            ..Default::default()
        };
        assert_eq!(settings.label().unwrap().length_mm, None);
        settings.fit_length = false;
        assert_eq!(settings.label().unwrap().length_mm, Some(60.0));
        // A precut label has a length of its own, whatever is set here.
        settings.stock_id = "12x40".to_string();
        assert_eq!(settings.label().unwrap().length_mm, None);
    }

    #[test]
    fn fonts_and_sizes_are_passed_on() {
        let settings = LabelSettings {
            font: Some("Arial".to_string()),
            bold: true,
            auto_size: false,
            size: 33.0,
            align: Align::Right,
            margin: 7.0,
            ..Default::default()
        };
        let label = settings.label().unwrap();
        let text = text_element(&label);
        assert_eq!(
            text.font,
            FontSpec {
                source: FontSource::Family("Arial".to_string()),
                bold: true,
                italic: false
            }
        );
        assert!(matches!(text.size, D30Scale::Value(size) if size == 33.0));
        assert_eq!((text.align, text.margin), (Align::Right, 7.0));
    }

    #[test]
    fn lead_in_and_feed_default_to_what_the_stock_needs() {
        let mut settings = LabelSettings::default();
        let precut = LabelStock::parse("12x40").unwrap();
        let tape = LabelStock::parse("15-continuous").unwrap();
        let counted = LabelStock::parse("14x30").unwrap();
        let rows = |plan: TapePlan| (plan.lead_rows, plan.feed_rows);
        assert_eq!(rows(settings.tape_plan(&precut)), (0, 0));
        assert_eq!(
            rows(settings.tape_plan(&tape)),
            (tape.default_lead_rows(), tape.default_feed_rows())
        );
        // The 14 x 30 mm labels are counted: nothing before, and the gap plus the part of the
        // label that cannot be printed (8 mm + 3 mm = 88 rows) after, printed as uncut tape.
        let plan = settings.tape_plan(&counted);
        assert_eq!(
            (plan.media, rows(plan)),
            (d30::stock::Media::Continuous, (0, 88))
        );
        settings.lead_in = Some(5);
        settings.feed = Some(9);
        assert_eq!(rows(settings.tape_plan(&tape)), (5, 9));
        assert_eq!(rows(settings.tape_plan(&counted)), (5, 9));
    }

    #[test]
    fn what_is_remembered_survives_a_round_trip_through_toml() {
        let settings = LabelSettings {
            text: "Kitchen\nPantry".to_string(),
            lines: Lines::Split(3),
            stock_id: "15-continuous".to_string(),
            font: Some("Comic Sans MS".to_string()),
            italic: true,
            align: Align::Left,
            copies: 3,
            ..Default::default()
        };
        let saved = Saved::new(&settings, Some("AA:BB:CC:DD:EE:FF".to_string()));
        let text = toml::to_string_pretty(&saved).unwrap();
        let back: Saved = toml::from_str(&text).unwrap();
        assert_eq!(back, saved);
        assert_eq!(back.printer.as_deref(), Some("AA:BB:CC:DD:EE:FF"));
        let restored = back.settings();
        assert_eq!(restored.text, settings.text);
        assert_eq!(restored.stock_id, "15-continuous");
        assert_eq!(restored.font.as_deref(), Some("Comic Sans MS"));
        assert!(restored.italic);
        assert_eq!(restored.align, Align::Left);
        assert_eq!(restored.copies, 3);
        assert_eq!(restored.lines, Lines::Split(3));
    }

    fn a_picture() -> Picture {
        Picture::new(&image::DynamicImage::ImageLuma8(image::GrayImage::new(
            4, 4,
        )))
        .unwrap()
    }

    fn picture_element(label: &Label) -> &PictureElement {
        let Some(Element::Picture(picture)) = label.elements.get(1) else {
            panic!("the label should have a picture after the text");
        };
        picture
    }

    #[test]
    fn a_picture_goes_on_the_label_with_the_way_it_is_set() {
        assert_eq!(LabelSettings::default().label().unwrap().elements.len(), 1);

        let mut picture = PictureSettings::new("jar.png".into(), a_picture());
        let settings = LabelSettings {
            margin: 9.0,
            picture: Some(picture.clone()),
            ..Default::default()
        };
        let label = settings.label().unwrap();
        let element = picture_element(&label);
        assert_eq!(element.picture, picture.picture);
        assert_eq!((element.side, element.margin), (Side::Left, 9.0));
        assert_eq!(element.tone, Tone::Threshold(128));
        assert!(!element.invert);
        assert_eq!(element.height, None);

        picture.side = Side::Right;
        picture.dither = true;
        picture.invert = true;
        picture.fit_height = false;
        picture.height = 40.0;
        let settings = LabelSettings {
            picture: Some(picture.clone()),
            ..Default::default()
        };
        let label = settings.label().unwrap();
        let element = picture_element(&label);
        assert_eq!(element.side, Side::Right);
        assert_eq!(element.tone, Tone::Dither);
        assert!(element.invert);
        assert_eq!(element.height, Some(40.0));

        picture.dither = false;
        picture.level = 77;
        let settings = LabelSettings {
            picture: Some(picture),
            ..Default::default()
        };
        assert_eq!(
            picture_element(&settings.label().unwrap()).tone,
            Tone::Threshold(77)
        );
    }

    #[test]
    fn the_picture_and_how_it_is_set_survive_being_saved() {
        let mut picture = PictureSettings::new("C:\\Pictures\\jar.png".into(), a_picture());
        picture.side = Side::Right;
        picture.dither = true;
        picture.level = 90;
        picture.invert = true;
        picture.fit_height = false;
        picture.height = 33.0;
        let settings = LabelSettings {
            picture: Some(picture.clone()),
            ..Default::default()
        };

        let text = toml::to_string_pretty(&Saved::new(&settings, None)).unwrap();
        let saved: Saved = toml::from_str(&text).unwrap();
        let remembered = saved.picture.expect("the picture is remembered");
        // The picture itself is read again from its file.
        assert_eq!(
            remembered.path(),
            Some(PathBuf::from("C:\\Pictures\\jar.png"))
        );
        let restored = remembered.settings(remembered.path().unwrap(), a_picture());
        assert_eq!(restored.path, picture.path);
        assert_eq!(
            (
                restored.side,
                restored.dither,
                restored.level,
                restored.invert
            ),
            (Side::Right, true, 90, true)
        );
        assert_eq!((restored.fit_height, restored.height), (false, 33.0));

        // No picture, no table in the file.
        let none = toml::to_string_pretty(&Saved::new(&LabelSettings::default(), None)).unwrap();
        assert!(!none.contains("picture"), "{none}");
    }

    #[test]
    fn what_is_remembered_about_a_picture_makes_sense_whatever_the_file_says() {
        let saved: Saved = toml::from_str("[picture]\npath = \"jar.png\"").unwrap();
        let picture = saved.picture.unwrap();
        // Everything that is not there is as for a new picture.
        let same = a_picture();
        assert_eq!(
            picture.settings("jar.png".into(), same.clone()),
            PictureSettings::new("jar.png".into(), same)
        );

        let silly: Saved =
            toml::from_str("[picture]\nside = \"up\"\nheight = 5000.0\nlevel = 3").unwrap();
        let settings = silly
            .picture
            .unwrap()
            .settings("jar.png".into(), a_picture());
        assert_eq!(settings.side, Side::Left);
        assert_eq!(settings.height, 96.0);
        assert_eq!(settings.level, 3);
        assert_eq!(picture.path(), Some(PathBuf::from("jar.png")));
        // Without a path there is nothing to read again.
        assert_eq!(SavedPicture::default().path(), None);
    }

    #[test]
    fn a_text_with_emoji_survives_being_saved() {
        let settings = LabelSettings {
            text: "🍎 Apples\n🥕 Carrots ☕".to_string(),
            ..Default::default()
        };
        let text = toml::to_string_pretty(&Saved::new(&settings, None)).unwrap();
        let saved: Saved = toml::from_str(&text).unwrap();
        assert_eq!(saved.settings().text, settings.text);
    }

    #[test]
    fn the_lines_setting_is_passed_on_to_the_text() {
        let label = LabelSettings::default().label().unwrap();
        assert_eq!(text_element(&label).lines, Lines::AsTyped);
        let settings = LabelSettings {
            lines: Lines::Split(2),
            ..Default::default()
        };
        let label = settings.label().unwrap();
        assert_eq!(text_element(&label).lines, Lines::Split(2));
    }

    #[test]
    fn the_remembered_number_of_lines_makes_sense_whatever_the_file_says() {
        let lines = |text: &str| toml::from_str::<Saved>(text).unwrap().settings().lines;
        assert_eq!(lines(""), Lines::AsTyped);
        assert_eq!(lines("lines = 0"), Lines::AsTyped);
        assert_eq!(lines("lines = 2"), Lines::Split(2));
        assert_eq!(lines("lines = 500"), Lines::Split(MOST_LINES as usize));
    }

    #[test]
    fn an_empty_or_partial_file_gives_the_defaults() {
        let nothing: Saved = toml::from_str("").unwrap();
        assert_eq!(nothing.settings(), LabelSettings::default());
        let partial: Saved = toml::from_str("stock = \"14x30\"\nunknown_future_key = 1").unwrap();
        assert_eq!(partial.settings().stock_id, "14x30");
        // A copy count that makes no sense is brought back into range.
        let silly: Saved = toml::from_str("copies = 500").unwrap();
        assert_eq!(silly.settings().copies, 99);
    }
}
