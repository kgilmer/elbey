use std::collections::HashMap;
use std::path::{Path, PathBuf};

use freedesktop_icons::lookup;
use iced::widget::image::Handle as ImageHandle;
use iced::widget::svg::Handle as SvgHandle;
use serde::{Deserialize, Serialize};
use sled::{Config, Db, IVec};

use crate::app::AppDescriptor;
use crate::values::{IconHandle, DEFAULT_ICON_SIZE, FALLBACK_ICON_HANDLE};

static SCAN_KEY: [u8; 4] = 0_i32.to_be_bytes();

#[derive(Debug, Serialize, Deserialize, Clone)]
enum CachedIcon {
    Raster(Vec<u8>),
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    Svg(Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CachedAppDescriptor {
    pub appid: String,
    pub title: String,
    #[serde(default)]
    pub lower_title: String,
    pub exec: String,
    pub exec_count: usize,
    pub icon_name: Option<String>,
    #[serde(default)]
    pub icon_path: Option<PathBuf>,
    #[serde(default)]
    pub icon_data: Option<CachedIcon>,
}

/// Tracks state to sort apps by usage
pub(crate) struct Cache {
    apps_loader: fn() -> Vec<AppDescriptor>,
    db: Db,
}

fn is_empty_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
}

fn icon_data_from_path(path: &Path) -> Option<CachedIcon> {
    if is_empty_path(path) {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    let is_svg = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"))
        .unwrap_or(false);

    if is_svg {
        Some(CachedIcon::Svg(bytes))
    } else {
        decode_raster(bytes.as_slice())
    }
}

fn icon_handle_from_data(icon_data: &CachedIcon) -> IconHandle {
    match icon_data {
        CachedIcon::Raster(bytes) => IconHandle::Raster(ImageHandle::from_bytes(bytes.clone())),
        CachedIcon::Rgba {
            width,
            height,
            pixels,
        } => IconHandle::Raster(ImageHandle::from_rgba(*width, *height, pixels.clone())),
        CachedIcon::Svg(bytes) => IconHandle::Vector(SvgHandle::from_memory(bytes.clone())),
    }
}

fn decode_raster(bytes: &[u8]) -> Option<CachedIcon> {
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(CachedIcon::Rgba {
        width,
        height,
        pixels: rgba.into_raw(),
    })
}

fn populate_icon_data(entry: &mut CachedAppDescriptor) -> bool {
    if entry.icon_data.is_some() {
        return false;
    }

    if let Some(path) = entry.icon_path.as_ref() {
        if let Some(icon_data) = icon_data_from_path(path) {
            entry.icon_data = Some(icon_data);
            return true;
        }
    }

    let icon_name = match entry.icon_name.as_deref() {
        Some(name) => name,
        None => return false,
    };

    let path = match lookup(icon_name)
        .with_size(DEFAULT_ICON_SIZE)
        .with_cache()
        .find()
    {
        Some(path) => path,
        None => {
            entry.icon_path = Some(PathBuf::new());
            return true;
        }
    };

    entry.icon_path = Some(path.clone());
    entry.icon_data = icon_data_from_path(&path);
    entry.icon_data.is_some()
}

impl CachedAppDescriptor {
    fn normalize(mut self) -> Self {
        if self.lower_title.is_empty() {
            self.lower_title = self.title.to_lowercase();
        }
        self
    }

    fn from_app_descriptor(
        app: AppDescriptor,
        cached_icon: Option<CachedIcon>,
    ) -> CachedAppDescriptor {
        let icon_data = cached_icon.or_else(|| {
            app.icon_path
                .as_ref()
                .and_then(|path| icon_data_from_path(path))
        });

        CachedAppDescriptor {
            appid: app.appid,
            title: app.title,
            lower_title: app.lower_title,
            exec: app.exec,
            exec_count: app.exec_count,
            icon_name: app.icon_name,
            icon_path: app.icon_path,
            icon_data,
        }
        .normalize()
    }

    fn into_app_descriptor(self) -> AppDescriptor {
        let lower_title = if self.lower_title.is_empty() {
            self.title.to_lowercase()
        } else {
            self.lower_title
        };
        let icon_handle = if let Some(ref data) = self.icon_data {
            icon_handle_from_data(data)
        } else if self
            .icon_path
            .as_ref()
            .map(|path| is_empty_path(path))
            .unwrap_or(false)
        {
            FALLBACK_ICON_HANDLE.clone()
        } else {
            IconHandle::NotLoaded
        };

        AppDescriptor {
            appid: self.appid,
            title: self.title,
            lower_title,
            exec: self.exec,
            exec_count: self.exec_count,
            icon_name: self.icon_name,
            icon_path: self.icon_path,
            icon_handle,
        }
    }
}

impl Cache {
    pub fn new(apps_loader: fn() -> Vec<AppDescriptor>) -> Self {
        let path = Self::resolve_db_file_path();
        let config = Config::new().path(path);
        let db = config.open().unwrap();

        Cache { apps_loader, db }
    }

    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    pub fn read_all(&self) -> Option<Vec<AppDescriptor>> {
        let entries = self.read_cached_entries()?;
        Some(
            entries
                .into_iter()
                .map(CachedAppDescriptor::into_app_descriptor)
                .collect(),
        )
    }

    fn write_snapshot(
        &mut self,
        apps: impl IntoIterator<Item = CachedAppDescriptor>,
    ) -> anyhow::Result<()> {
        let mut snapshot: Vec<CachedAppDescriptor> = apps.into_iter().collect();
        snapshot.sort_by(|a, b| (b.exec_count, &a.title).cmp(&(a.exec_count, &b.title)));

        self.db.clear()?;
        for (count, app_descriptor) in snapshot.into_iter().enumerate() {
            let encoded: Vec<u8> = bincode::serialize(&app_descriptor)?;
            self.db.insert(count.to_be_bytes(), IVec::from(encoded))?;
        }
        self.db.flush()?;
        Ok(())
    }

    // Update the cache from local system and update usage stat
    pub fn update(&mut self, selected_app: &AppDescriptor) -> anyhow::Result<()> {
        // load data
        let latest_entries = (self.apps_loader)();
        let cached_entries = self.read_cached_entries().unwrap_or_default();
        let mut cached_by_id: HashMap<String, CachedAppDescriptor> = cached_entries
            .into_iter()
            .map(|entry| (entry.appid.clone(), entry))
            .collect();

        // create new wrapper vec
        let mut updated_entry_wrappers: Vec<CachedAppDescriptor> =
            Vec::with_capacity(latest_entries.len());

        for mut latest_entry in latest_entries {
            let cached_entry = cached_by_id.remove(&latest_entry.appid);
            let (count, cached_icon_path, cached_icon_data) = if let Some(entry) = cached_entry {
                (entry.exec_count, entry.icon_path, entry.icon_data)
            } else {
                (0, None, None)
            };

            latest_entry.exec_count = if latest_entry.appid == selected_app.appid {
                count + 1
            } else {
                count
            };
            latest_entry.icon_path = cached_icon_path.or(latest_entry.icon_path);

            updated_entry_wrappers.push(CachedAppDescriptor::from_app_descriptor(
                latest_entry,
                cached_icon_data,
            ));
        }

        // sort
        self.write_snapshot(updated_entry_wrappers)
    }

    pub fn store_snapshot(&mut self, apps: &[AppDescriptor]) -> anyhow::Result<()> {
        let cached_icons: HashMap<String, Option<CachedIcon>> = self
            .read_cached_entries()
            .unwrap_or_default()
            .into_iter()
            .map(|entry| (entry.appid, entry.icon_data))
            .collect();

        let snapshot = apps.iter().cloned().map(|app| {
            let cached_icon = cached_icons.get(&app.appid).cloned().flatten();
            CachedAppDescriptor::from_app_descriptor(app, cached_icon)
        });

        self.write_snapshot(snapshot)
    }

    fn read_cached_entries(&self) -> Option<Vec<CachedAppDescriptor>> {
        let iter = self.db.range(SCAN_KEY..);

        let mut app_descriptors: Vec<CachedAppDescriptor> = vec![];
        for item in iter {
            let (_key, desc_ivec) = item.ok()?;

            let mut cached: CachedAppDescriptor =
                match bincode::deserialize::<CachedAppDescriptor>(&desc_ivec[..]) {
                    Ok(entry) => entry,
                    Err(_) => {
                        let app: AppDescriptor = bincode::deserialize(&desc_ivec[..]).ok()?;
                        CachedAppDescriptor::from_app_descriptor(app, None)
                    }
                };

            cached = cached.normalize();
            app_descriptors.push(cached);
        }

        Some(app_descriptors)
    }

    pub(crate) fn resolve_db_file_path() -> PathBuf {
        let mut path = dirs::cache_dir().unwrap();
        path.push(format!(
            "{}-{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ));
        path
    }

    pub fn build_snapshot_with_icons(&mut self, apps: &[AppDescriptor]) -> anyhow::Result<()> {
        let snapshot = apps.iter().cloned().map(|app| {
            let mut cached = CachedAppDescriptor::from_app_descriptor(app, None);
            populate_icon_data(&mut cached);
            cached
        });

        self.write_snapshot(snapshot)
    }
}

pub(crate) fn delete_cache_dir() -> std::io::Result<()> {
    let path = Cache::resolve_db_file_path();
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;
    use std::sync::OnceLock;

    fn set_test_cache_home() -> PathBuf {
        static CACHE_HOME: OnceLock<PathBuf> = OnceLock::new();
        let cache_dir = CACHE_HOME.get_or_init(|| {
            let mut dir = std::env::temp_dir();
            dir.push(format!("elbey-cache-test-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            dir
        });
        std::env::set_var("XDG_CACHE_HOME", cache_dir);
        cache_dir.clone()
    }

    #[test]
    fn test_cache_reads_icons_as_rgba() {
        let cache_home = set_test_cache_home();
        let icon_path = cache_home.join("test-icon.png");
        let mut png_bytes = Vec::new();
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode test icon");
        std::fs::write(&icon_path, &png_bytes).expect("write test icon");

        let mut cache = Cache::new(Vec::new);
        let app = AppDescriptor {
            appid: "test-app".to_string(),
            title: "Test App".to_string(),
            lower_title: "test app".to_string(),
            exec: "/bin/true".to_string(),
            exec_count: 0,
            icon_name: None,
            icon_path: Some(icon_path),
            icon_handle: IconHandle::NotLoaded,
        };

        cache.store_snapshot(&[app]).expect("store snapshot");
        let apps = cache.read_all().expect("read snapshot");

        assert!(matches!(apps[0].icon_handle, IconHandle::Raster(_)));
    }
}
