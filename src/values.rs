//! Constants and literal values used throughout the application.
use std::sync::LazyLock;

use iced::widget::Id;
use iced::Theme;

pub use elbey_cache::{IconHandle, DEFAULT_ICON_SIZE, FALLBACK_ICON_HANDLE};

pub static PROGRAM_NAME: LazyLock<String> = LazyLock::new(|| String::from("Elbey"));
pub const DEFAULT_WINDOW_HEIGHT: u32 = 200;
pub const DEFAULT_WINDOW_WIDTH: u32 = 320;
pub const DEFAULT_THEME: Theme = Theme::Nord;
pub const DEFAULT_TEXT_SIZE: u16 = 16;
pub const DEFAULT_HINT: &str = "drun";

pub static ENTRY_WIDGET_ID: LazyLock<Id> = LazyLock::new(|| Id::new("entry"));
pub static ITEMS_WIDGET_ID: LazyLock<Id> = LazyLock::new(|| Id::new("items"));

// The max number of items to render in the list
pub const VIEWABLE_LIST_ITEM_COUNT: usize = 10;
