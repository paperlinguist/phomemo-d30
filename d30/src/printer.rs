//! Talking to the printer from a thread of its own, so that a window never has to wait for
//! Bluetooth.
//!
//! A program starts a [`Printer`] and sends it [`Command`]s: connect, print a [`Job`], disconnect.
//! What happens comes back as [`Event`]s, to be read whenever it suits the program, for instance
//! once per frame. The connection stays open between jobs, so only the first print has to wait for
//! it.

use std::{
    io::{self, Write},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use advmac::MacAddr6;

use crate::{
    protocol::{self, Raster},
    stock::{LabelStock, Media},
};

/// How many times to try to connect before giving up. Each attempt that fails takes a few
/// seconds, so this is what decides how long a printer that is switched off is waited for.
pub const CONNECT_ATTEMPTS: u32 = 3;

/// How a connection to the printer is made. [`Bluetooth`] is the real one; tests use a fake.
pub trait Transport: Send + 'static {
    type Stream: Write + Send + 'static;

    /// Connects to the printer with this address. It can take a few seconds. The error is
    /// something to show to the person.
    fn connect(&self, address: MacAddr6) -> Result<Self::Stream, String>;
}

/// Connects over Bluetooth.
#[cfg(all(feature = "bluetooth", any(target_os = "linux", windows)))]
pub struct Bluetooth;

#[cfg(all(feature = "bluetooth", any(target_os = "linux", windows)))]
impl Transport for Bluetooth {
    type Stream = crate::bt::RfcommStream;

    fn connect(&self, address: MacAddr6) -> Result<Self::Stream, String> {
        crate::bt::connect(address).map_err(|error| error.to_string())
    }
}

/// Something to print.
#[derive(Clone, Debug)]
pub struct Job {
    /// What kind of label stock is loaded, which the printer is told before the picture.
    pub media: Media,
    /// The picture, including any blank rows before and after it.
    pub raster: Raster,
    /// How many copies to print, at least one.
    pub copies: u32,
    /// A job to send after this one, once the printer has had the given time to finish it. The
    /// printer does not say when it is done, and it could be sent the next job at once but would
    /// not necessarily have settled where it stops by then.
    pub then: Option<(Duration, Box<Job>)>,
}

/// How long the printer is given to find the gap and settle after a blank label, before it is sent
/// the rest of [`position_tape`].
pub const SETTLE: Duration = Duration::from_secs(15);

/// The job that puts the tape where the program expects it to be when it counts the rows of the
/// labels of this stock (see [`LabelStock::tape_plan`]), without any guessing by eye: a blank label
/// in the printer's own gap mode, which finds the gap by itself and leaves the head at the start of
/// the gap, and then, as uncut tape, the rows that carry the tape on to the middle of the gap at the
/// tear edge. Tear off what comes out there. `None` for stocks that are not counted.
pub fn position_tape(stock: &LabelStock) -> Option<Job> {
    stock.counted_gap_mm()?;
    let blank_label = Raster::blank(stock.length_dots()? as usize).ok()?;
    let feed = Raster::blank(stock.position_feed_rows()).ok()?;
    Some(Job {
        media: Media::Gap,
        raster: blank_label,
        copies: 1,
        then: Some((
            SETTLE,
            Box::new(Job {
                media: Media::Continuous,
                raster: feed,
                copies: 1,
                then: None,
            }),
        )),
    })
}

#[derive(Debug)]
pub enum Command {
    /// Connect to the printer and keep the connection open.
    Connect(MacAddr6),
    /// Close the connection, which frees the printer for other programs.
    Disconnect,
    /// Print, connecting first if there is no connection to this printer yet.
    Print { address: MacAddr6, job: Job },
}

/// What the printer thread reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A connection attempt has started.
    Connecting {
        attempt: u32,
        of: u32,
    },
    Connected(MacAddr6),
    /// Every attempt to connect failed. The text is the error of the last one.
    ConnectFailed(String),
    Disconnected,
    /// A copy is being sent to the printer.
    Printing {
        copy: u32,
        of: u32,
    },
    /// Everything was sent to the printer.
    Printed {
        copies: u32,
    },
    PrintFailed(String),
}

/// A handle to the printer thread.
pub struct Printer {
    commands: Sender<Command>,
    events: Receiver<Event>,
}

impl Printer {
    /// Starts the printer thread. `wake` is called after every event, from the thread, so that a
    /// program can ask its window to repaint.
    pub fn start<T: Transport>(
        transport: T,
        attempts: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Printer {
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        thread::Builder::new()
            .name("printer".to_string())
            .spawn(move || {
                Worker {
                    transport,
                    attempts: attempts.max(1),
                    events: event_sender,
                    wake,
                    link: None,
                }
                .run(command_receiver)
            })
            .expect("could not start the printer thread");
        Printer {
            commands: command_sender,
            events: event_receiver,
        }
    }

    pub fn send(&self, command: Command) {
        // The thread only stops when this handle is dropped, so sending cannot fail.
        let _ = self.commands.send(command);
    }

    /// The events that have happened since the last call.
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }
}

struct Worker<T: Transport, W: Fn()> {
    transport: T,
    attempts: u32,
    events: Sender<Event>,
    wake: W,
    /// The open connection and the printer it goes to.
    link: Option<(MacAddr6, T::Stream)>,
}

/// Why a job could not be sent.
struct SendFailure {
    error: io::Error,
    /// The very first write failed, so nothing reached the printer. That is what a connection that
    /// has gone stale looks like, for instance after the printer switched itself off.
    nothing_sent: bool,
}

impl<T: Transport, W: Fn()> Worker<T, W> {
    fn emit(&self, event: Event) {
        // The receiver is gone only when the program is closing.
        let _ = self.events.send(event);
        (self.wake)();
    }

    fn run(mut self, commands: Receiver<Command>) {
        while let Ok(command) = commands.recv() {
            match command {
                Command::Connect(address) => {
                    self.connect(address);
                }
                Command::Disconnect => {
                    self.link = None;
                    self.emit(Event::Disconnected);
                }
                Command::Print { address, job } => self.print(address, &job),
            }
        }
    }

    /// Connects, trying a few times, and keeps the connection. Returns whether it worked.
    fn connect(&mut self, address: MacAddr6) -> bool {
        self.link = None;
        let mut last_error = String::new();
        for attempt in 1..=self.attempts {
            self.emit(Event::Connecting {
                attempt,
                of: self.attempts,
            });
            match self.transport.connect(address) {
                Ok(stream) => {
                    self.link = Some((address, stream));
                    self.emit(Event::Connected(address));
                    return true;
                }
                Err(error) => last_error = error,
            }
        }
        self.emit(Event::ConnectFailed(last_error));
        false
    }

    /// Prints the job, and the jobs that follow it, and reports how it went.
    fn print(&mut self, address: MacAddr6, job: &Job) {
        let mut step = job;
        loop {
            if let Err(message) = self.print_one(address, step) {
                self.emit(Event::PrintFailed(message));
                return;
            }
            match &step.then {
                Some((pause, next)) => {
                    thread::sleep(*pause);
                    // The next job goes over a connection of its own: a printer that was sent a
                    // blank label and then, over the same connection, the rest, did not move the
                    // tape for the rest, while it does for a job sent on a fresh connection.
                    self.link = None;
                    step = next;
                }
                None => break,
            }
        }
        self.emit(Event::Printed { copies: job.copies });
    }

    /// Sends one job, connecting first if there is no connection yet. On failure, what to tell
    /// the person.
    fn print_one(&mut self, address: MacAddr6, job: &Job) -> Result<(), String> {
        let connected = matches!(&self.link, Some((current, _)) if *current == address);
        if !connected && !self.connect(address) {
            return Err("could not connect to the printer".to_string());
        }

        match self.send(job) {
            Ok(()) => Ok(()),
            Err(failure) if failure.nothing_sent => {
                // The connection had most likely gone stale. Connect again, once, and send the
                // whole job again: nothing of it reached the printer, so nothing can print twice.
                if !self.connect(address) {
                    return Err(
                        "the connection to the printer was lost, and it could not be restored"
                            .to_string(),
                    );
                }
                self.send(job)
                    .map_err(|failure| failure.error.to_string())
            }
            // Part of the job reached the printer. Sending it again could print a label twice.
            Err(failure) => Err(failure.error.to_string()),
        }
    }

    /// Sends the job over the open connection. On failure the connection is closed.
    fn send(&mut self, job: &Job) -> Result<(), SendFailure> {
        let Some((address, mut stream)) = self.link.take() else {
            return Err(SendFailure {
                error: io::Error::new(io::ErrorKind::NotConnected, "not connected to the printer"),
                nothing_sent: true,
            });
        };

        stream
            .write_all(&protocol::init_sequence(job.media))
            .map_err(|error| SendFailure {
                error,
                nothing_sent: true,
            })?;

        let writes = protocol::image_writes(&job.raster);
        for copy in 1..=job.copies {
            self.emit(Event::Printing {
                copy,
                of: job.copies,
            });
            for write in &writes {
                stream
                    .write_all(write)
                    .and_then(|()| stream.flush())
                    .map_err(|error| SendFailure {
                        error,
                        nothing_sent: false,
                    })?;
            }
        }

        self.link = Some((address, stream));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use image::{GrayImage, Luma};

    use super::*;

    /// A printer that is not there: it records what it is sent, and fails as told.
    #[derive(Clone, Default)]
    struct Fake {
        state: Arc<Mutex<FakeState>>,
    }

    #[derive(Default)]
    struct FakeState {
        /// How many connects have worked so far.
        connects: u32,
        /// The next connects fail until this many have.
        connect_failures: u32,
        /// The next connections that work are stale: they fail on every write.
        stale_connections: u32,
        /// Every connection fails its n-th write (counting from 1).
        fail_write: Option<usize>,
        sent: Vec<u8>,
    }

    struct FakeStream {
        state: Arc<Mutex<FakeState>>,
        stale: bool,
        writes: usize,
    }

    impl Transport for Fake {
        type Stream = FakeStream;

        fn connect(&self, _address: MacAddr6) -> Result<FakeStream, String> {
            let mut state = self.state.lock().unwrap();
            if state.connect_failures > 0 {
                state.connect_failures -= 1;
                return Err("no answer".to_string());
            }
            state.connects += 1;
            let stale = state.stale_connections > 0;
            if stale {
                state.stale_connections -= 1;
            }
            Ok(FakeStream {
                state: self.state.clone(),
                stale,
                writes: 0,
            })
        }
    }

    impl Write for FakeStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            let mut state = self.state.lock().unwrap();
            if self.stale || state.fail_write == Some(self.writes) {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"));
            }
            state.sent.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn address() -> MacAddr6 {
        MacAddr6::new([2, 0, 0, 0, 0, 1])
    }

    fn job(copies: u32) -> Job {
        // 96 dots wide and 300 rows long: two writes of 255 and 45 rows.
        let picture = GrayImage::from_fn(96, 300, |x, y| {
            if (x * 7 + y) % 5 == 0 {
                Luma([255])
            } else {
                Luma([0])
            }
        });
        Job {
            media: Media::Gap,
            raster: Raster::from_gray(&picture).unwrap(),
            copies,
            then: None,
        }
    }

    /// What the printer should have been sent for a job.
    fn expected_bytes(job: &Job) -> Vec<u8> {
        let mut bytes = protocol::init_sequence(job.media);
        for _ in 0..job.copies {
            bytes.extend(protocol::image_writes(&job.raster).concat());
        }
        bytes
    }

    fn start(fake: &Fake) -> Printer {
        Printer::start(fake.clone(), 3, || {})
    }

    /// Collects events until one that ends the story arrives.
    fn events_until(printer: &Printer, done: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        loop {
            let event = printer
                .events
                .recv_timeout(Duration::from_secs(10))
                .expect("the printer thread should report something");
            let finished = done(&event);
            seen.push(event);
            if finished {
                return seen;
            }
        }
    }

    fn is_end_of_print(event: &Event) -> bool {
        matches!(event, Event::Printed { .. } | Event::PrintFailed(_))
    }

    #[test]
    fn printing_connects_sends_the_init_and_every_copy_and_says_so() {
        let fake = Fake::default();
        let printer = start(&fake);
        let job = job(2);
        printer.send(Command::Print {
            address: address(),
            job: job.clone(),
        });

        let events = events_until(&printer, is_end_of_print);
        assert_eq!(
            events,
            [
                Event::Connecting { attempt: 1, of: 3 },
                Event::Connected(address()),
                Event::Printing { copy: 1, of: 2 },
                Event::Printing { copy: 2, of: 2 },
                Event::Printed { copies: 2 },
            ]
        );
        assert_eq!(fake.state.lock().unwrap().sent, expected_bytes(&job));
    }

    #[test]
    fn the_connection_is_kept_for_the_next_print() {
        let fake = Fake::default();
        let printer = start(&fake);
        for _ in 0..2 {
            printer.send(Command::Print {
                address: address(),
                job: job(1),
            });
            events_until(&printer, is_end_of_print);
        }
        assert_eq!(fake.state.lock().unwrap().connects, 1);
    }

    #[test]
    fn connecting_first_lets_the_print_use_that_connection() {
        let fake = Fake::default();
        let printer = start(&fake);
        printer.send(Command::Connect(address()));
        events_until(&printer, |event| matches!(event, Event::Connected(_)));
        printer.send(Command::Print {
            address: address(),
            job: job(1),
        });
        let events = events_until(&printer, is_end_of_print);
        assert!(!events
            .iter()
            .any(|event| matches!(event, Event::Connecting { .. })));
        assert_eq!(fake.state.lock().unwrap().connects, 1);
    }

    #[test]
    fn printing_to_another_printer_connects_to_that_one() {
        let fake = Fake::default();
        let printer = start(&fake);
        printer.send(Command::Connect(address()));
        events_until(&printer, |event| matches!(event, Event::Connected(_)));
        printer.send(Command::Print {
            address: MacAddr6::new([2, 0, 0, 0, 0, 9]),
            job: job(1),
        });
        events_until(&printer, is_end_of_print);
        assert_eq!(fake.state.lock().unwrap().connects, 2);
    }

    #[test]
    fn a_stale_connection_fails_its_first_write_and_is_replaced_without_printing_twice() {
        let fake = Fake::default();
        // The first connection that is made is stale: its very first write fails.
        fake.state.lock().unwrap().stale_connections = 1;
        let printer = start(&fake);
        let job = job(1);
        printer.send(Command::Print {
            address: address(),
            job: job.clone(),
        });

        let events = events_until(&printer, is_end_of_print);
        assert_eq!(events.last(), Some(&Event::Printed { copies: 1 }));
        let state = fake.state.lock().unwrap();
        assert_eq!(
            state.connects, 2,
            "one stale connection, then one that works"
        );
        // Exactly one complete job reached the printer.
        assert_eq!(state.sent, expected_bytes(&job));
    }

    #[test]
    fn a_failure_in_the_middle_of_a_job_is_not_sent_again() {
        let fake = Fake::default();
        // Write 1 is the init, write 2 the first part of the picture: it fails.
        fake.state.lock().unwrap().fail_write = Some(2);
        let printer = start(&fake);
        printer.send(Command::Print {
            address: address(),
            job: job(1),
        });

        let events = events_until(&printer, is_end_of_print);
        assert!(matches!(events.last(), Some(Event::PrintFailed(_))));
        let state = fake.state.lock().unwrap();
        assert_eq!(state.connects, 1, "a half-sent job must not be sent again");
        // Only the init reached the printer; no part of a picture.
        assert_eq!(state.sent, protocol::init_sequence(Media::Gap));
    }

    #[test]
    fn after_a_failed_print_the_next_one_connects_afresh() {
        let fake = Fake::default();
        fake.state.lock().unwrap().fail_write = Some(2);
        let printer = start(&fake);
        printer.send(Command::Print {
            address: address(),
            job: job(1),
        });
        events_until(&printer, is_end_of_print);

        fake.state.lock().unwrap().fail_write = None;
        let job = job(1);
        printer.send(Command::Print {
            address: address(),
            job: job.clone(),
        });
        let events = events_until(&printer, is_end_of_print);
        assert_eq!(events.last(), Some(&Event::Printed { copies: 1 }));
        assert_eq!(fake.state.lock().unwrap().connects, 2);
    }

    #[test]
    fn connecting_is_tried_a_few_times() {
        let fake = Fake::default();
        fake.state.lock().unwrap().connect_failures = 2;
        let printer = start(&fake);
        printer.send(Command::Connect(address()));
        let events = events_until(&printer, |event| {
            matches!(event, Event::Connected(_) | Event::ConnectFailed(_))
        });
        assert_eq!(
            events,
            [
                Event::Connecting { attempt: 1, of: 3 },
                Event::Connecting { attempt: 2, of: 3 },
                Event::Connecting { attempt: 3, of: 3 },
                Event::Connected(address()),
            ]
        );
    }

    #[test]
    fn a_printer_that_never_answers_is_reported_after_the_last_attempt() {
        let fake = Fake::default();
        fake.state.lock().unwrap().connect_failures = 99;
        let printer = start(&fake);
        printer.send(Command::Print {
            address: address(),
            job: job(1),
        });
        let events = events_until(&printer, is_end_of_print);
        assert!(events.contains(&Event::ConnectFailed("no answer".to_string())));
        assert!(matches!(events.last(), Some(Event::PrintFailed(_))));
        assert!(fake.state.lock().unwrap().sent.is_empty());
    }

    #[test]
    fn disconnecting_closes_the_connection_so_the_next_print_reconnects() {
        let fake = Fake::default();
        let printer = start(&fake);
        printer.send(Command::Connect(address()));
        printer.send(Command::Disconnect);
        events_until(&printer, |event| matches!(event, Event::Disconnected));
        printer.send(Command::Print {
            address: address(),
            job: job(1),
        });
        events_until(&printer, is_end_of_print);
        assert_eq!(fake.state.lock().unwrap().connects, 2);
    }

    #[test]
    fn the_wake_up_call_comes_with_every_event() {
        let woken = Arc::new(Mutex::new(0u32));
        let counter = woken.clone();
        let fake = Fake::default();
        let printer = Printer::start(fake, 3, move || *counter.lock().unwrap() += 1);
        printer.send(Command::Connect(address()));
        // Connecting + Connected.
        events_until(&printer, |event| matches!(event, Event::Connected(_)));
        // The wake-up call follows each event, so give the thread a moment to make the last one.
        for _ in 0..200 {
            if *woken.lock().unwrap() >= 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(*woken.lock().unwrap(), 2);
    }

    #[test]
    fn a_job_can_be_followed_by_another_and_is_reported_once() {
        let fake = Fake::default();
        let printer = start(&fake);
        let mut first = job(1);
        let mut second = job(1);
        second.media = Media::Continuous;
        first.then = Some((Duration::ZERO, Box::new(second.clone())));
        printer.send(Command::Print {
            address: address(),
            job: first.clone(),
        });

        let events = events_until(&printer, is_end_of_print);
        // Each job goes over a connection of its own, and there is one report at the end.
        assert_eq!(
            events,
            [
                Event::Connecting { attempt: 1, of: 3 },
                Event::Connected(address()),
                Event::Printing { copy: 1, of: 1 },
                Event::Connecting { attempt: 1, of: 3 },
                Event::Connected(address()),
                Event::Printing { copy: 1, of: 1 },
                Event::Printed { copies: 1 },
            ]
        );
        // The init of each job comes before its picture, in order.
        let mut expected = expected_bytes(&first);
        expected.extend(expected_bytes(&second));
        let state = fake.state.lock().unwrap();
        assert_eq!(state.sent, expected);
        assert_eq!(state.connects, 2);
    }

    #[test]
    fn when_the_second_job_fails_halfway_it_is_not_sent_again_and_nothing_is_reported_as_printed() {
        let fake = Fake::default();
        // Every connection fails its third write. The first job is short, an init and one part of
        // a picture, so it gets through; the second is an init and two parts, and fails at the
        // second part.
        fake.state.lock().unwrap().fail_write = Some(3);
        let printer = start(&fake);
        let mut first = job(1);
        first.raster = Raster::blank(100).unwrap();
        first.then = Some((Duration::ZERO, Box::new(job(1))));
        printer.send(Command::Print {
            address: address(),
            job: first,
        });

        let events = events_until(&printer, is_end_of_print);
        assert!(matches!(events.last(), Some(Event::PrintFailed(_))));
        assert!(!events
            .iter()
            .any(|event| matches!(event, Event::Printed { .. })));
        // One connection for each of the two jobs, and the half-sent one is not sent again.
        assert_eq!(fake.state.lock().unwrap().connects, 2);
    }

    #[test]
    fn positioning_the_tape_is_a_blank_label_in_gap_mode_and_then_the_feed_as_uncut_tape() {
        let stock = LabelStock::parse("14x30").unwrap();
        let first = position_tape(&stock).expect("the 14 x 30 mm labels are counted");
        // A whole label of nothing, in the printer's own gap mode, so that it finds the gap.
        assert_eq!(first.media, Media::Gap);
        assert_eq!(first.raster.rows(), 242);
        assert!(first.raster.bytes().iter().all(|byte| *byte == 0));
        assert_eq!(first.copies, 1);
        // Then, after it has had time to settle, the feed to the middle of the gap, as uncut tape.
        let (pause, second) = first.then.as_ref().expect("there is a second step");
        assert_eq!(*pause, SETTLE);
        assert_eq!(second.media, Media::Continuous);
        assert_eq!(second.raster.rows(), 98);
        assert!(second.raster.bytes().iter().all(|byte| *byte == 0));
        assert!(second.then.is_none());

        // Labels that the printer finds by itself, and tape, have nothing to position.
        for id in ["12x40", "14x31", "15-continuous"] {
            assert!(position_tape(&LabelStock::parse(id).unwrap()).is_none(), "{id}");
        }
    }
}
