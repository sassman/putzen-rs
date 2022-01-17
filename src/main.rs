use std::convert::TryFrom;
use std::io::Result;
use std::path::Path;

use clap::{crate_version, App, AppSettings, Arg};

mod lib;

use lib::*;

/// all supported this to clean up
static FOLDER_TO_CLEANUP: [FileToFolderMatch; 2] = [
    FileToFolderMatch::new("Cargo.toml", "target"),
    FileToFolderMatch::new("package.json", "node_modules"),
];

fn main() -> Result<()> {
    let matches = App::new("purify-cli")
        .version(crate_version!())
        .author("Sven Assmann <sven.assmann.it@gmail.com>")
        .about("This tool helps you keeping your disk clean of build and dependency artifacts for rust, node and whatever you like.")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("folder")
                .index(1)
                .required(true)
                .help("Path of where to start with disk clean up."),
        )
        .arg(
            Arg::with_name("dry")
                .short("d")
                .long("dry-run")
                .takes_value(false)
                .required(false)
                .help("Turn on dry mode, so it does not remove anything, just prints out `rm -rf` commands."),
        )
        .arg(
            Arg::with_name("follow")
                .short("L")
                .long("follow")
                .takes_value(false)
                .required(false)
                .help("Follow symlinks when walking thru directories."),
        )
        .arg(
            Arg::with_name("not_skip_dot_folder")
                .long("not-skip-dot-folder")
                .takes_value(false)
                .required(false)
                .help("Follow dot folder like `.git` when walking thru directories."),
        )
        .get_matches();

    let args = PurifyArgs {
        dry_run: matches.is_present("dry"),
        follow: matches.is_present("follow"),
        not_skip_dot_folder: matches.is_present("not_skip_dot_folder"),
    };

    let folder = matches
        .value_of("folder")
        .map(|p| Path::new(p).to_path_buf())
        .unwrap();

    visit_path(&folder, &args, &FOLDER_TO_CLEANUP)

    // clean_folder(&folder, &args, &FOLDER_TO_CLEANUP)
}

fn visit_path(
    path: impl AsRef<Path>,
    args: &PurifyArgs,
    to_clean: &[FileToFolderMatch; 2],
) -> Result<()> {
    'folders: for folder in std::fs::read_dir(path.as_ref())?
        .into_iter()
        .filter(|e| e.is_ok())
        .map(|e| Folder::try_from(e.unwrap()))
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap())
    {
        for rule in to_clean {
            if args.dry_run {
                if folder.accept(rule).is_ok() {
                    continue 'folders;
                }
            } else {
                let rule = DryRun::wrap(rule);
                if folder.accept(&rule).is_ok() {
                    continue 'folders;
                }
            }
        }
        visit_path(&folder, &args, to_clean)?
    }

    Ok(())
}

struct PurifyArgs {
    dry_run: bool,
    follow: bool,
    not_skip_dot_folder: bool,
}

// fn clean_folder(
//     folder: &impl AsRef<Path>,
//     args: &PurifyArgs,
//     targets: &[FileToFolderMatch],
// ) -> Result<()> {
//     for i in WalkDir::new(folder)
//         .follow_links(args.follow)
//         // .max_depth(1)
//         .into_iter()
//         .filter_map(|e| e.ok())
//         .filter(|e| e.metadata().unwrap().is_dir())
//         .map(|e| e.path())
//         .cartesian_product(targets.iter())
//         .map(|(f, t)| (Folder(f), t))
//         .filter_map(|(e, t)| {
//             if e.accept(t).is_ok() {
//                 Some((e, t))
//             } else {
//                 None
//             }
//         })
//     // .map(|path_to_remove| {
//     //     if args.dry_run {
//     //         remove_folder_dry(&path_to_remove)
//     //     } else {
//     //         remove_folder(&path_to_remove)
//     //     }
//     // })
//     {
//         println!(
//             "{} in {} removed",
//             i.1.folder_to_remove,
//             i.0.as_ref().display()
//         );
//     }
//
//     dive_deeper(folder, args, &targets[..]).expect("something went wrong.");
//
//     Ok(())
// }

// fn dive_deeper(
//     folder: &impl AsRef<Path>,
//     args: &PurifyArgs,
//     targets: &[FileToFolderMatch],
// ) -> Result<()> {
//     let _results: Vec<Result<()>> = WalkDir::new(folder)
//         .follow_links(args.follow)
//         .max_depth(1)
//         .min_depth(1)
//         // .into_iter()
//         .into_iter()
//         .filter_map(|e| e.ok())
//         .filter(|e| e.metadata().unwrap().is_dir() && !e.file_name().eq("."))
//         .filter(|e| !e.file_name().to_str().unwrap().starts_with(".") || args.not_skip_dot_folder)
//         .map(|e| e.path())
//         .cartesian_product(targets.iter())
//         .filter(|(e, t)| e.file_name().unwrap().to_str().unwrap() != t.folder_to_remove)
//         .map(|(e, _)| e.canonicalize().unwrap())
//         // .map(|e| e.path().canonicalize().unwrap())
//         // .inspect(|f| println!("↘️ {}", f.display()))
//         .map(|f| clean_folder(&f, args, targets))
//         .collect();
//
//     Ok(())
// }
