//! Functions and other types for `iced` UI to view, filter, and launch apps
use std::cmp::{max, min};
use std::process::exit;

use elbey_cache::AppDescriptor;
use iced::keyboard::key::Named;
use iced::keyboard::Key;
use iced::widget::button::{primary, text as text_style};
use iced::widget::operation::focus;
use iced::widget::{
    button, column, container, image, row, scrollable, svg, text, text_input, Column,
};
use iced::{border, event, window, Alignment, Element, Event, Length, Pixels, Task, Theme};
use iced_layershell::to_layer_message;

use crate::values::*;
use crate::CACHE;
use crate::PROGRAM_NAME;

fn persist_cache_snapshot(apps: &[AppDescriptor]) {
    if let Ok(mut cache) = CACHE.lock() {
        if let Err(e) = cache.save_snapshot(apps) {
            eprintln!("Failed to persist cache snapshot: {e}");
        }
    }
}

fn default_icon_handle() -> IconHandle {
    FALLBACK_ICON_HANDLE.clone()
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
    /// Indices of apps that match the current filter, to avoid re-filtering
    filtered_indices: Vec<usize>,
    /// The index of the item visibly selected in the UI
    selected_index: usize,
    /// A flag to indicate app window has received focus. Work around to some windowing environments passing `unfocused` unexpectedly.
    received_focus: bool,
}

/// Root struct of application
#[derive(Debug)]
pub struct Elbey {
    state: State,
    flags: ElbeyFlags,
}

/// Messages are how your logic mutates the app state and GUI
#[to_layer_message]
#[derive(Debug, Clone)]
pub enum ElbeyMessage {
    /// Signals that the `DesktopEntries` have been fully loaded into the vec
    ModelLoaded(Vec<AppDescriptor>),
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
    /// Triggers a follow-up render after initial model load.
    PostLoadRefresh,
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

    pub icon_size: u16,

    /// Placeholder text for the entry field.
    pub hint: String,

    /// Font size for the filter input.
    pub filter_font_size: u16,

    /// Font size for the entry list items.
    pub entries_font_size: u16,
}

impl Elbey {
    /// Initialize the app.  Only notable item here is probably the return type Task<ElbeyMessage> and what we pass
    /// back.  Here, within the async execution, we directly call the library to retrieve `DesktopEntry`'s which
    /// are the primary model of the [XDG Desktop Specification](https://www.freedesktop.org/wiki/Specifications/desktop-entry-spec/).
    /// Then we create and pass a layer shell as another task.
    pub fn new(flags: ElbeyFlags) -> (Self, Task<ElbeyMessage>) {
        // A task to load the app model
        let apps_loader = flags.apps_loader;
        let load_task = Task::perform(async move { (apps_loader)() }, ElbeyMessage::ModelLoaded);

        (
            Self {
                state: State {
                    entry: String::new(),
                    entry_lower: String::new(),
                    apps: vec![],
                    filtered_indices: vec![],
                    selected_index: 0,
                    received_focus: false,
                },
                flags,
            },
            load_task,
        )
    }

    pub fn namespace() -> String {
        PROGRAM_NAME.to_string()
    }

    /// Entry-point from `iced`` into app to construct UI
    pub fn view(&self) -> Element<'_, ElbeyMessage> {
        // Create the list UI elements based on the `DesktopEntry` model
        let app_elements: Vec<Element<ElbeyMessage>> = self
            .state
            .filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(filtered_index, original_index)| {
                self.state
                    .apps
                    .get(*original_index)
                    .map(|entry| (filtered_index, entry))
            })
            .filter(|(filtered_index, _)| {
                (self.state.selected_index..self.state.selected_index + VIEWABLE_LIST_ITEM_COUNT)
                    .contains(filtered_index)
            }) // Only show entries in selection range
            .map(|(filtered_index, entry)| {
                let name = entry.title.as_str();
                let selected = self.state.selected_index == filtered_index;
                let icon_handle_to_render = match &entry.icon_handle {
                    IconHandle::NotLoaded => default_icon_handle(),
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
                    IconHandle::NotLoaded => unreachable!(),
                };
                let content = row![
                    icon,
                    text(name).size(Pixels::from(u32::from(self.flags.entries_font_size)))
                ]
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
        let content = column![
            text_input(&self.flags.hint, &self.state.entry)
                .id(ENTRY_WIDGET_ID.clone())
                .on_input(ElbeyMessage::EntryUpdate)
                .size(Pixels::from(u32::from(self.flags.filter_font_size)))
                .width(Length::Fill),
            scrollable(Column::with_children(app_elements))
                .width(Length::Fill)
                .height(Length::Fill)
                .id(ITEMS_WIDGET_ID.clone()),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(1)
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style::default()
                    .background(palette.background.base.color)
                    .border(border::width(1).color(palette.background.base.text))
            })
            .into()
    }

    /// Entry-point from `iced` to handle user and system events
    pub fn update(&mut self, message: ElbeyMessage) -> Task<ElbeyMessage> {
        match message {
            // The model has been loaded, initialize the UI
            ElbeyMessage::ModelLoaded(items) => {
                self.state.apps = items;
                self.state.entry_lower = self.state.entry.to_lowercase();
                self.refresh_filtered_indices();
                let focus_task = focus(ENTRY_WIDGET_ID.clone());
                let refresh_task = Task::perform(async {}, |_| ElbeyMessage::PostLoadRefresh);
                Task::batch(vec![focus_task, refresh_task])
            }
            // Rebuild the select list based on the updated text entry
            ElbeyMessage::EntryUpdate(entry_text) => {
                self.state.entry = entry_text;
                self.state.entry_lower = self.state.entry.to_lowercase();
                self.state.selected_index = 0;
                self.refresh_filtered_indices();
                Task::none()
            }
            // Launch an application selected by the user
            ElbeyMessage::ExecuteSelected() => {
                if let Some(entry) = self.selected_entry() {
                    (self.flags.app_launcher)(entry).expect("Failed to launch app");
                }
                Task::none()
            }
            // Handle keyboard entries
            ElbeyMessage::KeyEvent(key) => match key {
                Key::Named(Named::Escape) => {
                    persist_cache_snapshot(&self.state.apps);
                    exit(0)
                }
                Key::Named(Named::ArrowUp) => {
                    self.navigate_items(-1);
                    Task::none()
                }
                Key::Named(Named::ArrowDown) => {
                    self.navigate_items(1);
                    Task::none()
                }
                Key::Named(Named::PageUp) => {
                    self.navigate_items(-(VIEWABLE_LIST_ITEM_COUNT as i32));
                    Task::none()
                }
                Key::Named(Named::PageDown) => {
                    self.navigate_items(VIEWABLE_LIST_ITEM_COUNT as i32);
                    Task::none()
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
                focus(ENTRY_WIDGET_ID.clone())
            }
            ElbeyMessage::LostFocus => {
                if self.state.received_focus {
                    persist_cache_snapshot(&self.state.apps);
                    exit(0);
                }
                Task::none()
            }
            ElbeyMessage::PostLoadRefresh => Task::none(),
            ElbeyMessage::AnchorChange(anchor) => {
                dbg!(anchor);
                Task::none()
            }
            ElbeyMessage::SetInputRegion(_action_callback) => Task::none(),
            ElbeyMessage::AnchorSizeChange(anchor, _) => {
                dbg!(anchor);
                Task::none()
            }
            ElbeyMessage::ExclusiveZoneChange(exclusive_zone) => {
                dbg!(exclusive_zone);
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
    pub fn subscription(&self) -> iced::Subscription<ElbeyMessage> {
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
                repeat: _,
            }) => Some(ElbeyMessage::KeyEvent(key)),
            _ => None,
        })
    }

    pub fn theme(&self) -> Theme {
        self.flags.theme.clone()
    }
}

impl Elbey {
    // Return ref to the selected item from the app list after applying filter
    fn selected_entry(&self) -> Option<&AppDescriptor> {
        self.state
            .filtered_indices
            .get(self.state.selected_index)
            .and_then(|original_index| self.state.apps.get(*original_index))
    }

    fn navigate_items(&mut self, delta: i32) {
        let filtered_len = self.state.filtered_indices.len();
        if filtered_len == 0 {
            self.state.selected_index = 0;
            return;
        }

        if delta < 0 {
            self.state.selected_index = max(0, self.state.selected_index as i32 + delta) as usize;
        } else {
            self.state.selected_index = min(
                filtered_len as i32 - 1,
                self.state.selected_index as i32 + delta,
            ) as usize;
        }
    }

    // Compute the items in the list to display based on the model
    fn text_entry_filter(entry: &AppDescriptor, model: &State) -> bool {
        entry.lower_title.contains(&model.entry_lower)
    }

    fn refresh_filtered_indices(&mut self) {
        self.state.filtered_indices = self
            .state
            .apps
            .iter()
            .enumerate()
            .filter(|(_, e)| Self::text_entry_filter(e, &self.state))
            .map(|(i, _)| i)
            .collect();

        if self.state.selected_index >= self.state.filtered_indices.len() {
            self.state.selected_index = self.state.filtered_indices.len().saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{LazyLock, OnceLock};

    fn set_test_cache_home() {
        static CACHE_HOME: OnceLock<PathBuf> = OnceLock::new();
        let cache_dir = CACHE_HOME.get_or_init(|| {
            let mut dir = std::env::temp_dir();
            dir.push(format!("elbey-test-cache-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            dir
        });
        std::env::set_var("XDG_CACHE_HOME", cache_dir);
    }

    static EMPTY_LOADER: fn() -> Vec<AppDescriptor> = || vec![];

    static TEST_DESKTOP_ENTRY_1: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_1".to_string(),
        title: "t1".to_string(),
        lower_title: "t1".to_string(),
        exec: None,
        exec_count: 0,
        icon_name: None,
        icon_path: None,
        icon_handle: IconHandle::NotLoaded,
    });

    static TEST_DESKTOP_ENTRY_2: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_2".to_string(),
        title: "t2".to_string(),
        lower_title: "t2".to_string(),
        exec: None,
        exec_count: 0,
        icon_name: None,
        icon_path: None,
        icon_handle: IconHandle::NotLoaded,
    });

    static TEST_DESKTOP_ENTRY_3: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_3".to_string(),
        title: "t2".to_string(),
        lower_title: "t2".to_string(),
        exec: None,
        exec_count: 0,
        icon_name: None,
        icon_path: None,
        icon_handle: IconHandle::NotLoaded,
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
            theme: DEFAULT_THEME,
            icon_size: 48,
            hint: DEFAULT_HINT.to_string(),
            filter_font_size: DEFAULT_TEXT_SIZE,
            entries_font_size: DEFAULT_TEXT_SIZE,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_no_apps_try_launch() {
        let test_launcher: fn(&AppDescriptor) -> anyhow::Result<()> = |_e| {
            unreachable!("should never get here");
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
            theme: DEFAULT_THEME,
            icon_size: 48,
            hint: DEFAULT_HINT.to_string(),
            filter_font_size: DEFAULT_TEXT_SIZE,
            entries_font_size: DEFAULT_TEXT_SIZE,
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
            theme: DEFAULT_THEME,
            icon_size: 48,
            hint: DEFAULT_HINT.to_string(),
            filter_font_size: DEFAULT_TEXT_SIZE,
            entries_font_size: DEFAULT_TEXT_SIZE,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowUp)));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_loaded_icons_render_immediately() {
        set_test_cache_home();
        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: |_| Ok(()),
            theme: DEFAULT_THEME,
            icon_size: 48,
            hint: DEFAULT_HINT.to_string(),
            filter_font_size: DEFAULT_TEXT_SIZE,
            entries_font_size: DEFAULT_TEXT_SIZE,
        });
        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));

        assert!(matches!(
            unit.state.apps[0].icon_handle,
            IconHandle::Vector(_) | IconHandle::Raster(_) | IconHandle::NotLoaded
        ));
    }
}
