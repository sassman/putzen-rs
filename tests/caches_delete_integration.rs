//! Integration test: build a fake cache tree, run `scan::collect`, then
//! simulate the delete path the run loop would take.

use putzen_cli::caches::scan;
use std::fs::{self, File};
use std::io::Write;

#[test]
fn collect_then_delete_marked_removes_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_root = tmp.path().join("seed");
    fs::create_dir(&cache_root).unwrap();
    let target = cache_root.join("victim");
    fs::create_dir(&target).unwrap();
    File::create(target.join("blob")).unwrap().write_all(&[0u8; 1024]).unwrap();

    let mut caches = scan::collect(std::slice::from_ref(&cache_root));
    assert_eq!(caches.len(), 1);
    assert!(caches[0].path.ends_with("victim"));

    let to_delete = caches.remove(0).path;
    fs::remove_dir_all(&to_delete).unwrap();
    assert!(!to_delete.exists());
}
