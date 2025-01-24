//! Elbey - a bare bones desktop app launcher
#![doc(html_logo_url = "https://github.com/kgilmer/elbey/blob/main/elbey.svg")]
mod app;

use std::process::exit;
use std::sync::LazyLock;

use anyhow::Context;
use app::{Elbey, ElbeyFlags};
use freedesktop_desktop_entry::{default_paths, get_languages_from_env, DesktopEntry, Iter};
use iced::{Font, Pixels};
use iced::Theme;

static PROGRAM_NAME: LazyLock<String> = std::sync::LazyLock::new(|| String::from("Elbey"));

/// Program entrypoint.  Just configures the app, window, and kicks off the iced runtime.
fn main() -> iced::Result {
    let iced_settings = iced::settings::Settings {
        id: Some(PROGRAM_NAME.to_string()),
        fonts: vec![],
        default_font: Font::DEFAULT,
        default_text_size: Pixels::from(18),
        antialiasing: true,
        exit_on_close_request: true,
        is_daemon: false,
    };

    // A function that returns the app struct
    let app_factory = || {
        Elbey::new(ElbeyFlags {
            apps_loader: load_apps,
            app_launcher: launch_app,
        })
    };

    iced::daemon(PROGRAM_NAME.as_str(), Elbey::update, Elbey::view)
        .settings(iced_settings)
        .theme(|_, _| Theme::Nord)
        .subscription(Elbey::subscription)
        .run_with(app_factory)
}

/// Launch an app described by `entry`.  This implementation exits the process upon successful launch.
fn launch_app(entry: &DesktopEntry) -> anyhow::Result<()> {
    let args = shell_words::split(
        entry
            .exec()
            .context("Failed to read exec from app descriptor")?,
    )?;
    let args = args
        .iter()
        // Filter out special freedesktop syntax
        .filter(|entry| !entry.starts_with('%'))
        .collect::<Vec<&String>>();

    std::process::Command::new(args[0])
        .args(&args[1..])
        .spawn()
        .context("Failed to spawn app")
        .map(|_| ())?;

    exit(0);
}

/// Load DesktopEntry's from `DesktopIter`
fn load_apps() -> Vec<DesktopEntry> {
    let locales = get_languages_from_env();

    Iter::new(default_paths())
        .entries(Some(&locales))
        .collect::<Vec<_>>()
}
