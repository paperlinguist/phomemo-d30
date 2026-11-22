//! Windows backend: RFCOMM client sockets through Winsock (`AF_BTH`).
//!
//! Windows exposes Bluetooth Classic through the ordinary socket API. Given the printer's address
//! and the Serial Port service class, Windows itself performs the SDP lookup of the RFCOMM channel
//! (the job BlueZ's `sdp_*` calls do on Linux), so no channel number is needed here either.
//!
//! The printer has to be paired with this PC first (Settings > Bluetooth & devices).

use std::{
    io::{self, Write},
    mem, ptr,
};

use advmac::MacAddr6;
use windows_sys::{
    core::GUID,
    Win32::{
        Devices::Bluetooth::{
            BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice, AF_BTH,
            BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BTHPROTO_RFCOMM, SOCKADDR_BTH,
        },
        Foundation::{GetLastError, ERROR_NO_MORE_ITEMS},
        Networking::WinSock::{
            closesocket, connect as winsock_connect, send, socket, WSACleanup, WSAGetLastError,
            WSAStartup, INVALID_SOCKET, SOCKADDR, SOCKET, SOCKET_ERROR, SOCK_STREAM, WSADATA,
        },
    },
};

use super::PairedDevice;

/// Everything the Windows Bluetooth stack can fail with is a plain OS error (a Winsock code).
pub type Error = io::Error;

/// The Bluetooth SIG's "Serial Port" service class, 0x1101, as the full 128-bit UUID
/// `00001101-0000-1000-8000-00805F9B34FB`.
const SERIAL_PORT_SERVICE_CLASS: GUID = GUID::from_u128(0x00001101_0000_1000_8000_00805f9b34fb);

/// Keeps Winsock initialised. Windows reference-counts `WSAStartup`/`WSACleanup`, so this is safe
/// even if something else in the process (e.g. `std::net`) has started Winsock as well.
struct Winsock;

impl Winsock {
    fn start() -> io::Result<Self> {
        // SAFETY: `WSADATA` is plain old data for which all-zeroes is a valid value, and
        // `WSAStartup` only writes to it.
        let mut data: WSADATA = unsafe { mem::zeroed() };
        // 0x0202 asks for Winsock 2.2.
        let rc = unsafe { WSAStartup(0x0202, &mut data) };
        if rc != 0 {
            // Unlike the other Winsock calls, WSAStartup returns the error code directly.
            return Err(io::Error::from_raw_os_error(rc));
        }
        Ok(Winsock)
    }
}

impl Drop for Winsock {
    fn drop(&mut self) {
        // SAFETY: balances the successful `WSAStartup` in `start`.
        unsafe { WSACleanup() };
    }
}

/// A socket that is closed when dropped.
struct Socket(SOCKET);

impl Drop for Socket {
    fn drop(&mut self) {
        // SAFETY: we own the socket and nothing uses it after this.
        unsafe { closesocket(self.0) };
    }
}

/// An open RFCOMM connection to the printer.
pub struct RfcommStream {
    // Field order matters: fields are dropped in order, and the socket has to be closed before
    // Winsock is shut down.
    socket: Socket,
    _winsock: Winsock,
}

/// Opens an RFCOMM connection to the printer at `addr`.
///
/// Windows looks up the serial port channel through SDP first, which can block for a few seconds.
pub fn connect(addr: MacAddr6) -> Result<RfcommStream, Error> {
    let winsock = Winsock::start()?;

    // SAFETY: plain FFI call; every argument is passed by value.
    let raw = unsafe { socket(i32::from(AF_BTH), SOCK_STREAM, BTHPROTO_RFCOMM as i32) };
    if raw == INVALID_SOCKET {
        return Err(last_error());
    }
    let socket = Socket(raw);

    let target = SOCKADDR_BTH {
        addressFamily: AF_BTH,
        btAddr: bth_addr(addr),
        serviceClassId: SERIAL_PORT_SERVICE_CLASS,
        // Port 0 means "look the channel up in SDP, using `serviceClassId`".
        port: 0,
    };
    // SAFETY: `target` is a fully initialised `SOCKADDR_BTH` and we pass exactly its size.
    let rc = unsafe {
        winsock_connect(
            socket.0,
            ptr::addr_of!(target).cast::<SOCKADDR>(),
            mem::size_of::<SOCKADDR_BTH>() as i32,
        )
    };
    if rc == SOCKET_ERROR {
        return Err(last_error());
    }

    Ok(RfcommStream {
        socket,
        _winsock: winsock,
    })
}

impl Write for RfcommStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // `send` takes an `i32` length, so a larger buffer is just written in several calls.
        let len = buf.len().min(i32::MAX as usize) as i32;
        // SAFETY: `buf` is valid for reads of `len` bytes.
        let sent = unsafe { send(self.socket.0, buf.as_ptr(), len, 0) };
        if sent == SOCKET_ERROR {
            Err(last_error())
        } else {
            Ok(sent as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // Nothing is buffered on our side; `send` hands the bytes straight to the Bluetooth stack.
        Ok(())
    }
}

/// Lists the Bluetooth devices Windows has paired ("remembered" devices).
///
/// This only reads what Windows already knows and does not scan the air for new devices, so it
/// returns right away.
pub fn list_paired_devices() -> io::Result<Vec<PairedDevice>> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: mem::size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: 1,
        fReturnRemembered: 1,
        fReturnUnknown: 0,
        fReturnConnected: 1,
        fIssueInquiry: 0,
        cTimeoutMultiplier: 0,
        // A null radio handle searches all Bluetooth radios.
        hRadio: ptr::null_mut(),
    };
    // SAFETY: `BLUETOOTH_DEVICE_INFO` is plain old data for which all-zeroes is a valid value. Its
    // size field has to be filled in before it is passed to the search functions.
    let mut info: BLUETOOTH_DEVICE_INFO = unsafe { mem::zeroed() };
    info.dwSize = mem::size_of::<BLUETOOTH_DEVICE_INFO>() as u32;

    // SAFETY: both pointers refer to fully initialised structures that outlive the call.
    let find = unsafe { BluetoothFindFirstDevice(&params, &mut info) };
    if find.is_null() {
        // SAFETY: takes no arguments and only reads thread-local state.
        let code = unsafe { GetLastError() };
        // Having no paired devices (or no Bluetooth radio) is not an error.
        return if code == ERROR_NO_MORE_ITEMS {
            Ok(Vec::new())
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        };
    }

    let mut devices = vec![device_from(&info)];
    // SAFETY: `find` is a live search handle and `info` has its size set.
    while unsafe { BluetoothFindNextDevice(find, &mut info) } != 0 {
        devices.push(device_from(&info));
    }
    // SAFETY: `find` is a live search handle, closed exactly once.
    unsafe { BluetoothFindDeviceClose(find) };
    Ok(devices)
}

fn device_from(info: &BLUETOOTH_DEVICE_INFO) -> PairedDevice {
    // SAFETY: both views of the address union are plain integers, so reading `ullLong` is valid.
    let number = unsafe { info.Address.Anonymous.ullLong };
    let bytes = number.to_be_bytes();
    let name_len = info
        .szName
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(info.szName.len());
    PairedDevice {
        name: String::from_utf16_lossy(&info.szName[..name_len]),
        // The address is a 48-bit number held in a u64, so its first two big-endian bytes are zero.
        address: MacAddr6::new([bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]),
        connected: info.fConnected != 0,
    }
}

/// Windows' `BTH_ADDR`: the six address bytes read as one big-endian number, so that
/// `AA:BB:CC:DD:EE:FF` becomes `0xAABB_CCDD_EEFF`.
fn bth_addr(addr: MacAddr6) -> u64 {
    addr.to_array()
        .into_iter()
        .fold(0, |acc, byte| (acc << 8) | u64::from(byte))
}

/// The calling thread's most recent Winsock error.
fn last_error() -> io::Error {
    // SAFETY: takes no arguments and only reads thread-local state.
    io::Error::from_raw_os_error(unsafe { WSAGetLastError() })
}

/// Advice for when the printer never answers; `addr` is its address as `AA:BB:CC:DD:EE:FF`.
pub fn connection_help(_addr: &str) -> String {
    "- it is not paired with this PC\n\
     Open Settings > Bluetooth & devices, make sure Bluetooth is on and the printer is listed \
     as paired (remove it and pair it again if not).\n\
     Windows should also list a \"Standard Serial over Bluetooth link\" port for it under \
     Device Manager > Ports (COM & LPT)."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bth_addr_is_the_address_as_a_big_endian_number() {
        let addr: MacAddr6 = "01:23:45:67:89:AB".parse().unwrap();
        assert_eq!(bth_addr(addr), 0x0123_4567_89AB);
    }

    #[test]
    fn sockaddr_bth_has_the_packed_size_windows_expects() {
        // 2 (family) + 8 (address) + 16 (service class) + 4 (port), without any padding.
        assert_eq!(mem::size_of::<SOCKADDR_BTH>(), 30);
    }

    #[test]
    fn serial_port_service_class_is_the_sig_uuid() {
        // 00001101-0000-1000-8000-00805F9B34FB
        let uuid = SERIAL_PORT_SERVICE_CLASS;
        assert_eq!(uuid.data1, 0x0000_1101);
        assert_eq!(uuid.data2, 0x0000);
        assert_eq!(uuid.data3, 0x1000);
        assert_eq!(uuid.data4, [0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB]);
    }
}
