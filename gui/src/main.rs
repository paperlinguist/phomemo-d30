//! A label designer for the Phomemo D30.

// No console window behind the program when it is started by double-clicking it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod emoji;
mod preview;
mod settings;

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("d30-gui only has Bluetooth backends for Linux (BlueZ) and Windows (Winsock)");

use std::path::PathBuf;

use app::{App, Options};
use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init_from_env(
        env_logger::Env::default()
            .filter_or(env_logger::DEFAULT_FILTER_ENV, "warn,wgpu=error,naga=error"),
    );

    let options = options_from_args();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(app::WINDOW_SIZE)
            .with_min_inner_size(app::MIN_WINDOW_SIZE)
            .with_title("Phomemo D30 labels"),
        ..Default::default()
    };
    eframe::run_native(
        "Phomemo D30 labels",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc, options)))),
    )
}

/// The options for trying things out: `--screenshot FILE` saves a picture of the window and closes
/// it, and `--text`, `--lines`, `--label` and `--font` start with those chosen. So does
/// `--picture FILE`, with `--picture-side left|right`, `--picture-dither` and `--picture-invert`.
/// `--qr TEXT` puts a QR code of a link or text on the label, and `--wifi NAME` with
/// `--wifi-password PASSWORD` one of a Wi-Fi network (`--qr-side left|right` says where).
/// `--emoji` opens the emoji picker.
fn options_from_args() -> Options {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshot" => options.screenshot = args.next().map(PathBuf::from),
            "--text" => options.text = args.next(),
            "--lines" => options.lines = args.next().and_then(|count| count.parse().ok()),
            "--picture" => options.picture = args.next().map(PathBuf::from),
            "--picture-side" => {
                options.picture_side = args.next().and_then(|side| side.parse().ok())
            }
            "--picture-dither" => options.picture_dither = true,
            "--picture-invert" => options.picture_invert = true,
            "--qr" => options.qr = args.next(),
            "--qr-side" => options.qr_side = args.next().and_then(|side| side.parse().ok()),
            "--wifi" => options.wifi = args.next(),
            "--wifi-password" => options.wifi_password = args.next(),
            "--emoji" => options.emoji = true,
            "--label" => options.label = args.next(),
            "--font" => options.font = args.next(),
            other => eprintln!("Ignoring the unknown option {other}"),
        }
    }
    options
}
