mod cleaner;
mod decider;

pub use crate::cleaner::*;
pub use crate::decider::*;

use jwalk::{ClientState, DirEntry, Parallelism};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct FileToFolderMatch {
    file_to_check: &'static str,
    folder_to_remove: &'static str,
}

pub enum FolderProcessed {
    /// The folder was cleaned and the amount of bytes removed is given
    Cleaned(usize),
    /// The folder was not cleaned because it did not match any rule
    NoRuleMatch,
    /// The folder was skipped, e.g. user decided to skip it
    Skipped,
    /// The folder was aborted, e.g. user decided to abort the whole process
    Abort,
}

impl FileToFolderMatch {
    pub const fn new(file_to_check: &'static str, folder_to_remove: &'static str) -> Self {
        Self {
            file_to_check,
            folder_to_remove,
        }
    }

    /// builds the absolut path, that is to be removed, in the given folder
    pub fn path_to_remove(&self, folder: impl AsRef<Path>) -> Option<impl AsRef<Path>> {
        folder
            .as_ref()
            .canonicalize()
            .map(|x| x.join(self.folder_to_remove))
            .ok()
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Folder(PathBuf);

impl Folder {
    pub fn accept(
        &self,
        ctx: &DecisionContext,
        rule: &FileToFolderMatch,
        cleaner: &dyn DoCleanUp,
        decider: &mut impl Decide,
    ) -> Result<FolderProcessed> {
        // better double check here
        if !rule.is_folder_to_remove(self) {
            return Ok(FolderProcessed::NoRuleMatch);
        }

        let size_amount = self.calculate_size();
        let size = size_amount.as_human_readable();
        println!("{} ({})", self, size);
        println!(
            "  ├─ because of {}",
            PathBuf::from("..").join(rule.file_to_check).display()
        );

        let result = match decider.obtain_decision(ctx, "├─ delete directory recursively?") {
            Ok(Decision::Yes) => match cleaner.do_cleanup(self.as_ref())? {
                Clean::Cleaned => {
                    println!("  └─ deleted {}", size);
                    FolderProcessed::Cleaned(size_amount)
                }
                Clean::NotCleaned => {
                    println!(
                        "  └─ not deleted{}{}",
                        if ctx.is_dry_run { " [dry-run] " } else { "" },
                        size
                    );
                    FolderProcessed::Skipped
                }
            },
            Ok(Decision::Quit) => {
                println!("  └─ quiting");
                FolderProcessed::Abort
            }
            _ => {
                println!("  └─ skipped");
                FolderProcessed::Skipped
            }
        };
        println!();
        Ok(result)
    }

    fn calculate_size(&self) -> usize {
        jwalk::WalkDirGeneric::<((), Option<usize>)>::new(self.as_ref())
            .skip_hidden(false)
            .follow_links(false)
            .parallelism(Parallelism::RayonDefaultPool {
                busy_timeout: Duration::from_secs(60),
            })
            .process_read_dir(|_, _, _, dir_entry_results| {
                dir_entry_results.iter_mut().for_each(|dir_entry_result| {
                    if let Ok(dir_entry) = dir_entry_result {
                        if !dir_entry.file_type.is_dir() {
                            dir_entry.client_state = Some(
                                dir_entry
                                    .metadata()
                                    .map(|m| m.len() as usize)
                                    .unwrap_or_default(),
                            );
                        }
                    }
                })
            })
            .into_iter()
            .filter_map(|f| f.ok())
            .filter_map(|e| e.client_state)
            .sum()
    }
}

impl Display for Folder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl<A: ClientState> TryFrom<DirEntry<A>> for Folder {
    type Error = std::io::Error;

    fn try_from(value: DirEntry<A>) -> std::result::Result<Self, Self::Error> {
        let path = value.path();
        path.try_into() // see below..
    }
}

impl TryFrom<PathBuf> for Folder {
    type Error = std::io::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        if !path.is_dir() || path.eq(Path::new(".")) || path.eq(Path::new("..")) {
            Err(Error::from(ErrorKind::Unsupported))
        } else {
            let p = path.canonicalize()?;
            Ok(Self(p))
        }
    }
}

impl TryFrom<&str> for Folder {
    type Error = std::io::Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        Folder::try_from(PathBuf::from(value))
    }
}

impl AsRef<Path> for Folder {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

#[deprecated(since = "2.0.0", note = "use trait `IsFolderToRemove` instead")]
pub trait PathToRemoveResolver {
    fn resolve_path_to_remove(&self, folder: impl AsRef<Path>) -> Result<Folder>;
}

#[allow(deprecated)]
impl PathToRemoveResolver for FileToFolderMatch {
    fn resolve_path_to_remove(&self, folder: impl AsRef<Path>) -> Result<Folder> {
        let folder = folder.as_ref();
        let file_to_check = folder.join(self.file_to_check);

        if file_to_check.exists() {
            let path_to_remove = folder.join(self.folder_to_remove);
            if path_to_remove.exists() {
                return path_to_remove.try_into();
            }
        }

        Err(Error::from(ErrorKind::Unsupported))
    }
}

/// Trait to check if a folder should be removed
/// This is the successor of the deprecated `PathToRemoveResolver` and should be used instead.
///
/// The trait is implemented for `FileToFolderMatch` and can be used to check if a folder should be removed
/// according to the rules defined in the `FileToFolderMatch` instance.
pub trait IsFolderToRemove {
    fn is_folder_to_remove(&self, folder: &Folder) -> bool;
}

impl IsFolderToRemove for FileToFolderMatch {
    fn is_folder_to_remove(&self, folder: &Folder) -> bool {
        folder.as_ref().parent().map_or_else(
            || false,
            |parent| {
                parent.join(self.file_to_check).exists()
                    && parent
                        .join(self.folder_to_remove)
                        .starts_with(folder.as_ref())
            },
        )
    }
}

pub trait HumanReadable {
    fn as_human_readable(&self) -> String;
}

impl HumanReadable for usize {
    fn as_human_readable(&self) -> String {
        const KIBIBYTE: usize = 1024;
        const MEBIBYTE: usize = KIBIBYTE << 10;
        const GIBIBYTE: usize = MEBIBYTE << 10;
        const TEBIBYTE: usize = GIBIBYTE << 10;
        const PEBIBYTE: usize = TEBIBYTE << 10;
        const EXBIBYTE: usize = PEBIBYTE << 10;

        let size = *self;
        let (size, symbol) = match size {
            size if size < KIBIBYTE => (size as f64, "B"),
            size if size < MEBIBYTE => (size as f64 / KIBIBYTE as f64, "KiB"),
            size if size < GIBIBYTE => (size as f64 / MEBIBYTE as f64, "MiB"),
            size if size < TEBIBYTE => (size as f64 / GIBIBYTE as f64, "GiB"),
            size if size < PEBIBYTE => (size as f64 / TEBIBYTE as f64, "TiB"),
            size if size < EXBIBYTE => (size as f64 / PEBIBYTE as f64, "PiB"),
            _ => (size as f64 / EXBIBYTE as f64, "EiB"),
        };

        format!("{:.1}{}", size, symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_size() {
        assert_eq!(1_048_576, 1024 << 10);
    }

    #[test]
    fn test_trait_is_folder_to_remove() {
        let rule = FileToFolderMatch::new("Cargo.toml", "target");

        let target_folder =
            Folder::try_from(Path::new(".").canonicalize().unwrap().join("target")).unwrap();
        assert!(rule.is_folder_to_remove(&target_folder));

        let crate_root_folder = Folder::try_from(Path::new(".").canonicalize().unwrap()).unwrap();
        assert!(!rule.is_folder_to_remove(&crate_root_folder));
    }
}
