# Building Fleck

Fleck is a Rust workspace: `core/` is the storage and logic, with no GUI
dependencies, and `fleck/` is the app, the note windows and the panel applet in
one binary.

## What you need

A Rust toolchain ([rustup](https://rustup.rs)), [just](https://github.com/casey/just),
and these packages. The names are Pop!_OS and Ubuntu's:

```bash
sudo apt install pkg-config libxkbcommon-dev libwayland-dev libfontconfig-dev libexpat1-dev libfreetype6-dev libpulse-dev cmake libclang-dev
```

`libpulse-dev` is for recording and playing sound; `cmake` and `libclang-dev`
are for whisper-rs, which dictation uses.

## Building and running

```bash
just build-release
just run
```

Use release builds for anything you look at: debug builds of libcosmic are very
slow. `fleck --quit` stops a running copy first, since Fleck is a single
instance that owns a D-Bus name.

The applet is the same binary: `fleck --applet`. Add **Fleck** in COSMIC
Settings → Desktop → Panel → Configure panel applets. After rebuilding, the
panel keeps running the old binary until `systemctl --user restart cosmic-panel`.

## Before you commit

```bash
just check
```

That is what CI runs, literally: `cargo fmt --all --check`, `cargo clippy
--release --workspace --all-targets -D warnings` (pedantic, with opt-outs listed
in the root `Cargo.toml`), `cargo test --release --workspace`, and validation of
the desktop entries and the AppStream metainfo.

The checks run in release so that one compiled copy of the dependency tree
serves both them and the app you run - a debug copy of libcosmic, iced and
whisper is tens of gigabytes on its own. The release profile keeps
`overflow-checks` on so the tests do not quietly lose their arithmetic
guarantees. If a build ever gets away from you, `cargo clean` costs only the
time to compile again.

Two tests need hardware or the network and stay out of `just check`:

```bash
cargo test -- --ignored microphone   # records half a second from the real device
cargo test -- --ignored download     # fetches a small file over TLS
```

## Installing

```bash
just install-user     # ~/.local, no root
sudo just install     # /usr
```

`just model` fetches the speech model (about 142 MB) ahead of time; otherwise
the first press of a note's microphone fetches it.

## Packages

```bash
just deb        # a .deb in target/deb
just apt-repo   # the signed apt repository, as the release workflow builds it
just flatpak    # build and install the Flatpak from the working tree
```

The Flatpak needs `flatpak-builder` and, once, a few GB of runtimes:

```bash
sudo apt install flatpak-builder
flatpak install --user flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08 org.freedesktop.Sdk.Extension.rust-stable//25.08 org.freedesktop.Sdk.Extension.llvm21//25.08 com.system76.Cosmic.BaseApp
```

When `Cargo.lock` changes, the Flatpak's offline crate list has to be
regenerated, which needs the upstream generator:

```bash
python3 -m venv .venv && .venv/bin/pip install aiohttp toml tomlkit
curl -fsSLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
just flatpak-sources
```

One constraint worth knowing: `libcosmic` must stay unpinned. Pinning a `rev`
makes the lock hold two commits of the same repository (libcosmic's `applet`
feature pulls `cosmic-panel-config`, which depends on libcosmic itself), and
Cargo cannot vendor that, so the Flatpak build fails.
