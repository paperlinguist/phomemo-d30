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
    /// The gap between two labels on the roll, in mm, where it is known. Labels with a known gap are
    /// not left to the printer's own label sensing, which cannot print the whole of a label with a
    /// wide gap: the program counts the rows instead, see [`LabelStock::tape_plan`].
    pub gap_mm: Option<f32>,
    /// Whether this has been tried on a real printer.
    pub tested: bool,
}

const fn precut(
    id: &'static str,
    name: &'static str,
    width_mm: f32,
    length_mm: f32,
    gap_mm: Option<f32>,
    tested: bool,
) -> LabelStock {
    LabelStock {
        id: Cow::Borrowed(id),
        name: Cow::Borrowed(name),
        width_mm,
        length_mm: Some(length_mm),
        media: Media::Gap,
        gap_mm,
        tested,
    }
}

const fn continuous(
    id: &'static str,
    name: &'static str,
    width_mm: f32,
    tested: bool,
) -> LabelStock {
    LabelStock {
        id: Cow::Borrowed(id),
        name: Cow::Borrowed(name),
        width_mm,
        length_mm: None,
        media: Media::Continuous,
        gap_mm: None,
        tested,
    }
}

/// How far the tear edge of the printer is beyond the print head, in mm: measured on a D30 as about
/// 7 mm (roughly 55 rows).
pub const HEAD_TO_TEAR_MM: f32 = 7.0;

/// Where the printer's own label sensing leaves the print head when a print in gap mode is done: about
/// 0.6 mm past the back edge of the label it printed, at the start of the gap. Measured on 14 × 30 mm
/// labels, and the same after every print.
pub const GAP_MODE_PARK_MM: f32 = 0.6;

/// How the rows of one label go onto the tape: what the printer is told about the stock, how many
/// rows of the label to leave out, and how many blank rows come before and after it. See
/// [`LabelStock::tape_plan`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TapePlan {
    /// What the printer is told about the stock.
    pub media: Media,
    /// Rows at the front of the label that are not printed, because the print head is already past
    /// them.
    pub skip_rows: usize,
    /// Blank rows to print before the label.
    pub lead_rows: usize,
    /// Blank rows to feed after the label.
    pub feed_rows: usize,
}

/// How many blank rows to print in front of a label on uncut tape.
///
/// The first rows a printer prints can come out squashed together while the tape gets moving. On a
/// D30 the last `e` and the `!` of a label ran into each other with 15 blank dots in front of them,
/// and came out clear of each other with 45. A label has a margin of 15 dots of its own, so this
/// adds the other 30.
pub const CONTINUOUS_LEAD_ROWS: usize = 30;

/// How many blank rows to feed after a label on uncut tape, so that the label comes far enough out
/// of the printer to be torn off with its text in the middle.
///
/// The print head sits about 7 mm (roughly 55 rows) behind the tear edge, so the label's last dots
/// are still inside the printer when it has been printed. Torn off at the edge, the blank tape on
/// the end side of the text is then its margin, the lead-in and those 7 mm, and the start side
/// only gets what is fed beyond the 7 mm. Feeding twice the 7 mm plus the lead-in would even them
/// out (140 rows). Measured on a D30 with 15 mm tape: 72 rows left about 4 mm on the start side
/// against 9 to 10 mm on the end side, and 4 mm more (32 rows) looked right, which with the
/// default margin and lead-in comes to 134.
pub const CONTINUOUS_FEED_ROWS: usize = 134;

/// The label stock Phomemo lists for the D30 (and the sizes from other D30 drivers).
///
/// Anything else can be given as `WIDTHxLENGTH`, for example `12x35`, or `WIDTH-continuous`.
pub static STOCKS: &[LabelStock] = &[
    precut("12x40", "12 × 40 mm precut labels", 12.0, 40.0, None, true),
    precut("12x30", "12 × 30 mm precut labels", 12.0, 30.0, None, false),
    precut("14x40", "14 × 40 mm precut labels", 14.0, 40.0, None, false),
    // The gap is 8 mm on the labels it was tried with, which the program counts itself.
    precut(
        "14x30",
        "14 × 30 mm precut labels",
        14.0,
        30.0,
        Some(8.0),
        true,
    ),
    precut("14x28", "14 × 28 mm precut labels", 14.0, 28.0, None, false),
    precut("14x50", "14 × 50 mm precut labels", 14.0, 50.0, None, false),
    precut("15x50", "15 × 50 mm precut labels", 15.0, 50.0, None, false),
    continuous(
        "12-continuous",
        "12 mm continuous (uncut) tape",
        12.0,
        false,
    ),
    continuous(
        "14-continuous",
        "14 mm continuous (uncut) tape",
        14.0,
        false,
    ),
    continuous("15-continuous", "15 mm continuous (uncut) tape", 15.0, true),
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

    /// The blank rows to print before a label of this stock: none for precut labels, and
    /// [`CONTINUOUS_LEAD_ROWS`] for uncut tape.
    pub fn default_lead_rows(&self) -> usize {
        if self.is_continuous() {
            CONTINUOUS_LEAD_ROWS
        } else {
            0
        }
    }

    /// The gap between two labels, for labels of a set length whose gap is known. Those are the
    /// labels the program keeps track of itself instead of leaving it to the printer.
    pub fn counted_gap_mm(&self) -> Option<f32> {
        self.length_mm.and(self.gap_mm)
    }

    /// How the printer is told to find its way on this stock: the label's own way, except where the
    /// program counts the rows itself (see [`LabelStock::tape_plan`]), when it is told the tape is
    /// uncut so that it does not try to find the labels, and prints every row it is sent.
    pub fn print_media(&self) -> Media {
        if self.counted_gap_mm().is_some() {
            Media::Continuous
        } else {
            self.media
        }
    }

    /// How many rows at the front of a counted label (the end that comes out first) are already
    /// past the print head when printing starts, so that they cannot be printed.
    ///
    /// The tape is torn in the middle of the gap, where the cut line is, and the tear edge is
    /// [`HEAD_TO_TEAR_MM`] beyond the print head. After a tear there the head is that much less half
    /// the gap already into the next label: with the 8 mm gap of the 14 × 30 mm labels, 7 - 4 =
    /// 3 mm, which is 24 rows.
    pub fn dead_front_rows(&self) -> usize {
        self.counted_gap_mm().map_or(0, |gap| {
            mm_to_dots((HEAD_TO_TEAR_MM - gap / 2.0).max(0.0)) as usize
        })
    }

    /// How many blank rows to feed, as uncut tape, after a blank label in gap mode (which the
    /// printer finds the gap for by itself, and then parks the head at the start of the gap, see
    /// [`GAP_MODE_PARK_MM`]) to leave the tape where the program expects it when it counts the rows:
    /// the middle of the gap at the tear edge. That is the gap, what the printer parks beyond the
    /// label, and the rows that cannot be printed: 8 + 0.6 mm and 3 mm, 93 rows, for the 14 × 30 mm
    /// labels. `0` for stocks that are not counted.
    pub fn position_feed_rows(&self) -> usize {
        self.counted_gap_mm().map_or(0, |gap| {
            mm_to_dots(gap + GAP_MODE_PARK_MM) as usize + self.dead_front_rows()
        })
    }

    /// The blank rows to feed after printing a label of this stock: [`CONTINUOUS_FEED_ROWS`] for uncut
    /// tape, nothing for precut labels (the printer positions the 12 × 40 mm labels itself), and for
    /// labels that are counted (see [`LabelStock::tape_plan`]) the gap plus the rows that could not
    /// be printed, which makes the print as long as the distance from one label to the next. That
    /// is what leaves the middle of the gap at the tear edge. With the 14 × 30 mm labels it is
    /// 64 + 24 = 88 rows. It can be set from the command line (`--feed-lines`) and in the window.
    pub fn default_feed_rows(&self) -> usize {
        if self.is_continuous() {
            CONTINUOUS_FEED_ROWS
        } else {
            self.counted_gap_mm()
                .map_or(0, |gap| mm_to_dots(gap) as usize + self.dead_front_rows())
        }
    }

    /// How to lay out the rows of one label on the tape, for printing with `media` (the stock's
    /// own way by default), `lead` blank rows in front and `feed` blank rows behind the label
    /// (the defaults of the stock where they are `None`).
    ///
    /// **Counted labels.** The D30's own way of finding labels (gaps) starts the print in the gap
    /// behind the label before, about 9 mm ahead of the label for the 14 × 30 mm labels, and stops
    /// it when the label is over, so that the last 8 mm of the label cannot be printed. So these
    /// labels, whose gap is known, are printed as if they were uncut tape and the program keeps
    /// count. A print is exactly one label plus its gap long, 38 mm (304 rows) for the 14 × 30 mm
    /// labels, and it ends with the middle of the gap, where the cut line is, at the tear edge.
    /// The tape is torn there, which is where the next print starts: the print head then is
    /// [`LabelStock::dead_front_rows`] into the label, which is why those rows are left out.
    /// Everything stays in step as long as the tape is torn at the tear edge and not pulled or
    /// fed by hand, and the printer's button is not pressed (that feeds the tape to where the
    /// printer thinks the next label is).
    pub fn tape_plan(
        &self,
        media: Option<Media>,
        lead: Option<usize>,
        feed: Option<usize>,
    ) -> TapePlan {
        let media = media.unwrap_or_else(|| self.print_media());
        let counted = self.counted_gap_mm().is_some() && media == Media::Continuous;
        let default_feed = if counted || self.is_continuous() {
            self.default_feed_rows()
        } else {
            0
        };
        TapePlan {
            media,
            skip_rows: if counted { self.dead_front_rows() } else { 0 },
            lead_rows: lead.unwrap_or_else(|| self.default_lead_rows()),
            feed_rows: feed.unwrap_or(default_feed),
        }
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
                    gap_mm: None,
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
                    gap_mm: None,
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
    fn uncut_tape_gets_a_lead_in_and_a_feed_and_precut_labels_get_neither() {
        for id in ["12x40", "14x50"] {
            let stock = LabelStock::parse(id).unwrap();
            assert_eq!(stock.default_lead_rows(), 0, "{id}");
            assert_eq!(stock.default_feed_rows(), 0, "{id}");
        }
        for id in ["12-continuous", "15-continuous", "13-continuous"] {
            let stock = LabelStock::parse(id).unwrap();
            assert_eq!(stock.default_lead_rows(), CONTINUOUS_LEAD_ROWS, "{id}");
            assert_eq!(stock.default_feed_rows(), CONTINUOUS_FEED_ROWS, "{id}");
        }
        // The head is about 7 mm (56 rows) behind the tear edge; the feed has to clear that.
        assert!(CONTINUOUS_FEED_ROWS as f32 > 7.0 * DOTS_PER_MM);
    }

    #[test]
    fn labels_with_a_known_gap_are_counted_and_end_with_the_middle_of_the_gap_at_the_tear_edge() {
        let stock = LabelStock::parse("14x30").unwrap();
        assert_eq!(stock.gap_mm, Some(8.0));
        assert_eq!(stock.counted_gap_mm(), Some(8.0));
        assert_eq!(stock.media, Media::Gap);
        assert_eq!(stock.print_media(), Media::Continuous);
        // The middle of the gap is 4 mm from the label and the tear edge 7 mm from the head, so
        // the head starts 3 mm (24 rows) into each label.
        assert_eq!(stock.dead_front_rows(), 24);
        // The gap is 64 rows, and with the rows that were left out that makes 88.
        assert_eq!(stock.default_feed_rows(), 88);
        assert_eq!(stock.default_lead_rows(), 0);
        assert_eq!(
            stock.tape_plan(None, None, None),
            TapePlan {
                media: Media::Continuous,
                skip_rows: 24,
                lead_rows: 0,
                feed_rows: 88,
            }
        );

        // Getting there from where the printer parks the head after a blank label (8 + 0.6 mm
        // past the label, then the 24 rows that cannot be printed): 69 + 24 = 93 rows.
        assert_eq!(stock.position_feed_rows(), 93);
        assert_eq!(LabelStock::parse("12x40").unwrap().position_feed_rows(), 0);

        // One print is one label plus its gap long (30 + 8 mm = 304 rows), so that the next print
        // starts where this one ends, in step with the labels.
        let plan = stock.tape_plan(None, None, None);
        let label_rows = stock.length_dots().unwrap() as usize;
        assert_eq!(
            label_rows - plan.skip_rows + plan.lead_rows + plan.feed_rows,
            mm_to_dots(30.0 + 8.0) as usize
        );

        // The head must not be further into the label than the tear edge leaves it: the rows that
        // cannot be printed are fewer than the rows of the label.
        for stock in STOCKS.iter().filter(|stock| stock.gap_mm.is_some()) {
            assert!(stock.dead_front_rows() < stock.length_dots().unwrap() as usize);
        }
    }

    #[test]
    fn other_stocks_are_left_to_the_printer_and_to_the_defaults_they_had() {
        // Where the gap is not known nothing is counted and nothing is fed: the 12 x 40 mm labels
        // come out far enough by themselves, and a size that is typed in could be anything.
        for id in ["12x40", "14x50", "14x31", "12x35"] {
            let stock = LabelStock::parse(id).unwrap();
            assert_eq!(stock.counted_gap_mm(), None, "{id}");
            assert_eq!(stock.print_media(), Media::Gap, "{id}");
            assert_eq!(stock.dead_front_rows(), 0, "{id}");
            assert_eq!(
                stock.tape_plan(None, None, None),
                TapePlan {
                    media: Media::Gap,
                    skip_rows: 0,
                    lead_rows: 0,
                    feed_rows: 0,
                },
                "{id}"
            );
        }
        let tape = LabelStock::parse("15-continuous").unwrap();
        assert_eq!(tape.print_media(), Media::Continuous);
        assert_eq!(
            tape.tape_plan(None, None, None),
            TapePlan {
                media: Media::Continuous,
                skip_rows: 0,
                lead_rows: CONTINUOUS_LEAD_ROWS,
                feed_rows: CONTINUOUS_FEED_ROWS,
            }
        );
    }

    #[test]
    fn what_is_asked_for_replaces_the_defaults_and_another_media_turns_counting_off() {
        let counted = LabelStock::parse("14x30").unwrap();
        let plan = counted.tape_plan(None, Some(5), Some(7));
        assert_eq!(
            (plan.skip_rows, plan.lead_rows, plan.feed_rows),
            (24, 5, 7)
        );

        // Told to leave it to the printer, the labels are not counted: nothing is left out or fed.
        let plan = counted.tape_plan(Some(Media::Gap), None, None);
        assert_eq!(
            plan,
            TapePlan {
                media: Media::Gap,
                skip_rows: 0,
                lead_rows: 0,
                feed_rows: 0,
            }
        );

        // Uncut tape on a label whose gap is not known has no front to leave out.
        let plain = LabelStock::parse("12x40").unwrap();
        assert_eq!(
            plain.tape_plan(Some(Media::Continuous), None, None),
            TapePlan {
                media: Media::Continuous,
                skip_rows: 0,
                lead_rows: 0,
                feed_rows: 0,
            }
        );
    }

    #[test]
    fn only_what_has_been_tried_on_a_printer_is_marked_as_tested() {
        let tested: Vec<_> = STOCKS
            .iter()
            .filter(|stock| stock.tested)
            .map(|stock| stock.id.to_string())
            .collect();
        assert_eq!(tested, ["12x40", "14x30", "15-continuous"]);
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
