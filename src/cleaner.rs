#[cfg(target_family = "windows")]
use remove_dir_all::remove_dir_all;

#[cfg(not(target_family = "windows"))]
use std::fs::remove_dir_all;

use std::io::Result;
use std::path::Path;

pub enum Clean {
    Cleaned,
    NotCleaned,
}

pub trait DoCleanUp {
    fn do_cleanup(&self, path_to_remove: impl AsRef<Path>) -> Result<Clean>;
}

#[derive(Default)]
pub struct ProperCleaner;
impl DoCleanUp for ProperCleaner {
    fn do_cleanup(&self, path_to_remove: impl AsRef<Path>) -> Result<Clean> {
        remove_dir_all(path_to_remove.as_ref()).map(|_| Clean::Cleaned)
    }
}

#[derive(Default)]
pub struct DryRunCleaner;
impl DoCleanUp for DryRunCleaner {
    /// dry run means do nothing but printing
    fn do_cleanup(&self, _: impl AsRef<Path>) -> Result<Clean> {
        Ok(Clean::NotCleaned)
    }
}
