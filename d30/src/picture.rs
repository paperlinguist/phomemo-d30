//! Pictures for labels: reading them, and turning them into black dots.
//!
//! A [`Picture`] is read once (it can be a file in any common format) and kept as shades of gray.
//! Putting it on a label scales it to the size it gets there and then decides, dot by dot, whether
//! the printer burns it: see [`Tone`].

use std::{fmt, path::Path, sync::Arc};

use image::{
    imageops::{self, FilterType},
    DynamicImage, GrayImage, Luma,
};

use crate::D30Error;

/// The most pixels a picture keeps when it is read. A label is 96 dots high, so a picture of a
/// million pixels has far more detail than can be printed, and a bigger one would only make drawing
/// the label slow.
const MOST_PIXELS: u64 = 1_000_000;

/// How the grays of a picture become black dots and white.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Black where the picture is darker than this level (0 to 255) and white elsewhere. Good for
    /// drawings, logos and writing.
    Threshold(u8),
    /// Dots in patterns that follow the grays, like a newspaper photograph. Good for photographs.
    Dither,
}

impl Default for Tone {
    /// Halfway between black and white.
    fn default() -> Self {
        Tone::Threshold(128)
    }
}

/// A picture to put on a label, as shades of gray with transparent parts made white.
///
/// Cloning is cheap: the picture itself is shared. For the same reason two pictures are equal when
/// they are the same one that was read (this tells whether a label has to be drawn again without
/// comparing a million pixels).
#[derive(Clone)]
pub struct Picture {
    gray: Arc<GrayImage>,
}

impl fmt::Debug for Picture {
    // Not the million pixels: this ends up in the output of failing tests.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Picture({} x {})", self.width(), self.height())
    }
}

impl PartialEq for Picture {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.gray, &other.gray)
    }
}

impl Eq for Picture {}

impl Picture {
    /// A picture from an image in memory. Colors become grays, and the transparent parts of the
    /// image become white, so that a logo on a transparent background prints as the logo and not
    /// as a black square. A very large image is made smaller, keeping its shape.
    pub fn new(image: &DynamicImage) -> Result<Picture, D30Error> {
        let (width, height) = (image.width(), image.height());
        if width == 0 || height == 0 {
            return Err(D30Error::InvalidPicture {
                reason: "the picture has no pixels".to_string(),
            });
        }
        let mut gray = flatten(image);
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MOST_PIXELS {
            let factor = (MOST_PIXELS as f64 / pixels as f64).sqrt();
            let smaller_width = ((f64::from(width) * factor).round() as u32).max(1);
            let smaller_height = ((f64::from(height) * factor).round() as u32).max(1);
            gray = imageops::resize(&gray, smaller_width, smaller_height, FilterType::Triangle);
        }
        Ok(Picture {
            gray: Arc::new(gray),
        })
    }

    /// Reads a picture from a file: PNG, JPEG, GIF, BMP, ICO, TIFF or WebP. What kind it is comes
    /// from what is in the file, not from what it is called.
    pub fn open(path: impl AsRef<Path>) -> Result<Picture, D30Error> {
        let path = path.as_ref();
        let read = || -> image::ImageResult<DynamicImage> {
            image::io::Reader::open(path)?
                .with_guessed_format()?
                .decode()
        };
        let image = read().map_err(|source| D30Error::CouldNotReadPicture {
            path: path.to_path_buf(),
            source,
        })?;
        Picture::new(&image)
    }

    pub fn width(&self) -> u32 {
        self.gray.width()
    }

    pub fn height(&self) -> u32 {
        self.gray.height()
    }

    /// The picture as dots at the size it gets on the label: 255 where the printer burns, 0
    /// elsewhere. `invert` swaps black and white first.
    pub(crate) fn dots(&self, width: u32, height: u32, tone: Tone, invert: bool) -> GrayImage {
        let (width, height) = (width.max(1), height.max(1));
        // At its own size the picture is left as it is: a QR code is made of squares of whole
        // dots, and resampling it, however gently, could blur their edges.
        let mut gray = if (width, height) == (self.gray.width(), self.gray.height()) {
            (*self.gray).clone()
        } else {
            imageops::resize(&*self.gray, width, height, FilterType::Lanczos3)
        };
        if invert {
            imageops::invert(&mut gray);
        }
        match tone {
            Tone::Threshold(level) => {
                for pixel in gray.pixels_mut() {
                    pixel.0[0] = if pixel.0[0] < level { 255 } else { 0 };
                }
                gray
            }
            Tone::Dither => dither(&gray),
        }
    }
}

/// The image as grays, with what is transparent turned white.
fn flatten(image: &DynamicImage) -> GrayImage {
    let rgba = image.to_rgba8();
    GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [red, green, blue, alpha] = rgba.get_pixel(x, y).0.map(u32::from);
        let gray = (2126 * red + 7152 * green + 722 * blue) / 10_000;
        // The color laid over white paper: all of it where it is opaque, none where it is clear.
        Luma([((gray * alpha + 255 * (255 - alpha) + 127) / 255) as u8])
    })
}

/// Floyd-Steinberg error diffusion: each gray becomes a black dot or a white one, and the error
/// that makes is passed on to the dots after it, so that the dots in an area add up to its gray.
/// The result has 255 where the printer burns (the dark dots).
fn dither(gray: &GrayImage) -> GrayImage {
    let (width, height) = (gray.width() as usize, gray.height() as usize);
    let mut level: Vec<f32> = gray.pixels().map(|pixel| f32::from(pixel.0[0])).collect();
    let mut dots = GrayImage::new(gray.width(), gray.height());
    for y in 0..height {
        for x in 0..width {
            let here = y * width + x;
            let wanted = level[here];
            let burn = wanted < 127.5;
            if burn {
                dots.put_pixel(x as u32, y as u32, Luma([255]));
            }
            let error = wanted - if burn { 0.0 } else { 255.0 };
            if x + 1 < width {
                level[here + 1] += error * 7.0 / 16.0;
            }
            if y + 1 < height {
                if x > 0 {
                    level[here + width - 1] += error * 3.0 / 16.0;
                }
                level[here + width] += error * 5.0 / 16.0;
                if x + 1 < width {
                    level[here + width + 1] += error / 16.0;
                }
            }
        }
    }
    dots
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    use super::*;

    fn gray_picture(width: u32, height: u32, gray: impl Fn(u32, u32) -> u8) -> Picture {
        let image = GrayImage::from_fn(width, height, |x, y| Luma([gray(x, y)]));
        Picture::new(&DynamicImage::ImageLuma8(image)).unwrap()
    }

    /// How many of the dots are burned.
    fn burned(dots: &GrayImage) -> usize {
        dots.pixels().filter(|dot| dot.0[0] > 127).count()
    }

    #[test]
    fn what_is_transparent_is_white_and_not_black() {
        // Invisible black: a logo's background.
        let clear = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let picture = Picture::new(&DynamicImage::ImageRgba8(clear)).unwrap();
        assert_eq!(burned(&picture.dots(8, 8, Tone::default(), false)), 0);

        let opaque = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        let picture = Picture::new(&DynamicImage::ImageRgba8(opaque)).unwrap();
        assert_eq!(burned(&picture.dots(8, 8, Tone::default(), false)), 64);

        // Half of the picture is a clear background, half a black shape.
        let half = RgbaImage::from_fn(8, 8, |x, _| {
            if x < 4 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([0, 0, 0, 255])
            }
        });
        let picture = Picture::new(&DynamicImage::ImageRgba8(half)).unwrap();
        let dots = picture.dots(8, 8, Tone::default(), false);
        assert_eq!(burned(&dots), 32);
        assert!((0..8).all(|y| dots.get_pixel(0, y).0[0] == 0 && dots.get_pixel(7, y).0[0] == 255));
    }

    #[test]
    fn colors_become_grays_by_how_light_they_look() {
        let rgb = |red, green, blue| {
            let image = image::RgbImage::from_pixel(4, 4, image::Rgb([red, green, blue]));
            Picture::new(&DynamicImage::ImageRgb8(image)).unwrap()
        };
        let burned_by = |picture: &Picture| burned(&picture.dots(4, 4, Tone::default(), false));
        // Yellow is light, blue is dark, whatever the sum of their colors.
        assert_eq!(burned_by(&rgb(255, 255, 0)), 0);
        assert_eq!(burned_by(&rgb(0, 0, 255)), 16);
        assert_eq!(burned_by(&rgb(255, 255, 255)), 0);
        assert_eq!(burned_by(&rgb(0, 0, 0)), 16);
    }

    #[test]
    fn the_level_decides_what_is_burned_and_invert_swaps_it() {
        // Dark on the left, light on the right.
        let picture = gray_picture(10, 10, |x, _| if x < 5 { 50 } else { 200 });
        let columns = |dots: &GrayImage| -> Vec<u32> {
            (0..10)
                .filter(|&x| dots.get_pixel(x, 5).0[0] > 127)
                .collect()
        };
        let left: Vec<u32> = (0..5).collect();
        let right: Vec<u32> = (5..10).collect();
        let all: Vec<u32> = (0..10).collect();

        assert_eq!(
            columns(&picture.dots(10, 10, Tone::Threshold(128), false)),
            left
        );
        assert_eq!(
            columns(&picture.dots(10, 10, Tone::Threshold(128), true)),
            right
        );
        // A level below the dark part burns nothing, one above the light part burns everything.
        assert!(columns(&picture.dots(10, 10, Tone::Threshold(40), false)).is_empty());
        assert_eq!(
            columns(&picture.dots(10, 10, Tone::Threshold(220), false)),
            all
        );
        // Pure white is never burned, whatever the level.
        let white = gray_picture(4, 4, |_, _| 255);
        assert_eq!(burned(&white.dots(4, 4, Tone::Threshold(255), false)), 0);
        assert_eq!(burned(&white.dots(4, 4, Tone::Threshold(0), false)), 0);
    }

    #[test]
    fn dots_in_a_dither_add_up_to_the_gray() {
        let share = |gray: u8| {
            let picture = gray_picture(40, 40, |_, _| gray);
            burned(&picture.dots(40, 40, Tone::Dither, false)) as f32 / 1600.0
        };
        assert_eq!(share(255), 0.0);
        assert_eq!(share(0), 1.0);
        for (gray, expected) in [(128u8, 0.5f32), (64, 0.75), (191, 0.25), (230, 0.1)] {
            let got = share(gray);
            assert!(
                (got - expected).abs() < 0.05,
                "gray {gray} should burn {expected} of the dots, burned {got}"
            );
        }
    }

    #[test]
    fn dithering_copes_with_pictures_of_every_size() {
        // The image crate's own dither panics on pictures one dot wide.
        for (width, height) in [(1, 1), (1, 7), (7, 1), (2, 2), (3, 5)] {
            let picture = gray_picture(width, height, |x, y| ((x * 40 + y * 30) % 256) as u8);
            let dots = picture.dots(width, height, Tone::Dither, false);
            assert_eq!(dots.dimensions(), (width, height));
        }
    }

    #[test]
    fn a_picture_is_scaled_to_the_size_it_is_asked_for() {
        let picture = gray_picture(100, 50, |_, _| 0);
        let dots = picture.dots(40, 20, Tone::default(), false);
        assert_eq!(dots.dimensions(), (40, 20));
        assert_eq!(burned(&dots), 800);
        // Also bigger than it is: a small icon on a big label.
        let icon = gray_picture(4, 4, |x, y| if (x + y) % 2 == 0 { 0 } else { 255 });
        assert_eq!(
            icon.dots(40, 40, Tone::default(), false).dimensions(),
            (40, 40)
        );
    }

    #[test]
    fn a_picture_with_nothing_in_it_is_refused() {
        let nothing = DynamicImage::ImageLuma8(GrayImage::new(0, 0));
        assert!(matches!(
            Picture::new(&nothing),
            Err(D30Error::InvalidPicture { .. })
        ));
    }

    #[test]
    fn a_huge_picture_is_made_smaller_and_keeps_its_shape() {
        let big = GrayImage::from_pixel(4000, 2000, Luma([10]));
        let picture = Picture::new(&DynamicImage::ImageLuma8(big)).unwrap();
        let pixels = u64::from(picture.width()) * u64::from(picture.height());
        assert!(pixels <= MOST_PIXELS + 2_000, "{pixels} pixels");
        assert!(pixels > MOST_PIXELS / 2, "{pixels} pixels");
        let shape = picture.width() as f32 / picture.height() as f32;
        assert!((shape - 2.0).abs() < 0.01, "shape {shape}");

        // A thin one stays at least a pixel high.
        let thin = GrayImage::from_pixel(3_000_000, 1, Luma([10]));
        let picture = Picture::new(&DynamicImage::ImageLuma8(thin)).unwrap();
        assert!(picture.height() >= 1 && picture.width() >= 1);
    }

    #[test]
    fn pictures_are_equal_when_they_are_the_same_one() {
        let picture = gray_picture(4, 4, |_, _| 0);
        assert_eq!(picture, picture.clone());
        assert_ne!(picture, gray_picture(4, 4, |_, _| 0));
    }

    #[test]
    fn a_file_can_be_opened_and_a_missing_or_broken_one_is_reported() {
        let dir = std::env::temp_dir().join(format!("d30-picture-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let good = dir.join("good.png");
        RgbaImage::from_pixel(6, 3, Rgba([0, 0, 0, 255]))
            .save(&good)
            .unwrap();
        let picture = Picture::open(&good).unwrap();
        assert_eq!((picture.width(), picture.height()), (6, 3));

        let broken = dir.join("broken.png");
        std::fs::write(&broken, b"this is not a picture").unwrap();
        let error = Picture::open(&broken).err().unwrap();
        assert!(matches!(error, D30Error::CouldNotReadPicture { .. }));
        assert!(error.to_string().contains("broken.png"), "{error}");

        let missing = Picture::open(dir.join("missing.png")).err().unwrap();
        assert!(matches!(missing, D30Error::CouldNotReadPicture { .. }));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
