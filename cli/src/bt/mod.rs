//! The Bluetooth link to the printer.
//!
//! The D30 is driven over a Bluetooth Classic serial link (RFCOMM, a.k.a. the Serial Port
//! Profile). Opening that link is the one genuinely platform-specific part of this program, so
//! every supported platform provides the same small interface:
//!
//! * [`connect`] opens an RFCOMM connection to the printer with the given Bluetooth address. The
//!   RFCOMM channel is discovered through SDP, so the address is all that is needed. This can block
//!   for a few seconds.
//! * [`RfcommStream`] is that open connection. Write the printer's bytes to it.
//! * [`Error`] is whatever the platform's Bluetooth stack reports when something goes wrong.
//! * [`connection_help`] is the platform-specific part of the advice shown when connecting keeps
//!   failing.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{connect, connection_help, Error, RfcommStream};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{connect, connection_help, Error, RfcommStream};

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("d30-cli only has Bluetooth backends for Linux (BlueZ) and Windows (Winsock)");
