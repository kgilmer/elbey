use std::env;
use std::process::exit;
use std::time::Instant;

use elbey_cache::{AppDescriptor, Cache};
use freedesktop_desktop_entry::{
    current_desktop, default_paths, get_languages_from_env, DesktopEntry, Iter,
};

fn main() -> anyhow::Result<()> {
    let count = parse_count();
    let mut cache = Cache::new(find_all_apps);

    let read_start = Instant::now();
    let apps = cache.top_apps(count).unwrap_or_default();
    let read_elapsed = read_start.elapsed();

    println!("title\tusage\ticon_path");
    for app in apps {
        let icon_path = app
            .icon_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string());
        println!("{}\t{}\t{}", app.title, app.exec_count, icon_path);
    }
    println!("read_ms\t{}", read_elapsed.as_millis());

    let _ = cache.load_apps();

    let update_start = Instant::now();
    if let Err(err) = cache.refresh() {
        eprintln!("Failed to update cache: {err}");
    }
    let update_elapsed = update_start.elapsed();
    println!("update_ms\t{}", update_elapsed.as_millis());

    Ok(())
}

fn parse_count() -> usize {
    let mut args = env::args().skip(1);
    let count = args
        .next()
        .and_then(|arg| arg.parse::<usize>().ok())
        .unwrap_or_else(|| {
            eprintln!("Usage: cache_inspect <N>");
            exit(2);
        });
    if count == 0 {
        eprintln!("N must be greater than zero");
        exit(2);
    }
    count
}

/// Load DesktopEntry's from `DesktopIter`.
fn find_all_apps() -> Vec<AppDescriptor> {
    let locales = get_languages_from_env();

    let app_list_iter = Iter::new(default_paths())
        .entries(Some(&locales))
        .filter(|entry| !entry.no_display())
        .filter(|entry| entry.desktop_entry("Name").is_some()) // Ignore apps w/out titles
        .filter(|entry| entry.exec().is_some());

    // If current desktop is known, filter items that only apply to that desktop.
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

// Return true if the entry and current desktop have a matching element, or if no desktop is available or the entry has no desktop spec.
// False otherwise.
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

// Return false if the entry and current desktop have a matching element. Return true if no desktop is available or the entry has no desktop spec.
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
