// TODO: Figure out how to handle 'fruit' labels
// TODO: Implement templates with fixed font sizes and positions
// TODO: Implement 'arbitrary image' feature

use std::{
    fs,
    io::{self, Cursor, Write},
    path::PathBuf,
    process::{exit, Command, Stdio},
};

use advmac::{MacAddr6, MacAddrFormat, ParseError};
use clap::{Parser, Subcommand};
use d30::bt;
use d30::fonts::{FontSource, FontSpec, Fonts};
use d30::render::{self, Align, Element, Label, Lines, TextElement};
use d30::stock::{LabelStock, Media, STOCKS};
use d30::D30Scale;
use image::{imageops, DynamicImage, ImageError, ImageFormat};
use inquire::InquireError;
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("d30-cli only has Bluetooth backends for Linux (BlueZ) and Windows (Winsock)");

#[derive(Debug, Parser)]
#[command(name = "d30")]
#[command(version, about = "A userspace Phomemo D30 controller.")]
/// `Arguments` stores the command line arguments passed in from the user or script
struct Arguments {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(short_flag = 't')]
    PrintText(Box<ArgsPrintText>),
    /// List the Bluetooth devices paired with this computer, to find the printer's address
    ListPrinters,
    /// List the kinds of label and tape the printer can use
    ListLabels,
    /// List the fonts that labels can use
    ListFonts,
}

#[derive(clap::Args, Debug, Clone)]
struct ArgsPrintText {
    #[arg(long)]
    dry_run: bool,
    #[arg(short, long)]
    device: Option<String>,
    /// The text of the label. `\n` starts a new line
    text: String,
    /// The label stock in the printer: a name from `list-labels`, or a size such as 12x35
    #[arg(short, long, default_value = d30::stock::DEFAULT_STOCK_ID)]
    label: String,
    /// How long to make the label, in mm. Only for continuous (uncut) tape, where it is cut to fit
    /// the text unless you say how long
    #[arg(long)]
    length: Option<f32>,
    /// The font: the name of an installed font (see `list-fonts`) or the path of a font file
    #[arg(short, long)]
    font: Option<String>,
    #[arg(long)]
    bold: bool,
    #[arg(long)]
    italic: bool,
    /// How the lines of text line up: left, center or right
    #[arg(short, long, default_value = "center")]
    align: Align,
    /// Split the text over this many lines, as evenly as the words allow, instead of breaking it
    /// only where it has a `\n` (which then counts as a space). A word is never broken, so a text
    /// of two words has two lines at the most
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=8))]
    lines: Option<u8>,
    /// How the printer tells labels apart: gap, continuous or marks. The label stock's own by default
    #[arg(long)]
    media: Option<Media>,
    /// Blank lines to print before the label. The first lines a printer prints can come out
    /// squashed together while the tape gets moving. Uncut tape gets 30 by default (about 4 mm),
    /// precut labels none
    #[arg(long)]
    lead_in_lines: Option<usize>,
    /// Blank lines to feed after printing, to push the label far enough out of the printer to tear
    /// it off with its text in the middle. Uncut tape gets 134 by default (about 17 mm), precut
    /// labels none
    #[arg(long)]
    feed_lines: Option<usize>,
    /// Also save the label as a picture at actual size (one pixel for each dot)
    #[arg(long)]
    save_image: Option<PathBuf>,
    #[arg(short, long)]
    #[arg(default_value = "auto")]
    scale: D30Scale,
    #[arg(long)]
    #[arg(default_value = "0")]
    minus_scale: f32,
    #[arg(short, long)]
    #[arg(default_value = "15.0")]
    margins: f32,
    #[arg(short, long)]
    preview: bool,
    #[arg(short, long)]
    #[arg(default_value = "1")]
    number_of_images: i32,
    #[arg(long)]
    #[arg(default_value = "10")]
    max_retries: usize,
    /// Retry wait in seconds
    #[arg(long)]
    #[arg(default_value = "1")]
    retry_wait: f32,
}

// ---------------------
// End CLI Processing

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum PreviewType {
    Wezterm,
    CustomCommand(Vec<String>),
    ShowImage,
    /// GNOME's `gio open`.
    Gio,
    /// Whatever the OS opens images with: `explorer` on Windows, `xdg-open` elsewhere.
    System,
}

impl Default for PreviewType {
    fn default() -> Self {
        // `gio` comes with GNOME-ish Linux desktops, while every Windows has a default image viewer.
        if cfg!(windows) {
            PreviewType::System
        } else {
            PreviewType::Gio
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    dry_run: Option<bool>,
    enable_preview: Option<bool>,
    preview: Option<PreviewType>,
    d30_config: Option<d30::D30Config>,
}

// #[derive(Debug, Snafu)]
// pub enum ReadD30CliConfigError {
// }

impl Config {
    fn load_config() -> Result<Self, CLIError> {
        let config_path = d30::place_config_file("phomemo-cli-config.toml").context(D30LibSnafu)?;
        let contents = fs::read_to_string(config_path).context(CouldNotReadFileSnafu)?;
        Ok(toml::from_str(contents.as_str()).context(CouldNotParseTOMLSnafu)?)
    }
}

fn run(args: Vec<String>) -> Result<std::process::Child, CLIError> {
    debug!("Running child process: {:?}", args);
    match args.as_slice() {
        [cmd, args @ ..] => std::process::Command::new(cmd)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context(CouldNotCallBinarySnafu { binary_name: cmd }),
        // .with_whatever_context(|_| format!("Failed to execute child process: {:?}", cmd)),
        [] => {
            // whatever!("No program specified");
            Err(CLIError::BinaryUnspecified)
        }
    }
}

fn wezterm_imgcat(target: impl AsRef<str>) -> Result<(), CLIError> {
    std::process::Command::new("wezterm")
        .arg("imgcat")
        .arg(target.as_ref())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context(CouldNotCallBinarySnafu {
            binary_name: "wezterm",
        })?;
    Ok(())
}

enum Accepted {
    Yes,
    No,
    Unknown,
}

fn backend_show_image(preview_image: DynamicImage) -> Result<Accepted, CLIError> {
    // let mut accepted = Accepted::Unknown;

    // `d30-cli-preview.exe` on Windows.
    let preview_exe = format!("d30-cli-preview{}", std::env::consts::EXE_SUFFIX);
    let possible_child_targets: Vec<PathBuf> = vec![
        std::env::current_exe()
            .context(IOSnafu {
                task: "find current exe",
            })?
            .parent()
            .context(ParentDirectoryMissingSnafu {
                task: "find parent of current exe",
            })?
            .join(&preview_exe),
        preview_exe.into(),
    ];
    for target in possible_child_targets {
        match Command::new(target).stdin(Stdio::piped()).spawn() {
            Ok(mut child) => {
                let mut stdin = child.stdin.take().expect("Failed to take child stdin");
                let mut bytes = Cursor::new(Vec::new());
                preview_image
                    .write_to(&mut bytes, ImageFormat::Png)
                    .context(ImageSnafu {
                        task: "write to PNG buffer",
                    })?;

                stdin.write_all(&bytes.into_inner()).context(IOSnafu {
                    task: "write image bytes",
                })?;
                drop(stdin);
                return Ok(
                    match child
                        .wait()
                        .context(IOSnafu {
                            task: "get child status",
                        })?
                        .code()
                        .unwrap_or(1)
                    {
                        0 => Accepted::Yes,
                        46 => Accepted::No,
                        _ => Accepted::Unknown,
                    },
                );
            }
            Err(e) => {
                info!("Could not find: {}", e);
                continue;
            }
        }
    }
    Ok(Accepted::Unknown)
}
fn cmd_show_preview(
    preview: Option<PreviewType>,
    preview_image: DynamicImage,
) -> Result<Accepted, CLIError> {
    let preview = preview.unwrap_or_default();
    let preview_image_file = temp_file::TempFile::new().context(IOSnafu {
        task: "init path to temporary file",
    })?;

    let path = preview_image_file
        .path()
        .with_extension("png")
        .into_os_string()
        .into_string()
        .unwrap();

    debug!("{:?}", &path);

    if preview != PreviewType::ShowImage {
        preview_image.save(&path).context(ImageSnafu {
            task: "write image to temporary file",
        })?;
    }

    let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
    preview_image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .ok();
    let bytes = bytes.into_inner();
    let preview_image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .expect("Failed to load");
    // preview_image.into_rgb8().

    debug!("Preview type: {:?}", preview);

    Ok(match preview {
        PreviewType::Wezterm => {
            wezterm_imgcat(&path)?;
            Accepted::Unknown
        }

        PreviewType::CustomCommand(mut custom_cmd) => {
            custom_cmd.push(path);
            run(custom_cmd)?;
            Accepted::Unknown
        }

        PreviewType::ShowImage => backend_show_image(preview_image)?,

        PreviewType::Gio => {
            run(vec!["gio".to_string(), "open".to_string(), path])?;
            Accepted::Unknown
        }

        PreviewType::System => {
            let opener = if cfg!(windows) {
                "explorer.exe"
            } else {
                "xdg-open"
            };
            run(vec![opener.to_string(), path])?;
            Accepted::Unknown
        }
    })
}

fn get_addr(config: &mut Config, user_maybe_addr: Option<String>) -> Result<MacAddr6, CLIError> {
    match (user_maybe_addr, d30::D30Config::read_d30_config()) {
        // The case that the user has specified an address, and we have a config loaded
        // We must use config to attempt to resolve the address
        (Some(user_specified_addr), Ok(d30_config)) => {
            info!("Device specified by user. Resolving via config.");
            let resolved_addr = d30_config
                .resolve_addr(&user_specified_addr)
                .context(D30LibSnafu)?;
            config.d30_config = Some(d30_config);
            Ok(resolved_addr)
        }
        // The case that the user has specified an address, but we do NOT have a config
        // We must hope that the user gave us a fully quallified address & not a hostname
        (Some(user_specified_addr), Err(_)) => {
            info!("Address specified by user. NO config. This will fail if address is not fully qualified.");
            Ok(user_specified_addr
                .parse::<MacAddr6>()
                .context(CouldNotParseMacAddrSnafu {
                    address: user_specified_addr.clone(),
                })?)
        }
        // No address on CLI, but there IS a config!
        // Try to resolve from config
        (Option::None, Ok(config)) => {
            info!("No address on CLI, but we have a config. Will attempt to identify default target from config.");
            match config.resolve_default() {
                Ok(addr) => Ok(addr),
                Err(e) => {
                    error!("No address specified on command line or config.\nNo way to know what device we are targeting. This is a critical failure.");
                    Err(e).context(D30LibSnafu)
                }
            }
        }

        (Option::None, Err(_)) => {
            error!("No address specified on command line or config. No way to know what device we are targeting. This is a critical failure.");
            todo!()
        }
    }
}

#[derive(Debug, Snafu)]
enum CLIError {
    #[snafu(display("D30 library error"))]
    D30LibError { source: d30::D30Error },
    #[snafu(display("Failed to prompt user in interactive mode"))]
    FailedToPromptUser { source: InquireError },

    #[snafu(display("Error while attempting task `{task}` in bluetooth backend: {source}"))]
    BluetoothBackend { source: bt::Error, task: String },

    #[snafu(display("IO error while attempting to execute task: {task}"))]
    IOError {
        task: String,
        source: std::io::Error,
    },

    #[snafu(display("IO error while attempting to execute task: {task}"))]
    ImageError { task: String, source: ImageError },

    #[snafu(display("Failed to read in automatically detected D30 CLI configuration path"))]
    CouldNotReadFile { source: io::Error },
    #[snafu(display("Failed to serialize TOML D30 config"))]
    CouldNotParseTOML { source: toml::de::Error },

    #[snafu(display("Could not parse MAC address: {address}"))]
    CouldNotParseMacAddr { source: ParseError, address: String },

    #[snafu(display("Failed to call external binary `{binary_name}`. Check program environment"))]
    CouldNotCallBinary {
        source: std::io::Error,
        binary_name: String,
    },

    #[snafu(display("Binary unspecified"))]
    BinaryUnspecified,

    #[snafu(display("{reason}"))]
    BadOption { reason: String },

    #[snafu(display("Parent directory missing while performing task: {task}"))]
    ParentDirectoryMissing { task: String },
}

fn cmd_print(config: &mut Config, args: &ArgsPrintText) -> Result<(), CLIError> {
    trace!("Call: cmd_print");
    let mut args = args.to_owned();
    let dry_run = config.dry_run.unwrap_or(false) || args.dry_run;
    let show_preview = config.enable_preview.unwrap_or(false) || args.preview;
    let addr = get_addr(config, args.device.clone())?;
    debug!(
        "Generating image {} with scale {:?}",
        &args.text, &args.scale
    );
    let args_text = unescape::unescape(&args.text).expect("Failed to unescape input");
    if args.minus_scale != 0.0 {
        match &mut args.scale {
            D30Scale::Value(_) => {
                warn!("Not sure why you gave me a minus scale when I'm not autoscaling. Ignoring value");
            }
            D30Scale::Auto { ref mut minus } => {
                *minus = args.minus_scale;
            }
        }
    }
    let stock = LabelStock::parse(&args.label).map_err(|reason| CLIError::BadOption { reason })?;
    let media = args.media.unwrap_or(stock.media);

    let (mut font, fonts) = match args.font.as_deref() {
        None => (FontSpec::default(), Fonts::embedded()),
        Some(name_or_path) => {
            let font = FontSpec::parse(name_or_path);
            // Looking through the installed fonts takes a moment, so only do it when needed.
            let fonts = match font.source {
                FontSource::Family(_) => Fonts::system(),
                _ => Fonts::embedded(),
            };
            (font, fonts)
        }
    };
    font.bold = args.bold;
    font.italic = args.italic;
    if font.source == FontSource::Embedded && (args.bold || args.italic) {
        warn!(
            "The built-in font has no bold or italic. Pick a font with --font (see `list-fonts`)"
        );
    }

    let mut text = TextElement::new(args_text);
    text.font = font;
    text.size = args.scale;
    text.margin = args.margins;
    text.align = args.align;
    if let Some(lines) = args.lines {
        text.lines = Lines::Split(usize::from(lines));
    }
    let mut label = Label::new(stock);
    label.length_mm = args.length;
    label.elements.push(Element::Text(text));

    let rendered = render::render(&label, &fonts).context(D30LibSnafu)?;
    let preview = rendered.preview();
    if let Some(path) = &args.save_image {
        preview.save(path).context(ImageSnafu {
            task: "save the label picture",
        })?;
        println!("Saved the label picture to {}", path.display());
    }
    // The picture is only a few hundred dots wide, which is tiny on a screen: show every dot as a
    // square of 4 by 4 pixels.
    let preview_image = DynamicImage::ImageLuma8(imageops::resize(
        &preview,
        preview.width() * 4,
        preview.height() * 4,
        imageops::FilterType::Nearest,
    ));
    let raster = rendered
        .raster()
        .and_then(|raster| {
            raster.with_lead_rows(
                args.lead_in_lines
                    .unwrap_or_else(|| label.stock.default_lead_rows()),
            )
        })
        .and_then(|raster| {
            raster.with_blank_rows(
                args.feed_lines
                    .unwrap_or_else(|| label.stock.default_feed_rows()),
            )
        })
        .context(D30LibSnafu)?;
    if show_preview {
        let should_accept = match cmd_show_preview(config.preview.clone(), preview_image)? {
            Accepted::Yes => true,
            Accepted::No => false,
            Accepted::Unknown => inquire::Confirm::new("Displayed preview. Accept this print?")
                .with_default(false)
                .prompt_skippable()
                .context(FailedToPromptUserSnafu)?
                .unwrap_or(false),
        };
        if !should_accept {
            println!("Goodbye UwU");
            return Ok(());
        }
    }

    let mut socket: Option<bt::RfcommStream> = None;
    println!("Connecting...");
    'retry: for retries in 0.. {
        info!("Retry #{}", retries);
        if retries > args.max_retries {
            error!(
                "Failed to connect after {} retries!\n\
                 The printer never answered. Usual causes:\n\
                 - it is switched off, asleep, or out of range\n\
                 {}",
                args.max_retries,
                bt::connection_help(&addr.format_string(MacAddrFormat::ColonNotation)),
            );
            exit(1);
        }
        if dry_run {
            break 'retry;
        }

        match bt::connect(addr).context(BluetoothBackendSnafu {
            task: "connecting to the printer".to_string(),
        }) {
            Ok(new_socket) => {
                socket = Some(new_socket);
                break 'retry;
            }
            Err(e) => {
                error!(
                    "Error while trying to connect, on attempt #{}:\n{}",
                    retries, e
                );
                continue 'retry;
            }
        }
    }

    debug!("Init connection");
    if let Some(socket) = &mut socket {
        socket
            .write_all(&d30::protocol::init_sequence(media))
            .context(IOSnafu {
                task: "send magic init bytes".to_string(),
            })?;
    }
    debug!("Extend output");

    // The picture has to be sent in chunks of 255 lines.
    let writes = d30::protocol::image_writes(&raster);
    for image_num in 0..args.number_of_images {
        for write in &writes {
            debug!("Write output to socket");
            if let Some(socket) = &mut socket {
                socket.write_all(write).context(IOSnafu {
                    task: format!("write image #{}", image_num),
                })?;
            }
            debug!("Flush socket");
            if let Some(socket) = &mut socket {
                socket.flush().context(IOSnafu {
                    task: "flush socket".to_string(),
                })?;
            }
        }
    }
    Ok(())
}

fn cmd_list_labels() {
    println!("{:<15} {:<34} TRIED ON A D30", "NAME", "KIND");
    for stock in STOCKS {
        println!(
            "{:<15} {:<34} {}",
            stock.id,
            stock.name,
            if stock.tested { "yes" } else { "no" }
        );
    }
    println!(
        "\nUse one with --label, e.g. `--label {}`. Any other size works too: `--label 12x35` for \
         labels 12 mm wide and 35 mm long, or `--label 13-continuous` for uncut tape 13 mm wide.",
        STOCKS[0].id
    );
}

fn cmd_list_fonts() {
    for family in Fonts::system().families() {
        println!("{family}");
    }
}

fn cmd_list_printers() -> Result<(), CLIError> {
    let devices = bt::list_paired_devices().context(IOSnafu {
        task: "list the paired Bluetooth devices",
    })?;
    if devices.is_empty() {
        println!("No paired Bluetooth devices found. Pair the printer with this computer first.");
    }
    for device in devices {
        println!(
            "{}  {}{}{}",
            device.address.format_string(MacAddrFormat::ColonNotation),
            device.name,
            if device.connected {
                "  (connected)"
            } else {
                ""
            },
            if device.looks_like_d30() {
                "  <- looks like a D30"
            } else {
                ""
            },
        );
    }
    Ok(())
}

#[snafu::report]
#[tokio::main]
async fn main() -> Result<(), CLIError> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "warn,naga=off"),
    );

    let args = Arguments::parse();
    debug!("Args: {:#?}", &args);
    let mut config = match Config::load_config() {
        Ok(config) => config,
        Err(CLIError::CouldNotReadFile { source }) => {
            debug!("Could not read file: {}", source);
            Config::default()
        }
        Err(CLIError::D30LibError {
            source: d30::D30Error::CouldNotPlaceConfigFile { source },
        }) => {
            debug!("Could not place config file: {}", source);
            Config::default()
        }

        Err(e) => {
            error!("Encountered surprising error: {}", e);
            Config::default()
        }
    };

    match &args.command {
        Commands::PrintText(args) => {
            cmd_print(&mut config, &args)?;
        }
        Commands::ListPrinters => cmd_list_printers()?,
        Commands::ListLabels => cmd_list_labels(),
        Commands::ListFonts => cmd_list_fonts(),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview_from_config(toml: &str) -> Option<PreviewType> {
        toml::from_str::<Config>(toml).unwrap().preview
    }

    #[test]
    fn preview_names_in_config_files() {
        assert_eq!(
            preview_from_config(r#"preview = "gio""#),
            Some(PreviewType::Gio)
        );
        assert_eq!(
            preview_from_config(r#"preview = "system""#),
            Some(PreviewType::System)
        );
        assert_eq!(
            preview_from_config(r#"preview = "show_image""#),
            Some(PreviewType::ShowImage)
        );
        assert_eq!(
            preview_from_config(r#"preview = "wezterm""#),
            Some(PreviewType::Wezterm)
        );
        assert_eq!(
            preview_from_config(r#"preview = { custom_command = ["mspaint"] }"#),
            Some(PreviewType::CustomCommand(vec!["mspaint".to_string()]))
        );
    }

    #[test]
    fn default_preview_is_something_the_platform_has() {
        let expected = if cfg!(windows) {
            PreviewType::System
        } else {
            PreviewType::Gio
        };
        assert_eq!(PreviewType::default(), expected);
    }
}
