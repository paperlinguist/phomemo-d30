//! The printers the person can pick from: the Bluetooth devices the computer has paired, plus the
//! printers named in the config file the command line program uses.

use crate::D30Config;
use advmac::{MacAddr6, MacAddrFormat};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub address: MacAddr6,
    /// Whether the device is connected to this computer right now, as far as the system says.
    pub connected: bool,
}

impl Device {
    pub fn address_text(&self) -> String {
        self.address.format_string(MacAddrFormat::ColonNotation)
    }

    /// How the device is shown in a list: its name and address.
    pub fn label(&self) -> String {
        format!("{}  ({})", self.name, self.address_text())
    }

    pub fn looks_like_d30(&self) -> bool {
        self.name.to_ascii_lowercase().contains("d30")
    }
}

/// What was found, and why the list may be incomplete.
#[derive(Debug, Default)]
pub struct Discovery {
    pub devices: Vec<Device>,
    /// Set when the computer could not say which devices are paired.
    pub note: Option<String>,
    /// The device that the config file names as the default printer, if it names one.
    pub default_address: Option<MacAddr6>,
}

/// Combines the paired devices with the printers named in the config file. A printer that is in
/// both keeps one entry that shows both names. Anything that looks like a D30 comes first.
pub fn merge(paired: Vec<Device>, config: Option<&D30Config>) -> Vec<Device> {
    let mut devices = paired;
    if let Some(config) = config {
        for (name, address) in &config.resolution {
            match devices.iter_mut().find(|device| device.address == *address) {
                Some(device) => device.name = format!("{name} ({})", device.name),
                None => devices.push(Device {
                    name: name.clone(),
                    address: *address,
                    connected: false,
                }),
            }
        }
    }
    devices.sort_by(|a, b| {
        b.looks_like_d30()
            .cmp(&a.looks_like_d30())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    devices
}

/// Asks the system which devices are paired and reads the config file.
pub fn discover() -> Discovery {
    // Read the config quietly: it is fine for it not to exist.
    let config = crate::place_config_file("phomemo-config.toml")
        .ok()
        .and_then(|path| D30Config::load_toml(&path).ok());

    let (paired, note) = match crate::bt::list_paired_devices() {
        Ok(found) => (
            found
                .into_iter()
                .map(|device| Device {
                    name: device.name,
                    address: device.address,
                    connected: device.connected,
                })
                .collect(),
            None,
        ),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };

    Discovery {
        devices: merge(paired, config.as_ref()),
        note,
        default_address: config
            .as_ref()
            .and_then(|config| config.resolve_default().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, last_byte: u8) -> Device {
        Device {
            name: name.to_string(),
            address: MacAddr6::new([2, 0, 0, 0, 0, last_byte]),
            connected: false,
        }
    }

    fn config(text: &str) -> D30Config {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn a_device_is_shown_with_its_address() {
        assert_eq!(device("D30", 5).label(), "D30  (02:00:00:00:00:05)");
    }

    #[test]
    fn devices_that_look_like_a_d30_come_first_then_by_name() {
        let merged = merge(
            vec![device("Titan 2", 1), device("D30", 2), device("Bose", 3)],
            None,
        );
        let names: Vec<_> = merged.iter().map(|device| device.name.as_str()).collect();
        assert_eq!(names, ["D30", "Bose", "Titan 2"]);
    }

    #[test]
    fn printers_from_the_config_file_are_added() {
        let config = config("[resolution]\nkitchen = \"02:00:00:00:00:09\"\n");
        let merged = merge(vec![device("Bose", 3)], Some(&config));
        assert_eq!(merged.len(), 2);
        let kitchen = merged
            .iter()
            .find(|device| device.name == "kitchen")
            .unwrap();
        assert_eq!(kitchen.address, MacAddr6::new([2, 0, 0, 0, 0, 9]));
        assert!(!kitchen.connected);
    }

    #[test]
    fn a_printer_that_is_paired_and_in_the_config_shows_both_names_once() {
        let config = config("[resolution]\nkitchen = \"02:00:00:00:00:02\"\n");
        let merged = merge(vec![device("D30", 2)], Some(&config));
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "kitchen (D30)");
        assert!(merged[0].looks_like_d30());
    }

    #[test]
    fn no_devices_and_no_config_is_an_empty_list() {
        assert!(merge(Vec::new(), None).is_empty());
    }
}
