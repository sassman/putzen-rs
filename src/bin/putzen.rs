use std::convert::TryFrom;
use std::io::Result;
use std::path::PathBuf;

use argh::FromArgs;
use jwalk::Parallelism;

use putzen_cli::{
    DecisionContext, DoCleanUp, DryRunCleaner, FileToFolderMatch, Folder, FolderProcessed,
    HumanReadable, IsFolderToRemove, NiceInteractiveDecider, ProperCleaner,
};

/// all supported this to clean up
static FOLDER_TO_CLEANUP: [FileToFolderMatch; 3] = [
    FileToFolderMatch::new("Cargo.toml", "target"),
    FileToFolderMatch::new("package.json", "node_modules"),
    FileToFolderMatch::new("CMakeLists.txt", "build"),
];

#[derive(FromArgs)]
/// help keeping your disk clean of build and dependency artifacts
struct PutzenCliArgs {
    /// show the version number
    #[argh(switch, short = 'v')]
    version: bool,

    /// dry-run will never delete anything, good for simulations
    #[argh(switch, short = 'd')]
    dry_run: bool,

    /// switch to say yes to all questions
    #[argh(switch, short = 'y')]
    yes_to_all: bool,

    /// follow symbolic links
    #[argh(switch, short = 'L')]
    follow: bool,

    /// dive into hidden folders too, e.g. `.git`
    #[argh(switch, short = 'a')]
    dive_into_hidden_folders: bool,

    /// path where to start with disk clean up.
    #[argh(positional, default = "PathBuf::from(\".\")")]
    folder: PathBuf,
}

fn main() -> Result<()> {
    let args: PutzenCliArgs = argh::from_env();
    if args.version {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        Ok(())
    } else {
        visit_path(&args)
    }
}

fn visit_path(args: &PutzenCliArgs) -> Result<()> {
    let to_clean = &FOLDER_TO_CLEANUP;
    let mut decider = NiceInteractiveDecider::default();
    let mut amount_cleaned = 0;
    let folder = args
        .folder
        .canonicalize()
        .expect("Folder cannot be canonicalized.");
    let ctx = DecisionContext {
        working_dir: folder.clone(),
        is_dry_run: args.dry_run,
        yes_to_all: args.yes_to_all,
    };

    let cleaner: Box<dyn DoCleanUp> = if args.dry_run {
        Box::new(DryRunCleaner)
    } else {
        Box::new(ProperCleaner)
    };

    ctx.println(format!("Start cleaning at {}", folder.display()));
    for folder in jwalk::WalkDirGeneric::<((), Option<Folder>)>::new(folder)
        .skip_hidden(!args.dive_into_hidden_folders)
        .follow_links(args.follow)
        .parallelism(Parallelism::RayonNewPool(8))
        .process_read_dir(move |_, _, _, children| {
            children.retain(|dir_entry_result| {
                dir_entry_result
                    .as_ref()
                    .map(|dir| dir.path().is_dir())
                    .unwrap_or(false)
            });

            children.iter_mut().for_each(|child| {
                if let Ok(child) = child {
                    if let Ok(folder) = Folder::try_from(child.path()) {
                        for rule in to_clean {
                            if rule.is_folder_to_remove(&folder) {
                                child.client_state = Some(folder);
                                child.read_children_path = None;
                                return;
                            }
                        }
                    }
                }
            });
        })
        .into_iter()
        .filter_map(|f| f.ok())
        .filter_map(|f| f.client_state)
    {
        'rules: for rule in to_clean {
            let result = folder.accept(&ctx, rule, &*cleaner, &mut decider);
            match result {
                Ok(FolderProcessed::Abort) => return Ok(()),
                Ok(FolderProcessed::Cleaned(size)) => {
                    amount_cleaned += size;
                    continue 'rules;
                }
                Ok(FolderProcessed::NoRuleMatch) => continue 'rules,
                Ok(FolderProcessed::Skipped) => continue 'rules,
                Err(error) => return Err(error),
            };
        }
    }

    if amount_cleaned > 0 {
        ctx.println(format!("Freed: {}", amount_cleaned.as_human_readable()));
    } else {
        ctx.println("No space freed ;-(");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_scenario() {
        let root_folder = tempfile::TempDir::new().unwrap();
        let target_folder = root_folder.path().join("target");
        std::fs::create_dir(&target_folder).unwrap();
        std::fs::File::create(root_folder.path().join("Cargo.toml")).unwrap();

        // create a target folder with one simple file in it
        std::fs::File::create(target_folder.join("some_artefact")).unwrap();

        // create also a node case in the root folder
        let node_modules_folder = root_folder.path().join("node_modules");
        std::fs::create_dir(&node_modules_folder).unwrap();
        std::fs::File::create(root_folder.path().join("package.json")).unwrap();
        std::fs::File::create(node_modules_folder.join("some_artefact")).unwrap();

        // now we create a nested node case inside the root folder
        let second_node_root_folder = root_folder.path().join("bar");
        std::fs::create_dir(&second_node_root_folder).unwrap();
        let nested_node_modules_folder = second_node_root_folder.join("node_modules");
        std::fs::create_dir(&nested_node_modules_folder).unwrap();
        std::fs::File::create(second_node_root_folder.join("package.json")).unwrap();
        std::fs::File::create(nested_node_modules_folder.join("some_artefact")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: false,
            folder: root_folder.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(!target_folder.exists());
        assert!(!node_modules_folder.exists());
        assert!(!nested_node_modules_folder.exists());

        assert!(root_folder.path().join("Cargo.toml").exists());
        assert!(root_folder.path().join("package.json").exists());
        assert!(second_node_root_folder.join("package.json").exists());
    }
}
