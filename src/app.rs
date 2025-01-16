//! Functions and other types for `iced` UI to view, filter, and launch apps
use std::process::exit;
use std::sync::LazyLock;

use freedesktop_desktop_entry::DesktopEntry;
use iced::widget::button::{primary, text};
use iced::widget::scrollable::{snap_to, RelativeOffset};
use iced::widget::{button, column, scrollable, text_input, Column};
use iced::{event, window, Element, Event, Length, Task};
use iced_core::keyboard::key::Named;
use iced_core::keyboard::Key;

/// A magic value to calculate relative pixel hight to move one item in the scrollable
const ITEM_HEIGHT_SCALE_FACTOR: f32 = 0.00750;

static ENTRY_WIDGET_ID: LazyLock<iced::widget::text_input::Id> =
    std::sync::LazyLock::new(|| iced::widget::text_input::Id::new("entry"));
static ITEMS_WIDGET_ID: LazyLock<iced::widget::scrollable::Id> =
    std::sync::LazyLock::new(|| iced::widget::scrollable::Id::new("items"));

/// The application model type.  See [the iced book](https://book.iced.rs/) for details.
#[derive(Debug)]
pub struct State {
    entry: String, // A text entry box where a user can enter list filter criteria
    apps: Vec<DesktopEntry<'static>>, // The complete list of DesktopEntry, as retrieved by lib
    selected_index: usize, // The index of the item visibly selected in the UI
}

/// Root struct of application
#[derive(Debug)]
pub struct Elbey {
    state: State,
    flags: ElbeyFlags,
}

/// Messages are how your logic mutates the app state and GUI
#[derive(Debug, Clone)]
pub enum ElbeyMessage {
    /**
     * Signals that the `DesktopEntries` have been fully loaded into the vec.
     */
    ModelLoaded(Vec<DesktopEntry<'static>>),
    /**
     * Signals that the primary text edit box on the UI has been changed by the user, including the new text.
     */
    EntryUpdate(String),
    /**
     * Signals that the user has taken primary action on a selection.  In the case of a desktop app launcher, the app is launched.
     */
    ExecuteSelected(),
    /**
     * Signals that the user has pressed a key.
     */
    KeyEvent(Key),
    /**
     * Signals that the window has lost focus.
     */
    LostFocus,
}

/// Provide some initial configuration to app to facilitate testing
#[derive(Debug, Clone)]
pub struct ElbeyFlags {
    /**
     * A function that returns a list of `DesktopEntry`s
     */
    pub apps_loader: fn() -> Vec<DesktopEntry<'static>>,
    /**
     * A function that launches a process from a `DesktopEntry`
     */
    pub app_launcher: fn(&DesktopEntry) -> anyhow::Result<()>, //TODO ~ return a task that exits app
}

impl Elbey {
    /// Initialize the app.  Only notable item here is probably the return type Command<_> and what we pass
    /// back.  Here, within the async execution, we directly call the library to retrieve `DesktopEntry`'s which
    /// are the primary model of the [XDG Desktop Specification](https://www.freedesktop.org/wiki/Specifications/desktop-entry-spec/).
    pub fn new(flags: ElbeyFlags) -> (Self, Task<ElbeyMessage>) {
        (
            Self {
                state: State {
                    entry: String::new(),
                    apps: vec![],
                    selected_index: 0,
                },
                flags: flags.clone(),
            },
            Task::perform(async {}, move |_| {
                ElbeyMessage::ModelLoaded((flags.apps_loader)())
            }),
        )
    }

    /// Entry-point from `iced`` into app to construct UI
    pub fn view(&self) -> Element<'_, ElbeyMessage> {
        // Create the list UI elements based on the `DesktopEntry` model
        let app_elements: Vec<Element<ElbeyMessage>> = self
            .state
            .apps
            .iter()
            .filter(|e| Self::text_entry_filter(e, &self.state))
            .enumerate()
            .map(|(index, entry)| {
                let name = entry.desktop_entry("Name").unwrap_or("err");
                let selected = self.state.selected_index == index;
                button(name)
                    .style(move |theme, status| {
                        if selected {
                            primary(theme, status)
                        } else {
                            text(theme, status)
                        }
                    })
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
                .width(320),
            scrollable(Column::with_children(app_elements))
                .width(320)
                .id(ITEMS_WIDGET_ID.clone()),
        ]
        .into()
    }

    /// Entry-point from `iced` to handle user and system events
    pub fn update(&mut self, message: ElbeyMessage) -> Task<ElbeyMessage> {
        match message {
            // The model has been loaded, initialize the UI
            ElbeyMessage::ModelLoaded(items) => {
                self.state.apps = items;
                text_input::focus::<ElbeyMessage>(ENTRY_WIDGET_ID.clone())
            }
            // Rebuild the select list based on the updated text entry
            ElbeyMessage::EntryUpdate(entry_text) => {
                self.state.entry = entry_text;
                self.state.selected_index = 0;

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
                Key::Named(Named::Escape) => exit(0),
                Key::Named(Named::ArrowUp) => self.navigate_items(-1),
                Key::Named(Named::ArrowDown) => self.navigate_items(1),
                Key::Named(Named::Enter) => {
                    if let Some(entry) = self.selected_entry() {
                        (self.flags.app_launcher)(entry).expect("Failed to launch app");
                    }
                    Task::none()
                }
                _ => Task::none(),
            },
            // Handle window events
            ElbeyMessage::LostFocus => {
                // iced::window::close(window::Id::MAIN)
                exit(0);
            }
        }
    }

    /// The `iced` entry-point to setup event listeners
    pub fn subscription(&self) -> iced::Subscription<ElbeyMessage> {
        // Framework code to integrate with underlying user interface devices; keyboard, mouse.
        event::listen_with(|event, _status, _| match event {
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

    // Return ref to the selected item from the app list after applying filter
    fn selected_entry(&self) -> Option<&DesktopEntry> {
        self.state
            .apps
            .iter()
            .filter(|e| Self::text_entry_filter(e, &self.state))
            .nth(self.state.selected_index)
    }

    // Change the selected item and update the UI with the returned `Task`
    fn navigate_items(&mut self, delta: i32) -> iced::Task<ElbeyMessage> {
        let new_index = (self.state.selected_index as i32 + delta) as usize;
        let size = self
            .state
            .apps
            .iter()
            .filter(|e| Self::text_entry_filter(e, &self.state))
            .count();

        if (0..size).contains(&new_index) {
            self.state.selected_index = new_index;

            snap_to::<ElbeyMessage>(
                ITEMS_WIDGET_ID.clone(),
                RelativeOffset {
                    x: 0.0,
                    y: new_index as f32 * ITEM_HEIGHT_SCALE_FACTOR,
                },
            )
        } else {
            Task::none() // If the new location is out of bounds, ignore
        }
    }

    // Compute the items in the list to display based on the model
    fn text_entry_filter(entry: &DesktopEntry, model: &State) -> bool {
        if let Some(name) = entry.desktop_entry("Name") {
            name.to_lowercase().contains(&model.entry.to_lowercase())
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static EMPTY_LOADER: fn() -> Vec<DesktopEntry<'static>> = || vec![];

    static TEST_DESKTOP_ENTRY_1: LazyLock<DesktopEntry> =
        std::sync::LazyLock::new(|| DesktopEntry::from_appid("test_app_id_1"));
    static TEST_DESKTOP_ENTRY_2: LazyLock<DesktopEntry> =
        std::sync::LazyLock::new(|| DesktopEntry::from_appid("test_app_id_2"));
    static TEST_DESKTOP_ENTRY_3: LazyLock<DesktopEntry> =
        std::sync::LazyLock::new(|| DesktopEntry::from_appid("test_app_id_3"));

    static TEST_ENTRY_LOADER: fn() -> Vec<DesktopEntry<'static>> = || {
        vec![
            TEST_DESKTOP_ENTRY_1.clone(),
            TEST_DESKTOP_ENTRY_2.clone(),
            TEST_DESKTOP_ENTRY_3.clone(),
        ]
    };

    #[test]
    fn test_default_app_launch() {
        let test_launcher: fn(&DesktopEntry) -> anyhow::Result<()> = |e| {
            assert!(e.appid == "test_app_id_1");
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_no_apps_try_launch() {
        let test_launcher: fn(&DesktopEntry) -> anyhow::Result<()> = |_e| {
            assert!(false); // should never get here
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(EMPTY_LOADER()));
        let _result = unit.update(ElbeyMessage::ExecuteSelected());
    }

    #[test]
    fn test_app_navigation() {
        let test_launcher: fn(&DesktopEntry) -> anyhow::Result<()> = |e| {
            assert!(e.appid == "test_app_id_2");
            Ok(())
        };

        let (mut unit, _) = Elbey::new(ElbeyFlags {
            apps_loader: TEST_ENTRY_LOADER,
            app_launcher: test_launcher,
        });

        let _ = unit.update(ElbeyMessage::ModelLoaded(TEST_ENTRY_LOADER()));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowDown)));
        let _ = unit.update(ElbeyMessage::KeyEvent(Key::Named(Named::ArrowUp)));
        let _ = unit.update(ElbeyMessage::ExecuteSelected());
    }
}
