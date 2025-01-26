//! Elbey - a bare bones desktop app launcher
#![doc(html_logo_url = "https://github.com/kgilmer/elbey/blob/main/elbey.svg")]
mod app;

use std::process::exit;
use std::sync::LazyLock;

use anyhow::Context;
use app::{Elbey, ElbeyFlags};
use freedesktop_desktop_entry::{
    current_desktop, default_paths, get_languages_from_env, DesktopEntry, Iter,
};
use iced::{Font, Pixels};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use iced_layershell::Application;

static PROGRAM_NAME: LazyLock<String> = std::sync::LazyLock::new(|| String::from("Elbey"));

/// Program entrypoint.  Just configures the app, window, and kicks off the iced runtime.
fn main() -> Result<(), iced_layershell::Error> {
    let iced_settings = Settings {
        layer_settings: LayerShellSettings {
            size: Some((320, 200)),
            exclusive_zone: 200,
            anchor: Anchor::all(),
            start_mode: StartMode::Active,
            layer: Layer::Overlay,
            margin: (0, 0, 0, 0),
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            events_transparent: false,
        },
        flags: ElbeyFlags {
            apps_loader: load_apps,
            app_launcher: launch_app,
        },
        id: Some(PROGRAM_NAME.to_string()),
        fonts: vec![],
        default_font: Font::DEFAULT,
        default_text_size: Pixels::from(18),
        antialiasing: true,
        virtual_keyboard_support: None,
    };

    Elbey::run(iced_settings)
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

    let app_list_iter = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| !entry.no_display());

    // If current desktop is known, filter items that only apply to that desktop
    let mut app_list = if let Some(current_desktop) = current_desktop() {
        app_list_iter
            .filter(|entry| matching_show_in_filter(entry, &current_desktop))
            .filter(|entry| matching_no_show_in_filter(entry, &current_desktop))
            .collect::<Vec<_>>()
    } else {
        app_list_iter.collect::<Vec<_>>()
    };

    // TODO: bubble frequently used apps to the top
    app_list.sort_by(|a, b| a.name(&locales).cmp(&b.name(&locales)));

    app_list
}

// Return true if the entry and current desktop have a matching element, or if no desktop is available or the entry has no desktop spec.  False otherwise.
fn matching_show_in_filter(entry: &DesktopEntry, current_desktop: &[String]) -> bool {
    if let Some(show_in) = entry.only_show_in() {
        for show_in_desktop in show_in {
            for desktop in current_desktop.iter() {
                if show_in_desktop == desktop {
                    return true;
                }
            }
        }
        false
    } else {
        true
    }
}

// Return false if the entry and current desktop have a matching element.  Return true if no desktop is available or the entry has no desktop spec.
fn matching_no_show_in_filter(entry: &DesktopEntry, current_desktop: &[String]) -> bool {
    if let Some(no_show_in) = entry.not_show_in() {
        for show_in_desktop in no_show_in {
            for desktop in current_desktop.iter() {
                if show_in_desktop == desktop {
                    return false;
                }
            }
        }
        true
    } else {
        true
    }
}
