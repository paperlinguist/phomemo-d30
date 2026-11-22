//! QR codes for labels: what goes into them (text, links, the details of a Wi-Fi network) and the
//! squares they are made of. Drawing them on a label is done by [`crate::render`].

use std::str::FromStr;

use image::{GrayImage, Luma};
use qrcode::{Color, EcLevel, QrCode};

use crate::D30Error;

/// How much of a QR code can be damaged or hidden and still be read. A higher level has more
/// squares for the same content, so on a small label they come out smaller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QrLevel {
    /// About 7% can be lost. The smallest code, which is what a small label wants.
    #[default]
    Low,
    /// About 15%.
    Medium,
    /// About 25%.
    Quartile,
    /// About 30%.
    High,
}

impl QrLevel {
    /// Every level, from the smallest code to the sturdiest.
    pub const ALL: [QrLevel; 4] = [
        QrLevel::Low,
        QrLevel::Medium,
        QrLevel::Quartile,
        QrLevel::High,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            QrLevel::Low => "low",
            QrLevel::Medium => "medium",
            QrLevel::Quartile => "quartile",
            QrLevel::High => "high",
        }
    }

    fn ec_level(self) -> EcLevel {
        match self {
            QrLevel::Low => EcLevel::L,
            QrLevel::Medium => EcLevel::M,
            QrLevel::Quartile => EcLevel::Q,
            QrLevel::High => EcLevel::H,
        }
    }
}

impl FromStr for QrLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "low" | "l" => Ok(QrLevel::Low),
            "medium" | "m" => Ok(QrLevel::Medium),
            "quartile" | "q" => Ok(QrLevel::Quartile),
            "high" | "h" => Ok(QrLevel::High),
            other => Err(format!(
                "unknown error correction level `{other}`: use low, medium, quartile or high"
            )),
        }
    }
}

/// How a Wi-Fi network is secured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WifiSecurity {
    /// WPA, WPA2 and WPA3 with a password: what nearly every network has.
    #[default]
    Wpa,
    /// The old WEP.
    Wep,
    /// An open network, without a password.
    None,
}

impl WifiSecurity {
    pub const ALL: [WifiSecurity; 3] = [WifiSecurity::Wpa, WifiSecurity::Wep, WifiSecurity::None];

    /// What to call it in a list.
    pub const fn name(self) -> &'static str {
        match self {
            WifiSecurity::Wpa => "WPA / WPA2 / WPA3",
            WifiSecurity::Wep => "WEP",
            WifiSecurity::None => "None (open)",
        }
    }

    /// The short word for the command line and for saving.
    pub const fn word(self) -> &'static str {
        match self {
            WifiSecurity::Wpa => "wpa",
            WifiSecurity::Wep => "wep",
            WifiSecurity::None => "none",
        }
    }

    /// What goes after `T:` in the text of the QR code.
    const fn code(self) -> &'static str {
        match self {
            WifiSecurity::Wpa => "WPA",
            WifiSecurity::Wep => "WEP",
            WifiSecurity::None => "nopass",
        }
    }
}

impl FromStr for WifiSecurity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "wpa" | "wpa2" | "wpa3" | "wpa2-psk" | "psk" => Ok(WifiSecurity::Wpa),
            "wep" => Ok(WifiSecurity::Wep),
            "none" | "nopass" | "open" => Ok(WifiSecurity::None),
            other => Err(format!(
                "unknown Wi-Fi security `{other}`: use wpa, wep or none"
            )),
        }
    }
}

/// A Wi-Fi network, which a phone joins when it reads the QR code of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WifiNetwork {
    /// The name of the network (its SSID).
    pub name: String,
    pub password: String,
    pub security: WifiSecurity,
    /// The network does not announce its name.
    pub hidden: bool,
}

impl WifiNetwork {
    /// The text that goes into the QR code, in the form that phones know:
    /// `WIFI:T:WPA;S:name;P:password;H:true;;`, with `\`, `;`, `,`, `:` and `"` in the name and the
    /// password marked by a `\` in front of them.
    pub fn payload(&self) -> String {
        let mut text = format!("WIFI:T:{};S:{};", self.security.code(), escape(&self.name));
        if self.security != WifiSecurity::None {
            text.push_str(&format!("P:{};", escape(&self.password)));
        }
        if self.hidden {
            text.push_str("H:true;");
        }
        text.push(';');
        text
    }
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// The squares of a QR code: which of them are dark, and how many there are across.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Squares {
    width: usize,
    dark: Vec<bool>,
}

impl Squares {
    /// The QR code of `content`, as small as `level` allows.
    pub fn new(content: &str, level: QrLevel) -> Result<Squares, D30Error> {
        if content.is_empty() {
            return Err(D30Error::InvalidLabel {
                reason: "the QR code has nothing in it".to_string(),
            });
        }
        let code = QrCode::with_error_correction_level(content.as_bytes(), level.ec_level())
            .map_err(|error| D30Error::InvalidLabel {
                reason: format!(
                    "that is too much for a QR code ({error}). Use less text, or a lower level of \
                     error correction"
                ),
            })?;
        Ok(Squares {
            width: code.width(),
            dark: code
                .to_colors()
                .into_iter()
                .map(|color| color == Color::Dark)
                .collect(),
        })
    }

    /// How many squares there are across (and down).
    pub fn width(&self) -> usize {
        self.width
    }

    /// Whether the square in column `x` and row `y` is dark.
    pub fn is_dark(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.width + x]
    }

    /// The code as a picture with every square `scale` pixels across: black where it is dark, white
    /// elsewhere. There is no extra white around it.
    pub fn image(&self, scale: u32) -> GrayImage {
        let scale = scale.max(1);
        let side = self.width as u32 * scale;
        GrayImage::from_fn(side, side, |x, y| {
            if self.is_dark((x / scale) as usize, (y / scale) as usize) {
                Luma([0])
            } else {
                Luma([255])
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wifi_network_makes_the_text_that_phones_know() {
        let network = WifiNetwork {
            name: "Guest".to_string(),
            password: "tea-and-biscuits".to_string(),
            security: WifiSecurity::Wpa,
            hidden: false,
        };
        assert_eq!(
            network.payload(),
            "WIFI:T:WPA;S:Guest;P:tea-and-biscuits;;"
        );

        let hidden = WifiNetwork {
            hidden: true,
            security: WifiSecurity::Wep,
            ..network.clone()
        };
        assert_eq!(
            hidden.payload(),
            "WIFI:T:WEP;S:Guest;P:tea-and-biscuits;H:true;;"
        );
    }

    #[test]
    fn an_open_network_has_no_password_in_its_text() {
        let network = WifiNetwork {
            name: "Cafe".to_string(),
            password: "ignored".to_string(),
            security: WifiSecurity::None,
            hidden: false,
        };
        assert_eq!(network.payload(), "WIFI:T:nopass;S:Cafe;;");
    }

    #[test]
    fn the_characters_that_mean_something_in_the_text_are_marked() {
        let network = WifiNetwork {
            name: r#"Joe's; "net""#.to_string(),
            password: r"a\b:c,d;e".to_string(),
            security: WifiSecurity::Wpa,
            hidden: false,
        };
        assert_eq!(
            network.payload(),
            r#"WIFI:T:WPA;S:Joe's\; \"net\";P:a\\b\:c\,d\;e;;"#
        );
        // An apostrophe and other characters mean nothing and stay as they are.
        assert_eq!(escape("it's é 日本"), "it's é 日本");
    }

    #[test]
    fn the_levels_and_securities_can_be_typed_and_have_names() {
        for level in QrLevel::ALL {
            assert_eq!(level.name().parse::<QrLevel>().unwrap(), level);
        }
        assert_eq!("M".parse::<QrLevel>().unwrap(), QrLevel::Medium);
        assert!("extreme".parse::<QrLevel>().is_err());
        for security in WifiSecurity::ALL {
            assert_eq!(security.word().parse::<WifiSecurity>().unwrap(), security);
        }
        assert_eq!("WPA2".parse::<WifiSecurity>().unwrap(), WifiSecurity::Wpa);
        assert_eq!("open".parse::<WifiSecurity>().unwrap(), WifiSecurity::None);
        assert!("kerberos".parse::<WifiSecurity>().is_err());
        assert_eq!(QrLevel::default(), QrLevel::Low);
    }

    #[test]
    fn the_squares_have_the_size_of_a_qr_code_and_the_finder_patterns() {
        // A short text is the smallest code, 21 x 21 squares.
        let squares = Squares::new("HELLO", QrLevel::Low).unwrap();
        assert_eq!(squares.width(), 21);
        // The three finder patterns: a dark ring around a light ring around a dark block.
        for (x0, y0) in [(0, 0), (14, 0), (0, 14)] {
            assert!(squares.is_dark(x0, y0));
            assert!(squares.is_dark(x0 + 6, y0 + 6));
            assert!(!squares.is_dark(x0 + 1, y0 + 1));
            assert!(squares.is_dark(x0 + 3, y0 + 3));
        }
        // More text or more correction needs more squares.
        let longer = Squares::new(&"x".repeat(60), QrLevel::Low).unwrap();
        assert!(longer.width() > 21);
        let sturdier = Squares::new("https://example.com/menu", QrLevel::High).unwrap();
        let plainer = Squares::new("https://example.com/menu", QrLevel::Low).unwrap();
        assert!(sturdier.width() > plainer.width());
    }

    #[test]
    fn the_picture_has_every_square_as_a_block_of_pixels() {
        let squares = Squares::new("HELLO", QrLevel::Low).unwrap();
        let image = squares.image(3);
        assert_eq!((image.width(), image.height()), (63, 63));
        for y in 0..63u32 {
            for x in 0..63u32 {
                let dark = squares.is_dark((x / 3) as usize, (y / 3) as usize);
                assert_eq!(image.get_pixel(x, y).0[0] == 0, dark, "{x},{y}");
            }
        }
        // Only black and white, no grays.
        assert!(image.pixels().all(|pixel| matches!(pixel.0[0], 0 | 255)));
    }

    #[test]
    fn nothing_and_too_much_are_errors_with_a_reason() {
        let error = Squares::new("", QrLevel::Low).unwrap_err().to_string();
        assert!(error.contains("nothing"), "{error}");
        let error = Squares::new(&"x".repeat(5000), QrLevel::Low)
            .unwrap_err()
            .to_string();
        assert!(error.contains("too much"), "{error}");
    }
}
