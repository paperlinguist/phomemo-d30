//! Listing the paired Bluetooth devices through BlueZ's own tool, `bluetoothctl`.
//!
//! BlueZ is the Bluetooth stack of Linux. Asking it for the paired devices directly means talking
//! D-Bus, which needs a large library for a short list, while `bluetoothctl` is installed with BlueZ
//! and prints the list in a form that has stayed the same for years:
//!
//! ```text
//! Device 60:DD:F0:3E:1D:D6 D30
//! Device AA:BB:CC:DD:EE:FF Bose 700
//! ```
//!
//! The logic is kept apart from running the program (it takes a function that does that), so that
//! it can be tried, and its tests run, on any system.

use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use advmac::{MacAddr6, MacAddrFormat};

use super::PairedDevice;

/// How long `bluetoothctl` is given to answer. It answers at once unless the Bluetooth service is
/// stuck, and a window must not wait for ever on that.
const TIMEOUT: Duration = Duration::from_secs(8);

/// Runs `bluetoothctl` with these arguments and returns what it printed.
pub type Run<'a> = &'a dyn Fn(&[&str]) -> io::Result<String>;

/// The paired devices, and whether each is connected, found by asking `run`.
pub fn list_paired_devices(run: Run) -> io::Result<Vec<PairedDevice>> {
    let mut text = run(&["paired-devices"])?;
    // Newer versions can filter `devices`, and may one day drop the old command.
    if text.contains("Invalid command") {
        text = run(&["devices", "Paired"])?;
    }
    Ok(parse_devices(&text)
        .into_iter()
        .map(|(address, name)| PairedDevice {
            connected: run(&["info", &colon(address)])
                .map(|info| is_connected(&info))
                .unwrap_or(false),
            name,
            address,
        })
        .collect())
}

/// The devices in what `bluetoothctl devices` printed: lines like `Device AA:BB:CC:DD:EE:FF Name`.
/// Anything else it printed (notices such as `[NEW] Device ...`) is left out, and so are the colour
/// codes it may put in front of a line.
pub fn parse_devices(text: &str) -> Vec<(MacAddr6, String)> {
    text.lines()
        .filter_map(|line| {
            let line = without_colour_codes(line);
            let rest = line.trim().strip_prefix("Device ")?;
            let (address, name) = match rest.split_once(' ') {
                Some((address, name)) => (address, name.trim()),
                None => (rest.trim(), ""),
            };
            let address: MacAddr6 = address.parse().ok()?;
            // A device without a name is shown by its address.
            let name = if name.is_empty() {
                colon(address)
            } else {
                name.to_string()
            };
            Some((address, name))
        })
        .collect()
}

/// The line without the terminal colour codes (an escape, `[`, some digits and `;`, a letter).
fn without_colour_codes(line: &str) -> String {
    let mut clean = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            clean.push(c);
        } else if chars.peek() == Some(&'[') {
            // The code ends at its first letter.
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }
    clean
}

/// An address as `AA:BB:CC:DD:EE:FF`, the way `bluetoothctl` takes and prints it.
fn colon(address: MacAddr6) -> String {
    address.format_string(MacAddrFormat::ColonNotation)
}

/// Whether what `bluetoothctl info` printed says the device is connected.
pub fn is_connected(info: &str) -> bool {
    info.lines()
        .map(str::trim)
        .any(|line| line.eq_ignore_ascii_case("Connected: yes"))
}

/// Runs the real `bluetoothctl`.
///
/// A missing program, a Bluetooth service that is not running and a program that does not answer
/// are errors with a sentence that says what to do.
pub fn run_bluetoothctl(args: &[&str]) -> io::Result<String> {
    let mut child = Command::new("bluetoothctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "`bluetoothctl` was not found: install BlueZ (the `bluez` package) to list the \
                     paired devices",
                )
            } else {
                error
            }
        })?;

    // Read the output on threads of their own, so that a program that prints a lot cannot fill a
    // pipe and wait for ever, and so that this can give up on it.
    let readers: Vec<_> = [
        child.stdout.take().map(|pipe| read_all(pipe)),
        child.stderr.take().map(|pipe| read_all(pipe)),
    ]
    .into_iter()
    .flatten()
    .collect();

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "`bluetoothctl` did not answer: is the Bluetooth service running?",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };

    let mut outputs = readers
        .into_iter()
        .map(|reader| reader.join().unwrap_or_default());
    let stdout = outputs.next().unwrap_or_default();
    let stderr = outputs.next().unwrap_or_default();
    if status.success() {
        Ok(stdout)
    } else {
        let said = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|text| !text.is_empty())
            .unwrap_or("it failed without saying why");
        Err(io::Error::other(format!(
            "`bluetoothctl` failed: {said}. Is the Bluetooth service running?"
        )))
    }
}

fn read_all(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// What a `bluetoothctl` with these devices would print.
    fn fake(
        devices: &'static str,
        connected: &'static [&'static str],
    ) -> impl Fn(&[&str]) -> io::Result<String> {
        move |args: &[&str]| match args {
            ["paired-devices"] | ["devices", "Paired"] => Ok(devices.to_string()),
            ["info", address] => Ok(format!(
                "Device {address} (public)\n\tName: x\n\tPaired: yes\n\tConnected: {}\n",
                if connected.contains(address) { "yes" } else { "no" }
            )),
            other => panic!("unexpected {other:?}"),
        }
    }

    const TWO: &str = "Device 60:DD:F0:3E:1D:D6 D30\nDevice AA:BB:CC:DD:EE:FF Bose 700\n";

    #[test]
    fn the_paired_devices_come_with_their_names_and_whether_they_are_connected() {
        let found = list_paired_devices(&fake(TWO, &["60:DD:F0:3E:1D:D6"])).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "D30");
        assert_eq!(colon(found[0].address), "60:DD:F0:3E:1D:D6");
        assert!(found[0].connected && found[0].looks_like_d30());
        // A name can have spaces in it.
        assert_eq!(found[1].name, "Bose 700");
        assert!(!found[1].connected);
    }

    #[test]
    fn no_paired_devices_is_an_empty_list_not_an_error() {
        assert!(list_paired_devices(&fake("", &[])).unwrap().is_empty());
    }

    #[test]
    fn notices_and_colour_codes_in_what_it_printed_are_left_out() {
        let text = "Waiting to connect to bluetoothd...\n\
                    \x1b[0mDevice 60:DD:F0:3E:1D:D6 D30\n\
                    [NEW] Device 11:22:33:44:55:66 Elsewhere\n\
                    Device not-an-address Broken\n\
                    Device 11:22:33:44\n\
                    Device AA:BB:CC:DD:EE:FF\n\
                    \n";
        let found = parse_devices(text);
        let names: Vec<&str> = found.iter().map(|(_, name)| name.as_str()).collect();
        // The one without a name is shown by its address.
        assert_eq!(names, ["D30", "AA:BB:CC:DD:EE:FF"]);
    }

    #[test]
    fn a_bluez_without_the_old_command_is_asked_the_new_way() {
        let asked = RefCell::new(Vec::new());
        let run = |args: &[&str]| -> io::Result<String> {
            asked.borrow_mut().push(args.join(" "));
            match args {
                ["paired-devices"] => Ok("Invalid command in menu main: paired-devices\n".into()),
                ["devices", "Paired"] => Ok(TWO.to_string()),
                ["info", _] => Ok("Connected: no\n".to_string()),
                other => panic!("unexpected {other:?}"),
            }
        };
        assert_eq!(list_paired_devices(&run).unwrap().len(), 2);
        assert_eq!(asked.borrow()[..2], ["paired-devices", "devices Paired"]);
    }

    #[test]
    fn a_failure_to_run_it_is_an_error_for_the_person_to_read() {
        let missing = |_: &[&str]| -> io::Result<String> {
            Err(io::Error::new(io::ErrorKind::NotFound, "not there"))
        };
        assert_eq!(
            list_paired_devices(&missing).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn a_device_is_connected_when_its_info_says_so() {
        assert!(is_connected("Device 1\n\tConnected: yes\n"));
        assert!(!is_connected("Device 1\n\tConnected: no\n"));
        assert!(!is_connected("Device 1\n\tPaired: yes\n"));
        assert!(!is_connected(""));
    }

    #[test]
    fn running_a_program_that_is_not_there_says_to_install_bluez() {
        // Only meaningful where there is no `bluetoothctl`, which is nearly everywhere but a Linux
        // desktop. Where there is one, it just has to run.
        match run_bluetoothctl(&["--version"]) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                assert!(error.to_string().contains("BlueZ"), "{error}");
            }
            Err(_) => {}
        }
    }
}
