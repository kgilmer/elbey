# `elbey`

<img align="left" src="elbey.svg" alt="logo">

A [desktop app](https://www.freedesktop.org/wiki/Specifications/desktop-entry-spec/) launcher implemented in Rust on [the `iced` UI framework](https://github.com/iced-rs/iced) for Linux.  Upon launch, the program lists all of the locally installed desktop apps with a filter widget set above.  To launch, hit `enter` on the selected item, or use the mouse.

## Status

The application is functional in that desktop apps can be launched from the dialog.

## Tests

There are a [few unit tests](https://github.com/kgilmer/elbey/blob/main/src/app.rs#L234) that exercise some basic functions.

## Documentation

The `rustdoc` is available here: [https://kgilmer.github.io/elbey](https://kgilmer.github.io/elbey)

## Get it

<img align="right" src="screenshot.png" alt="Screenshot">

Elbey can be installed via `cargo install elbey` or from source in this git repository:

### Build

```shell
cargo build --release
```

### Launch/Test

```shell
./target/release/elbey
```

### Install

```shell
cargo install --path .
which elbey # where cargo installed the binary
```

## Credit

This program was inspired by the friendly docs of [`iced`](https://github.com/iced-rs/iced) itself, other desktop app launchers such as [pop-launcher](https://github.com/pop-os/launcher) and [onagre](https://github.com/onagre-launcher/onagre), [`gauntlet`](https://github.com/project-gauntlet/gauntlet/), [`iced_launcher`](https://github.com/Decodetalkers/iced_launcher), and the greater Rust desktop cohort.  Of course the venerable [`rofi`](https://github.com/davatorium/rofi) must also be mentioned.

Project logo was created by Mira Gilmer.
