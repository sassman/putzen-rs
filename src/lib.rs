mod cleaner;
mod decider;

pub use crate::cleaner::*;
pub use crate::decider::*;

use jwalk::{ClientState, DirEntry, Parallelism};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Display, Formatter};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

pub struct FileToFolderMatch {
    file_to_check: &'static str,
    folder_to_remove: &'static str,
}

pub enum FolderProcessed {
    Cleaned(usize),
    Skipped,
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

#[derive(Debug)]
pub struct Folder(PathBuf);

impl Folder {
    pub fn accept(
        &self,
        ctx: &DecisionContext,
        rule: &FileToFolderMatch,
        cleaner: &impl DoCleanUp,
        decider: &mut impl Decide,
    ) -> Result<FolderProcessed> {
        if let Ok(folder_to_remove) = rule.resolve_path_to_remove(self) {
            let size_amount = folder_to_remove.calculate_size();
            let size = size_amount.as_human_readable();
            println!("{} ({})", folder_to_remove, size);
            println!("  ├─ because of ../{}", rule.file_to_check);

            let result = match decider.obtain_decision(ctx, "├─ delete directory recursively?")
            {
                Ok(Decision::Yes) => match cleaner.do_cleanup(folder_to_remove)? {
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
        } else {
            Err(Error::from(ErrorKind::Unsupported))
        }
    }

    fn calculate_size(&self) -> usize {
        jwalk::WalkDirGeneric::<((), Option<usize>)>::new(self.0.as_path())
            .skip_hidden(false)
            .follow_links(false)
            .parallelism(Parallelism::RayonNewPool(0))
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
            .filter(|f| f.is_ok())
            .map(|e| e.unwrap().client_state)
            .flatten()
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
            Ok(Self(path))
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

pub trait PathToRemoveResolver {
    fn resolve_path_to_remove(&self, folder: impl AsRef<Path>) -> Result<Folder>;
}

impl PathToRemoveResolver for FileToFolderMatch {
    fn resolve_path_to_remove(&self, folder: impl AsRef<Path>) -> Result<Folder> {
        let folder = folder.as_ref();
        let file_to_check = folder.join(self.file_to_check);

        if file_to_check.exists() {
            let path_to_remove = folder.join(self.folder_to_remove);
            if path_to_remove.exists() {
                return Ok(Folder(path_to_remove));
            }
        }

        Err(Error::from(ErrorKind::Unsupported))
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
    fn should_tell_if_folder_is_to_remove() {
        let rule = FileToFolderMatch::new("Cargo.toml", "target");
        let folder = Folder::try_from(Path::new("..").join("..")).unwrap();

        assert!(rule.resolve_path_to_remove(folder).is_err());

        let folder = Folder::try_from(Path::new(".").canonicalize().unwrap()).unwrap();
        assert!(rule.resolve_path_to_remove(folder).is_ok());
    }

    #[test]
    fn should_size() {
        assert_eq!(1_048_576, 1024 << 10);
    }
}
