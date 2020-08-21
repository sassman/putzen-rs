use clap::{crate_version, App, AppSettings, Arg};
use std::io::{Result};
use std::path::Path;
use walkdir::{WalkDir};
use std::thread;

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
        not_skip_dot_folder: matches.is_present("not_skip_dot_folder")
    };
    let mut targets: Vec<PurifyTarget> = Vec::new();
    targets.push(PurifyTarget {
        file_to_check: "Cargo.toml".to_owned(),
        folder_to_remove: "target".to_owned(),
    });
    targets.push(PurifyTarget {
        file_to_check: "package.json".to_owned(),
        folder_to_remove: "node_modules".to_owned(),
    });
    let folder = Path::new(matches.value_of("folder").unwrap());
    clean_folder(
        folder,
        &args,
        &targets,
    )
}

struct PurifyArgs {
    dry_run: bool,
    follow: bool,
    not_skip_dot_folder: bool,
}

struct PurifyTarget {
    file_to_check: String,
    folder_to_remove: String,
}

fn remove_folder(folder_to_remove: &Path) -> Result<()> {
    println!("rm -rf {}", folder_to_remove.display());
    Ok(())
}

fn remove_folder_dry(folder_to_remove: &Path) -> Result<()> {
    println!("[dry-run] rm -rf {}", folder_to_remove.display());
    Ok(())
}

fn clean_folder(folder: &Path, args: &PurifyArgs, targets: &Vec<PurifyTarget>) -> Result<()> {
    for i in WalkDir::new(folder)
        .follow_links(args.follow)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().unwrap().is_file())
        .zip(targets.iter())
        .filter(|(e, t)| e.file_name().to_str().unwrap() == t.file_to_check)
        .filter_map(|(e, t)| {
            let path_to_remove = e.path().parent().unwrap().canonicalize().unwrap().join(&t.folder_to_remove);
            if path_to_remove.exists() && path_to_remove.is_dir() {
                Some(path_to_remove)
            } else {
                None
            }
        })
        .map(|path_to_remove| {
            if args.dry_run {
                remove_folder_dry(&path_to_remove)
            } else {
                remove_folder(&path_to_remove)
            }
        })
        {
            match i {
                Ok(_) => {},
                Err(_) => panic!("One removal failed"),
            }
        };

    // thread::spawn(|| {
    dive_deeper(folder, args, targets).expect("something went wrong.");
    // });

    Ok(())
}

fn dive_deeper(folder: &Path, args: &PurifyArgs, targets: &Vec<PurifyTarget>) -> Result<()> {
    let _results: Vec<Result<()>> = WalkDir::new(folder)
        .follow_links(args.follow)
        .max_depth(1)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.metadata().unwrap().is_dir() && !e.file_name().eq("."))
        .filter(|e| !e.file_name().to_str().unwrap().starts_with(".") || args.not_skip_dot_folder)
        .map(|e| e.path().canonicalize().unwrap())
        // .inspect(|f| println!("↘️ {}", f.display()))
        .map(|f| clean_folder(&f, args, targets))
        .collect();

    Ok(())
}