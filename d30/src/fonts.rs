//! Finding fonts: the one built into the library, the ones installed on the computer, and font
//! files.

use std::{fs, path::PathBuf};

use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use rusttype::Font;
use snafu::{OptionExt, ResultExt};

use crate::{CouldNotInitFontSnafu, CouldNotReadFontFileSnafu, D30Error};

const EMBEDDED_FONT: &[u8] = include_bytes!("DejaVuSans.ttf");

/// The family name of the font that is built into the library.
pub const EMBEDDED_FAMILY: &str = "DejaVu Sans";

/// Where a font comes from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FontSource {
    /// DejaVu Sans, which is built into the library and so always there.
    #[default]
    Embedded,
    /// A font installed on the computer, by family name, e.g. `Arial`. Capitals do not matter.
    Family(String),
    /// A font file (`.ttf`, `.otf` or `.ttc`).
    File(PathBuf),
}

/// A font and which style of it to use.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FontSpec {
    pub source: FontSource,
    pub bold: bool,
    pub italic: bool,
}

impl FontSpec {
    pub fn family(name: impl Into<String>) -> Self {
        FontSpec {
            source: FontSource::Family(name.into()),
            ..Default::default()
        }
    }

    /// Reads what a person typed for a font: the path of an existing font file, or a family name.
    pub fn parse(name_or_path: &str) -> Self {
        let path = PathBuf::from(name_or_path);
        let looks_like_font_file = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["ttf", "otf", "ttc"]
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            });
        let source = if path.is_file() || looks_like_font_file {
            FontSource::File(path)
        } else {
            FontSource::Family(name_or_path.to_string())
        };
        FontSpec {
            source,
            ..Default::default()
        }
    }
}

/// The fonts that labels can use.
pub struct Fonts {
    /// The fonts installed on the computer, if they were asked for.
    installed: Option<Database>,
    /// The family names of everything above, plus the built-in one, in alphabetical order.
    families: Vec<String>,
}

impl Fonts {
    /// Only the font that is built into the library. It does not touch the file system.
    pub fn embedded() -> Self {
        Fonts {
            installed: None,
            families: vec![EMBEDDED_FAMILY.to_string()],
        }
    }

    /// The font built into the library and every font installed on this computer. Looking at
    /// the installed fonts takes a moment.
    pub fn system() -> Self {
        let mut database = Database::new();
        database.load_system_fonts();

        let mut families: Vec<String> = database
            .faces()
            .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
            .chain(std::iter::once(EMBEDDED_FAMILY.to_string()))
            .collect();
        families.sort_by_key(|name| name.to_lowercase());
        families.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());

        Fonts {
            installed: Some(database),
            families,
        }
    }

    /// The names of the font families that can be used, in alphabetical order.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// Loads the font for `spec`.
    pub fn load(&self, spec: &FontSpec) -> Result<Font<'static>, D30Error> {
        match &spec.source {
            FontSource::Embedded => embedded_font(),
            FontSource::File(path) => {
                let data = fs::read(path).context(CouldNotReadFontFileSnafu { path })?;
                Font::try_from_vec(data).context(CouldNotInitFontSnafu)
            }
            FontSource::Family(name) => {
                if let Some(font) = self.load_installed(name, spec) {
                    return Ok(font);
                }
                if name.eq_ignore_ascii_case(EMBEDDED_FAMILY) {
                    return embedded_font();
                }
                Err(D30Error::FontNotFound { name: name.clone() })
            }
        }
    }

    fn load_installed(&self, name: &str, spec: &FontSpec) -> Option<Font<'static>> {
        let database = self.installed.as_ref()?;
        // The database compares family names exactly, so find the name the way it is spelled.
        let wanted = name.to_lowercase();
        let family = database
            .faces()
            .flat_map(|face| face.families.iter())
            .find(|(candidate, _)| candidate.to_lowercase() == wanted)
            .map(|(candidate, _)| candidate.clone())?;

        let query = Query {
            families: &[Family::Name(&family)],
            weight: if spec.bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            },
            stretch: Stretch::Normal,
            style: if spec.italic {
                Style::Italic
            } else {
                Style::Normal
            },
        };
        let id = database.query(&query)?;
        database
            .with_face_data(id, |data, index| {
                Font::try_from_vec_and_index(data.to_vec(), index)
            })
            .flatten()
    }
}

fn embedded_font() -> Result<Font<'static>, D30Error> {
    Font::try_from_vec(EMBEDDED_FONT.to_vec()).context(CouldNotInitFontSnafu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_font_loads_without_touching_the_file_system() {
        let fonts = Fonts::embedded();
        assert_eq!(fonts.families(), [EMBEDDED_FAMILY]);
        assert!(fonts.load(&FontSpec::default()).is_ok());
    }

    #[test]
    fn the_embedded_family_can_be_asked_for_by_name_in_any_case() {
        let fonts = Fonts::embedded();
        assert!(fonts.load(&FontSpec::family("dejavu sans")).is_ok());
        assert!(fonts.load(&FontSpec::family("DEJAVU SANS")).is_ok());
    }

    #[test]
    fn an_unknown_family_is_reported_by_name() {
        let Err(error) = Fonts::embedded().load(&FontSpec::family("No Such Font")) else {
            panic!("there is no such font");
        };
        assert!(matches!(error, D30Error::FontNotFound { ref name } if name == "No Such Font"));
    }

    #[test]
    fn a_missing_font_file_is_reported_with_its_path() {
        let spec = FontSpec::parse("definitely-not-here.ttf");
        assert!(matches!(spec.source, FontSource::File(_)));
        let Err(error) = Fonts::embedded().load(&spec) else {
            panic!("there is no such file");
        };
        assert!(matches!(error, D30Error::CouldNotReadFontFile { .. }));
    }

    #[test]
    fn what_people_type_is_read_as_a_family_or_a_file() {
        assert_eq!(
            FontSpec::parse("Arial").source,
            FontSource::Family("Arial".to_string())
        );
        assert!(matches!(
            FontSpec::parse(r"C:\Windows\Fonts\arial.TTF").source,
            FontSource::File(_)
        ));
        // A file that exists is a file even if it has an unusual name.
        let here = FontSpec::parse(concat!(env!("CARGO_MANIFEST_DIR"), "/src/fonts.rs"));
        assert!(matches!(here.source, FontSource::File(_)));
    }

    #[test]
    fn the_embedded_font_bytes_are_a_real_font() {
        // Guards against the include_bytes! path silently pointing at a stub.
        assert!(EMBEDDED_FONT.len() > 100_000);
    }
}
