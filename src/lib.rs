use std::convert::{TryFrom, TryInto};
use std::fs::DirEntry;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct FileToFolderMatch {
    file_to_check: &'static str,
    folder_to_remove: &'static str,
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

pub struct Folder(PathBuf);

impl Folder {
    pub fn accept(&self, cleaner: &impl CleanUpAction) -> Result<()> {
        if self.0.is_dir() && cleaner.is_supported(&self.0).is_ok() {
            cleaner.do_cleanup(&self.0)
        } else {
            Err(Error::from(ErrorKind::Unsupported))
        }
    }
}

impl AsRef<Path> for Folder {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl TryFrom<DirEntry> for Folder {
    type Error = std::io::Error;

    fn try_from(value: DirEntry) -> std::result::Result<Self, Self::Error> {
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

pub trait CleanUpAction {
    fn is_supported(&self, folder: impl AsRef<Path>) -> Result<()>;
    fn do_cleanup(&self, folder: impl AsRef<Path>) -> Result<()>;
}

impl CleanUpAction for FileToFolderMatch {
    fn is_supported(&self, folder: impl AsRef<Path>) -> Result<()> {
        let folder = folder.as_ref();
        let file_to_check = folder.join(self.file_to_check);
        let path_to_remove = self.path_to_remove(folder);

        if file_to_check.exists()
            && path_to_remove.is_some()
            && path_to_remove.unwrap().as_ref().exists()
        {
            Ok(())
        } else {
            Err(Error::from(ErrorKind::Unsupported))
        }
    }

    fn do_cleanup(&self, folder: impl AsRef<Path>) -> Result<()> {
        if let Some(folder_to_remove) = self.path_to_remove(folder) {
            let folder_to_remove = folder_to_remove.as_ref();
            println!("echo rm -rf {}", folder_to_remove.display());
            Command::new("echo")
                .args(&["rm", "-rf", folder_to_remove.to_str().unwrap()])
                .output()
                .map_err(|_| Error::from(ErrorKind::Other))
                .map(|_| ())
        } else {
            Err(Error::from(ErrorKind::Unsupported))
        }
    }
}

pub struct DryRun<'a>(&'a FileToFolderMatch);

impl<'a> DryRun<'a> {
    pub fn wrap(to_wrap: &'a FileToFolderMatch) -> Self {
        Self(to_wrap)
    }
}

impl<'a> CleanUpAction for DryRun<'a> {
    /// delegating
    fn is_supported(&self, folder: impl AsRef<Path>) -> Result<()> {
        self.0.is_supported(folder)
    }

    /// dry run means do nothing but printing
    fn do_cleanup(&self, folder: impl AsRef<Path>) -> Result<()> {
        let folder_to_remove = self.0.path_to_remove(folder).unwrap();
        println!("# rm -rf {}", folder_to_remove.as_ref().display());

        Ok(())
    }
}
