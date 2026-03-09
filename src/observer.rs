pub trait RunObserver {
    /// Called after a folder is successfully cleaned.
    /// Returns an optional hint string (e.g. "🏆 new highscore!") to display inline.
    fn on_folder_cleaned(&mut self, size: u64) -> Option<String>;

    /// Called after the entire run completes.
    /// Returns an optional string with medal ASCII art to display.
    fn on_run_complete(&mut self, total: u64) -> Option<String>;
}

pub struct NoOpObserver;

impl RunObserver for NoOpObserver {
    fn on_folder_cleaned(&mut self, _size: u64) -> Option<String> {
        None
    }

    fn on_run_complete(&mut self, _total: u64) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_observer_returns_none() {
        let mut observer = NoOpObserver;
        assert!(observer.on_folder_cleaned(1024).is_none());
        assert!(observer.on_run_complete(2048).is_none());
    }
}
