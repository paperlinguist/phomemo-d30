//! The label stock the printer can be loaded with: the sizes Phomemo sells, how the printer finds
//! the end of a label, and how much of the tape the print head covers.

use std::{borrow::Cow, str::FromStr};

/// Dots per millimetre. The D30 prints at 203 dpi, which Phomemo's label sizes round to 8 dots per
/// millimetre (a 40 mm label is 320 dots long).
pub const DOTS_PER_MM: f32 = 8.0;

/// Dots the print head covers across the tape. That is 12 mm, whatever the width of the tape.
pub const PRINT_WIDTH_DOTS: u32 = 96;

/// The width of the print head in millimetres.
pub const PRINT_WIDTH_MM: f32 = 12.0;

/// The widest tape the D30 takes.
pub const MAX_TAPE_WIDTH_MM: f32 = 15.0;

/// How the printer finds where one label ends and the next begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Media {
    /// Precut labels separated by gaps, like the 12 × 40 mm labels.
    Gap,
    /// Uncut ("continuous") tape. The label is as long as what is printed on it.
    Continuous,
    /// Labels separated by a black mark on the backing paper.
    BlackMark,
}

impl Media {
    /// The byte that selects this media in the `1f 11 xx` command of the printer's init sequence.
    ///
    /// `0x0a` (gaps) is what the official app sends for the 12 × 40 mm labels. The other two come
    /// from other Phomemo printers that share the command.
    pub const fn code(self) -> u8 {
        match self {
            Media::Gap => 0x0a,
            Media::Continuous => 0x0b,
            Media::BlackMark => 0x26,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Media::Gap => "gap",
            Media::Continuous => "continuous",
            Media::BlackMark => "marks",
        }
    }
}

impl FromStr for Media {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "gap" | "gaps" | "precut" => Ok(Media::Gap),
            "continuous" | "uncut" | "tape" => Ok(Media::Continuous),
            "marks" | "mark" | "black-mark" | "blackmark" => Ok(Media::BlackMark),
            other => Err(format!(
                "unknown media `{other}`: use gap, continuous or marks"
            )),
        }
    }
}

/// One kind of label or tape.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelStock {
    /// What to type to select it, e.g. `12x40`.
    pub id: Cow<'static, str>,
    pub name: Cow<'static, str>,
    /// The width of the label or tape.
    pub width_mm: f32,
    /// The length of one label, or `None` for continuous tape.
    pub length_mm: Option<f32>,
    pub media: Media,
    /// Whether this has been tried on a real printer.
    pub tested: bool,
}

const fn precut(
    id: &'static str,
    name: &'static str,
    width_mm: f32,
    length_mm: f32,
    tested: bool,
) -> LabelStock {
    LabelStock {
        id: Cow::Borrowed(id),
        name: Cow::Borrowed(name),
        width_mm,
        length_mm: Some(length_mm),
        media: Media::Gap,
        tested,
    }
}

const fn continuous(id: &'static str, name: &'static str, width_mm: f32) -> LabelStock {
    LabelStock {
        id: Cow::Borrowed(id),
        name: Cow::Borrowed(name),
        width_mm,
        length_mm: None,
        media: Media::Continuous,
        tested: false,
    }
}

/// The label stock Phomemo lists for the D30 (and the sizes from other D30 drivers).
///
/// Anything else can be given as `WIDTHxLENGTH`, for example `12x35`, or `WIDTH-continuous`.
pub static STOCKS: &[LabelStock] = &[
    precut("12x40", "12 × 40 mm precut labels", 12.0, 40.0, true),
    precut("12x30", "12 × 30 mm precut labels", 12.0, 30.0, false),
    precut("14x40", "14 × 40 mm precut labels", 14.0, 40.0, false),
    precut("14x30", "14 × 30 mm precut labels", 14.0, 30.0, false),
    precut("14x28", "14 × 28 mm precut labels", 14.0, 28.0, false),
    precut("14x50", "14 × 50 mm precut labels", 14.0, 50.0, false),
    precut("15x50", "15 × 50 mm precut labels", 15.0, 50.0, false),
    continuous("12-continuous", "12 mm continuous (uncut) tape", 12.0),
    continuous("14-continuous", "14 mm continuous (uncut) tape", 14.0),
    continuous("15-continuous", "15 mm continuous (uncut) tape", 15.0),
];

/// What `--label` defaults to: the 12 × 40 mm precut labels.
pub const DEFAULT_STOCK_ID: &str = "12x40";

impl LabelStock {
    /// The default stock, 12 × 40 mm precut labels.
    pub fn default_stock() -> LabelStock {
        Self::parse(DEFAULT_STOCK_ID).expect("the default stock is in the list")
    }

    pub fn is_continuous(&self) -> bool {
        self.length_mm.is_none()
    }

    /// The length of one label in dots, or `None` for continuous tape.
    pub fn length_dots(&self) -> Option<u32> {
        self.length_mm.map(mm_to_dots)
    }

    /// How much tape is left on each side of the print head when the tape is wider than 12 mm.
    pub fn side_margin_mm(&self) -> f32 {
        ((self.width_mm - PRINT_WIDTH_MM) / 2.0).max(0.0)
    }

    /// Finds a stock by its id, e.g. `12x40`, `12x40mm` or `12-uncut`, or makes one from a size.
    ///
    /// * `WIDTHxLENGTH` (in mm) is a precut label of that size, e.g. `12x35`.
    /// * `WIDTH-continuous` is uncut tape of that width, e.g. `14-continuous`.
    pub fn parse(spec: &str) -> Result<LabelStock, String> {
        let wanted = normalize(spec);
        if let Some(stock) = STOCKS.iter().find(|stock| stock.id == wanted.as_str()) {
            return Ok(stock.clone());
        }

        if let Some((width, length)) = wanted.split_once('x') {
            if let (Ok(width), Ok(length)) = (width.parse::<f32>(), length.parse::<f32>()) {
                check_width(width)?;
                if length.is_nan() || length <= 0.0 {
                    return Err(format!(
                        "the label length must be more than 0 mm, got {length}"
                    ));
                }
                return Ok(LabelStock {
                    id: Cow::Owned(format!("{}x{}", number(width), number(length))),
                    name: Cow::Owned(format!(
                        "{} × {} mm precut labels (custom size)",
                        number(width),
                        number(length)
                    )),
                    width_mm: width,
                    length_mm: Some(length),
                    media: Media::Gap,
                    tested: false,
                });
            }
        }

        if let Some(width) = wanted.strip_suffix("-continuous") {
            if let Ok(width) = width.parse::<f32>() {
                check_width(width)?;
                return Ok(LabelStock {
                    id: Cow::Owned(format!("{}-continuous", number(width))),
                    name: Cow::Owned(format!(
                        "{} mm continuous (uncut) tape (custom width)",
                        number(width)
                    )),
                    width_mm: width,
                    length_mm: None,
                    media: Media::Continuous,
                    tested: false,
                });
            }
        }

        Err(format!(
            "unknown label `{spec}`. Run `d30-cli list-labels` for the list, or give a size like 12x35 or 14-continuous"
        ))
    }
}

/// `12x40mm`, `12×40`, `12 X 40` and `12x40` all mean the same; so do `12-uncut` and `12-continuous`.
fn normalize(spec: &str) -> String {
    let lowered = spec
        .trim()
        .to_lowercase()
        .replace("mm", "")
        .replace(['×', '*'], "x");
    // "12 x 40" is a size, so its spaces go; the spaces in "12 uncut" separate words.
    let joined = if lowered.contains('x') {
        lowered.replace(' ', "")
    } else {
        lowered.replace([' ', '_'], "-")
    };
    joined.replace("uncut", "continuous")
}

fn check_width(width_mm: f32) -> Result<(), String> {
    if !(PRINT_WIDTH_MM..=MAX_TAPE_WIDTH_MM).contains(&width_mm) {
        return Err(format!(
            "tape {width_mm} mm wide is not supported: the print head is {PRINT_WIDTH_MM} mm wide \
             and the printer takes tape up to {MAX_TAPE_WIDTH_MM} mm"
        ));
    }
    Ok(())
}

pub fn mm_to_dots(mm: f32) -> u32 {
    (mm * DOTS_PER_MM).round() as u32
}

/// `12.0` as `12`, `12.5` as `12.5`.
fn number(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_stock_is_the_12x40_precut_label() {
        let stock = LabelStock::default_stock();
        assert_eq!(stock.id, "12x40");
        assert_eq!(stock.length_dots(), Some(320));
        assert_eq!(stock.media, Media::Gap);
        assert!(stock.tested);
    }

    #[test]
    fn every_preset_has_a_unique_id_that_parses_back_to_itself() {
        for stock in STOCKS {
            assert_eq!(&LabelStock::parse(&stock.id).unwrap(), stock);
        }
        let mut ids: Vec<_> = STOCKS.iter().map(|stock| stock.id.to_string()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), STOCKS.len());
    }

    #[test]
    fn presets_fit_the_print_head_and_the_tape() {
        for stock in STOCKS {
            assert!(
                (PRINT_WIDTH_MM..=MAX_TAPE_WIDTH_MM).contains(&stock.width_mm),
                "{}",
                stock.id
            );
            assert_eq!(stock.is_continuous(), stock.media == Media::Continuous);
        }
    }

    #[test]
    fn ids_can_be_typed_loosely() {
        for spec in ["12x40mm", "12X40", "12 x 40", "12×40", " 12x40 "] {
            assert_eq!(LabelStock::parse(spec).unwrap().id, "12x40", "{spec}");
        }
        for spec in ["12-uncut", "12mm uncut", "12-continuous", "12MM-Continuous"] {
            assert_eq!(
                LabelStock::parse(spec).unwrap().id,
                "12-continuous",
                "{spec}"
            );
        }
    }

    #[test]
    fn other_sizes_can_be_given_directly() {
        let stock = LabelStock::parse("12x35").unwrap();
        assert_eq!((stock.width_mm, stock.length_mm), (12.0, Some(35.0)));
        assert_eq!(stock.length_dots(), Some(280));
        assert!(!stock.tested);

        let tape = LabelStock::parse("13-continuous").unwrap();
        assert_eq!((tape.width_mm, tape.length_mm), (13.0, None));
        assert_eq!(tape.media, Media::Continuous);
    }

    #[test]
    fn unsupported_sizes_are_rejected_with_a_reason() {
        assert!(LabelStock::parse("9x30")
            .unwrap_err()
            .contains("not supported"));
        assert!(LabelStock::parse("20-continuous")
            .unwrap_err()
            .contains("not supported"));
        assert!(LabelStock::parse("12x0")
            .unwrap_err()
            .contains("more than 0"));
        assert!(LabelStock::parse("banana")
            .unwrap_err()
            .contains("unknown label"));
    }

    #[test]
    fn wide_tape_leaves_margins_beside_the_print_head() {
        assert_eq!(LabelStock::parse("12x40").unwrap().side_margin_mm(), 0.0);
        assert_eq!(LabelStock::parse("14x30").unwrap().side_margin_mm(), 1.0);
        assert_eq!(LabelStock::parse("15x50").unwrap().side_margin_mm(), 1.5);
    }

    #[test]
    fn media_codes_and_names() {
        assert_eq!(Media::Gap.code(), 0x0a);
        assert_eq!(Media::Continuous.code(), 0x0b);
        assert_eq!(Media::BlackMark.code(), 0x26);
        assert_eq!("uncut".parse::<Media>().unwrap(), Media::Continuous);
        assert!("sticky".parse::<Media>().is_err());
    }
}
