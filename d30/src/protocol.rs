//! The bytes the D30 understands: its init sequence and the raster picture command.

use image::GrayImage;

use crate::{stock::Media, D30Error, INIT_BASE_FLAT};

/// How many rows of the picture go into one write to the printer.
pub const ROWS_PER_WRITE: usize = 255;

/// The most rows one raster command can declare, because its height is 16 bits.
pub const MAX_ROWS: usize = u16::MAX as usize;

/// Where the media byte sits in [`INIT_BASE_FLAT`]: the last byte of its `1f 11 0a` command.
const MEDIA_BYTE: usize = 23;

/// The printer's init sequence, which the official app sends before printing, with the byte that
/// says what kind of label stock is loaded.
///
/// For precut labels with gaps this is exactly [`INIT_BASE_FLAT`].
pub fn init_sequence(media: Media) -> Vec<u8> {
    let mut sequence = INIT_BASE_FLAT.to_vec();
    sequence[MEDIA_BYTE] = media.code();
    sequence
}

/// A picture in the form the printer takes it: one bit per dot, rows of whole bytes, the leftmost
/// dot in the highest bit, and a set bit meaning a dot is burned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    width_bytes: usize,
    rows: usize,
    data: Vec<u8>,
}

impl Raster {
    /// Packs a grayscale picture, where pixels brighter than 127 become dots.
    ///
    /// The picture has to be the way the printer takes it: as wide as the print head (96 dots for
    /// the D30) and as long as the label, which is the picture rotated a quarter turn from how
    /// the label reads.
    pub fn from_gray(image: &GrayImage) -> Result<Raster, D30Error> {
        let width = image.width() as usize;
        let rows = image.height() as usize;
        if rows == 0 || width == 0 {
            return Err(too_big_or_empty("the label is empty"));
        }
        if rows > MAX_ROWS {
            return Err(too_big_or_empty(&format!(
                "the label is {rows} dots long, but the printer takes at most {MAX_ROWS}"
            )));
        }
        let width_bytes = width.div_ceil(8);
        let mut data = vec![0u8; width_bytes * rows];
        for (x, y, pixel) in image.enumerate_pixels() {
            if pixel[0] > 127 {
                data[y as usize * width_bytes + x as usize / 8] |= 0x80 >> (x % 8);
            }
        }
        Ok(Raster {
            width_bytes,
            rows,
            data,
        })
    }

    /// How many bytes make up one row.
    pub fn width_bytes(&self) -> usize {
        self.width_bytes
    }

    /// How many rows (along the length of the label) the picture has.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The packed rows, top to bottom.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Adds `extra` blank rows after the picture. The printer feeds them like any other row, which
    /// is how uncut tape is pushed far enough out of the printer to be torn off.
    pub fn with_blank_rows(mut self, extra: usize) -> Result<Raster, D30Error> {
        let rows = self.rows + extra;
        if rows > MAX_ROWS {
            return Err(too_big_or_empty(&format!(
                "the label with its feed is {rows} dots long, but the printer takes at most {MAX_ROWS}"
            )));
        }
        self.data.resize(self.width_bytes * rows, 0);
        self.rows = rows;
        Ok(self)
    }
}

fn too_big_or_empty(reason: &str) -> D30Error {
    D30Error::InvalidLabel {
        reason: reason.to_string(),
    }
}

/// The start of a picture: the sniffed `1f 11 24 00`, `ESC @` (initialise), then the `GS v 0` raster
/// command with the width in bytes and the height in rows.
pub fn image_header(raster: &Raster) -> [u8; 14] {
    // Both fit in 16 bits: `Raster` does not allow more than `MAX_ROWS` rows.
    let width = (raster.width_bytes as u16).to_le_bytes();
    let rows = (raster.rows as u16).to_le_bytes();
    [
        0x1f, 0x11, 0x24, 0x00, 0x1b, 0x40, 0x1d, 0x76, 0x30, 0x00, width[0], width[1], rows[0],
        rows[1],
    ]
}

/// What to send for one copy of a picture, split into writes of at most [`ROWS_PER_WRITE`] rows.
/// The header goes at the start of the first write.
pub fn image_writes(raster: &Raster) -> Vec<Vec<u8>> {
    let header = image_header(raster);
    let step = raster.width_bytes * ROWS_PER_WRITE;
    raster
        .data
        .chunks(step)
        .enumerate()
        .map(|(index, rows)| {
            let mut write = Vec::with_capacity(header.len() + rows.len());
            if index == 0 {
                write.extend_from_slice(&header);
            }
            write.extend_from_slice(rows);
            write
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use image::Luma;

    use super::*;
    use crate::{IMG_PRECURSOR, INIT_BASE_FLAT};

    fn picture(width: u32, rows: u32) -> GrayImage {
        GrayImage::from_fn(width, rows, |x, y| {
            if (x + y) % 2 == 0 {
                Luma([255])
            } else {
                Luma([0])
            }
        })
    }

    #[test]
    fn the_header_for_a_12x40_label_is_the_one_that_was_hard_coded() {
        let raster = Raster::from_gray(&picture(96, 320)).unwrap();
        assert_eq!(&image_header(&raster)[..], IMG_PRECURSOR);
    }

    #[test]
    fn the_header_follows_the_size_of_the_picture() {
        // 96 dots = 12 bytes wide; 500 rows = 0x01f4 -> f4 01.
        let raster = Raster::from_gray(&picture(96, 500)).unwrap();
        let header = image_header(&raster);
        assert_eq!(&header[10..], &[12, 0, 0xf4, 0x01]);
        assert_eq!(&header[..10], &IMG_PRECURSOR[..10]);
    }

    #[test]
    fn the_init_for_gap_labels_is_exactly_the_one_that_was_hard_coded() {
        assert_eq!(init_sequence(Media::Gap), INIT_BASE_FLAT);
        // The byte we replace is the media byte of the `1f 11 0a` command.
        assert_eq!(
            &INIT_BASE_FLAT[MEDIA_BYTE - 2..=MEDIA_BYTE],
            &[0x1f, 0x11, 0x0a]
        );
    }

    #[test]
    fn the_init_for_other_media_differs_only_in_the_media_byte() {
        for media in [Media::Continuous, Media::BlackMark] {
            let sequence = init_sequence(media);
            assert_eq!(sequence.len(), INIT_BASE_FLAT.len());
            for (index, (new, old)) in sequence.iter().zip(INIT_BASE_FLAT).enumerate() {
                if index == MEDIA_BYTE {
                    assert_eq!(*new, media.code());
                } else {
                    assert_eq!(new, old, "byte {index}");
                }
            }
        }
    }

    #[test]
    fn bits_are_packed_leftmost_dot_first() {
        let mut image = GrayImage::new(16, 2);
        image.put_pixel(0, 0, Luma([255])); // first dot of row 0
        image.put_pixel(15, 0, Luma([255])); // last dot of row 0
        image.put_pixel(9, 1, Luma([200])); // second byte of row 1, second dot
        image.put_pixel(3, 1, Luma([127])); // not bright enough: stays blank
        let raster = Raster::from_gray(&image).unwrap();
        assert_eq!(raster.width_bytes(), 2);
        assert_eq!(raster.bytes(), &[0b1000_0000, 0b0000_0001, 0, 0b0100_0000]);
    }

    #[test]
    fn long_pictures_are_written_in_chunks_with_the_header_once() {
        let raster = Raster::from_gray(&picture(96, 320)).unwrap();
        let writes = image_writes(&raster);
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].len(), 14 + 255 * 12);
        assert_eq!(writes[1].len(), 65 * 12);
        let all: Vec<u8> = writes.concat();
        assert_eq!(&all[..14], IMG_PRECURSOR);
        assert_eq!(&all[14..], raster.bytes());
    }

    #[test]
    fn a_picture_of_exactly_one_chunk_does_not_end_with_an_empty_write() {
        let raster = Raster::from_gray(&picture(96, 255)).unwrap();
        assert_eq!(image_writes(&raster).len(), 1);
        let raster = Raster::from_gray(&picture(96, 510)).unwrap();
        assert_eq!(image_writes(&raster).len(), 2);
    }

    #[test]
    fn blank_rows_extend_the_picture_and_the_declared_height() {
        let raster = Raster::from_gray(&picture(96, 100))
            .unwrap()
            .with_blank_rows(20)
            .unwrap();
        assert_eq!(raster.rows(), 120);
        assert_eq!(raster.bytes().len(), 120 * 12);
        assert!(raster.bytes()[100 * 12..].iter().all(|byte| *byte == 0));
        assert_eq!(&image_header(&raster)[12..], &[120, 0]);
    }

    #[test]
    fn pictures_the_printer_cannot_take_are_rejected() {
        assert!(Raster::from_gray(&GrayImage::new(96, 0)).is_err());
        assert!(Raster::from_gray(&GrayImage::new(96, MAX_ROWS as u32 + 1)).is_err());
        let raster = Raster::from_gray(&GrayImage::new(96, MAX_ROWS as u32)).unwrap();
        assert!(raster.with_blank_rows(1).is_err());
    }
}
