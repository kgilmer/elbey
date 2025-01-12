# `elbey`

A basic [desktop app](https://www.freedesktop.org/wiki/Specifications/desktop-entry-spec/) launcher implemented in Rust on [the `iced` UI framework](https://github.com/iced-rs/iced).  This project is intentionally simple and "low abstraction" to aid in discovery and experimentation.

## Status

The application is functional in that desktop apps can be launched from the dialog.  There is no caching, modes, persistent memory, custom theming, or other fancy features.  But, you can add them yourself if you want!

## Get it

#### Build

```shell
cargo build --release
```

#### Launch/Test

```shell
./target/release/elbey
```

#### Install

```shell
cargo install --path .
which elbey # where cargo installed the binary
```

## Credit

This program was inspired by the friendly docs of iced itself, other desktop app launchers such as [pop-launcher](https://github.com/pop-os/launcher) and [onagre](https://github.com/onagre-launcher/onagre), and the greater Rust desktop cohort.
