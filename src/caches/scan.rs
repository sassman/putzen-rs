//! Filesystem walk: enumerate seeds → ranked `Cache` entries.

use crate::caches::model::{Cache, TopFile};
use jwalk::WalkDir;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const TOP_K: usize = 64;

/// Walk a single directory and aggregate its size, newest mtime, and counts.
/// Symlinks are not followed. Permission errors are silenced.
pub fn stat_dir(root: &Path) -> Cache {
    let mut size_bytes = 0u64;
    let mut newest = None::<SystemTime>;
    let mut file_count = 0u64;
    let mut dir_count = 0u64;
    let mut unreadable = 0u64;
    let mut heap: BinaryHeap<Reverse<(u64, String, Option<SystemTime>)>> = BinaryHeap::new();

    // skip_hidden(false): cache directories often signal "fresh use" via
    // dotfiles (`.lock`, `.tmp`, `.index`); excluding them would shift
    // newest_mtime onto the OLDER visible files and make active caches
    // look dormant.
    for entry in WalkDir::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .into_iter()
        .flatten()
    {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        if meta.is_dir() {
            dir_count += 1;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        file_count += 1;
        size_bytes += meta.len();
        let file_mtime = meta.modified().ok();
        if let Some(m) = file_mtime {
            newest = Some(newest.map_or(m, |prev| prev.max(m)));
        }
        let name = entry.file_name().to_string_lossy().to_string();
        heap.push(Reverse((meta.len(), name, file_mtime)));
        if heap.len() > TOP_K {
            heap.pop();
        }
    }

    // dir_count includes `root` itself; subtract.
    let dir_count = dir_count.saturating_sub(1);

    let label = root
        .file_name()
        .map(|s| s.to_string_lossy().trim_start_matches('.').to_string())
        .unwrap_or_default();

    let mut top_files: Vec<TopFile> = heap
        .into_iter()
        .map(|Reverse((size, name, mtime))| TopFile { name, size_bytes: size, mtime })
        .collect();
    top_files.sort_by_key(|f| Reverse(f.size_bytes));

    Cache {
        label,
        path: root.to_path_buf(),
        size_bytes,
        newest_mtime: newest,
        file_count,
        dir_count,
        top_files,
        unreadable,
    }
}

/// Enumerate immediate children of `seed`; each becomes one `Cache`.
/// Non-existent or non-directory seeds yield an empty vec.
pub fn enumerate_seed(seed: &Path) -> Vec<Cache> {
    let Ok(read) = std::fs::read_dir(seed) else { return Vec::new() };
    read.flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| stat_dir(&e.path()))
        .collect()
}

/// Walk every seed and concatenate, de-duplicating by canonicalised absolute
/// path. Order is preserved (first occurrence wins).
pub fn collect(seeds: &[PathBuf]) -> Vec<Cache> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in seeds {
        let Ok(canonical) = s.canonicalize() else { continue };
        for c in enumerate_seed(&canonical) {
            let canon = c.path.canonicalize().unwrap_or_else(|_| c.path.clone());
            if seen.insert(canon) {
                out.push(c);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn stat_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let c = stat_dir(tmp.path());
        assert_eq!(c.size_bytes, 0);
        assert_eq!(c.file_count, 0);
        assert_eq!(c.dir_count, 0);
        assert!(c.newest_mtime.is_none());
    }

    #[test]
    fn stat_sums_sizes_and_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        File::create(tmp.path().join("a/one")).unwrap().write_all(&[0u8; 100]).unwrap();
        File::create(tmp.path().join("a/b/two")).unwrap().write_all(&[0u8; 200]).unwrap();

        let c = stat_dir(tmp.path());
        assert_eq!(c.size_bytes, 300);
        assert_eq!(c.file_count, 2);
        // a/ and a/b/ are 2 dirs (root is subtracted)
        assert_eq!(c.dir_count, 2);
        assert!(c.newest_mtime.is_some());
    }

    #[test]
    fn newest_mtime_picks_max_across_files() {
        let tmp = tempfile::tempdir().unwrap();
        // Older file
        let old = tmp.path().join("old");
        File::create(&old).unwrap().write_all(&[0u8; 10]).unwrap();
        // Newer file
        let new = tmp.path().join("new");
        File::create(&new).unwrap().write_all(&[0u8; 10]).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(86_400),
        )).ok();
        filetime::set_file_mtime(&new, filetime::FileTime::from_system_time(later)).ok();

        let c = stat_dir(tmp.path());
        // The youngest file's mtime wins.
        let nm = c.newest_mtime.expect("expected a newest_mtime");
        assert!(nm >= later - std::time::Duration::from_secs(1));
    }

    #[test]
    fn hidden_files_count_toward_newest_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        // One visible old file, one hidden recent file. If skip_hidden defaulted
        // to true the hidden file would not contribute and newest_mtime would
        // be the old file. With our explicit skip_hidden(false) the hidden
        // file's recent mtime wins.
        let old = tmp.path().join("old");
        File::create(&old).unwrap().write_all(&[0u8; 1]).unwrap();
        filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(60),
        )).ok();

        let hidden = tmp.path().join(".lock");
        File::create(&hidden).unwrap().write_all(&[0u8; 1]).unwrap();
        let later = std::time::SystemTime::now();
        filetime::set_file_mtime(&hidden, filetime::FileTime::from_system_time(later)).ok();

        let c = stat_dir(tmp.path());
        let nm = c.newest_mtime.expect("expected a newest_mtime");
        // newest_mtime is from the hidden file (today), not the visible 1970 one.
        assert!(nm > std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3600 * 24 * 365));
    }

    #[test]
    fn label_strips_leading_dot() {
        let tmp = tempfile::tempdir().unwrap();
        let hidden = tmp.path().join(".npm");
        fs::create_dir(&hidden).unwrap();
        let c = stat_dir(&hidden);
        assert_eq!(c.label, "npm");
    }

    #[test]
    fn enumerate_returns_immediate_children() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        fs::create_dir(tmp.path().join("beta")).unwrap();
        File::create(tmp.path().join("alpha/file")).unwrap().write_all(&[0u8; 50]).unwrap();

        let mut caches = super::enumerate_seed(tmp.path());
        caches.sort_by(|a, b| a.label.cmp(&b.label));
        let labels: Vec<_> = caches.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["alpha", "beta"]);
    }

    #[test]
    fn enumerate_seed_skips_missing() {
        let path = std::path::PathBuf::from("/nonexistent/putzen/should/never/exist");
        assert!(super::enumerate_seed(&path).is_empty());
    }

    #[test]
    fn top_files_lists_largest_files_sorted_desc() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path()).unwrap();
        fs::write(tmp.path().join("small"), [0u8; 10]).unwrap();
        fs::write(tmp.path().join("big"), [0u8; 1_000_000]).unwrap();
        fs::write(tmp.path().join("medium"), [0u8; 5_000]).unwrap();
        let c = stat_dir(tmp.path());
        let names: Vec<_> = c.top_files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["big", "medium", "small"]);
    }

    #[test]
    fn top_files_capped_at_64() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path()).unwrap();
        for i in 0..100 {
            fs::write(tmp.path().join(format!("f{:03}", i)), vec![0u8; (i + 1) as usize]).unwrap();
        }
        let c = stat_dir(tmp.path());
        assert_eq!(c.top_files.len(), 64);
        // largest one ("f099" with 100 bytes) must be present
        assert!(c.top_files.iter().any(|f| f.name == "f099"));
    }

    #[test]
    fn collect_dedups_by_canonical_path() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("alpha")).unwrap();
        // pass the same seed twice
        let seeds = vec![tmp.path().to_path_buf(), tmp.path().to_path_buf()];
        let caches = super::collect(&seeds);
        assert_eq!(caches.len(), 1, "duplicate seed should yield one cache");
        assert_eq!(caches[0].label, "alpha");
    }
}
