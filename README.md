# phomemo-d30

Library & utilities for controlling the [Phomemo D30](https://phomemo.com/products/d30-label-maker) label maker, using a reverse engineered protocol.

This library contains components heavily based on code available in the [polskafan phomemo_d30](https://github.com/polskafan/phomemo_d30) repo,
but takes no code directly from said library. That library in turn is based heavily on the work of others,
including [viver](https://github.com/vivier/phomemo-tools) and [theacodes](https://github.com/theacodes/phomemo_m02s).

The gist of it is that there are several magic sequences sent to the appliance by their 'Print Master' Android app. These were sniffed,
and now can be blindly transmitted by a number of scripts and utilities available on Github. This is one such utility.

# Usage

## CLI usage (imperative)
```sh
cargo install d30-cli

# if using internal preview mechanism ("show_preview")
cargo install d30-cli-preview

# Once installed
d30-cli --help
```


## CLI usage (dev, imperative)

```sh
git clone 'https://github.com/crabdancing/phomemo-d30'
cd phomemo-d30
cargo run --bin d30-cli -- [ARGS]
```

## CLI usage (dev, declarative)

```sh
git clone 'https://github.com/crabdancing/phomemo-d30'
cd phomemo-d30
nix develop
cargo run --bin d30-cli -- [ARGS]
```



## CLI usage (declarative, Nix)

```
nix run github:crabdancing/phomemo-d30
```

This will fetch dependencies, compile, and run the program all in one go.

## CLI usage and the label designer (Linux)

Both programs run on Linux, with BlueZ, the Bluetooth stack that nearly every distribution has. The command line program and the window do the same on Linux as on Windows: everything below "The label designer" and "Labels, fonts and uncut tape" applies.

**1. What it needs.** BlueZ with its service running (`systemctl status bluetooth`), and for the window a desktop with Wayland or X11 and OpenGL or Vulkan drivers (Mesa will do). On Debian and Ubuntu that is `sudo apt install bluez libbluetooth3`. The file dialog of the window ("Choose a picture…") is the one of your desktop, through xdg-desktop-portal; dropping a file on the window works without it.

**2. Get the programs.** Download `d30-linux-x86_64.tar.gz` from the `nightly` release on this repository's Releases page (it is rebuilt automatically whenever a pull request is merged) and unpack it: `tar -xzf d30-linux-x86_64.tar.gz`. It holds `d30-cli`, the command line program, and `d30-gui`, the label designer. It is built on Ubuntu 22.04, so it runs on that and on anything newer (Debian 12, Fedora, Arch, ...). To build them yourself instead: install [Rust](https://rustup.rs) and `libbluetooth-dev` (`bluez-libs-devel` on Fedora), and from a checkout of this repository run `cargo install --path cli` and `cargo install --path gui --locked`.

**3. Pair the printer.** Switch it on and press **Pair…** in the window, which opens the Bluetooth settings of GNOME, KDE or Blueman. Or in a terminal:

```sh
bluetoothctl
[bluetooth]# power on
[bluetooth]# scan on          # wait until the D30 shows up, with its address
[bluetooth]# pair AA:BB:CC:DD:EE:FF
[bluetooth]# trust AA:BB:CC:DD:EE:FF
[bluetooth]# quit
```

`bluetoothctl info AA:BB:CC:DD:EE:FF` should list a "Serial Port" service (UUID 00001101). If it does not, pair again.

**4. Find the address and print.**

```sh
d30-cli list-printers                                    # the paired devices, the D30 pointed out
d30-cli print-text "Hello" --device AA:BB:CC:DD:EE:FF    # a label
d30-cli position-tape --device AA:BB:CC:DD:EE:FF         # for 14x30 labels, see below
./d30-gui                                                # the label designer
```

Put the address in the config file (see below) to leave out `--device`. Only one program can use the printer at a time: close the window before you use the command line program.

**What has been tried.** The Linux programs are built and tested on every change (the library, the command line program and the window's own tests, on Ubuntu), the command line program runs there without a printer, and a picture of the window on a virtual screen is made. They have not been tried against a real printer on Linux; the Bluetooth part there is the one of the original program, which worked on Linux, and the list of paired printers is new. If something does not work, `bluetoothctl info AA:BB:CC:DD:EE:FF` and the message the window shows are the first things to look at.

## CLI usage (Windows)

The CLI runs natively on Windows 10 and 11. It talks to the printer through the Windows Bluetooth stack (an RFCOMM socket via Winsock), so no extra driver, COM port or WSL is needed.

**1. Pair the printer** in *Settings → Bluetooth & devices → Add device → Bluetooth*. Windows then also lists a "Standard Serial over Bluetooth link" port for it under *Device Manager → Ports (COM & LPT)*. That is just a sign that pairing worked; the CLI does not use that port.

**2. Get the programs.** Either download `d30-windows-x86_64.zip` from the `nightly` release on this repository's Releases page and unzip it (it is rebuilt automatically whenever a pull request is merged), or build them yourself: install [Rust](https://rustup.rs) (its installer offers to set up the Visual Studio C++ build tools that it needs) and, from a checkout of this repository, run `cargo install --path cli` for the command line program and `cargo install --path gui --locked` for the label designer. The zip holds both: `d30-gui.exe`, the label designer described below, and `d30-cli.exe`.

**3. Find the printer's Bluetooth address.**

```powershell
d30-cli list-printers
```

This lists the Bluetooth devices paired with this PC, with their addresses, and points out the one that looks like a D30.

**4. Print a label.**

```powershell
d30-cli -t "Hello" --device AA:BB:CC:DD:EE:FF
```

Put the address in the config file (see below) to leave out `--device`.

The label preview (`--preview`) opens the image in Windows' default image viewer.

## The label designer

`d30-gui` is a window for making labels without the command line:

* **Printer** (top): pick the printer from the Bluetooth devices your computer has paired (plus any printers named in the command line program's config file), then press **Connect**. The connection stays open, so printing starts at once; **Disconnect** frees the printer for other programs. **Pair…** opens the Bluetooth settings of Windows (or, on Linux, of your desktop: GNOME, KDE or Blueman), and **Refresh** looks again for paired printers. If the computer cannot say which devices are paired, the window says why, and the printer's address can be typed under "Other address…".
* **Settings** (left): the text, the font with bold and italic, the size or "as large as fits", alignment, margin, a picture, and the kind of label or tape. For uncut tape you can cut the label to fit the text or give a length. For labels with a wide gap (14 × 30 mm) there is a **Position the tape** button, which sets the tape up for them: see "Labels with a wide gap" below.
  * **Emoji**: the "😀 Emoji" button next to "Text" opens a picker with about 1,400 emoji in groups (faces and hearts, people and hands, nature and animals, food and drink, travel and places, activities and music, home and things, office and tools, signs and symbols) and a search box that knows what they are called and other words for them ("clown", "carrot", "home", "heart"). The picker leaves out the few that your computer's fonts cannot draw. Clicking an emoji puts it into the text where the cursor is and leaves the picker open for the next one. Emoji are printed in black and white, with Segoe UI Emoji on Windows, so the label has every current emoji; they are drawn whatever font you chose for the letters.
  * **Lines**: "As typed" breaks the text only where you press Enter. Choosing 1, 2 or 3 makes the program split the text over that many lines for you, with the breaks where the longest line is shortest, so that the text can be as large as possible. A long text on uncut tape gets much shorter that way.
  * **Picture**: press "Choose a picture…" or drop a picture file on the window (PNG, JPEG, GIF, BMP, ICO, TIFF or WebP). It goes beside the text, at the left or the right end of the label, as high as the label allows (or as high as you say), and the text is made as large as fits what is left. Without any text the picture is centered. Choose "Black and white" for drawings, logos and writing (the slider says where gray turns black) or "Photo" for patterns of dots that follow the grays, and "Swap black and white" for a light picture on a dark background. Transparent parts of a picture are white.
  * **QR code**: choose "Wi-Fi network" (its name, the password, how it is secured, and whether it is hidden) or "Link or text". The code goes beside the text, at the left or the right end of the label, as large as the label allows, and a phone that reads a Wi-Fi code offers to join the network. "Error correction" says how much of the code can be lost and still be read: "Low" makes the fewest and so the largest squares, which is what a small label wants. If there is too much for a code on the label, the preview says so. The password is not saved when you close the window.
* **Preview** (middle): the label exactly as it will print, one square for each dot.
* **Print** (bottom): how many copies, and the print button. Connecting and printing happen in the background, so the window never freezes while the printer is slow to answer.

Your last printer, text, picture and settings are remembered next time.

## Labels, fonts and uncut tape

`print-text` prints on the 12 × 40 mm precut labels unless you say otherwise.

```sh
d30-cli list-labels                                       # the labels and tape the printer can use
d30-cli position-tape                                     # after a new roll of 14x30 labels: feed to the cut line
d30-cli list-fonts                                        # the fonts you can pick
d30-cli print-text "Hello" --label 14x50                  # another size of precut label
d30-cli print-text 'Kitchen\nPantry' --font Arial --bold --align left
d30-cli print-text "Kitchen pantry flour" --lines 2       # split the text over two lines for you
d30-cli print-text "🍎 Apples"                              # emoji work too
d30-cli print-text "Pantry" --picture jar.png             # a picture beside the text
d30-cli print-text "" --picture photo.jpg --picture-dither   # a picture on its own, as patterns of dots
d30-cli print-text "Guest Wi-Fi" --wifi Guest --wifi-password tea-and-biscuits   # a QR code that joins the network
d30-cli print-text "Menu" --qr https://example.com/menu --qr-side right          # a QR code of a link
d30-cli print-text "Hello, tape!" --label 15-continuous   # uncut tape, cut to fit the text
```

* `--label` takes a name from `list-labels`, or any size: `12x35` is a precut label 12 mm wide and 35 mm long, and `13-continuous` is uncut tape 13 mm wide. The D30 prints a band 12 mm wide, so on wider tape (up to 15 mm) the text is centred with a margin on each side.
* `\n` in the text starts a new line, and `--align` lines the lines up left, center or right.
* `--lines 2` (or 3, up to 8) splits the text over that many lines for you, with the breaks where the longest line comes out shortest, which lets the text be as large as possible. Line breaks you typed then count as spaces. A word is never broken, so a text of two words has two lines at the most.
* Emoji can be part of the text. When the font has no glyph for a character, an emoji font draws it, in black and white, which is how a label printer prints it. On Windows that is Segoe UI Emoji, which has all the current emoji; where it is missing (on Linux, for instance), a font built into the library draws nearly all of them (the color emoji fonts of Linux cannot be used: they are pictures, not outlines). Skin tones and the characters that only choose the emoji form are left out, and emoji made of several (families, flags) come out as their parts.
* `--picture FILE` puts a picture (PNG, JPEG, GIF, BMP, ICO, TIFF or WebP) at the left end of the label, or at the right with `--picture-side right`. It is as high as the label allows (less the margins), keeps its shape, and the text gets the room that is left. With an empty text (`""`) the picture is on its own, in the middle. On a label of a set length the pictures together take at most 60% of it, so that the text always has some room; uncut tape that is cut to fit just gets longer.
* Colors become grays, and transparent parts are white, so a logo with a transparent background prints as the logo and not as a black square. Then the grays become dots: by default black where the picture is darker than half (`--picture-level` from 0 to 255 moves that), which suits drawings, logos and writing; `--picture-dither` makes patterns of dots instead, which suits photographs. `--picture-invert` swaps black and white (for a white logo on a dark background), and `--picture-height 6` makes the picture 6 mm high.
* `--qr TEXT` puts a QR code of a link or some text beside the text, at the left end of the label or (`--qr-side right`) at the right end. `--wifi NAME` makes one for a Wi-Fi network instead, with `--wifi-password`, `--wifi-security wpa|wep|none` (`wpa` is also for WPA2 and WPA3) and `--wifi-hidden`; a phone that reads it offers to join the network. The code is as large as the label allows (on 12 mm tape a typical Wi-Fi code has squares of 3 dots, 0.4 mm) with every square a whole number of dots, and is never made smaller to make room: if there is too much for a code on the label you get an error saying so. `--qr-level low|medium|quartile|high` is how much of the code can be lost and still be read. `low`, the default, has the fewest squares. A password on the command line stays in the history of the shell, which the window does not do.
* `--font` takes the name of an installed font (capitals do not matter) or the path of a font file, and `--bold` and `--italic` pick that style of the font if it has one. Without `--font` the built-in DejaVu Sans is used.
* `--save-image label.png` also saves the label as a picture, one pixel for each dot, which is exactly what gets printed. Together with `--dry-run` that works without a printer.

**Uncut tape.** The label is cut to fit the text, or as long as you say with `--length 60` (in mm). Two things are different from precut labels:

* The print head sits about 7 mm behind the edge you tear the tape on, so the end of a label is still inside the printer when it has been printed. The program feeds some blank tape after the label (`--feed-lines`, 134 by default) so that you can tear it off with its text in the middle.
* The first rows a printer prints can come out squashed together while the tape gets moving. The program prints some blank tape before the label (`--lead-in-lines`, 30 by default) to take that.

**Labels with a wide gap.** The 12 × 40 mm labels come out of the printer far enough to cut or peel by themselves, and the printer finds them on its own. The 14 × 30 mm labels have a gap of 8 mm between them, with a cut line printed in the middle of it, and the printer cannot print the whole of a label like that by itself: it starts printing in the gap before the label, and stops a few millimeters before the end of the label. So these labels are printed as if they were uncut tape, and the program keeps count of the tape itself:

* Every print is exactly one label plus its gap long (38.2 mm: the labels are 30.2 mm and the gap 8 mm), and ends with the cut line at the edge of the printer. Tear each label off there. The next label then starts printing where this one ended.
* The print head is 7 mm behind the edge and the cut line is only 4 mm from the label, so after a tear on the cut line the head is already 3 mm into the next label. The first 3 mm of each label cannot be printed, so text and pictures are kept at least 3 mm from both ends, which also keeps them off the rounded corners. On a 30 mm label that leaves about 24 mm in the middle.
* This only works while the tape is left alone: tear at the edge, do not pull the tape out, and do not press the printer's button. After a new roll, or if the tape has been moved, run `d30-cli position-tape` (or press **Position the tape** in the window). It feeds a blank label, so that the printer finds the gap by itself, and then moves the tape on until the cut line is at the edge. Tear off what came out there, and you are ready. It takes about 20 seconds.
* `--feed-lines` (and "After the label" in the window) is 88 for these labels: the gap, and the 3 mm that are not printed. Do not change it unless you want the strip to stop somewhere else, and then the next label will not start where it should either.

**What has been tried.** Only the 12 × 40 mm precut labels, the 14 × 30 mm labels (counted as above) and 15 mm uncut tape have been tried on a real D30. The other sizes come from Phomemo's own listings and from other D30 drivers, so they should work, but `list-labels` marks them as not tried. Tape narrower than 12 mm is not supported, because the print head is 12 mm wide. Labels shaped for fruit are not supported yet.

## Configuration (imperative)

The program will work without any on-disk config, but if you want to use some of the more sophisticated features, here's a brief explanation of the current state of affairs.

There are two config files. On Linux, they are stored under `$XDG_CONFIG_HOME/phomemo-library`. On most systems, that will be `~/.config/phomemo-library` -- so I'll be using that path in this mini-guide to make this more concrete. On Windows, they are stored under `%APPDATA%\phomemo-library`, which is usually `C:\Users\<you>\AppData\Roaming\phomemo-library`.

Under this directory, it expects two config files. One is for the library itself:

`~/.config/phomemo-library/phomemo-config.toml`


The next file is for the CLI component:

`~/.config/phomemo-library/phomemo-cli-config.toml`

Check [example-config](https://github.com/crabdancing/phomemo-d30/tree/master/example-config) directory for working example files.

## Configuration, declarative (via NixOS & system flake)

Add to your flake inputs:

```nix
inputs = {
  # Somewhere in your `inputs` context, put a `phomemo-d30` input:
  phomemo-d30.url = "github:crabdancing/phomemo-d30";
};
```

Insert module into `sharedModules`:


```nix
{pkgs, ...}: {
  config = {
    home-manager.sharedModules = [
      inputs.phomemo-d30.homeManagerModules.default
    ];
  };
}
```

In home-manager context, you can then configure via:

```nix
programs.phomemo-d30 = {
  enable = true;
  default_device = "kitchen";
  resolution = {
    my_desk = "40:5B:A4:2F:05:46";
    kitchen = "DB:1E:B4:E7:A3:75";
  };
};

```

This can be configured per-user.
