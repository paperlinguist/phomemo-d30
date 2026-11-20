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

**2. Find the printer's Bluetooth address.** In PowerShell (replace `D30` if you renamed the device):

```powershell
(Get-PnpDevice -Class Bluetooth -FriendlyName 'D30').InstanceId -replace '.*DEV_(..)(..)(..)(..)(..)(..).*','$1:$2:$3:$4:$5:$6'
```

**3. Get the program.** Either download `d30-cli-windows-x86_64.zip` from the `nightly` release on this repository's Releases page and unzip it (it is rebuilt automatically whenever a pull request is merged), or build it yourself: install [Rust](https://rustup.rs) (its installer offers to set up the Visual Studio C++ build tools that it needs) and, from a checkout of this repository, run `cargo install --path cli`.

**4. Print a label.**

```powershell
d30-cli -t "Hello" --device AA:BB:CC:DD:EE:FF
```

Put the address in the config file (see below) to leave out `--device`.

The label preview (`--preview`) opens the image in Windows' default image viewer.

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
