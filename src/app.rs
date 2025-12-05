//! Functions and other types for `iced` UI to view, filter, and launch apps
use std::cmp::{max, min};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;

use freedesktop_desktop_entry::DesktopEntry;
use freedesktop_icons::lookup;
use iced::keyboard::key::Named;
use iced::keyboard::Key;
use iced::widget::button::{primary, text as text_style};
use iced::widget::image::Handle as ImageHandle;
use iced::widget::svg::Handle as SvgHandle;
use iced::widget::{button, column, image, row, scrollable, svg, text, text_input, Column};
use iced::{event, window, Alignment, Element, Event, Length, Task, Theme};
use iced_layershell::{to_layer_message, Application};
use serde::{Deserialize, Serialize};

use crate::values::*;
use crate::PROGRAM_NAME;

fn default_icon_handle() -> IconHandle {
    FALLBACK_ICON_HANDLE.clone()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppDescriptor {
    pub appid: String,
    pub title: String,
    #[serde(default)]
    pub lower_title: String,
    pub exec: String,
    pub exec_count: usize,
    pub icon_name: Option<String>,
    #[serde(skip, default = "default_icon_handle")]
    pub icon_handle: IconHandle,
}

impl From<DesktopEntry> for AppDescriptor {
    fn from(value: DesktopEntry) -> Self {
        AppDescriptor {
            appid: value.appid.clone(),
            title: value.desktop_entry("Name").expect("get name").to_string(),
            lower_title: value
                .desktop_entry("Name")
                .expect("get name")
                .to_lowercase(),
            exec: value.exec().expect("has exec").to_string(),
            exec_count: 0,
            icon_name: value.icon().map(str::to_string),
            icon_handle: default_icon_handle(),
        }
    }
}

/// The application model type.  See [the iced book](https://book.iced.rs/) for details.
#[derive(Debug)]
pub struct State {
    /// A text entry box where a user can enter list filter criteria
    entry: String,
    /// Lowercased entry text to avoid repeated allocations during filtering
    entry_lower: String,
    /// The complete list of DesktopEntry, as retrieved by lib
    apps: Vec<AppDescriptor>,
    /// The index of the item visibly selected in the UI
    selected_index: usize,
    /// A flag to indicate app window has received focus. Work around to some windowing environments passing `unfocused` unexpectedly.
    received_focus: bool,
    /// Cache of icon handles keyed by icon name to avoid repeated theme lookups
    icon_cache: HashMap<String, IconHandle>,
}

/// Root struct of application
#[derive(Debug)]
pub struct Elbey {
    state: State,
    flags: ElbeyFlags,
}

/// Number of icons to prefetch beyond the current viewport to avoid UI jank when scrolling.
const PREFETCH_ICON_COUNT: usize = VIEWABLE_LIST_ITEM_COUNT;

/// Messages are how your logic mutates the app state and GUI
#[to_layer_message]
#[derive(Debug, Clone)]
pub enum ElbeyMessage {
    /// Signals that the `DesktopEntries` have been fully loaded into the vec
    ModelLoaded(Vec<AppDescriptor>),
    /// Signals that an icon path has been found for an app
    IconLoaded(usize, Option<PathBuf>),
    /// Signals that the primary text edit box on the UI has been changed by the user, including the new text.
    EntryUpdate(String),
    /// Signals that the user has taken primary action on a selection.  In the case of a desktop app launcher, the app is launched.
    ExecuteSelected(),
    /// Signals that the user has pressed a key
    KeyEvent(Key),
    /// Signals that the window has gained focus
    GainedFocus,
    /// Signals that the window has lost focus
    LostFocus,
}

/// Provide some initial configuration to app to facilitate testing
#[derive(Debug, Clone)]
pub struct ElbeyFlags {
    /**
     * A function that returns a list of `DesktopEntry`s
     */
    pub apps_loader: fn() -> Vec<AppDescriptor>,
    /**
     * A function that launches a process from a `DesktopEntry`
     */
    pub app_launcher: fn(&AppDescriptor) -> anyhow::Result<()>, //TODO ~ return a task that exits app

    pub theme: Theme,

    pub window_size: (u16, u16),

    pub icon_size: u16,
}

impl Application for Elbey {
    type Message = ElbeyMessage;
    type Flags = ElbeyFlags;
    type Theme = Theme;
    type Executor = iced::executor::Default;

    /// Initialize the app.  Only notable item here is probably the return type Task<ElbeyMessage> and what we pass
    /// back.  Here, within the async execution, we directly call the library to retrieve `DesktopEntry`'s which
    /// are the primary model of the [XDG Desktop Specification](https://www.freedesktop.org/wiki/Specifications/desktop-entry-spec/).
    /// Then we create and pass a layer shell as another task.
    fn new(flags: ElbeyFlags) -> (Self, Task<ElbeyMessage>) {
        // A task to load the app model
        let apps_loader = flags.apps_loader;
        let load_task = Task::perform(async move { (apps_loader)() }, ElbeyMessage::ModelLoaded);

        (
            Self {
                state: State {
                    entry: String::new(),
                    entry_lower: String::new(),
                    apps: vec![],
                    selected_index: 0,
                    received_focus: false,
                    icon_cache: HashMap::new(),
                },
                flags,
            },
            load_task,
        )
    }

    fn namespace(&self) -> String {
        PROGRAM_NAME.to_string()
    }

    /// Entry-point from `iced`` into app to construct UI
    fn view(&self) -> Element<'_, ElbeyMessage> {
        // Create the list UI elements based on the `DesktopEntry` model
        let app_elements: Vec<Element<ElbeyMessage>> = self
            .state
            .apps
            .iter()
            .filter(|e| Self::text_entry_filter(e, &self.state)) // Only show entries that match filter
            .enumerate()
            .filter(|(index, _)| {
                (self.state.selected_index..self.state.selected_index + VIEWABLE_LIST_ITEM_COUNT)
                    .contains(index)
            }) // Only show entries in selection range
            .map(|(index, entry)| {
                let name = entry.title.as_str();
                let selected = self.state.selected_index == index;
                let icon_handle_to_render = match &entry.icon_handle {
                    IconHandle::Loading => default_icon_handle(),
                    other => other.clone(),
                };
                let icon: Element<'_, ElbeyMessage> = match icon_handle_to_render {
                    IconHandle::Raster(handle) => image(handle)
                        .width(Length::Fixed(self.flags.icon_size.into()))
                        .height(Length::Fixed(self.flags.icon_size.into()))
                        .into(),
                    IconHandle::Vector(handle) => svg(handle)
                        .width(Length::Fixed(self.flags.icon_size.into()))
                        .height(Length::Fixed(self.flags.icon_size.into()))
                        .into(),
                    IconHandle::Loading => unreachable!(),
                };
                let content = row![icon, text(name)]
                    .spacing(10)
                    .align_y(Alignment::Center);

                button(content)
                    .style(if selected { primary } else { text_style })
                    .width(Length::Fill)
                    .on_press(ElbeyMessage::ExecuteSelected())
                    .into()
            })
            .collect();

        // Bare bones!
        // TODO: Fancier layout?
        column![
            text_input("drun", &self.state.entry)
                .id(ENTRY_WIDGET_ID.clone())
                .on_input(ElbeyMessage::EntryUpdate)
                .width(self.flags.window_size.0),
            scrollable(Column::with_children(app_elements))
                .width(self.flags.window_size.0)
                .id(ITEMS_WIDGET_ID.clone()),
        ]
        .into()
    }

    /// Entry-point from `iced` to handle user and system events
    fn update(&mut self, message: ElbeyMessage) -> Task<ElbeyMessage> {
        match message {
            // The model has been loaded, initialize the UI
            ElbeyMessage::ModelLoaded(items) => {
                self.state.apps = items;
                self.state.entry_lower = self.state.entry.to_lowercase();
                let focus_task = text_input::focus(ENTRY_WIDGET_ID.clone());
                let load_icons_task = self.load_visible_icons();
                Task::batch(vec![focus_task, load_icons_task])
            }
            // Rebuild the select list based on the updated text entry
            ElbeyMessage::EntryUpdate(entry_text) => {
                self.state.entry = entry_text;
                self.state.entry_lower = self.state.entry.to_lowercase();
                self.state.selected_index = 0;
                self.load_visible_icons()
            }
            // Launch an application selected by the user
            ElbeyMessage::ExecuteSelected() => {
                if let Some(entry) = self.selected_entry() {
                    (self.flags.app_launcher)(entry).expect("Failed to launch app");
                }
                Task::none()
            }
            ElbeyMessage::IconLoaded(index, path) => {
                if let Some(app) = self.state.apps.get_mut(index) {
                    if let Some(p) = path {
                        let handle = if p.extension().and_then(|s| s.to_str()) == Some("svg") {
                            IconHandle::Vector(SvgHandle::from_path(p))
                        } else {
                            IconHandle::Raster(ImageHandle::from_path(p))
                        };
                        if let Some(icon_name) = app.icon_name.clone() {
                            self.state.icon_cache.insert(icon_name, handle.clone());
                        }
                        app.icon_handle = handle;
                    } else {
                        let fallback = default_icon_handle();
                        if let Some(icon_name) = app.icon_name.clone() {
                            self.state.icon_cache.insert(icon_name, fallback.clone());
                        }
                        app.icon_handle = fallback;
                        app.icon_name = None;
                    }
                }
                Task::none()
            }
            // Handle keyboard entries
            ElbeyMessage::KeyEvent(key) => match key {
                Key::Named(Named::Escape) => exit(0),
                Key::Named(Named::ArrowUp) => {
                    self.navigate_items(-1);
                    self.load_visible_icons()
                }
                Key::Named(Named::ArrowDown) => {
                    self.navigate_items(1);
                    self.load_visible_icons()
                }
                Key::Named(Named::PageUp) => {
                    self.navigate_items(-(VIEWABLE_LIST_ITEM_COUNT as i32));
                    self.load_visible_icons()
                }
                Key::Named(Named::PageDown) => {
                    self.navigate_items(VIEWABLE_LIST_ITEM_COUNT as i32);
                    self.load_visible_icons()
                }
                Key::Named(Named::Enter) => {
                    if let Some(entry) = self.selected_entry() {
                        (self.flags.app_launcher)(entry).expect("Failed to launch app");
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
            // Handle window events
            ElbeyMessage::GainedFocus => {
                self.state.received_focus = true;
                text_input::focus(ENTRY_WIDGET_ID.clone())
            }
            ElbeyMessage::LostFocus => {
                if self.state.received_focus {
                    exit(0);
                }
                Task::none()
            }
            ElbeyMessage::AnchorChange(anchor) => {
                dbg!(anchor);
                Task::none()
            }
            ElbeyMessage::SetInputRegion(_action_callback) => Task::none(),
            ElbeyMessage::AnchorSizeChange(anchor, _) => {
                dbg!(anchor);
                Task::none()
            }
            ElbeyMessage::LayerChange(layer) => {
                dbg!(layer);
                Task::none()
            }
            ElbeyMessage::MarginChange(mc) => {
                dbg!(mc);
                Task::none()
            }
            ElbeyMessage::SizeChange(sc) => {
                dbg!(sc);
                Task::none()
            }
            ElbeyMessage::VirtualKeyboardPressed { time, key } => {
                dbg!(time, key);
                Task::none()
            }
        }
    }

    /// The `iced` entry-point to setup event listeners
    fn subscription(&self) -> iced::Subscription<ElbeyMessage> {
        // Framework code to integrate with underlying user interface devices; keyboard, mouse.
        event::listen_with(|event, _status, _| match event {
            Event::Window(window::Event::Focused) => Some(ElbeyMessage::GainedFocus),
            Event::Window(window::Event::Unfocused) => Some(ElbeyMessage::LostFocus),
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                modifiers: _,
                text: _,
                key,
                location: _,
                modified_key: _,
                physical_key: _,
            }) => Some(ElbeyMessage::KeyEvent(key)),
            _ => None,
        })
    }

    fn theme(&self) -> Self::Theme {
        self.flags.theme.clone()
    }
}

impl Elbey {
    // Return ref to the selected item from the app list after applying filter
    fn selected_entry(&self) -> Option<&AppDescriptor> {
        self.state
            .apps
            .iter()
            .filter(|e| Self::text_entry_filter(e, &self.state))
            .nth(self.state.selected_index)
    }

    fn navigate_items(&mut self, delta: i32) {
        if delta < 0 {
            self.state.selected_index = max(0, self.state.selected_index as i32 + delta) as usize;
        } else {
            self.state.selected_index = min(
                self.state.apps.len() as i32 - 1,
                self.state.selected_index as i32 + delta,
            ) as usize;
        }
    }

    // Compute the items in the list to display based on the model
    fn text_entry_filter(entry: &AppDescriptor, model: &State) -> bool {
        entry.lower_title.contains(&model.entry_lower)
    }

    fn queue_icon_load(
        &mut self,
        original_index: usize,
        icon_size: u16,
        tasks: &mut Vec<Task<ElbeyMessage>>,
    ) {
        if let Some(app) = self.state.apps.get_mut(original_index) {
            if let Some(icon_name) = app.icon_name.clone() {
                if let Some(cached) = self.state.icon_cache.get(&icon_name) {
                    app.icon_handle = cached.clone();
                    return;
                }
                if app.icon_handle == IconHandle::Loading {
                    return;
                }
                if app.icon_handle == default_icon_handle() {
                    app.icon_handle = IconHandle::Loading;
                    tasks.push(Task::perform(
                        async move { lookup(&icon_name).with_size(icon_size).find() },
                        move |path| ElbeyMessage::IconLoaded(original_index, path),
                    ));
                }
            }
        }
    }

    fn load_visible_icons(&mut self) -> Task<ElbeyMessage> {
        let filtered_app_indices: Vec<usize> = self
            .state
            .apps
            .iter()
            .enumerate()
            .filter(|(_, e)| Self::text_entry_filter(e, &self.state))
            .map(|(i, _)| i)
            .collect();

        let view_start = self.state.selected_index;
        let view_end =
            (self.state.selected_index + VIEWABLE_LIST_ITEM_COUNT).min(filtered_app_indices.len());

        let icon_size = self.flags.icon_size;

        let mut tasks = vec![];

        if let Some(visible_indices) = filtered_app_indices.get(view_start..view_end) {
            for &original_index in visible_indices {
                self.queue_icon_load(original_index, icon_size, &mut tasks);
            }
        }

        let prefetch_end = (view_end + PREFETCH_ICON_COUNT).min(filtered_app_indices.len());
        if let Some(prefetch_indices) = filtered_app_indices.get(view_end..prefetch_end) {
            for &original_index in prefetch_indices {
                self.queue_icon_load(original_index, icon_size, &mut tasks);
            }
        }
        Task::batch(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;
    use std::time::Instant;

    static EMPTY_LOADER: fn() -> Vec<AppDescriptor> = || vec![];

    static TEST_DESKTOP_ENTRY_1: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_1".to_string(),
        title: "t1".to_string(),
        lower_title: "t1".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: default_icon_handle(),
    });

    static TEST_DESKTOP_ENTRY_2: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_2".to_string(),
        title: "t2".to_string(),
        lower_title: "t2".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: default_icon_handle(),
    });

    static TEST_DESKTOP_ENTRY_3: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_3".to_string(),
        title: "t2".to_string(),
        lower_title: "t2".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: default_icon_handle(),
    });

    static TEST_ENTRY_LOADER: fn() -> Vec<AppDescriptor> = || {
        vec![
            TEST_DESKTOP_ENTRY_1.clone(),
            TEST_DESKTOP_ENTRY_2.clone(),
            TEST_DESKTOP_ENTRY_3.clone(),
        ]
    };

    #[test]
    fn test_default_app_launch() {
        let test_launcher: fn(&AppDescriptor) -> anyhow::Result<()> = |e| {
            assert!(e.appid == "test_app_id_1");
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_no_apps_try_launch() {
        let test_launcher: fn(&AppDescriptor) -> anyhow::Result<()> = |_e| {
            assert!(false); // should never get here
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(EMPTY_LOADER()));
        let _result = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_app_navigation() {
        let test_launcher: fn(&AppDescriptor) -> anyhow::Result<()> = |e| {
            assert!(e.appid == "test_app_id_2");
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowUp)));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_icon_loaded_png() {
        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: |_| Ok(()),
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });
        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));

        let png_path = PathBuf::from("test.png");
        let _ = unit.update(ElbeyMessage::IconLoaded(0, Some(png_path)));

        assert!(matches!(
            unit.state.apps[0].icon_handle,
            IconHandle::Raster(_)
        ));
    }

    #[test]
    fn test_icon_loaded_svg() {
        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: |_| Ok(()),
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });
        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));

        let svg_path = PathBuf::from("test.svg");
        let _ = unit.update(ElbeyMessage::IconLoaded(0, Some(svg_path)));

        assert!(matches!(
            unit.state.apps[0].icon_handle,
            IconHandle::Vector(_)
        ));
    }

    #[test]
    fn test_icon_loaded_fallback() {
        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: |_| Ok(()),
            theme: Theme::default(),
            window_size: (0, 0),
            icon_size: 48,
        });
        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));

        let _ = unit.update(ElbeyMessage::IconLoaded(0, None));

        assert!(matches!(
            unit.state.apps[0].icon_handle,
            IconHandle::Vector(_)
        ));
    }

    /// Ignored by default; run with `cargo test measure_load_visible_icons_time -- --ignored --nocapture`
    /// to capture the elapsed time for filtering and preparing icon loads over a large dataset.
    #[test]
    #[ignore]
    fn measure_load_visible_icons_time() {
        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: EMPTY_LOADER,
            app_launcher: |_| Ok(()),
            theme: Theme::default(),
            window_size: (320, 320),
            icon_size: 48,
        });

        let app_count = 50_000;
        unit.state.apps = (0..app_count)
            .map(|i| AppDescriptor {
                appid: format!("test_app_id_{i}"),
                title: format!("App {i}"),
                lower_title: format!("app {i}"),
                exec: "".to_string(),
                exec_count: 0,
                icon_name: None,
                icon_handle: default_icon_handle(),
            })
            .collect();
        unit.state.entry = "app 4".to_string();
        unit.state.entry_lower = unit.state.entry.to_lowercase();
        unit.state.selected_index = 0;

        let start = Instant::now();
        let _ = unit.load_visible_icons();
        let elapsed = start.elapsed();
        println!(
            "load_visible_icons on {app_count} apps took {:?} (view size {})",
            elapsed, VIEWABLE_LIST_ITEM_COUNT
        );
    }
}
