use std::collections::HashMap;
use std::path::{Path, PathBuf};

use freedesktop_icons::lookup;
use iced::widget::image::Handle as ImageHandle;
use iced::widget::svg::Handle as SvgHandle;
use serde::{Deserialize, Serialize};
use sled::{Config, Db, IVec};

use crate::{AppDescriptor, IconHandle, DEFAULT_ICON_SIZE, FALLBACK_ICON_HANDLE};

const CACHE_NAMESPACE: &str = "elbey";

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
    pub exec: Option<String>,
    pub exec_count: usize,
    pub icon_name: Option<String>,
    #[serde(default)]
    pub icon_path: Option<PathBuf>,
    #[serde(default)]
    pub icon_data: Option<CachedIcon>,
}

/// Tracks state to sort apps by usage and persist cached metadata.
pub struct Cache {
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
    /// Create a cache using the default Elbey cache namespace.
    pub fn new(apps_loader: fn() -> Vec<AppDescriptor>) -> Self {
        let path = resolve_db_file_path();
        let config = Config::new().path(path);
        let db = config.open().unwrap();

        Cache { apps_loader, db }
    }

    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    /// Load all cached entries into app descriptors, if available.
    pub fn read_all(&mut self) -> Option<Vec<AppDescriptor>> {
        let entries = self.read_cached_entries()?;
        if !entries.is_empty() || !self.db.is_empty() {
            return Some(
                entries
                    .into_iter()
                    .map(CachedAppDescriptor::into_app_descriptor)
                    .collect(),
            );
        }

        let apps = (self.apps_loader)();
        if apps.is_empty() {
            return Some(Vec::new());
        }
        if self.build_snapshot_with_icons(&apps).is_ok() {
            let entries = self.read_cached_entries()?;
            return Some(
                entries
                    .into_iter()
                    .map(CachedAppDescriptor::into_app_descriptor)
                    .collect(),
            );
        }

        Some(apps)
    }

    /// Load up to `count` cached entries into app descriptors, if available.
    pub fn read_top(&mut self, count: usize) -> Option<Vec<AppDescriptor>> {
        let entries = self.read_cached_entries_top(count)?;
        if !entries.is_empty() || !self.db.is_empty() {
            return Some(
                entries
                    .into_iter()
                    .map(CachedAppDescriptor::into_app_descriptor)
                    .collect(),
            );
        }

        if count == 0 {
            return Some(Vec::new());
        }

        let apps = (self.apps_loader)();
        if apps.is_empty() {
            return Some(Vec::new());
        }
        if self.build_snapshot_with_icons(&apps).is_ok() {
            let entries = self.read_cached_entries_top(count)?;
            return Some(
                entries
                    .into_iter()
                    .map(CachedAppDescriptor::into_app_descriptor)
                    .collect(),
            );
        }

        Some(apps.into_iter().take(count).collect())
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        self.update_from_loader(None)
    }

    /// Load from cache when present, falling back to the loader and populating icons.
    pub fn load_from_apps_loader(&mut self) -> Vec<AppDescriptor> {
        self.read_all().unwrap_or_else(|| {
            let apps = (self.apps_loader)();
            let _ = self.build_snapshot_with_icons(&apps);
            apps
        })
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

    fn update_from_loader(&mut self, selected_appid: Option<&str>) -> anyhow::Result<()> {
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

            let is_selected = selected_appid == Some(latest_entry.appid.as_str());
            latest_entry.exec_count = if is_selected { count + 1 } else { count };
            latest_entry.icon_path = cached_icon_path.or(latest_entry.icon_path);

            updated_entry_wrappers.push(CachedAppDescriptor::from_app_descriptor(
                latest_entry,
                cached_icon_data,
            ));
        }

        // sort
        self.write_snapshot(updated_entry_wrappers)
    }

    // Update the cache from local system and update usage stat
    /// Refresh from the loader and increment usage for the selected app.
    pub fn update(&mut self, selected_app: &AppDescriptor) -> anyhow::Result<()> {
        self.update_from_loader(Some(selected_app.appid.as_str()))
    }

    /// Store a snapshot of apps, reusing cached icon data when possible.
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

    fn read_cached_entries_top(&self, count: usize) -> Option<Vec<CachedAppDescriptor>> {
        let iter = self.db.range(SCAN_KEY..);
        let mut app_descriptors: Vec<CachedAppDescriptor> = Vec::with_capacity(count);
        for item in iter.take(count) {
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

    pub fn build_snapshot_with_icons(&mut self, apps: &[AppDescriptor]) -> anyhow::Result<()> {
        let snapshot = apps.iter().cloned().map(|app| {
            let mut cached = CachedAppDescriptor::from_app_descriptor(app, None);
            populate_icon_data(&mut cached);
            cached
        });

        self.write_snapshot(snapshot)
    }
}

fn resolve_db_file_path() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap();
    path.push(format!("{}-{}", CACHE_NAMESPACE, env!("CARGO_PKG_VERSION")));
    path
}

/// Remove the cache directory for the default namespace.
pub fn delete_cache_dir() -> std::io::Result<()> {
    let path = resolve_db_file_path();
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;
    use std::sync::{LazyLock, Mutex, OnceLock};

    static LOADER_APPS: LazyLock<Mutex<Vec<AppDescriptor>>> =
        LazyLock::new(|| Mutex::new(Vec::new()));
    static CACHE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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

    fn prepare_test_cache() -> std::sync::MutexGuard<'static, ()> {
        let guard = CACHE_LOCK.lock().expect("lock cache tests");
        set_test_cache_home();
        let path = resolve_db_file_path();
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
        guard
    }

    fn empty_loader() -> Vec<AppDescriptor> {
        Vec::new()
    }

    fn shared_loader() -> Vec<AppDescriptor> {
        LOADER_APPS.lock().expect("lock loader apps").clone()
    }

    fn make_app(
        appid: &str,
        title: &str,
        exec_count: usize,
        icon_path: Option<PathBuf>,
    ) -> AppDescriptor {
        AppDescriptor {
            appid: appid.to_string(),
            title: title.to_string(),
            lower_title: title.to_lowercase(),
            exec: "/bin/true".to_string(),
            exec_count,
            icon_name: None,
            icon_path,
            icon_handle: IconHandle::NotLoaded,
        }
    }

    #[test]
    fn test_cache_reads_icons_as_rgba() {
        let _guard = prepare_test_cache();
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

    #[test]
    fn test_write_snapshot_sorts_by_count_then_title() {
        let _guard = prepare_test_cache();
        let mut cache = Cache::new(empty_loader);
        let apps = vec![
            make_app("app-1", "Zoo", 5, None),
            make_app("app-2", "Alpha", 5, None),
            make_app("app-3", "Beta", 2, None),
        ];

        cache.store_snapshot(&apps).expect("store snapshot");
        let apps = cache.read_all().expect("read snapshot");

        let titles: Vec<&str> = apps.iter().map(|app| app.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Zoo", "Beta"]);
    }

    #[test]
    fn test_refresh_preserves_count_and_cached_icon_data() {
        let _guard = prepare_test_cache();
        let cache_home = set_test_cache_home();
        let icon_path = cache_home.join("test-refresh-icon.png");
        let mut png_bytes = Vec::new();
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode refresh icon");
        std::fs::write(&icon_path, &png_bytes).expect("write refresh icon");

        let mut cache = Cache::new(shared_loader);
        let initial_app = make_app("app-1", "Cached App", 3, Some(icon_path.clone()));
        cache
            .build_snapshot_with_icons(&[initial_app.clone()])
            .expect("seed cache");

        let refreshed_app = AppDescriptor {
            icon_path: None,
            exec_count: 0,
            ..initial_app
        };
        *LOADER_APPS.lock().expect("lock loader apps") = vec![refreshed_app];

        cache.refresh().expect("refresh cache");
        let apps = cache.read_all().expect("read snapshot");

        assert_eq!(apps[0].exec_count, 3);
        assert_eq!(apps[0].icon_path.as_ref(), Some(&icon_path));
        assert!(matches!(apps[0].icon_handle, IconHandle::Raster(_)));
    }

    #[test]
    fn test_refresh_drops_missing_apps() {
        let _guard = prepare_test_cache();
        let mut cache = Cache::new(shared_loader);
        let apps = vec![
            make_app("app-1", "Keep", 1, None),
            make_app("app-2", "Drop", 2, None),
        ];
        cache.store_snapshot(&apps).expect("store snapshot");

        *LOADER_APPS.lock().expect("lock loader apps") = vec![make_app("app-1", "Keep", 0, None)];
        cache.refresh().expect("refresh cache");

        let apps = cache.read_all().expect("read snapshot");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].appid, "app-1");
    }

    #[test]
    fn test_legacy_decode_normalizes_titles() {
        let _guard = prepare_test_cache();
        let mut cache = Cache::new(empty_loader);
        let app = AppDescriptor {
            appid: "legacy-app".to_string(),
            title: "Legacy App".to_string(),
            lower_title: String::new(),
            exec: "/bin/true".to_string(),
            exec_count: 1,
            icon_name: None,
            icon_path: None,
            icon_handle: IconHandle::NotLoaded,
        };
        let encoded = bincode::serialize(&app).expect("serialize legacy app");
        cache
            .db
            .insert(0_u32.to_be_bytes(), IVec::from(encoded))
            .expect("insert legacy entry");
        cache.db.flush().expect("flush legacy entry");

        let apps = cache.read_all().expect("read snapshot");
        assert_eq!(apps[0].lower_title, "legacy app");
        assert!(matches!(apps[0].icon_handle, IconHandle::NotLoaded));
    }

    #[test]
    fn test_read_all_populates_cache_on_first_run() {
        let _guard = prepare_test_cache();
        *LOADER_APPS.lock().expect("lock loader apps") =
            vec![make_app("app-1", "First Run", 0, None)];

        let mut cache = Cache::new(shared_loader);
        let apps = cache.read_all().expect("read snapshot");

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].appid, "app-1");
        assert!(!cache.is_empty());
    }
}
