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

* **Printer** (top): pick the printer from the Bluetooth devices your computer has paired (plus any printers named in the command line program's config file), then press **Connect**. The connection stays open, so printing starts at once; **Disconnect** frees the printer for other programs. **Pair…** opens Windows' Bluetooth settings, and **Refresh** looks again for paired printers. On Linux the list of paired devices is not available yet, so type the printer's address under "Other address…".
* **Settings** (left): the text, the font with bold and italic, the size or "as large as fits", alignment, margin, a picture, and the kind of label or tape. For uncut tape you can cut the label to fit the text or give a length.
  * **Lines**: "As typed" breaks the text only where you press Enter. Choosing 1, 2 or 3 makes the program split the text over that many lines for you, with the breaks where the longest line is shortest, so that the text can be as large as possible. A long text on uncut tape gets much shorter that way.
  * **Picture**: press "Choose a picture…" or drop a picture file on the window (PNG, JPEG, GIF, BMP, ICO, TIFF or WebP). It goes beside the text, at the left or the right end of the label, as high as the label allows (or as high as you say), and the text is made as large as fits what is left. Without any text the picture is centered. Choose "Black and white" for drawings, logos and writing (the slider says where gray turns black) or "Photo" for patterns of dots that follow the grays, and "Swap black and white" for a light picture on a dark background. Transparent parts of a picture are white.
* **Preview** (middle): the label exactly as it will print, one square for each dot.
* **Print** (bottom): how many copies, and the print button. Connecting and printing happen in the background, so the window never freezes while the printer is slow to answer.

Your last printer, text, picture and settings are remembered next time.

## Labels, fonts and uncut tape

`print-text` prints on the 12 × 40 mm precut labels unless you say otherwise.

```sh
d30-cli list-labels                                       # the labels and tape the printer can use
d30-cli list-fonts                                        # the fonts you can pick
d30-cli print-text "Hello" --label 14x50                  # another size of precut label
d30-cli print-text 'Kitchen\nPantry' --font Arial --bold --align left
d30-cli print-text "Kitchen pantry flour" --lines 2       # split the text over two lines for you
d30-cli print-text "🍎 Apples"                              # emoji work too
d30-cli print-text "Pantry" --picture jar.png             # a picture beside the text
d30-cli print-text "" --picture photo.jpg --picture-dither   # a picture on its own, as patterns of dots
d30-cli print-text "Hello, tape!" --label 15-continuous   # uncut tape, cut to fit the text
```

* `--label` takes a name from `list-labels`, or any size: `12x35` is a precut label 12 mm wide and 35 mm long, and `13-continuous` is uncut tape 13 mm wide. The D30 prints a band 12 mm wide, so on wider tape (up to 15 mm) the text is centred with a margin on each side.
* `\n` in the text starts a new line, and `--align` lines the lines up left, center or right.
* `--lines 2` (or 3, up to 8) splits the text over that many lines for you, with the breaks where the longest line comes out shortest, which lets the text be as large as possible. Line breaks you typed then count as spaces. A word is never broken, so a text of two words has two lines at the most.
* Emoji can be part of the text. When the font has no glyph for a character, an emoji font draws it, in black and white, which is how a label printer prints it. On Windows that is Segoe UI Emoji, which has all the current emoji; where it is missing, a small font built into the library draws the older ones. Skin tones and the characters that only choose the emoji form are left out, and emoji made of several (families, flags) come out as their parts.
* `--picture FILE` puts a picture (PNG, JPEG, GIF, BMP, ICO, TIFF or WebP) at the left end of the label, or at the right with `--picture-side right`. It is as high as the label allows (less the margins), keeps its shape, and the text gets the room that is left. With an empty text (`""`) the picture is on its own, in the middle. On a label of a set length the pictures together take at most 60% of it, so that the text always has some room; uncut tape that is cut to fit just gets longer.
* Colors become grays, and transparent parts are white, so a logo with a transparent background prints as the logo and not as a black square. Then the grays become dots: by default black where the picture is darker than half (`--picture-level` from 0 to 255 moves that), which suits drawings, logos and writing; `--picture-dither` makes patterns of dots instead, which suits photographs. `--picture-invert` swaps black and white (for a white logo on a dark background), and `--picture-height 6` makes the picture 6 mm high.
* `--font` takes the name of an installed font (capitals do not matter) or the path of a font file, and `--bold` and `--italic` pick that style of the font if it has one. Without `--font` the built-in DejaVu Sans is used.
* `--save-image label.png` also saves the label as a picture, one pixel for each dot, which is exactly what gets printed. Together with `--dry-run` that works without a printer.

**Uncut tape.** The label is cut to fit the text, or as long as you say with `--length 60` (in mm). Two things are different from precut labels:

* The print head sits about 7 mm behind the edge you tear the tape on, so the end of a label is still inside the printer when it has been printed. The program feeds some blank tape after the label (`--feed-lines`, 134 by default) so that you can tear it off with its text in the middle.
* The first rows a printer prints can come out squashed together while the tape gets moving. The program prints some blank tape before the label (`--lead-in-lines`, 30 by default) to take that.

**What has been tried.** Only the 12 × 40 mm precut labels and 15 mm uncut tape have been tried on a real D30. The other sizes come from Phomemo's own listings and from other D30 drivers, so they should work, but `list-labels` marks them as not tried. Tape narrower than 12 mm is not supported, because the print head is 12 mm wide. Labels shaped for fruit are not supported yet.

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
