//! The window: printer and connection on top, the label's settings on the left, the label as it
//! will print in the middle, and the print button at the bottom.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use advmac::{MacAddr6, MacAddrFormat};
use d30::{
    bt,
    devices::{self, Device},
    fonts::Fonts,
    printer::{self, Command, Event, Job, Printer},
    render::{self, Align},
    stock::STOCKS,
};
use eframe::egui::{self, Color32, RichText};

use crate::{
    preview::Preview,
    settings::{LabelSettings, Saved},
};

/// How big the window starts out, and the smallest it can be made, in points.
pub const WINDOW_SIZE: [f32; 2] = [1000.0, 640.0];
pub const MIN_WINDOW_SIZE: [f32; 2] = [760.0, 460.0];

/// What can be asked of the program on the command line. They are mostly for trying things out.
#[derive(Default)]
pub struct Options {
    /// Save a picture of the window and close it.
    pub screenshot: Option<PathBuf>,
    pub text: Option<String>,
    pub label: Option<String>,
    pub font: Option<String>,
}

/// Which printer is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    /// One of the printers in the list.
    Device(MacAddr6),
    /// An address typed by hand.
    Other,
}

enum Connection {
    Disconnected,
    Connecting { attempt: u32, of: u32 },
    Connected(MacAddr6),
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Good,
    Bad,
}

struct Status {
    level: Level,
    text: String,
}

pub struct App {
    settings: LabelSettings,
    fonts: Fonts,

    devices: Vec<Device>,
    /// Why the list of printers may be incomplete.
    devices_note: Option<String>,
    config_default: Option<MacAddr6>,
    choice: Choice,
    manual_address: String,

    printer: Printer,
    connection: Connection,
    /// A print is on its way to the printer.
    printing: bool,
    status: Status,
    /// What to check when the printer cannot be reached.
    help: Option<String>,

    /// The label as drawn for `previewed`, or why it could not be drawn.
    preview: Option<Result<Preview, String>>,
    previewed: Option<LabelSettings>,

    screenshot: Option<PathBuf>,
    screenshot_taken: Arc<AtomicBool>,
    frames: u32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        let context = cc.egui_ctx.clone();
        let printer = Printer::start(printer::Bluetooth, printer::CONNECT_ATTEMPTS, move || {
            context.request_repaint()
        });
        Self::assemble(
            Saved::load(),
            devices::discover(),
            Fonts::system(),
            printer,
            options,
        )
    }

    /// Puts the window together from what the outside world has provided: what was remembered, the
    /// printers that were found, the fonts and the printer thread. Split from [`Self::new`] so that
    /// tests can supply their own.
    fn assemble(
        saved: Saved,
        discovery: devices::Discovery,
        fonts: Fonts,
        printer: Printer,
        options: Options,
    ) -> Self {
        let mut settings = saved.settings();
        if let Some(text) = options.text {
            settings.text = text.replace("\\n", "\n");
        }
        if let Some(label) = options.label {
            settings.stock_id = label;
        }
        if let Some(font) = options.font {
            settings.font = Some(font);
        }

        let saved_address = saved
            .printer
            .as_deref()
            .and_then(|text| text.parse::<MacAddr6>().ok());
        let (choice, manual_address) =
            initial_choice(&discovery.devices, saved_address, discovery.default_address);

        App {
            settings,
            fonts,
            devices: discovery.devices,
            devices_note: discovery.note,
            config_default: discovery.default_address,
            choice,
            manual_address,
            printer,
            connection: Connection::Disconnected,
            printing: false,
            status: Status {
                level: Level::Info,
                text: match choice {
                    Choice::Device(_) => "Ready.",
                    Choice::Other => "Choose a printer, or type its address.",
                }
                .to_string(),
            },
            help: None,
            preview: None,
            previewed: None,
            screenshot: options.screenshot,
            screenshot_taken: Arc::new(AtomicBool::new(false)),
            frames: 0,
        }
    }

    /// The address of the chosen printer, if there is a valid one.
    fn address(&self) -> Option<MacAddr6> {
        match self.choice {
            Choice::Device(address) => Some(address),
            Choice::Other => self.manual_address.trim().parse().ok(),
        }
    }

    fn address_text(address: MacAddr6) -> String {
        address.format_string(MacAddrFormat::ColonNotation)
    }

    fn refresh_devices(&mut self) {
        let discovery = devices::discover();
        self.devices = discovery.devices;
        self.devices_note = discovery.note;
        self.config_default = discovery.default_address;
        // Keep the choice if it is still there; otherwise pick again.
        if let Choice::Device(address) = self.choice {
            if !self.devices.iter().any(|device| device.address == address) {
                let (choice, manual) = initial_choice(&self.devices, None, self.config_default);
                self.choice = choice;
                self.manual_address = manual;
            }
        }
    }

    fn remember(&self) {
        // Trying things out with `--screenshot` must not change what is remembered.
        if self.screenshot.is_some() {
            return;
        }
        let printer = self.address().map(Self::address_text);
        if let Err(error) = Saved::new(&self.settings, printer).save() {
            log::warn!("Could not save the settings: {error}");
        }
    }

    fn say(&mut self, level: Level, text: impl Into<String>) {
        self.status = Status {
            level,
            text: text.into(),
        };
    }

    /// Takes in what the printer thread has reported.
    fn handle_printer_events(&mut self) {
        let events: Vec<Event> = self.printer.events().collect();
        for event in events {
            match event {
                Event::Connecting { attempt, of } => {
                    self.connection = Connection::Connecting { attempt, of };
                    self.help = None;
                    self.say(
                        Level::Info,
                        format!("Connecting (attempt {attempt} of {of})…"),
                    );
                }
                Event::Connected(address) => {
                    self.connection = Connection::Connected(address);
                    self.help = None;
                    // A connection that works is worth remembering.
                    self.remember();
                    if !self.printing {
                        self.say(Level::Good, "Connected.");
                    }
                }
                Event::ConnectFailed(error) => {
                    self.connection = Connection::Failed;
                    self.printing = false;
                    let help = self
                        .address()
                        .map(|address| bt::connection_help(&Self::address_text(address)));
                    self.help = help.map(|help| {
                        format!(
                            "The printer never answered. Usual causes:\n\
                             - it is switched off, asleep, or out of range\n\
                             {help}"
                        )
                    });
                    self.say(Level::Bad, format!("Could not connect: {error}"));
                }
                Event::Disconnected => {
                    self.connection = Connection::Disconnected;
                    self.say(Level::Info, "Disconnected.");
                }
                Event::Printing { copy, of } => {
                    self.printing = true;
                    self.say(Level::Info, format!("Printing label {copy} of {of}…"));
                }
                Event::Printed { copies } => {
                    self.printing = false;
                    self.remember();
                    self.say(
                        Level::Good,
                        if copies == 1 {
                            "Printed 1 label.".to_string()
                        } else {
                            format!("Printed {copies} labels.")
                        },
                    );
                }
                Event::PrintFailed(error) => {
                    self.printing = false;
                    // When the print failed because the printer could not be reached, the
                    // message about that, which says why, is already on show. Keep it.
                    if !matches!(self.connection, Connection::Failed) {
                        // The printer thread closes the connection when a print fails.
                        self.connection = Connection::Disconnected;
                        self.say(Level::Bad, format!("Printing failed: {error}"));
                    }
                }
            }
        }
    }

    /// Draws the label again if anything that matters for it has changed.
    fn update_preview(&mut self, ctx: &egui::Context) {
        if self.previewed.as_ref() == Some(&self.settings) {
            return;
        }
        self.previewed = Some(self.settings.clone());
        self.preview = Some(self.draw_label(ctx));
    }

    fn draw_label(&self, ctx: &egui::Context) -> Result<Preview, String> {
        let label = self.settings.label()?;
        let rendered = render::render(&label, &self.fonts).map_err(|error| error.to_string())?;
        Ok(Preview::new(ctx, rendered, label.stock))
    }

    fn print(&mut self) {
        let Some(address) = self.address() else {
            self.say(Level::Bad, "Choose a printer first.");
            return;
        };
        let Some(Ok(preview)) = &self.preview else {
            return;
        };
        let stock = &preview.stock;
        let raster = preview
            .rendered
            .raster()
            .and_then(|raster| raster.with_lead_rows(self.settings.lead_rows(stock)))
            .and_then(|raster| raster.with_blank_rows(self.settings.feed_rows(stock)));
        let raster = match raster {
            Ok(raster) => raster,
            Err(error) => {
                let text = error.to_string();
                self.say(Level::Bad, text);
                return;
            }
        };
        let job = Job {
            media: stock.media,
            raster,
            copies: self.settings.copies.max(1),
        };
        self.printing = true;
        self.help = None;
        self.say(Level::Info, "Sending the label to the printer…");
        self.printer.send(Command::Print { address, job });
    }

    // ---- the parts of the window -------------------------------------------------------------

    fn printer_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        // Wrapped, so that in a narrow window the connection controls move to a second row
        // instead of running off the edge.
        ui.horizontal_wrapped(|ui| {
            ui.label("Printer:");
            let shown = match self.choice {
                Choice::Device(address) => self
                    .devices
                    .iter()
                    .find(|device| device.address == address)
                    .map(Device::label)
                    .unwrap_or_else(|| Self::address_text(address)),
                Choice::Other => "Other address…".to_string(),
            };
            egui::ComboBox::from_id_salt("printer")
                .width(320.0)
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for device in &self.devices {
                        ui.selectable_value(
                            &mut self.choice,
                            Choice::Device(device.address),
                            device.label(),
                        );
                    }
                    ui.selectable_value(&mut self.choice, Choice::Other, "Other address…");
                });
            if self.choice == Choice::Other {
                ui.add(
                    egui::TextEdit::singleline(&mut self.manual_address)
                        .hint_text("AA:BB:CC:DD:EE:FF")
                        .desired_width(150.0),
                );
            }
            if ui
                .button("Refresh")
                .on_hover_text("Look again for paired printers")
                .clicked()
            {
                self.refresh_devices();
            }
            #[cfg(windows)]
            if ui
                .button("Pair…")
                .on_hover_text("Open Windows' Bluetooth settings, to pair the printer")
                .clicked()
            {
                let _ = std::process::Command::new("explorer.exe")
                    .arg("ms-settings:bluetooth")
                    .spawn();
            }

            ui.separator();
            self.connection_controls(ui);
        });
        if let Some(note) = &self.devices_note {
            ui.label(
                RichText::new(format!("Paired printers could not be listed: {note}"))
                    .small()
                    .color(ui.visuals().warn_fg_color),
            );
        }
        ui.add_space(4.0);
    }

    fn connection_controls(&mut self, ui: &mut egui::Ui) {
        let address = self.address();
        let (color, text) = match &self.connection {
            Connection::Disconnected => (Color32::from_gray(150), "Not connected".to_string()),
            Connection::Connecting { attempt, of } => (
                ui.visuals().warn_fg_color,
                format!("Connecting ({attempt}/{of})…"),
            ),
            Connection::Connected(address) => (
                Color32::from_rgb(60, 170, 90),
                format!("Connected to {}", Self::address_text(*address)),
            ),
            Connection::Failed => (ui.visuals().error_fg_color, "Could not connect".to_string()),
        };

        match &self.connection {
            Connection::Connected(connected) if Some(*connected) == address => {
                if ui.button("Disconnect").clicked() {
                    self.printer.send(Command::Disconnect);
                }
            }
            Connection::Connecting { .. } => {
                ui.add_enabled(false, egui::Button::new("Connect"));
            }
            _ => {
                let enabled = address.is_some() && !self.printing;
                if ui
                    .add_enabled(enabled, egui::Button::new("Connect"))
                    .on_hover_text("Connect now, so that printing starts at once")
                    .clicked()
                {
                    if let Some(address) = address {
                        self.printer.send(Command::Connect(address));
                    }
                }
            }
        }

        // A small round light and what it means.
        let (dot, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        ui.painter().circle_filled(dot.center(), 5.0, color);
        ui.label(text);
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("Text");
        ui.add(
            egui::TextEdit::multiline(&mut self.settings.text)
                .hint_text("Type the label here. Enter starts a new line.")
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );

        ui.add_space(10.0);
        ui.heading("Font");
        let font_text = self
            .settings
            .font
            .clone()
            .unwrap_or_else(|| "Built-in (DejaVu Sans)".to_string());
        egui::ComboBox::from_id_salt("font")
            .width(260.0)
            .selected_text(font_text)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.font, None, "Built-in (DejaVu Sans)");
                for family in self.fonts.families() {
                    ui.selectable_value(&mut self.settings.font, Some(family.clone()), family);
                }
            });
        let has_styles = self.settings.font.is_some();
        ui.horizontal(|ui| {
            ui.add_enabled(
                has_styles,
                egui::Checkbox::new(&mut self.settings.bold, "Bold"),
            );
            ui.add_enabled(
                has_styles,
                egui::Checkbox::new(&mut self.settings.italic, "Italic"),
            );
            if !has_styles {
                ui.label(
                    RichText::new("pick a font for bold and italic")
                        .small()
                        .weak(),
                );
            }
        });
        ui.checkbox(
            &mut self.settings.auto_size,
            "Make the text as large as fits",
        );
        if !self.settings.auto_size {
            ui.add(egui::Slider::new(&mut self.settings.size, 8.0..=200.0).text("size (dots)"));
        }
        ui.horizontal(|ui| {
            ui.label("Align:");
            ui.selectable_value(&mut self.settings.align, Align::Left, "Left");
            ui.selectable_value(&mut self.settings.align, Align::Center, "Center");
            ui.selectable_value(&mut self.settings.align, Align::Right, "Right");
        });
        ui.add(egui::Slider::new(&mut self.settings.margin, 0.0..=60.0).text("margin (dots)"));

        ui.add_space(10.0);
        ui.heading("Label");
        let preset_text = STOCKS
            .iter()
            .find(|stock| stock.id == self.settings.stock_id)
            .map(|stock| stock.name.to_string())
            .unwrap_or_else(|| self.settings.stock_id.clone());
        ui.add_enabled_ui(self.settings.custom_stock.trim().is_empty(), |ui| {
            egui::ComboBox::from_id_salt("stock")
                .width(260.0)
                .selected_text(preset_text)
                .show_ui(ui, |ui| {
                    for stock in STOCKS {
                        let name = if stock.tested {
                            stock.name.to_string()
                        } else {
                            format!("{} (not tried)", stock.name)
                        };
                        ui.selectable_value(
                            &mut self.settings.stock_id,
                            stock.id.to_string(),
                            name,
                        );
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("or a size:");
            ui.add(
                egui::TextEdit::singleline(&mut self.settings.custom_stock)
                    .hint_text("12x35 or 13-continuous")
                    .desired_width(160.0),
            );
        });
        match self.settings.stock() {
            Ok(stock) if stock.is_continuous() => {
                ui.checkbox(
                    &mut self.settings.fit_length,
                    "Cut the tape to fit the text",
                );
                if !self.settings.fit_length {
                    ui.horizontal(|ui| {
                        ui.label("Length:");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.length_mm)
                                .range(5.0..=500.0)
                                .suffix(" mm"),
                        );
                    });
                }
            }
            Ok(_) => {}
            Err(error) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        }

        ui.add_space(10.0);
        ui.collapsing("Tape handling", |ui| {
            let (lead_default, feed_default) = match self.settings.stock() {
                Ok(stock) => (stock.default_lead_rows(), stock.default_feed_rows()),
                Err(_) => (0, 0),
            };
            ui.label(
                RichText::new(
                    "Blank rows printed before and after a label. Uncut tape needs them so \
                     that the text is not squashed and the label comes out far enough to \
                     tear off. 8 rows are 1 mm.",
                )
                .small()
                .weak(),
            );
            optional_rows(
                ui,
                "Before the label",
                &mut self.settings.lead_in,
                lead_default,
            );
            optional_rows(ui, "After the label", &mut self.settings.feed, feed_default);
        });
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("Preview");
        match &self.preview {
            Some(Ok(preview)) => {
                ui.label(preview.size_text());
                ui.add_space(6.0);
                egui::ScrollArea::horizontal().show(ui, |ui| preview.show(ui));
            }
            Some(Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            None => {}
        }
    }

    fn print_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Copies:");
            ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=99));

            let ready = matches!(self.preview, Some(Ok(_)))
                && self.address().is_some()
                && !self.printing
                && !matches!(self.connection, Connection::Connecting { .. });
            let button = egui::Button::new(RichText::new("Print label").size(16.0))
                .min_size(egui::vec2(150.0, 34.0));
            let response = ui.add_enabled(ready, button);
            let response = if self.address().is_none() {
                response.on_disabled_hover_text("Choose a printer first")
            } else {
                response
            };
            if response.clicked() {
                self.print();
            }
        });

        // On a row of its own and told to wrap: in a row of widgets a label never wraps, so a long
        // message (Windows words "the printer is off" in about 190 characters) would run off the
        // edge of the window.
        let color = match self.status.level {
            Level::Info => ui.visuals().text_color(),
            Level::Good => Color32::from_rgb(60, 170, 90),
            Level::Bad => ui.visuals().error_fg_color,
        };
        ui.add(egui::Label::new(RichText::new(&self.status.text).color(color)).wrap());

        if let Some(help) = &self.help {
            ui.collapsing("What to check", |ui| {
                ui.label(help);
            });
        }
        ui.add_space(6.0);
    }

    /// For trying things out: save a picture of the window, then close it.
    fn handle_screenshot(&mut self, ui: &egui::Ui) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        self.frames += 1;
        // Wait a few frames, so that the layout has settled.
        if self.frames == 12 {
            let taken = self.screenshot_taken.clone();
            ui.ctx().request_screenshot(move |image| {
                let size = image.size;
                let mut rgba = Vec::with_capacity(size[0] * size[1] * 4);
                for pixel in &image.pixels {
                    rgba.extend_from_slice(&pixel.to_array());
                }
                if let Some(picture) =
                    image::RgbaImage::from_raw(size[0] as u32, size[1] as u32, rgba)
                {
                    if let Err(error) = picture.save(&path) {
                        eprintln!("Could not save the screenshot: {error}");
                    }
                }
                taken.store(true, Ordering::SeqCst);
            });
        }
        if self.screenshot_taken.load(Ordering::SeqCst) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ui.ctx().request_repaint();
    }
}

impl App {
    /// The whole window, once.
    fn show(&mut self, ui: &mut egui::Ui) {
        self.handle_printer_events();
        self.update_preview(ui.ctx());

        egui::Panel::top("printer_bar").show(ui, |ui| self.printer_bar(ui));
        egui::Panel::bottom("print_bar").show(ui, |ui| self.print_bar(ui));
        egui::Panel::left("settings")
            .default_size(340.0)
            .min_size(280.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.settings_panel(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.preview_panel(ui));

        self.handle_screenshot(ui);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    fn on_exit(&mut self) {
        self.remember();
    }
}

/// A number of blank rows that is either the default for the label stock or one the person set.
fn optional_rows(ui: &mut egui::Ui, label: &str, value: &mut Option<usize>, default: usize) {
    ui.horizontal(|ui| {
        let mut custom = value.is_some();
        if ui.checkbox(&mut custom, label).changed() {
            *value = if custom { Some(default) } else { None };
        }
        match value {
            Some(rows) => {
                ui.add(egui::DragValue::new(rows).range(0..=2000).suffix(" rows"));
            }
            None => {
                ui.label(RichText::new(format!("default: {default} rows")).weak());
            }
        }
    });
}

/// Which printer to start with: the one used last time, then the one named in the config file,
/// then anything that looks like a D30, then the first one there is. Failing all that, an address
/// typed by hand (the last one used, if it was not in the list).
fn initial_choice(
    devices: &[Device],
    last_used: Option<MacAddr6>,
    config_default: Option<MacAddr6>,
) -> (Choice, String) {
    let listed = |address: &MacAddr6| devices.iter().any(|device| device.address == *address);
    if let Some(address) = last_used.filter(listed) {
        return (Choice::Device(address), String::new());
    }
    if let Some(address) = config_default.filter(listed) {
        return (Choice::Device(address), String::new());
    }
    if let Some(device) = devices.iter().find(|device| device.looks_like_d30()) {
        return (Choice::Device(device.address), String::new());
    }
    if let Some(device) = devices.first() {
        return (Choice::Device(device.address), String::new());
    }
    let typed = last_used
        .map(|address| address.format_string(MacAddrFormat::ColonNotation))
        .unwrap_or_default();
    (Choice::Other, typed)
}

#[cfg(test)]
mod tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    fn device(name: &str, last_byte: u8) -> Device {
        Device {
            name: name.to_string(),
            address: MacAddr6::new([2, 0, 0, 0, 0, last_byte]),
            connected: false,
        }
    }

    fn address(last_byte: u8) -> MacAddr6 {
        MacAddr6::new([2, 0, 0, 0, 0, last_byte])
    }

    #[test]
    fn the_printer_used_last_time_is_chosen_if_it_is_still_listed() {
        let devices = [device("D30", 1), device("Other D30", 2)];
        let (choice, _) = initial_choice(&devices, Some(address(2)), Some(address(1)));
        assert_eq!(choice, Choice::Device(address(2)));
    }

    #[test]
    fn then_the_one_in_the_config_file_then_anything_that_looks_like_a_d30() {
        let devices = [device("Bose", 1), device("D30", 2), device("Titan", 3)];
        let (choice, _) = initial_choice(&devices, None, Some(address(3)));
        assert_eq!(choice, Choice::Device(address(3)));
        let (choice, _) = initial_choice(&devices, None, None);
        assert_eq!(choice, Choice::Device(address(2)));
        let devices = [device("Bose", 1), device("Titan", 3)];
        let (choice, _) = initial_choice(&devices, Some(address(9)), None);
        assert_eq!(choice, Choice::Device(address(1)));
    }

    #[test]
    fn with_no_printers_listed_the_address_is_typed_and_the_last_one_is_kept() {
        let (choice, typed) = initial_choice(&[], Some(address(7)), None);
        assert_eq!(choice, Choice::Other);
        assert_eq!(typed, "02:00:00:00:00:07");
        let (choice, typed) = initial_choice(&[], None, None);
        assert_eq!((choice, typed.as_str()), (Choice::Other, ""));
    }

    // ---- the window as a whole: is anything cut off? ----------------------------------------

    /// A printer that can never be reached, as if it were switched off.
    struct Unreachable;

    impl printer::Transport for Unreachable {
        type Stream = Vec<u8>;

        fn connect(&self, _address: MacAddr6) -> Result<Vec<u8>, String> {
            // Word for word what Windows says when the printer does not answer.
            Err(
                "A connection attempt failed because the connected party did not properly \
                 respond after a period of time, or established connection failed because \
                 connected host has failed to respond. (os error 10060)"
                    .to_string(),
            )
        }
    }

    fn test_app() -> App {
        let discovery = devices::Discovery {
            devices: vec![device("D30", 1)],
            ..Default::default()
        };
        App::assemble(
            Saved::default(),
            discovery,
            Fonts::embedded(),
            Printer::start(Unreachable, 1, || {}),
            Options::default(),
        )
    }

    /// Presses Connect on a printer that is switched off, and waits until the window has been told
    /// that it could not connect.
    fn failed_to_connect(app: &mut App) {
        let address = app.address().expect("a printer is chosen");
        app.printer.send(Command::Connect(address));
        // The printer thread answers in its own time.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !matches!(app.connection, Connection::Failed) {
            assert!(
                Instant::now() < deadline,
                "the printer thread never said that it could not connect"
            );
            app.handle_printer_events();
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// A piece of text as drawn on the screen.
    struct Shown {
        text: String,
        lines: usize,
        /// How far to the right the text reaches, in points.
        right: f32,
    }

    /// Lays the window out on a screen of this size, as egui would draw it, and returns the text.
    fn lay_out(app: &mut App, [width, height]: [f32; 2]) -> Vec<Shown> {
        fn collect(shape: &egui::Shape, shown: &mut Vec<Shown>) {
            match shape {
                egui::Shape::Text(text) => shown.push(Shown {
                    text: text.galley.text().to_string(),
                    lines: text.galley.rows.len(),
                    right: text.visual_bounding_rect().right(),
                }),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, shown)),
                _ => {}
            }
        }

        let context = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut shown = Vec::new();
        // Panels and the like find their sizes over a few frames, so keep what the last one drew.
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let output = context.run_ui(input, |ui| app.show(ui));
            shown.clear();
            for clipped in &output.shapes {
                collect(&clipped.shape, &mut shown);
            }
            output.drop_without_applying_deltas();
        }
        shown
    }

    /// Asserts that no text reaches past the right edge of the window, at its smallest and at the
    /// size it starts with.
    fn assert_fits(app: &mut App, what: &str) {
        for size in [MIN_WINDOW_SIZE, WINDOW_SIZE] {
            let beyond: Vec<String> = lay_out(app, size)
                .into_iter()
                .filter(|shown| shown.right > size[0] + 0.5)
                .map(|shown| shown.text)
                .collect();
            assert!(
                beyond.is_empty(),
                "{what}, in a window {} points wide, runs off the right edge: {beyond:?}",
                size[0]
            );
        }
    }

    #[test]
    fn the_error_for_a_printer_that_is_off_wraps_instead_of_running_off_the_window() {
        let mut app = test_app();
        failed_to_connect(&mut app);
        let message = app.status.text.clone();
        assert!(
            message.starts_with("Could not connect: ") && message.len() > 150,
            "the test needs the long error, but the status is {message:?}"
        );

        for size in [MIN_WINDOW_SIZE, WINDOW_SIZE] {
            let lines = lay_out(&mut app, size)
                .into_iter()
                .find(|shown| shown.text == message)
                .map(|shown| shown.lines);
            assert!(
                lines.is_some_and(|lines| lines >= 2),
                "in a window {} points wide the message should be wrapped onto several lines, \
                 but it took {lines:?}",
                size[0]
            );
        }
        assert_fits(&mut app, "the error for a printer that is off");
    }

    #[test]
    fn the_printer_row_fits_whichever_way_it_is_used() {
        let mut app = test_app();
        assert_fits(&mut app, "the window with a printer chosen");

        app.connection = Connection::Connected(address(1));
        assert_fits(&mut app, "the window when connected");

        // The widest the row gets: an address typed by hand next to a connection that is up.
        app.choice = Choice::Other;
        app.manual_address = "AA:BB:CC:DD:EE:FF".to_string();
        assert_fits(
            &mut app,
            "the window when connected, with an address typed in",
        );
    }
}
