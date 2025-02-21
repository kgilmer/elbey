use sled::{Config, Db, IVec};

use crate::app::AppDescriptor;

pub(crate) struct Cache {
    apps_loader: fn() -> Vec<AppDescriptor>,
    db: Db,
}

impl Cache {
    pub fn new(apps_loader: fn() -> Vec<AppDescriptor>) -> Self {
        let config = Config::new().path(db_filename());
        let db = config.open().unwrap();

        Cache { apps_loader, db }
    }

    pub fn read_all(&self) -> Option<Vec<AppDescriptor>> {
        if self.db.is_empty() {
            return None;
        }

        let scan_key = 0_i32.to_be_bytes();
        let iter = self.db.range(scan_key..);

        let mut app_descriptors: Vec<AppDescriptor> = vec![];
        for item in iter {
            let (_, desc_ivec) = item.ok()?;

            let app_descriptor: AppDescriptor = bincode::deserialize(&desc_ivec[..]).ok()?;

            app_descriptors.push(app_descriptor);
        }

        Some(app_descriptors)
    }

    pub fn update(&mut self, selected_app: &AppDescriptor) -> anyhow::Result<()> {
        // load data
        let latest_entries = (self.apps_loader)();
        let cached_entry_wrappers = self.read_all();

        // create new wrapper vec
        let mut updated_entry_wrappers: Vec<AppDescriptor> =
            Vec::with_capacity(latest_entries.len());

        for mut latest_entry in latest_entries {
            let count = if let Some(ref entry_wrappers) = cached_entry_wrappers {
                Cache::find_count(&latest_entry.appid, entry_wrappers).unwrap_or(0)
            } else {
                0
            };

            latest_entry.exec_count = if latest_entry.appid == selected_app.appid {
                count + 1
            } else {
                count
            } ;

            updated_entry_wrappers.push(latest_entry);
        }

        // sort
        updated_entry_wrappers.sort_by(|a, b| a.title.cmp(&b.title));
        updated_entry_wrappers.sort_by(|a, b| b.exec_count.cmp(&a.exec_count));

        // store
        self.db.clear()?; // Flush previous cache for new snapshot
        for (count, app_descriptor) in updated_entry_wrappers.into_iter().enumerate() {
            let encoded: Vec<u8> = bincode::serialize(&app_descriptor)?;
            self.db.insert(count.to_be_bytes(), IVec::from(encoded))?;
        }

        self.db.flush()?;
        Ok(())
    }

    fn find_count(app_id: &String, entries: &Vec<AppDescriptor>) -> Option<usize> {
        for ew in entries {
            if ew.appid == *app_id {
                return Some(ew.exec_count);
            }
        }
        None
    }
}

fn db_filename() -> String {
    String::from("/tmp/elbey.bin")
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn test_something() {
        assert!(true);
    }
}
