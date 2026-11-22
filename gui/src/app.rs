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
    picture::Picture,
    printer::{self, Command, Event, Job, Printer},
    qr::{QrLevel, WifiSecurity},
    render::{self, Align, Lines, Side},
    stock::{LabelStock, STOCKS},
};
use eframe::egui::{self, Color32, RichText};

use crate::{
    emoji::{self, Emoji},
    preview::Preview,
    settings::{LabelSettings, PictureSettings, QrKind, Saved},
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
    /// How many lines to split the text into; 0 for where it has new lines.
    pub lines: Option<usize>,
    pub label: Option<String>,
    pub font: Option<String>,
    /// A picture file to put on the label, and how: at which end, dithered, swapped.
    pub picture: Option<PathBuf>,
    pub picture_side: Option<Side>,
    pub picture_dither: bool,
    pub picture_invert: bool,
    /// A QR code of a link or text, or of a Wi-Fi network (name and password), and at which end.
    pub qr: Option<String>,
    pub qr_side: Option<Side>,
    pub wifi: Option<String>,
    pub wifi_password: Option<String>,
    /// Open the emoji picker.
    pub emoji: bool,
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

/// The emoji picker: what is typed in its search, which group it shows, and which emoji the fonts
/// can draw.
#[derive(Default)]
struct EmojiPicker {
    query: String,
    group: usize,
    /// For each group, the emoji of it that can be drawn. Found when the picker is first shown,
    /// because that can mean reading a large font.
    drawable: Option<Vec<Vec<Emoji>>>,
    /// The search box is to be given the keyboard as soon as it is there.
    focus_search: bool,
    /// The picker is to open on its own, which is for trying things out.
    open_now: bool,
}

impl EmojiPicker {
    /// Shows the picker, and returns the emoji that was clicked on, if one was.
    fn show(&mut self, ui: &mut egui::Ui, fonts: &Fonts) -> Option<char> {
        let Self {
            query,
            group,
            drawable,
            focus_search,
            open_now: _,
        } = self;
        let drawable = drawable.get_or_insert_with(|| {
            emoji::GROUPS
                .iter()
                .map(|group| {
                    group
                        .emoji()
                        .filter(|emoji| fonts.fallback_draws(emoji.symbol))
                        .collect()
                })
                .collect()
        });

        ui.set_width(330.0);
        let search = ui.add(
            egui::TextEdit::singleline(query)
                .hint_text("Search: carrot, home, heart…")
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(focus_search) {
            search.request_focus();
        }
        let searching = !query.trim().is_empty();
        if !searching {
            ui.horizontal_wrapped(|ui| {
                for (index, shown) in emoji::GROUPS.iter().enumerate() {
                    ui.selectable_value(group, index, shown.icon.to_string())
                        .on_hover_text(shown.name);
                }
            });
            ui.label(RichText::new(emoji::GROUPS[*group].name).small().weak());
        }
        let shown: Vec<Emoji> = if searching {
            emoji::search(query)
                .into_iter()
                .filter(|emoji| fonts.fallback_draws(emoji.symbol))
                .collect()
        } else {
            drawable[(*group).min(drawable.len() - 1)].clone()
        };

        let mut chosen = None;
        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                if shown.is_empty() {
                    ui.label("Nothing like that.");
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                    for emoji in &shown {
                        let button =
                            egui::Button::new(RichText::new(emoji.symbol.to_string()).size(22.0))
                                .min_size(egui::vec2(34.0, 34.0))
                                .frame(false);
                        if ui.add(button).on_hover_text(emoji.name).clicked() {
                            chosen = Some(emoji.symbol);
                        }
                    }
                });
            });
        chosen
    }
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
    /// What is on its way to the printer is the job that positions the tape, not a label.
    positioning: bool,
    status: Status,
    /// What to check when the printer cannot be reached.
    help: Option<String>,

    /// The label as drawn for `previewed`, or why it could not be drawn.
    preview: Option<Result<Preview, String>>,
    previewed: Option<LabelSettings>,

    /// Why the picture could not be read, if that is so.
    picture_error: Option<String>,
    /// The password of the Wi-Fi network shows as text instead of dots.
    show_wifi_password: bool,
    /// The person has asked to choose a picture file. The dialog is opened by [`eframe::App::ui`],
    /// which has the window to put it in front of.
    pick_picture: bool,

    emoji: EmojiPicker,

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
        let fonts = Fonts::system();
        install_emoji_font(&cc.egui_ctx, &fonts);
        Self::assemble(Saved::load(), devices::discover(), fonts, printer, options)
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
        if let Some(count) = options.lines {
            settings.lines = match count {
                0 => Lines::AsTyped,
                count => Lines::Split(count),
            };
        }
        if let Some(label) = options.label {
            settings.stock_id = label;
        }
        if let Some(font) = options.font {
            settings.font = Some(font);
        }
        if let Some(name) = options.wifi {
            settings.qr.kind = QrKind::Wifi;
            settings.qr.wifi.name = name;
            settings.qr.wifi.password = options.wifi_password.unwrap_or_default();
        } else if let Some(text) = options.qr {
            settings.qr.kind = QrKind::Text;
            settings.qr.text = text;
        }
        if let Some(side) = options.qr_side {
            settings.qr.side = side;
        }

        let saved_address = saved
            .printer
            .as_deref()
            .and_then(|text| text.parse::<MacAddr6>().ok());
        let (choice, manual_address) =
            initial_choice(&discovery.devices, saved_address, discovery.default_address);

        let mut app = App {
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
            positioning: false,
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
            picture_error: None,
            show_wifi_password: false,
            pick_picture: false,
            emoji: EmojiPicker {
                open_now: options.emoji,
                ..Default::default()
            },
            screenshot: options.screenshot,
            screenshot_taken: Arc::new(AtomicBool::new(false)),
            frames: 0,
        };

        // The picture on the label: the one that the command line names, or else the one that was
        // on it last time (if its file can still be read).
        if let Some(path) = options.picture {
            app.load_picture(path);
            if let Some(picture) = &mut app.settings.picture {
                picture.side = options.picture_side.unwrap_or(picture.side);
                picture.dither = options.picture_dither;
                picture.invert = options.picture_invert;
            }
        } else if let Some(remembered) = &saved.picture {
            if let Some(path) = remembered.path() {
                match Picture::open(&path) {
                    Ok(picture) => app.settings.picture = Some(remembered.settings(path, picture)),
                    Err(error) => app.picture_error = Some(error.to_string()),
                }
            }
        }
        app
    }

    /// Puts the picture in this file on the label, or says why it cannot.
    fn load_picture(&mut self, path: PathBuf) {
        match Picture::open(&path) {
            Ok(picture) => {
                self.picture_error = None;
                match &mut self.settings.picture {
                    // Another picture takes the place of the one there is, set in the same way.
                    Some(current) => {
                        current.path = path;
                        current.picture = picture;
                    }
                    None => self.settings.picture = Some(PictureSettings::new(path, picture)),
                }
            }
            Err(error) => self.picture_error = Some(error.to_string()),
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
                    self.positioning = false;
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
                    if self.positioning {
                        self.say(
                            Level::Info,
                            "Positioning the tape: a blank label comes out first, then the tape \
                             moves on a little. This takes about 20 seconds…",
                        );
                    } else {
                        self.say(Level::Info, format!("Printing label {copy} of {of}…"));
                    }
                }
                Event::Printed { .. } if self.positioning => {
                    self.printing = false;
                    self.positioning = false;
                    self.say(
                        Level::Good,
                        "The tape is positioned. Tear off what came out at the printer's edge: \
                         the printed line is on the edge.",
                    );
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
                    self.positioning = false;
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
        let plan = self.settings.tape_plan(&preview.stock);
        let raster = match preview.rendered.raster_for(&plan) {
            Ok(raster) => raster,
            Err(error) => {
                let text = error.to_string();
                self.say(Level::Bad, text);
                return;
            }
        };
        let job = Job {
            media: plan.media,
            raster,
            copies: self.settings.copies.max(1),
            then: None,
        };
        self.printing = true;
        self.help = None;
        self.say(Level::Info, "Sending the label to the printer…");
        self.printer.send(Command::Print { address, job });
    }

    /// Puts the tape where the program expects it to be for labels that it keeps count of: a
    /// blank label, which the printer finds the gap for itself, and then a little more tape.
    fn position_tape(&mut self, stock: &LabelStock) {
        let Some(address) = self.address() else {
            self.say(Level::Bad, "Choose a printer first.");
            return;
        };
        let Some(job) = printer::position_tape(stock) else {
            return;
        };
        self.printing = true;
        self.positioning = true;
        self.help = None;
        self.say(Level::Info, "Sending the blank label to the printer…");
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
        let text_id = egui::Id::unique("label text");
        ui.horizontal(|ui| {
            ui.heading("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.emoji_button(ui, text_id);
            });
        });
        let typing_hint = match self.settings.lines {
            Lines::AsTyped => "Type the label here. Enter starts a new line.",
            Lines::Split(_) => "Type the label here. It is split into lines for you.",
        };
        ui.add(
            egui::TextEdit::multiline(&mut self.settings.text)
                .id(text_id)
                .hint_text(typing_hint)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.label("Lines:");
            ui.selectable_value(&mut self.settings.lines, Lines::AsTyped, "As typed")
                .on_hover_text("A new line only where you press Enter");
            for count in 1..=MOST_LINES_OFFERED {
                ui.selectable_value(
                    &mut self.settings.lines,
                    Lines::Split(count),
                    count.to_string(),
                )
                .on_hover_text(format!(
                    "Split the text over {count} line{}, as evenly as the words allow",
                    if count == 1 { "" } else { "s" }
                ));
            }
        });
        if let Some(hint) = lines_hint(self.settings.lines, &self.settings.text) {
            ui.label(RichText::new(hint).small().weak());
        }

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

        self.picture_section(ui);
        self.qr_section(ui);

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
            Ok(stock) if stock.counted_gap_mm().is_some() => {
                // The program keeps track of these labels itself, which only works if the tape
                // is left alone between two labels.
                ui.label(
                    RichText::new(
                        "These labels have a wide gap, so the program keeps track of where \
                         they are. Tear each label off at the printer's edge (the printed \
                         line is then at the edge), and do not pull the tape out or press \
                         the printer's button. The first 3 mm of each label cannot be \
                         printed, so the text keeps that far from the ends. After a new \
                         roll, or if the tape was moved, press Position the tape.",
                    )
                    .small()
                    .weak(),
                );
                let ready = self.address().is_some()
                    && !self.printing
                    && !matches!(self.connection, Connection::Connecting { .. });
                let response = ui.add_enabled(ready, egui::Button::new("Position the tape"));
                let response = if self.address().is_none() {
                    response.on_disabled_hover_text("Choose a printer first")
                } else {
                    response.on_hover_text(
                        "Feeds a blank label, so that the printer finds the gap, and then a \
                         little more tape. Tear off what comes out at the printer's edge.",
                    )
                };
                if response.clicked() {
                    self.position_tape(&stock);
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
    /// The button that opens the emoji picker, and the picker when it is open. An emoji that is
    /// clicked goes into the text where the cursor was, and the picker stays open for the next.
    fn emoji_button(&mut self, ui: &mut egui::Ui, text_id: egui::Id) {
        let button = ui
            .button("😀 Emoji")
            .on_hover_text("Put an emoji in the text");
        if button.clicked() {
            self.emoji.focus_search = true;
        }
        let mut chosen = None;
        let open_now = std::mem::take(&mut self.emoji.open_now);
        egui::Popup::from_toggle_button_response(&button)
            .open_memory(if button.clicked() {
                Some(egui::SetOpenCommand::Toggle)
            } else if open_now {
                Some(egui::SetOpenCommand::Bool(true))
            } else {
                None
            })
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| chosen = self.emoji.show(ui, &self.fonts));
        if let Some(symbol) = chosen {
            self.insert_into_text(ui.ctx(), text_id, symbol);
        }
    }

    /// Puts `symbol` into the text where the cursor of the text box was (in place of what was
    /// selected), or at the end if the cursor was never there, puts the cursor after it, and gives
    /// the keyboard back to the text box.
    fn insert_into_text(&mut self, ctx: &egui::Context, text_id: egui::Id, symbol: char) {
        let mut state = egui::TextEdit::load_state(ctx, text_id).unwrap_or_default();
        let selected = state.cursor.char_range().map(|range| {
            let [start, end] = range.sorted_cursors();
            (start.index.0, end.index.0)
        });
        let cursor = insert_at_cursor(&mut self.settings.text, selected, &symbol.to_string());
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor),
            )));
        state.store(ctx, text_id);
        ctx.memory_mut(|memory| memory.request_focus(text_id));
    }

    /// The picture on the label: choosing it, and how it is turned into dots.
    fn picture_section(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("Picture");
        ui.horizontal(|ui| {
            if ui.button("Choose a picture…").clicked() {
                self.pick_picture = true;
            }
            if self.settings.picture.is_some() && ui.button("Remove").clicked() {
                self.settings.picture = None;
                self.picture_error = None;
            }
        });
        if let Some(error) = &self.picture_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let Some(picture) = &mut self.settings.picture else {
            ui.label(
                RichText::new("Or drop a picture file on the window.")
                    .small()
                    .weak(),
            );
            return;
        };

        let name = picture
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        ui.label(
            RichText::new(format!(
                "{name}  ({} × {} pixels)",
                picture.picture.width(),
                picture.picture.height()
            ))
            .small()
            .weak(),
        );
        ui.horizontal(|ui| {
            ui.label("Side:");
            ui.selectable_value(&mut picture.side, Side::Left, "Left");
            ui.selectable_value(&mut picture.side, Side::Right, "Right");
        });
        ui.horizontal(|ui| {
            ui.label("Dots:");
            ui.selectable_value(&mut picture.dither, false, "Black and white")
                .on_hover_text("For drawings, logos and writing");
            ui.selectable_value(&mut picture.dither, true, "Photo")
                .on_hover_text("Patterns of dots that follow the grays, for photographs");
        });
        if !picture.dither {
            ui.add(egui::Slider::new(&mut picture.level, 1..=254).text("black below"))
                .on_hover_text("Parts of the picture darker than this are black");
        }
        ui.checkbox(&mut picture.invert, "Swap black and white");
        ui.checkbox(&mut picture.fit_height, "As high as the label allows");
        if !picture.fit_height {
            ui.add(egui::Slider::new(&mut picture.height, 8.0..=96.0).text("height (dots)"));
        }
    }

    fn qr_section(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.heading("QR code");
        let qr = &mut self.settings.qr;
        egui::ComboBox::from_id_salt("qr kind")
            .width(200.0)
            .selected_text(qr.kind.name())
            .show_ui(ui, |ui| {
                for kind in QrKind::ALL {
                    ui.selectable_value(&mut qr.kind, kind, kind.name());
                }
            });
        match qr.kind {
            QrKind::Off => return,
            QrKind::Wifi => {
                ui.horizontal(|ui| {
                    ui.label("Network name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut qr.wifi.name)
                            .hint_text("the name of the Wi-Fi")
                            .desired_width(f32::INFINITY),
                    );
                });
                egui::ComboBox::from_id_salt("wifi security")
                    .width(200.0)
                    .selected_text(qr.wifi.security.name())
                    .show_ui(ui, |ui| {
                        for security in WifiSecurity::ALL {
                            ui.selectable_value(&mut qr.wifi.security, security, security.name());
                        }
                    });
                if qr.wifi.security != WifiSecurity::None {
                    ui.horizontal(|ui| {
                        ui.label("Password:");
                        ui.add(
                            egui::TextEdit::singleline(&mut qr.wifi.password)
                                .password(!self.show_wifi_password)
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.checkbox(&mut self.show_wifi_password, "Show the password")
                        .on_hover_text("The password is not saved when you close the window");
                }
                ui.checkbox(&mut qr.wifi.hidden, "A hidden network")
                    .on_hover_text("The network does not announce its name");
            }
            QrKind::Text => {
                ui.add(
                    egui::TextEdit::multiline(&mut qr.text)
                        .hint_text("A link (https://…) or some text")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
            }
        }
        ui.horizontal(|ui| {
            ui.label("Side:");
            ui.selectable_value(&mut qr.side, Side::Left, "Left");
            ui.selectable_value(&mut qr.side, Side::Right, "Right");
        });
        ui.horizontal(|ui| {
            ui.label("Error correction:");
            egui::ComboBox::from_id_salt("qr level")
                .width(110.0)
                .selected_text(level_name(qr.level))
                .show_ui(ui, |ui| {
                    for level in QrLevel::ALL {
                        ui.selectable_value(&mut qr.level, level, level_name(level))
                            .on_hover_text(level_hint(level));
                    }
                });
        });
        ui.label(
            RichText::new(
                "The code is as large as the label allows, and a longer text makes it smaller. \
                 Low makes the largest code.",
            )
            .small()
            .weak(),
        );
    }

    /// The whole window, once.
    fn show(&mut self, ui: &mut egui::Ui) {
        // A picture file dropped on the window goes on the label.
        let dropped = ui.ctx().input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_path_buf())
        });
        if let Some(path) = dropped {
            self.load_picture(path);
        }

        self.handle_printer_events();
        self.update_preview(ui.ctx());

        egui::Panel::top("printer_bar").show(ui, |ui| self.printer_bar(ui));
        egui::Panel::bottom("print_bar").show(ui, |ui| self.print_bar(ui));
        egui::Panel::left("settings")
            .default_size(340.0)
            .min_size(280.0)
            .show(ui, |ui| {
                // The settings can be taller than the window, and a scroll bar that only shows
                // when the mouse is over it does not tell that there is more below.
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                egui::ScrollArea::vertical().show(ui, |ui| self.settings_panel(ui));
            });
        egui::CentralPanel::default().show(ui, |ui| self.preview_panel(ui));

        show_what_is_being_dropped(ui.ctx());
        self.handle_screenshot(ui);
    }
}

/// What the levels of error correction of a QR code are called in the window.
fn level_name(level: QrLevel) -> &'static str {
    match level {
        QrLevel::Low => "Low",
        QrLevel::Medium => "Medium",
        QrLevel::Quartile => "Quartile",
        QrLevel::High => "High",
    }
}

fn level_hint(level: QrLevel) -> &'static str {
    match level {
        QrLevel::Low => "The smallest code, so the largest squares: what a small label wants",
        QrLevel::Medium => "A code that can be a little damaged and still be read",
        QrLevel::Quartile => "A sturdier code, with more and smaller squares",
        QrLevel::High => "The sturdiest code, with the most and the smallest squares",
    }
}

/// While a file is held over the window, covers it with what is going to happen if it is let go.
fn show_what_is_being_dropped(ctx: &egui::Context) {
    if ctx.input(|input| input.raw.hovered_files.is_empty()) {
        return;
    }
    let layer = egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::unique("file_drop_target"),
    );
    let painter = ctx.layer_painter(layer);
    let area = ctx.content_rect();
    painter.rect_filled(area, 0.0, Color32::from_black_alpha(180));
    painter.text(
        area.center(),
        egui::Align2::CENTER_CENTER,
        "Drop the picture here",
        egui::TextStyle::Heading.resolve(&ctx.global_style()),
        Color32::WHITE,
    );
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.show(ui);

        // The dialog is opened here and not in `show`, because it needs the window to be put in
        // front of. The program waits for it, which is what a person choosing a file expects.
        if std::mem::take(&mut self.pick_picture) {
            let picked = rfd::FileDialog::new()
                .set_title("Choose a picture for the label")
                .add_filter(
                    "Pictures",
                    &[
                        "png", "jpg", "jpeg", "gif", "bmp", "ico", "tif", "tiff", "webp",
                    ],
                )
                .add_filter("All files", &["*"])
                .set_parent(&*frame)
                .pick_file();
            if let Some(path) = picked {
                self.load_picture(path);
            }
        }
    }

    fn on_exit(&mut self) {
        self.remember();
    }
}

/// How many lines the window offers to split the text into. A label is 12 mm high, so more lines
/// than this would hardly be readable.
const MOST_LINES_OFFERED: usize = 3;

/// What to tell the person about splitting the text, if there is something to say.
fn lines_hint(lines: Lines, text: &str) -> Option<String> {
    let Lines::Split(count) = lines else {
        return None;
    };
    let words = text.split_whitespace().count();
    if words == 0 {
        None
    } else if words < count {
        // A word is never broken, so there are not as many lines as were asked for.
        let plural = if words == 1 { "" } else { "s" };
        Some(format!(
            "Only {words} word{plural}, so just {words} line{plural}."
        ))
    } else {
        Some("Split for you. New lines you type count as spaces.".to_string())
    }
}

/// Puts `text` into `target` from character `selected.0` to character `selected.1` (the selection
/// of a text box, which is where the cursor is when they are the same), or at the end if there is
/// no cursor. Returns the character the cursor goes to: right after what was put in.
fn insert_at_cursor(target: &mut String, selected: Option<(usize, usize)>, text: &str) -> usize {
    let count = target.chars().count();
    let (start, end) = selected.map_or((count, count), |(start, end)| {
        (start.min(count), end.min(count))
    });
    let byte = |character: usize| {
        target
            .char_indices()
            .nth(character)
            .map_or(target.len(), |(byte, _)| byte)
    };
    let (from, to) = (byte(start), byte(end));
    target.replace_range(from..to, text);
    start + text.chars().count()
}

/// Lets the window draw the emoji that the labels do: the installed emoji font (Windows has a
/// recent one) after the fonts the window has. Without it the window's own, older emoji font would
/// show a box for the newer emoji, in the text box and in the picker, though the label has them.
fn install_emoji_font(ctx: &egui::Context, fonts: &Fonts) {
    let Some((bytes, index)) = fonts.installed_emoji_font() else {
        return;
    };
    let mut data = egui::FontData::from_owned(bytes);
    data.index = index;
    let mut definitions = egui::FontDefinitions::default();
    definitions
        .font_data
        .insert("installed-emoji".to_string(), Arc::new(data));
    if let Some(proportional) = definitions
        .families
        .get_mut(&egui::FontFamily::Proportional)
    {
        // After the first font, which has the letters and digits, and before the others, so that
        // an emoji is drawn the way the label draws it.
        proportional.insert(1.min(proportional.len()), "installed-emoji".to_string());
    }
    ctx.set_fonts(definitions);
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
    fn the_person_is_told_when_a_text_has_too_few_words_for_the_lines_asked_for() {
        assert_eq!(lines_hint(Lines::AsTyped, "Kitchen pantry"), None);
        assert_eq!(lines_hint(Lines::Split(2), "   "), None);
        assert_eq!(
            lines_hint(Lines::Split(2), "Hello").as_deref(),
            Some("Only 1 word, so just 1 line.")
        );
        assert_eq!(
            lines_hint(Lines::Split(3), "Hello\nthere").as_deref(),
            Some("Only 2 words, so just 2 lines.")
        );
        assert!(lines_hint(Lines::Split(2), "Hello there you")
            .unwrap()
            .starts_with("Split for you"));
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
    fn lay_out(app: &mut App, size: [f32; 2]) -> Vec<Shown> {
        lay_out_with(app, size, |_| {})
    }

    /// Like [`lay_out`], with something done to what egui is told about the world each frame
    /// (a file held over the window, for instance).
    fn lay_out_with(
        app: &mut App,
        [width, height]: [f32; 2],
        adjust: impl Fn(&mut egui::RawInput),
    ) -> Vec<Shown> {
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
            let mut input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            adjust(&mut input);
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

    // ---- pictures ---------------------------------------------------------------------------

    /// A folder of its own for the files a test makes, removed when it is dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("d30-gui-test-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        /// A PNG, black all over, `size` dots square.
        fn black_picture(&self, name: &str, size: u32) -> PathBuf {
            let path = self.0.join(name);
            image::RgbaImage::from_pixel(size, size, image::Rgba([0, 0, 0, 255]))
                .save(&path)
                .unwrap();
            path
        }

        fn broken_file(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"this is not a picture").unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A file as egui hands it over when it is dropped on the window.
    #[derive(Debug)]
    struct Dropped(PathBuf);

    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &std::path::Path {
            &self.0
        }

        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn a_picture_dropped_on_the_window_goes_on_the_label() {
        let dir = TempDir::new("dropped");
        let path = dir.black_picture("jar.png", 20);
        let mut app = test_app();
        assert!(app.settings.picture.is_none());

        let context = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_SIZE[0], WINDOW_SIZE[1]),
            )),
            dropped_files: vec![Arc::new(Dropped(path.clone())) as egui::DroppedFileHandle],
            ..Default::default()
        };
        context
            .run_ui(input, |ui| app.show(ui))
            .drop_without_applying_deltas();

        let picture = app
            .settings
            .picture
            .as_ref()
            .expect("the picture is on the label");
        assert_eq!(picture.path, path);
        assert_eq!(
            (picture.picture.width(), picture.picture.height()),
            (20, 20)
        );
        assert!(app.picture_error.is_none());
        // And the preview shows it: 66 dots square, from column 15 on a 12x40 label.
        let Some(Ok(preview)) = &app.preview else {
            panic!("the label should have been drawn");
        };
        let canvas = preview.rendered.canvas();
        assert!(canvas.get_pixel(40, 48)[0] > 127);
    }

    #[test]
    fn while_a_file_is_held_over_the_window_it_says_what_will_happen() {
        let mut app = test_app();
        let said = |shown: &[Shown]| {
            shown
                .iter()
                .any(|text| text.text == "Drop the picture here")
        };
        assert!(!said(&lay_out(&mut app, WINDOW_SIZE)));
        let held = lay_out_with(&mut app, WINDOW_SIZE, |input| {
            input.hovered_files = vec![egui::HoveredFile::default()];
        });
        assert!(said(&held));
    }

    #[test]
    fn a_file_that_is_not_a_picture_is_reported_and_the_label_is_left_alone() {
        let dir = TempDir::new("broken");
        let mut app = test_app();
        app.load_picture(dir.broken_file("notes.png"));
        assert!(app.settings.picture.is_none());
        let error = app.picture_error.clone().expect("there is an error");
        assert!(error.contains("notes.png"), "{error}");

        // A picture that can be read clears the error.
        app.load_picture(dir.black_picture("jar.png", 8));
        assert!(app.settings.picture.is_some());
        assert!(app.picture_error.is_none());

        // And one that cannot be read does not take the picture away.
        app.load_picture(dir.0.join("missing.png"));
        assert!(app.settings.picture.is_some());
        assert!(app.picture_error.is_some());
    }

    #[test]
    fn another_picture_is_set_in_the_same_way_as_the_one_it_replaces() {
        let dir = TempDir::new("replace");
        let mut app = test_app();
        app.load_picture(dir.black_picture("first.png", 8));
        let first = app.settings.picture.clone().unwrap();
        {
            let picture = app.settings.picture.as_mut().unwrap();
            picture.side = Side::Right;
            picture.dither = true;
            picture.invert = true;
        }
        app.load_picture(dir.black_picture("second.png", 12));
        let second = app.settings.picture.as_ref().unwrap();
        assert!(second.path.ends_with("second.png"));
        assert_ne!(second.picture, first.picture);
        assert_eq!((second.picture.width(), second.picture.height()), (12, 12));
        assert_eq!(
            (second.side, second.dither, second.invert),
            (Side::Right, true, true)
        );
    }

    #[test]
    fn the_picture_settings_fit_the_smallest_window() {
        let dir = TempDir::new("fits");
        let mut app = test_app();
        // A name that is long, as the names of files can be, and an error of a good length.
        app.load_picture(dir.black_picture(
            "a picture with a really quite long name for a file that holds a small jar.png",
            16,
        ));
        assert_fits(&mut app, "the window with a picture");
        app.settings.picture.as_mut().unwrap().fit_height = false;
        assert_fits(&mut app, "the window with a picture of a chosen height");
        app.picture_error = Some(
            "Could not read the picture C:\\Users\\somebody\\Pictures\\a folder with a long name\\jar.png: \
             the file is not a picture that this program can read"
                .to_string(),
        );
        assert_fits(&mut app, "the window with an error about the picture");
    }

    #[test]
    fn the_picture_command_line_options_load_it_and_set_it() {
        let dir = TempDir::new("options");
        let path = dir.black_picture("jar.png", 8);
        let options = Options {
            picture: Some(path.clone()),
            picture_side: Some(Side::Right),
            picture_dither: true,
            picture_invert: true,
            ..Default::default()
        };
        let app = App::assemble(
            Saved::default(),
            devices::Discovery::default(),
            Fonts::embedded(),
            Printer::start(Unreachable, 1, || {}),
            options,
        );
        let picture = app
            .settings
            .picture
            .as_ref()
            .expect("the picture is loaded");
        assert_eq!(picture.path, path);
        assert_eq!(
            (picture.side, picture.dither, picture.invert),
            (Side::Right, true, true)
        );
    }

    #[test]
    fn the_picture_from_last_time_comes_back_if_its_file_is_still_there() {
        let dir = TempDir::new("remembered");
        let path = dir.black_picture("jar.png", 8);
        let mut settings = LabelSettings::default();
        let mut picture = PictureSettings::new(path.clone(), Picture::open(&path).unwrap());
        picture.side = Side::Right;
        picture.level = 99;
        settings.picture = Some(picture);
        let saved = Saved::new(&settings, None);

        let assemble = |saved: Saved| {
            App::assemble(
                saved,
                devices::Discovery::default(),
                Fonts::embedded(),
                Printer::start(Unreachable, 1, || {}),
                Options::default(),
            )
        };
        let app = assemble(saved.clone());
        let back = app
            .settings
            .picture
            .as_ref()
            .expect("the picture comes back");
        assert_eq!((back.side, back.level), (Side::Right, 99));
        assert!(app.picture_error.is_none());

        // Once the file is gone there is no picture, and the person is told why.
        std::fs::remove_file(&path).unwrap();
        let app = assemble(saved);
        assert!(app.settings.picture.is_none());
        assert!(app.picture_error.is_some());
    }

    // ---- emoji ------------------------------------------------------------------------------

    #[test]
    fn an_emoji_goes_where_the_cursor_is_or_in_place_of_what_is_selected() {
        let put = |text: &str, selected: Option<(usize, usize)>| {
            let mut text = text.to_string();
            let cursor = insert_at_cursor(&mut text, selected, "🍎");
            (text, cursor)
        };
        let result = |text: &str, cursor: usize| (text.to_string(), cursor);
        assert_eq!(put("Hello world", Some((5, 5))), result("Hello🍎 world", 6));
        assert_eq!(put("Hello world", Some((0, 0))), result("🍎Hello world", 1));
        // What was selected makes way for it.
        assert_eq!(put("Hello world", Some((6, 11))), result("Hello 🍎", 7));
        // Without a cursor, at the end.
        assert_eq!(put("Hello", None), result("Hello🍎", 6));
        assert_eq!(put("", None), result("🍎", 1));
        // A cursor that is past the end is at the end.
        assert_eq!(put("Hi", Some((50, 60))), result("Hi🍎", 3));
        // Places are characters, not bytes.
        assert_eq!(put("é🍌x", Some((2, 2))), result("é🍌🍎x", 3));
    }

    #[test]
    fn choosing_an_emoji_puts_it_in_the_text_box_at_its_cursor_and_the_cursor_after_it() {
        let context = egui::Context::default();
        let id = egui::Id::unique("label text");
        let mut app = test_app();
        app.settings.text = "Hello world".to_string();
        let mut state = egui::TextEdit::load_state(&context, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(5),
            )));
        state.store(&context, id);

        app.insert_into_text(&context, id, '🍎');
        assert_eq!(app.settings.text, "Hello🍎 world");
        let range = egui::TextEdit::load_state(&context, id)
            .and_then(|state| state.cursor.char_range())
            .expect("the cursor is kept");
        assert_eq!(
            (range.primary.index.0, range.secondary.index.0),
            (6, 6),
            "the cursor is right after the emoji"
        );
        // So the next one comes after the first.
        app.insert_into_text(&context, id, '🥕');
        assert_eq!(app.settings.text, "Hello🍎🥕 world");

        // A text box that nobody has put the cursor in takes it at the end.
        let mut fresh = test_app();
        fresh.settings.text = "Pantry".to_string();
        fresh.insert_into_text(&egui::Context::default(), id, '🫙');
        assert_eq!(fresh.settings.text, "Pantry🫙");
    }

    /// The emoji of the list that are drawn as buttons in what `lay_out` returned.
    fn emoji_buttons(shown: &[Shown]) -> Vec<char> {
        shown
            .iter()
            .filter_map(|text| {
                let mut characters = text.text.chars();
                let symbol = characters.next()?;
                characters.next().is_none().then_some(symbol)
            })
            .filter(|symbol| {
                emoji::GROUPS
                    .iter()
                    .any(|group| group.emoji().any(|emoji| emoji.symbol == *symbol))
            })
            .collect()
    }

    #[test]
    fn the_emoji_picker_opens_and_offers_emoji_and_the_search_narrows_them_down() {
        let mut app = test_app();
        assert!(
            emoji_buttons(&lay_out(&mut app, WINDOW_SIZE)).is_empty(),
            "the picker is closed to begin with"
        );

        app.emoji.open_now = true;
        let offered = emoji_buttons(&lay_out(&mut app, WINDOW_SIZE));
        assert!(offered.len() > 50, "{} emoji offered", offered.len());
        // (The buttons of the groups show an emoji each too, the apple for food and drink, so
        // the emoji to look for are ones that are not those.)
        assert!(offered.contains(&'🍌'));
        assert!(!offered.contains(&'🐶'), "only the first group is shown");

        // Another group.
        let mut app = test_app();
        app.emoji.open_now = true;
        app.emoji.group = 3;
        let offered = emoji_buttons(&lay_out(&mut app, WINDOW_SIZE));
        assert!(offered.contains(&'🐶') && !offered.contains(&'🍌'));

        // A search looks through all of them, and the buttons of the groups are out of the way.
        let mut app = test_app();
        app.emoji.open_now = true;
        app.emoji.query = "apple".to_string();
        let offered = emoji_buttons(&lay_out(&mut app, WINDOW_SIZE));
        assert!(offered.contains(&'🍎') && offered.contains(&'🍏'));
        assert!(offered.len() < 6, "{offered:?}");

        app.emoji.query = "zzzzzz".to_string();
        app.emoji.open_now = true;
        let shown = lay_out(&mut app, WINDOW_SIZE);
        assert!(emoji_buttons(&shown).is_empty());
        assert!(shown.iter().any(|text| text.text == "Nothing like that."));
    }

    #[test]
    fn the_open_emoji_picker_fits_the_smallest_window() {
        let mut app = test_app();
        app.emoji.open_now = true;
        assert_fits(&mut app, "the window with the emoji picker open");
    }

    #[test]
    fn an_emoji_in_the_text_is_on_the_label() {
        let mut app = test_app();
        app.settings.text = "🍎".to_string();
        lay_out(&mut app, WINDOW_SIZE);
        let Some(Ok(preview)) = &app.preview else {
            panic!("the label should have been drawn");
        };
        let ink = preview
            .rendered
            .canvas()
            .pixels()
            .filter(|pixel| pixel[0] > 127)
            .count();
        assert!(
            ink > 300,
            "an apple is a big shape, but there are {ink} dots"
        );
    }

    // ---- the QR code ---------------------------------------------------------------------------

    /// A screen that is tall enough for the whole of the settings, so that nothing in them is
    /// scrolled out of sight, and as narrow as the window can be.
    const TALL_AND_NARROW: [f32; 2] = [MIN_WINDOW_SIZE[0], 2400.0];

    #[test]
    fn the_qr_section_shows_only_the_choice_until_a_code_is_wanted() {
        let mut app = test_app();
        let shown = lay_out(&mut app, TALL_AND_NARROW);
        assert!(shown.iter().any(|text| text.text == "QR code"));
        assert!(shown.iter().any(|text| text.text == "No QR code"));
        assert!(!shown.iter().any(|text| text.text.contains("Network name")));
        assert!(!shown.iter().any(|text| text.text.contains("Error correction")));
    }

    #[test]
    fn a_wifi_code_has_fields_for_the_network_and_all_of_them_fit_the_narrowest_window() {
        let mut app = test_app();
        app.settings.qr.kind = QrKind::Wifi;
        let shown = lay_out(&mut app, TALL_AND_NARROW);
        for wanted in [
            "Wi-Fi network",
            "Network name:",
            "Password:",
            "Show the password",
            "A hidden network",
            "Side:",
            "Error correction:",
        ] {
            assert!(
                shown.iter().any(|text| text.text == wanted),
                "{wanted} is not there: {:?}",
                shown.iter().map(|text| text.text.as_str()).collect::<Vec<_>>()
            );
        }
        let beyond: Vec<&str> = shown
            .iter()
            .filter(|text| text.right > TALL_AND_NARROW[0] + 0.5)
            .map(|text| text.text.as_str())
            .collect();
        assert!(beyond.is_empty(), "runs off the right edge: {beyond:?}");

        // An open network has no password to type.
        app.settings.qr.wifi.security = WifiSecurity::None;
        let shown = lay_out(&mut app, TALL_AND_NARROW);
        assert!(!shown.iter().any(|text| text.text == "Password:"));
        assert!(shown.iter().any(|text| text.text == "A hidden network"));
    }

    #[test]
    fn a_code_of_a_link_has_a_box_for_it_and_no_wifi_fields() {
        let mut app = test_app();
        app.settings.qr.kind = QrKind::Text;
        let shown = lay_out(&mut app, TALL_AND_NARROW);
        assert!(shown.iter().any(|text| text.text == "Link or text"));
        assert!(shown.iter().any(|text| text.text == "Error correction:"));
        assert!(!shown.iter().any(|text| text.text.contains("Network name")));
    }

    #[test]
    fn what_is_missing_from_the_code_is_said_where_the_preview_would_be_and_nothing_is_printed() {
        let mut app = test_app();
        app.settings.qr.kind = QrKind::Wifi;
        app.settings.qr.wifi.name = "Guest".to_string();
        // No password yet.
        let shown = lay_out(&mut app, WINDOW_SIZE);
        assert!(
            shown
                .iter()
                .any(|text| text.text.contains("Type the password of the Wi-Fi network")),
            "{:?}",
            shown.iter().map(|text| text.text.as_str()).collect::<Vec<_>>()
        );
        assert!(matches!(app.preview, Some(Err(_))), "nothing can be printed yet");

        // With the password the label is drawn, and the message is gone.
        app.settings.qr.wifi.password = "tea-and-biscuits".to_string();
        let shown = lay_out(&mut app, WINDOW_SIZE);
        assert!(!shown
            .iter()
            .any(|text| text.text.contains("Type the password")));
        assert!(matches!(app.preview, Some(Ok(_))));
    }

    #[test]
    fn the_command_line_can_start_the_window_with_a_wifi_code_or_a_link() {
        let discovery = devices::Discovery {
            devices: vec![device("D30", 1)],
            ..Default::default()
        };
        let app = App::assemble(
            Saved::default(),
            discovery,
            Fonts::embedded(),
            Printer::start(Unreachable, 1, || {}),
            Options {
                wifi: Some("Guest".to_string()),
                wifi_password: Some("tea-and-biscuits".to_string()),
                qr_side: Some(Side::Right),
                ..Options::default()
            },
        );
        assert_eq!(app.settings.qr.kind, QrKind::Wifi);
        assert_eq!(app.settings.qr.wifi.name, "Guest");
        assert_eq!(app.settings.qr.wifi.password, "tea-and-biscuits");
        assert_eq!(app.settings.qr.side, Side::Right);

        let app = App::assemble(
            Saved::default(),
            devices::Discovery::default(),
            Fonts::embedded(),
            Printer::start(Unreachable, 1, || {}),
            Options {
                qr: Some("https://example.com".to_string()),
                ..Options::default()
            },
        );
        assert_eq!(app.settings.qr.kind, QrKind::Text);
        assert_eq!(app.settings.qr.text, "https://example.com");
    }
}
