# Development setup

## Prerequisites

- Rust (stable), e.g. `rustup default stable`
- `pkg-config` (Linux) so build scripts can find system libraries

## Linux system dependencies

The build links against X11, GTK 3, WebKitGTK (for the desktop webview), and OpenSSL. Install the dev packages for your distro **before** running `cargo build`.

### Debian / Ubuntu

```bash
sudo apt update
sudo apt install -y \
  pkg-config \
  libx11-dev \
  libxdo-dev \
  libxtst-dev \
  libssl-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libasound2-dev \
  libevdev-dev
```

- **X11 / input** (`libx11-dev`, `libxdo-dev`, `libxtst-dev`, `libevdev-dev`) — used by `rdev`, `device_query`, `tray-icon`, Dioxus/tao stack; `libevdev-dev` for the `evdev` crate (Linux input).
- **OpenSSL** (`libssl-dev`) — used by `reqwest` (TLS).
- **GTK 3** (`libgtk-3-dev`) — used by Dioxus desktop (tao/wry). Pulls in Cairo, Pango, ATK, GdkPixbuf, and GLib dev packages.
- **WebKitGTK 4.1** (`libwebkit2gtk-4.1-dev`) — provides `webkit2gtk-4.1` and `javascriptcoregtk-4.1`; required by wry for the Linux webview.
- **libsoup 3** (`libsoup-3.0-dev`) — HTTP library used by WebKit; required by `soup3-sys`.
- **ALSA** (`libasound2-dev`) — sound API; required by `alsa-sys` (used by rodio/cpal for audio output).

If the build still reports a missing `.pc` file, install the corresponding `-dev` package or rely on the meta-package dependencies.

### Fedora / RHEL

```bash
sudo dnf install -y \
  pkg-config \
  libX11-devel \
  libxdo-devel \
  libXtst-devel \
  openssl-devel \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libsoup3-devel \
  alsa-lib-devel \
  libevdev-devel
```

### Arch Linux

```bash
sudo pacman -S --needed \
  pkg-config \
  libx11 \
  libxdo \
  libxtst \
  openssl \
  gtk3 \
  webkit2gtk-4.1 \
  libsoup3 \
  alsa-lib \
  libevdev
```

## Build and run

```bash
cargo build          # debug
cargo build --release
cargo run
```

## Optional: dependency notes

- **x11** is pulled in by: `device_query`, `rdev`, `tray-icon` (via libxdo/muda), and **dioxus** desktop (via tao’s gdkx11-sys and muda/libxdo).
- **GTK/Cairo/Pango/ATK** are required by the Linux desktop stack (tao → gdk-sys, cairo-sys-rs, etc.).
- **WebKitGTK and libsoup** are required by wry for the in-app webview (webkit2gtk-sys, javascriptcore-rs-sys, soup3-sys).
- **ALSA** is required for audio (rodio/cpal → alsa-sys).
- **libevdev** is required by the `evdev` crate (Linux-only dependency in Cargo.toml).
