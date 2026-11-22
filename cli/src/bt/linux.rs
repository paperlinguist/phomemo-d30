//! Linux backend: RFCOMM sockets through BlueZ, via the `bluetooth-serial-port-async` crate.

use std::io::{self, Write};

use advmac::MacAddr6;
use bluetooth_serial_port_async::{BtAddr, BtProtocol, BtSocket};

pub use bluetooth_serial_port_async::BtError as Error;

/// An open RFCOMM connection to the printer.
pub struct RfcommStream(BtSocket);

/// Opens an RFCOMM connection to the printer at `addr`.
///
/// BlueZ discovers the serial port channel through SDP first, which can block for a few seconds.
pub fn connect(addr: MacAddr6) -> Result<RfcommStream, Error> {
    let mut socket = BtSocket::new(BtProtocol::RFCOMM)?;
    socket.connect(BtAddr(addr.to_array()))?;
    Ok(RfcommStream(socket))
}

impl Write for RfcommStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// Advice for when the printer never answers; `addr` is its address as `AA:BB:CC:DD:EE:FF`.
pub fn connection_help(addr: &str) -> String {
    format!(
        "- it is not bonded, in which case it ignores us entirely\n\
         Run `bluetoothctl info {addr}` and look for UUID 00001101 (Serial Port).\n\
         If that entry is missing, pair the device again."
    )
}
