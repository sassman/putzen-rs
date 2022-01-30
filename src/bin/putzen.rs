use std::convert::TryFrom;
use std::io::{ErrorKind, Result};
use std::path::PathBuf;

use argh::FromArgs;
use jwalk::Parallelism;

use putzen_cli::{
    DecisionContext, DryRunCleaner, FileToFolderMatch, Folder, FolderProcessed, HumanReadable,
    NiceInteractiveDecider, PathToRemoveResolver, ProperCleaner,
};

/// all supported this to clean up
static FOLDER_TO_CLEANUP: [FileToFolderMatch; 3] = [
    FileToFolderMatch::new("Cargo.toml", "target"),
    FileToFolderMatch::new("package.json", "node_modules"),
    FileToFolderMatch::new("CMakeLists.txt", "build"),
];

#[derive(FromArgs)]
/// help keeping your disk clean of build and dependency artifacts
struct PurifyArgs {
    /// dry-run will never delete anything, good for simulations
    #[argh(switch, short = 'd')]
    dry_run: bool,

    /// follow symbolic links
    #[argh(switch, short = 'L')]
    follow: bool,

    /// dive into hidden folders too, e.g. `.git`
    #[argh(switch, short = 'a')]
    dive_into_hidden_folders: bool,

    /// path of where to start with disk clean up.
    #[argh(positional)]
    folder: PathBuf,
}

fn main() -> Result<()> {
    visit_path(&argh::from_env())
}

fn visit_path(args: &PurifyArgs) -> Result<()> {
    let to_clean = &FOLDER_TO_CLEANUP;
    // let mut decider = InteractiveDecisionWithMemory::default();
    let mut decider = NiceInteractiveDecider::default();
    let mut amount_cleaned = 0;
    let ctx = DecisionContext {
        is_dry_run: args.dry_run,
    };
    'folders: for folder in
        jwalk::WalkDirGeneric::<((), Option<Folder>)>::new(args.folder.as_path())
            .skip_hidden(!args.dive_into_hidden_folders)
            .follow_links(args.follow)
            .parallelism(Parallelism::RayonNewPool(0))
            .process_read_dir(move |_, _, _, entries| {
                for e in entries
                    .iter_mut()
                    .filter(|e| e.is_ok() && e.as_ref().unwrap().path().is_dir())
                {
                    let mut e = e.as_mut().unwrap();
                    let potential_folder_to_remove = e.path();
                    for rule in to_clean {
                        let folder = Folder::try_from(potential_folder_to_remove.clone());
                        match folder {
                            Ok(folder) => {
                                if rule.resolve_path_to_remove(&folder).is_ok() {
                                    // now we gonna skip reading it's content, since it's going to be removed anyways
                                    e.read_children_path = None;
                                    e.client_state = Some(folder);

                                    // no further rules needs to be checked..
                                    break;
                                } else {
                                    e.client_state = Some(folder);
                                }
                            }
                            Err(_) => {
                                // now we gonna skip reading it's content, since it's going to be removed anyways
                                e.read_children_path = None;

                                // no further rules needs to be checked..
                                break;
                            }
                        }
                    }
                }
            })
            .into_iter()
            .filter(|f| f.is_ok())
            .map(|e| e.unwrap())
            .map(|e| e.client_state)
            .flatten()
    {
        for rule in to_clean {
            match if args.dry_run {
                let cleaner = DryRunCleaner::default();
                folder.accept(rule, &cleaner, &mut decider, &ctx)
            } else {
                let cleaner = ProperCleaner::default();
                folder.accept(rule, &cleaner, &mut decider, &ctx)
            } {
                Ok(FolderProcessed::Abort) => return Ok(()),
                Ok(FolderProcessed::Cleaned(size)) => {
                    amount_cleaned += size;
                    continue 'folders;
                }
                Err(error) => match error.kind() {
                    ErrorKind::Unsupported => continue,
                    _ => return Err(error),
                },
                _ => continue 'folders,
            };
        }
    }

    if amount_cleaned > 0 {
        println!("Freed: {}", amount_cleaned.as_human_readable());
    } else {
        println!("No space freed ;-(");
    }

    Ok(())
}
