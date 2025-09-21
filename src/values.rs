//! Constants and literal values used throughout the application.
use std::sync::LazyLock;

use iced::widget::image::Handle as ImageHandle;
use iced::widget::svg::Handle as SvgHandle;
use iced::widget::{scrollable, text_input};
use iced::Theme;

pub static PROGRAM_NAME: LazyLock<String> = LazyLock::new(|| String::from("Elbey"));
pub const DEFAULT_WINDOW_HEIGHT: u32 = 320;
pub const DEFAULT_WINDOW_WIDTH: u32 = 320;
pub const DEFAULT_ICON_SIZE: u16 = 48;
pub const DEFAULT_THEME: Theme = Theme::Nord;
pub const DEFAULT_TEXT_SIZE: u16 = 16;

pub static ENTRY_WIDGET_ID: LazyLock<text_input::Id> =
    LazyLock::new(|| text_input::Id::new("entry"));

// An SVG icon used as a fallback, from https://en.m.wikipedia.org/wiki/File:Application-x-executable.svg
static FALLBACK_ICON_DATA: &[u8] = r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="48" height="48">
  <defs>
    <linearGradient id="b">
      <stop offset="0" stop-opacity=".32673267"/>
      <stop offset="1" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="a" x1="99.7773" x2="153.0005" y1="15.4238" y2="248.6311" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#184375"/>
      <stop offset="1" stop-color="#c8bddc"/>
    </linearGradient>
    <linearGradient xlink:href="#a" id="d" x1="99.7773" x2="153.0005" y1="15.4238" y2="248.6311" gradientTransform="translate(-.585758 -1.050787) scale(.20069)" gradientUnits="userSpaceOnUse"/>
    <radialGradient xlink:href="#b" id="c" cx="14.287618" cy="68.872971" r="11.68987" fx="14.287618" fy="72.568001" gradientTransform="matrix(1.39926 0 0 .51326 4.365074 4.839285)" gradientUnits="userSpaceOnUse"/>
  </defs>
  <path fill="url(#c)" fill-rule="evenodd" d="M44.285715 38.714287a19.928572 9.837245 0 1 1-39.8571433 0 19.928572 9.837245 0 1 1 39.8571433 0z" color="#000" overflow="visible" style="marker:none" transform="translate(-4.539687 -7.794678) scale(1.18638)"/>
  <path fill="url(#d)" stroke="#3f4561" stroke-linecap="round" stroke-linejoin="round" d="M24.285801 43.196358 4.3751874 23.285744 24.285801 3.3751291 44.196415 23.285744 24.285801 43.196358h0z"/>
  <path fill="#fff" d="M43.505062 23.285744 24.285801 4.0664819 5.0665401 23.285744l.7810675.624932L24.45724 5.4825431 43.505256 23.285744h-.000194z" opacity=".72000003"/>
  <path fill="#fff" d="m8.9257729 27.145172.7384498-1.024184c.6367493.268492 1.3006183.485069 1.9861833.644885l-.005812 1.576858c.427728.088335.86301.156136 1.304105.204371l.481774-1.501889c.344041.028477.691764.044167 1.043361.044167.351209 0 .699124-.015497 1.043166-.044167l.481775 1.501889c.441288-.048235.876376-.116036 1.304104-.204371l-.006005-1.577051c.685758-.159623 1.349433-.3762 1.986182-.644692l.92248 1.279502c.402351-.182094.794241-.382591 1.174895-.600522l-.492817-1.498016c.59723-.36225 1.161723-.773319 1.687471-1.227972l1.272141.931779c.325638-.296581.637329-.608272.933716-.93391l-.931585-1.271947c.454847-.525748.865916-1.090047 1.228166-1.687665l1.498015.493011c.217932-.380848.418623-.772932.600329-1.175088l-1.279308-.922287c.268492-.636749.485068-1.300618.645079-1.986376l1.576663.005811c.088335-.427727.156137-.86301.204178-1.304298l-1.501695-.481774c.028864-.343848.044167-.691764.044167-1.043167 0-.351403-.015691-.699125-.044167-1.043361l1.501695-.481774c-.047847-.441094-.116037-.876183-.203984-1.304104l-1.577051.006005c-.159817-.685759-.376393-1.349627-.644691-1.9861811l1.279308-.9222887c-.181707-.4023513-.382591-.7942415-.600135-1.1750898l-1.498209.4930113c-.362251-.5974244-.773319-1.1617232-1.227973-1.6872772l.931586-1.2721409c-.278372-.3058794-.571078-.5980048-.875408-.8781198L5.0669275 23.285938l1.0069418 1.006942.2987118-.218706c.5257484.454653 1.0900465.865722 1.6874698 1.227972l-.2419526.735157 1.1080622 1.108062-.0003876-.000193zm19.5232031 5.045944c0-6.484682 4.233883-11.979469 10.08724-13.874023l-2.226972-2.227167c-.016854.006975-.0339.01298-.05056.020147l-.181513-.251832-1.412004-1.412004c-.463178.2189-.91667.45446-1.359314.707648l.694089 2.109193c-.841314.509669-1.635748 1.08869-2.375747 1.728732l-1.79111-1.311659c-.458721.41746-.897297.856036-1.314564 1.314565l1.311465 1.790914c-.640041.740195-1.218868 1.534628-1.728731 2.375748l-2.109387-.694089c-.306654.536403-.589093 1.088304-.844994 1.654732l1.801182 1.298293c-.377942.896329-.682852 1.831014-.907758 2.796501l-2.219999-.008524c-.124172.602266-.219869 1.215188-.287476 1.836051l2.114423.678398c-.040293.484293-.061991.97401-.061991 1.46857 0 .494753.021698.98447.061991 1.468763l-2.114423.677816c.067607.621251.163304 1.233979.28767 1.836245l2.219805-.00833c.224906.965487.529816 1.900172.907758 2.796502l-1.801182 1.298486c.142382.31479.293869.624931.452136.930423l3.804023-3.803636c-.61602-1.614245-.95425-3.365836-.95425-5.196269l.000193-.000194z" opacity=".49999997"/>
  <path d="M5.2050478 23.424252 24.285801 42.505005l19.219261-19.219261-.715099-.682219-18.479649 18.438152L5.2050478 23.424059v.000193z" opacity=".34999999"/>
</svg>"##.as_bytes();

pub static FALLBACK_ICON_HANDLE: LazyLock<IconHandle> =
    LazyLock::new(|| IconHandle::Vector(SvgHandle::from_memory(FALLBACK_ICON_DATA)));

pub static ITEMS_WIDGET_ID: LazyLock<scrollable::Id> =
    LazyLock::new(|| scrollable::Id::new("items"));

// The max number of items to render in the list
pub const VIEWABLE_LIST_ITEM_COUNT: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub enum IconHandle {
    Raster(ImageHandle),
    Vector(SvgHandle),
    /// Represents an icon that is currently being loaded.
    Loading,
}