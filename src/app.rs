//! Functions and other types for `iced` UI to view, filter, and launch apps
use std::cmp::{max, min};
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

use crate::PROGRAM_NAME;
use crate::values::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppDescriptor {
    pub appid: String,
    pub title: String,
    pub exec: String,
    pub exec_count: usize,
    pub icon_name: Option<String>,
    #[serde(skip, default)]
    pub icon_handle: Option<IconHandle>,
}

impl From<DesktopEntry> for AppDescriptor {
    fn from(value: DesktopEntry) -> Self {
        AppDescriptor {
            appid: value.appid.clone(),
            title: value.desktop_entry("Name").expect("get name").to_string(),
            exec: value.exec().expect("has exec").to_string(),
            exec_count: 0,
            icon_name: value.icon().map(str::to_string),
            icon_handle: Some(FALLBACK_ICON_HANDLE.clone()),
        }
    }
}

/// The application model type.  See [the iced book](https://book.iced.rs/) for details.
#[derive(Debug)]
pub struct State {
    /// A text entry box where a user can enter list filter criteria
    entry: String,
    /// The complete list of DesktopEntry, as retrieved by lib
    apps: Vec<AppDescriptor>,
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
        let load_task = Task::perform(async {}, move |_| {
            ElbeyMessage::ModelLoaded((flags.apps_loader)())
        });

        (
            Self {
                state: State {
                    entry: String::new(),
                    apps: vec![],
                    selected_index: 0,
                    received_focus: false,
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
                let icon_handle = entry.icon_handle.as_ref().unwrap();
                let icon: Element<'_, ElbeyMessage> = match icon_handle {
                    IconHandle::Raster(handle) => image(handle.clone())
                        .width(Length::Fixed(self.flags.icon_size.into()))
                        .height(Length::Fixed(self.flags.icon_size.into()))
                        .into(),
                    IconHandle::Vector(handle) => svg(handle.clone())
                        .width(Length::Fixed(self.flags.icon_size.into()))
                        .height(Length::Fixed(self.flags.icon_size.into()))
                        .into(),
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
                let icon_size = self.flags.icon_size;
                let mut tasks: Vec<Task<ElbeyMessage>> = self
                    .state
                    .apps
                    .iter()
                    .enumerate()
                    .map(|(i, app)| {
                        let icon_name = app.icon_name.clone();
                        Task::perform(
                            async move {
                                icon_name
                                    .as_deref()
                                    .and_then(|name| lookup(name).with_size(icon_size).find())
                            },
                            move |path| ElbeyMessage::IconLoaded(i, path),
                        )
                    })
                    .collect();
                tasks.push(text_input::focus(ENTRY_WIDGET_ID.clone()));
                Task::batch(tasks)
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
            ElbeyMessage::IconLoaded(index, path) => {
                if let Some(p) = path {
                    if let Some(app) = self.state.apps.get_mut(index) {
                        app.icon_handle = if p.extension().and_then(|s| s.to_str()) == Some("svg") {
                            Some(IconHandle::Vector(SvgHandle::from_path(p)))
                        } else {
                            Some(IconHandle::Raster(ImageHandle::from_path(p)))
                        };
                    }
                }
                Task::none()
            }
            // Handle keyboard entries
            ElbeyMessage::KeyEvent(key) => match key {
                Key::Named(Named::Escape) => exit(0),
                Key::Named(Named::ArrowUp) => self.navigate_items(-1),
                Key::Named(Named::ArrowDown) => self.navigate_items(1),
                Key::Named(Named::PageUp) => {
                    self.navigate_items(-(VIEWABLE_LIST_ITEM_COUNT as i32))
                }
                Key::Named(Named::PageDown) => self.navigate_items(VIEWABLE_LIST_ITEM_COUNT as i32),
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

    fn navigate_items(&mut self, delta: i32) -> iced::Task<ElbeyMessage> {
        if delta < 0 {
            self.state.selected_index = max(0, self.state.selected_index as i32 + delta) as usize;
        } else {
            self.state.selected_index = min(
                self.state.apps.len() as i32 - 1,
                self.state.selected_index as i32 + delta,
            ) as usize;
        }
        Task::none()
    }

    // Compute the items in the list to display based on the model
    fn text_entry_filter(entry: &AppDescriptor, model: &State) -> bool {
        entry
            .title
            .to_lowercase()
            .contains(&model.entry.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static EMPTY_LOADER: fn() -> Vec<AppDescriptor> = || vec![];

    static TEST_DESKTOP_ENTRY_1: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_1".to_string(),
        title: "t1".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: Some(FALLBACK_ICON_HANDLE.clone()),
    });

    static TEST_DESKTOP_ENTRY_2: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_2".to_string(),
        title: "t2".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: Some(FALLBACK_ICON_HANDLE.clone()),
    });

    static TEST_DESKTOP_ENTRY_3: LazyLock<AppDescriptor> = LazyLock::new(|| AppDescriptor {
        appid: "test_app_id_3".to_string(),
        title: "t2".to_string(),
        exec: "".to_string(),
        exec_count: 0,
        icon_name: None,
        icon_handle: Some(FALLBACK_ICON_HANDLE.clone()),
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
            Some(IconHandle::Raster(_))
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
            Some(IconHandle::Vector(_))
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
            Some(IconHandle::Vector(_))
        ));
    }
}
