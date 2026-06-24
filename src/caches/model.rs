//! Pure data types for the cache TUI.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug)]
pub struct Cache {
    /// Display name (label derived at scan time).
    pub label: String,
    /// Absolute, canonical path.
    pub path: PathBuf,
    pub size_bytes: u64,
    pub newest_mtime: Option<SystemTime>,
    pub file_count: u64,
    pub dir_count: u64,
    /// Top-N largest files inside this cache, sorted by descending size.
    /// Bounded so we don't keep millions for ~/.cache/huggingface; 64 is plenty.
    pub top_files: Vec<TopFile>,
    /// Count of dir entries that could not be read (permission, dangling symlink).
    pub unreadable: u64,
}

#[derive(Clone, Debug)]
pub struct TopFile {
    pub name: String,
    pub size_bytes: u64,
    pub mtime: Option<SystemTime>,
}

impl Cache {
    /// Duration since the newest file was touched. `None` for empty caches.
    pub fn age(&self, now: SystemTime) -> Option<Duration> {
        let mtime = self.newest_mtime?;
        now.duration_since(mtime).ok().or(Some(Duration::ZERO))
    }

    /// Score: (size_MB) × (age_days). 0.0 for empty caches.
    pub fn score(&self, now: SystemTime) -> f64 {
        let Some(age) = self.age(now) else { return 0.0 };
        let mb = self.size_bytes as f64 / 1_048_576.0;
        let days = age.as_secs_f64() / 86_400.0;
        mb * days
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Sort {
    Score,
    Size,
    Age,
}

impl Sort {
    pub fn next(self) -> Sort {
        match self {
            Sort::Score => Sort::Size,
            Sort::Size => Sort::Age,
            Sort::Age => Sort::Score,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MarkSet {
    /// Indices into the current sorted list.
    pub marked: std::collections::BTreeSet<usize>,
}

impl MarkSet {
    pub fn toggle(&mut self, i: usize) {
        if !self.marked.insert(i) {
            self.marked.remove(&i);
        }
    }
    pub fn mark_down_to(&mut self, last: usize) {
        for i in 0..=last {
            self.marked.insert(i);
        }
    }
    pub fn clear(&mut self) {
        self.marked.clear();
    }
    pub fn is_marked(&self, i: usize) -> bool {
        self.marked.contains(&i)
    }
    pub fn count(&self) -> usize {
        self.marked.len()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FloorPolicy {
    pub floor: Duration,
}

impl FloorPolicy {
    pub fn is_active(&self, age: Option<Duration>) -> bool {
        age.map(|a| a < self.floor).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn at(epoch_secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(epoch_secs)
    }

    fn cache(size: u64, mtime_secs: u64) -> Cache {
        Cache {
            label: "x".into(),
            path: PathBuf::from("/tmp/x"),
            size_bytes: size,
            newest_mtime: Some(at(mtime_secs)),
            file_count: 1,
            dir_count: 0,
            top_files: Vec::new(),
            unreadable: 0,
        }
    }

    #[test]
    fn score_zero_for_empty_cache() {
        let mut c = cache(1024, 0);
        c.newest_mtime = None;
        assert_eq!(c.score(at(86_400)), 0.0);
    }

    #[test]
    fn score_proportional_to_mb_days() {
        let now = at(2 * 86_400);
        let c = cache(1_048_576, 0); // 1 MB, 2 days cold
        assert!((c.score(now) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn sort_cycles() {
        assert_eq!(Sort::Score.next(), Sort::Size);
        assert_eq!(Sort::Size.next(), Sort::Age);
        assert_eq!(Sort::Age.next(), Sort::Score);
    }

    #[test]
    fn markset_toggle_inserts_and_removes() {
        let mut m = MarkSet::default();
        m.toggle(3);
        assert!(m.is_marked(3));
        m.toggle(3);
        assert!(!m.is_marked(3));
    }

    #[test]
    fn markset_mark_down_to_inclusive() {
        let mut m = MarkSet::default();
        m.mark_down_to(2);
        assert_eq!(m.count(), 3);
        for i in 0..=2 {
            assert!(m.is_marked(i));
        }
    }

    #[test]
    fn floor_active_when_cold_less_than_floor() {
        let p = FloorPolicy {
            floor: Duration::from_secs(7 * 86_400),
        };
        assert!(p.is_active(Some(Duration::from_secs(86_400))));
        assert!(!p.is_active(Some(Duration::from_secs(30 * 86_400))));
        assert!(!p.is_active(None));
    }
}
