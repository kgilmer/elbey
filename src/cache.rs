use freedesktop_desktop_entry::DesktopEntry;


pub(crate) struct Cache;

impl Cache {
    pub fn new() -> Self {
        return Cache {};
    }
    
    pub fn read_all(&self) -> Option<Vec<DesktopEntry>> {
        todo!();
    }

    pub fn update(&mut self, _entries: &Vec<DesktopEntry>) -> anyhow::Result<()> {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert!(true);
    }
}