//! Elbey - a desktop app launcher
#![doc(html_logo_url = "https://github.com/kgilmer/elbey/blob/main/elbey.svg")]
mod app;
mod cache;
mod values;

use std::process::exit;
use std::sync::{Arc, Mutex};

use crate::values::*;
use anyhow::Context;
use app::{AppDescriptor, Elbey, ElbeyFlags};
use argh::FromArgs;
use cache::Cache;
use freedesktop_desktop_entry::{
    current_desktop, default_paths, get_languages_from_env, DesktopEntry, Iter,
};
use iced::theme::{Custom, Palette};
use iced::{Color, Font, Pixels, Theme};
use iced_layershell::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings, StartMode};
use lazy_static::lazy_static;

lazy_static! {
    static ref CACHE: Arc<Mutex<Cache>> = Arc::new(Mutex::new(Cache::new(find_all_apps)));
}

#[derive(FromArgs)]
/// Desktop app launcher
struct EbleyArgs {
    /// height
    #[argh(option)]
    height: Option<u32>,

    /// width
    #[argh(option)]
    width: Option<u32>,

    /// theme name: CatppuccinFrappe,CatppuccinLatte,CatppuccinMacchiato,CatppuccinMocha,Dark,Dracula,Ferra,GruvboxDark,GruvboxLight,KanagawaDragon,KanagawaLotus,KanagawaWave,Light,Moonfly,Nightfly,Nord,Oxocarbon,TokyoNight,TokyoNightLight,TokyoNightStorm,AyuMirage
    #[argh(option)]
    theme: Option<String>,

    /// font size
    #[argh(option)]
    font_size: Option<u16>,

    /// icon size
    #[argh(option)]
    icon_size: Option<u16>,

    /// stylesheet (unsupported)
    #[argh(option, short = 't')]
    _style_sheet: Option<String>,
}

fn parse_theme(name: &str) -> Option<Theme> {
    match name {
        "CatppuccinFrappe" => Some(Theme::CatppuccinFrappe),
        "CatppuccinLatte" => Some(Theme::CatppuccinLatte),
        "CatppuccinMacchiato" => Some(Theme::CatppuccinMacchiato),
        "CatppuccinMocha" => Some(Theme::CatppuccinMocha),
        "Dark" => Some(Theme::Dark),
        "Dracula" => Some(Theme::Dracula),
        "Ferra" => Some(Theme::Ferra),
        "GruvboxDark" => Some(Theme::GruvboxDark),
        "GruvboxLight" => Some(Theme::GruvboxLight),
        "KanagawaDragon" => Some(Theme::KanagawaDragon),
        "KanagawaLotus" => Some(Theme::KanagawaLotus),
        "KanagawaWave" => Some(Theme::KanagawaWave),
        "Light" => Some(Theme::Light),
        "Moonfly" => Some(Theme::Moonfly),
        "Nightfly" => Some(Theme::Nightfly),
        "Nord" => Some(Theme::Nord),
        "Oxocarbon" => Some(Theme::Oxocarbon),
        "TokyoNight" => Some(Theme::TokyoNight),
        "TokyoNightLight" => Some(Theme::TokyoNightLight),
        "TokyoNightStorm" => Some(Theme::TokyoNightStorm),
        "AyuMirage" => Some(Theme::Custom(Arc::new(Custom::new(
            "AyuMirage".to_string(),
            Palette {
                background: Color::from_rgb8(0x1F, 0x24, 0x30),
                text: Color::from_rgb8(0x63, 0x75, 0x99),
                primary: Color::from_rgb8(0x17, 0x1B, 0x24),
                success: Color::from_rgb8(0xD5, 0xFF, 0x80),
                warning: Color::from_rgb8(0xFF, 0xC1, 0x4E),
                danger: Color::from_rgb8(0x12, 0x15, 0x1C),
            },
        )))),
        _ => None,
    }
}

/// Program entrypoint.  Just configures the app, window, and kicks off the iced runtime.
fn main() -> Result<(), iced_layershell::Error> {
    let args: EbleyArgs = argh::from_env();

    let flags = ElbeyFlags {
        apps_loader: load_apps,
        app_launcher: launch_app,
        theme: if args.theme.is_some() {
            if let Some(theme) = parse_theme(&args.theme.unwrap()) {
                theme
            } else {
                DEFAULT_THEME
            }
        } else {
            DEFAULT_THEME
        },
        window_size: (
            args.width
                .unwrap_or(DEFAULT_WINDOW_WIDTH)
                .try_into()
                .unwrap(),
            args.height
                .unwrap_or(DEFAULT_WINDOW_HEIGHT)
                .try_into()
                .unwrap(),
        ),
        icon_size: args.icon_size.unwrap_or(DEFAULT_ICON_SIZE),
    };

    let iced_settings = Settings {
        layer_settings: LayerShellSettings {
            size: Some((
                args.width.unwrap_or(DEFAULT_WINDOW_WIDTH),
                args.height.unwrap_or(DEFAULT_WINDOW_HEIGHT),
            )),
            exclusive_zone: DEFAULT_WINDOW_HEIGHT as i32,
            anchor: Anchor::all(),
            start_mode: StartMode::Active,
            layer: Layer::Overlay,
            margin: (0, 0, 0, 0),
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            events_transparent: false,
        },
        id: Some(PROGRAM_NAME.to_string()),
        fonts: vec![],
        default_font: Font::DEFAULT,
        default_text_size: Pixels::from(u32::from(args.font_size.unwrap_or(DEFAULT_TEXT_SIZE))),
        antialiasing: true,
        ..Settings::default()
    };

    let flags_for_boot = flags.clone();

    application(
        move || Elbey::new(flags_for_boot.clone()),
        Elbey::namespace,
        Elbey::update,
        Elbey::view,
    )
    .subscription(Elbey::subscription)
    .theme(|state: &Elbey| Some(state.theme()))
    .settings(iced_settings)
    .run()
}

/// Launch an app described by `entry`.  This implementation exits the process upon successful launch.
fn launch_app(entry: &AppDescriptor) -> anyhow::Result<()> {
    let args = shell_words::split(entry.exec.as_str())?;
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

    if let Ok(cache) = CACHE.lock().as_mut() {
        cache.update(entry)?;
    } else {
        eprint!("Failed to acquire cache");
    }

    exit(0);
}

fn load_apps() -> Vec<AppDescriptor> {
    let cache = CACHE.lock().expect("Failed to acquire cache");

    if cache.is_empty() {
        // No cache available, probably first launch of current version.  Traverse FS looking for apps.
        find_all_apps()
    } else {
        cache.read_all().unwrap_or(find_all_apps())
    }
}

/// Load DesktopEntry's from `DesktopIter`
fn find_all_apps() -> Vec<AppDescriptor> {
    let locales = get_languages_from_env();

    let app_list_iter = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| !entry.no_display())
        .filter(|entry| entry.desktop_entry("Name").is_some()) // Ignore apps w/out titles
        .filter(|entry| entry.exec().is_some());

    // If current desktop is known, filter items that only apply to that desktop
    let mut app_list = if let Some(current_desktop) = current_desktop() {
        app_list_iter
            .filter(|entry| matching_show_in_filter(entry, &current_desktop))
            .filter(|entry| matching_no_show_in_filter(entry, &current_desktop))
            .map(AppDescriptor::from)
            .collect::<Vec<_>>()
    } else {
        app_list_iter.map(AppDescriptor::from).collect::<Vec<_>>()
    };

    app_list.sort_by(|a, b| a.title.cmp(&b.title));

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
