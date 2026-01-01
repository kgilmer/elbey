use std::path::PathBuf;

use sled::{Config, Db, IVec};

use crate::app::AppDescriptor;

static SCAN_KEY: [u8; 4] = 0_i32.to_be_bytes();

/// Tracks state to sort apps by usage
pub(crate) struct Cache {
    apps_loader: fn() -> Vec<AppDescriptor>,
    db: Db,
}

fn find_cached_entry<'a>(
    app_id: &String,
    entries: &'a [AppDescriptor],
) -> Option<&'a AppDescriptor> {
    entries.iter().find(|entry| entry.appid == *app_id)
}

impl Cache {
    pub fn new(apps_loader: fn() -> Vec<AppDescriptor>) -> Self {
        let config = Config::new().path(Self::resolve_db_file_path());
        let db = config.open().unwrap();

        Cache { apps_loader, db }
    }

    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    pub fn read_all(&self) -> Option<Vec<AppDescriptor>> {
        let iter = self.db.range(SCAN_KEY..);

        let mut app_descriptors: Vec<AppDescriptor> = vec![];
        for item in iter {
            let (_, desc_ivec) = item.ok()?;

            let mut app_descriptor: AppDescriptor = bincode::deserialize(&desc_ivec[..]).ok()?;
            if app_descriptor.lower_title.is_empty() {
                app_descriptor.lower_title = app_descriptor.title.to_lowercase();
            }

            app_descriptors.push(app_descriptor);
        }

        Some(app_descriptors)
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        let _ = self.db.clear();
        let _ = self.db.flush();
    }

    fn write_snapshot(
        &mut self,
        apps: impl IntoIterator<Item = AppDescriptor>,
    ) -> anyhow::Result<()> {
        let mut snapshot: Vec<AppDescriptor> = apps.into_iter().collect();
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
        let cached_entry_wrappers = self.read_all();

        // create new wrapper vec
        let mut updated_entry_wrappers: Vec<AppDescriptor> =
            Vec::with_capacity(latest_entries.len());

        for mut latest_entry in latest_entries {
            let (count, cached_icon_path) = if let Some(ref entry_wrappers) = cached_entry_wrappers
            {
                if let Some(entry) = find_cached_entry(&latest_entry.appid, entry_wrappers) {
                    (entry.exec_count, entry.icon_path.clone())
                } else {
                    (0, None)
                }
            } else {
                (0, None)
            };

            latest_entry.exec_count = if latest_entry.appid == selected_app.appid {
                count + 1
            } else {
                count
            };
            latest_entry.icon_path = cached_icon_path.or(latest_entry.icon_path);

            updated_entry_wrappers.push(latest_entry);
        }

        // sort
        self.write_snapshot(updated_entry_wrappers)
    }

    pub fn store_snapshot(&mut self, apps: &[AppDescriptor]) -> anyhow::Result<()> {
        self.write_snapshot(apps.to_vec())
    }
    fn resolve_db_file_path() -> PathBuf {
        let mut path = dirs::cache_dir().unwrap();
        path.push(format!(
            "{}-{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ));
        path
    }
}
