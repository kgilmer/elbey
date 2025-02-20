use freedesktop_desktop_entry::{get_languages_from_env, DesktopEntry};
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
        
        return Cache { 
            apps_loader,
            db,
        };
    }

    pub fn read_all(&self) -> Option<Vec<AppDescriptor>> {
        todo!();
    }

    pub fn update(&mut self, _entry: &AppDescriptor) -> anyhow::Result<()> {
        // load data
        let latest_entries = (self.apps_loader)();
        let cached_entry_wrappers = self.read_all_native();

        // create new wrapper vec
        let mut updated_entry_wrappers: Vec<AppDescriptor> =
            Vec::with_capacity(latest_entries.len());

        for e in latest_entries {
            let count = if let Some(ref entry_wrappers) = cached_entry_wrappers {
                Cache::find_count(&e.appid, &entry_wrappers).unwrap_or(0)
            } else {
                0
            };

            updated_entry_wrappers.push(AppDescriptor {
                desktop_entry: e,
                exec_count: count,
            });
        }

        // sort
        let locales = get_languages_from_env();
        updated_entry_wrappers.sort_by(|a, b| {
            a.desktop_entry
                .name(&locales)
                .cmp(&b.desktop_entry.name(&locales))
        });
        updated_entry_wrappers.sort_by(|a, b| a.exec_count.cmp(&b.exec_count));

        // store
        let mut count: usize = 0;
        for w in updated_entry_wrappers {
            // self.db.insert(count.to_be_bytes(), w)?;
            count += 1;
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
    use super::*;

    #[test]
    fn test_something() {
        assert!(true);
    }
}
