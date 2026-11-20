//! The Bluetooth link to the printer.
//!
//! The D30 is driven over a Bluetooth Classic serial link (RFCOMM, a.k.a. the Serial Port
//! Profile). Opening that link is the one genuinely platform-specific part of talking to the
//! printer, so every supported platform (Linux and Windows) provides the same small interface:
//!
//! * [`connect`] opens an RFCOMM connection to the printer with the given Bluetooth address. The
//!   RFCOMM channel is discovered through SDP, so the address is all that is needed. This can block
//!   for a few seconds, so a graphical program should call it from a worker thread.
//! * [`RfcommStream`] is that open connection. Write the printer's bytes to it.
//! * [`Error`] is whatever the platform's Bluetooth stack reports when something goes wrong.
//! * [`connection_help`] is the platform-specific part of the advice to show when connecting keeps
//!   failing.
//! * [`list_paired_devices`] lists the Bluetooth devices the operating system has paired, so that a
//!   program can offer them as printers to connect to.

use advmac::MacAddr6;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{connect, connection_help, list_paired_devices, Error, RfcommStream};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{connect, connection_help, list_paired_devices, Error, RfcommStream};

/// A Bluetooth device that the operating system has paired.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedDevice {
    /// The name the device reports, e.g. `D30`.
    pub name: String,
    pub address: MacAddr6,
    /// Whether the device is connected to this computer right now.
    pub connected: bool,
}

impl PairedDevice {
    /// Whether the name suggests a Phomemo D30, e.g. to preselect it in a list of devices.
    pub fn looks_like_d30(&self) -> bool {
        self.name.to_ascii_lowercase().contains("d30")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str) -> PairedDevice {
        PairedDevice {
            name: name.to_string(),
            address: MacAddr6::new([1, 2, 3, 4, 5, 6]),
            connected: false,
        }
    }

    #[test]
    fn d30_is_recognised_by_name() {
        assert!(device("D30").looks_like_d30());
        assert!(device("Phomemo d30-1234").looks_like_d30());
        assert!(!device("Bose 700").looks_like_d30());
    }
}
