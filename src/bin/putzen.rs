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
static FOLDER_TO_CLEANUP: [FileToFolderMatch; 9] = [
    // Rust
    FileToFolderMatch::new(&["Cargo.toml"], "target"),
    // Node.js / JavaScript
    FileToFolderMatch::new(&["package.json"], "node_modules"),
    FileToFolderMatch::new(&["next.config.js", "next.config.ts"], ".next"),
    FileToFolderMatch::new(&["nuxt.config.js", "nuxt.config.ts"], ".nuxt"),
    // Python
    FileToFolderMatch::new(
        &["pyproject.toml", "setup.py", "requirements.txt"],
        "__pycache__",
    ),
    FileToFolderMatch::new(&["pytest.ini", "pyproject.toml"], ".pytest_cache"),
    // Java / Kotlin (Gradle)
    FileToFolderMatch::new(&["build.gradle", "build.gradle.kts"], "build"),
    // Java / Kotlin (Maven)
    FileToFolderMatch::new(&["pom.xml"], "target"),
    // CMake (already supported)
    FileToFolderMatch::new(&["CMakeLists.txt"], "build"),
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

    #[test]
    fn test_python_cache_cleanup() {
        let root_folder = tempfile::TempDir::new().unwrap();

        // Python __pycache__ with pyproject.toml
        let python_project = root_folder.path().join("python-app");
        std::fs::create_dir(&python_project).unwrap();
        let pycache_folder = python_project.join("__pycache__");
        std::fs::create_dir(&pycache_folder).unwrap();
        std::fs::File::create(python_project.join("pyproject.toml")).unwrap();
        std::fs::File::create(pycache_folder.join("module.pyc")).unwrap();

        // pytest cache with pytest.ini only
        let pytest_project = root_folder.path().join("pytest-app");
        std::fs::create_dir(&pytest_project).unwrap();
        let pytest_cache_folder = pytest_project.join(".pytest_cache");
        std::fs::create_dir(&pytest_cache_folder).unwrap();
        std::fs::File::create(pytest_project.join("pytest.ini")).unwrap();
        std::fs::File::create(pytest_cache_folder.join("cache_data")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: true, // needed for .pytest_cache
            folder: root_folder.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(!pycache_folder.exists());
        assert!(!pytest_cache_folder.exists());
        assert!(python_project.join("pyproject.toml").exists());
        assert!(pytest_project.join("pytest.ini").exists());
    }

    #[test]
    fn test_nodejs_framework_cache_cleanup() {
        let root_folder = tempfile::TempDir::new().unwrap();

        // Next.js cache
        let next_folder = root_folder.path().join(".next");
        std::fs::create_dir(&next_folder).unwrap();
        std::fs::File::create(root_folder.path().join("next.config.js")).unwrap();
        std::fs::File::create(next_folder.join("build-manifest.json")).unwrap();

        // Nuxt.js cache in a subfolder
        let nuxt_project = root_folder.path().join("nuxt-app");
        std::fs::create_dir(&nuxt_project).unwrap();
        let nuxt_folder = nuxt_project.join(".nuxt");
        std::fs::create_dir(&nuxt_folder).unwrap();
        std::fs::File::create(nuxt_project.join("nuxt.config.ts")).unwrap();
        std::fs::File::create(nuxt_folder.join("routes.json")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: true,
            folder: root_folder.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(!next_folder.exists());
        assert!(!nuxt_folder.exists());
        assert!(root_folder.path().join("next.config.js").exists());
        assert!(nuxt_project.join("nuxt.config.ts").exists());
    }

    #[test]
    fn test_java_gradle_cache_cleanup() {
        let root_folder = tempfile::TempDir::new().unwrap();

        // Gradle build folder with Kotlin DSL
        let build_folder = root_folder.path().join("build");
        std::fs::create_dir(&build_folder).unwrap();
        std::fs::File::create(root_folder.path().join("build.gradle.kts")).unwrap();
        std::fs::File::create(build_folder.join("classes.jar")).unwrap();

        // Gradle build folder with Groovy DSL in subfolder
        let gradle_project = root_folder.path().join("gradle-app");
        std::fs::create_dir(&gradle_project).unwrap();
        let gradle_build = gradle_project.join("build");
        std::fs::create_dir(&gradle_build).unwrap();
        std::fs::File::create(gradle_project.join("build.gradle")).unwrap();
        std::fs::File::create(gradle_build.join("libs.jar")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: false,
            folder: root_folder.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(!build_folder.exists());
        assert!(!gradle_build.exists());
        assert!(root_folder.path().join("build.gradle.kts").exists());
        assert!(gradle_project.join("build.gradle").exists());
    }

    #[test]
    fn test_java_maven_cache_cleanup() {
        let root_folder = tempfile::TempDir::new().unwrap();

        // Maven target folder
        let target_folder = root_folder.path().join("target");
        std::fs::create_dir(&target_folder).unwrap();
        std::fs::File::create(root_folder.path().join("pom.xml")).unwrap();
        std::fs::File::create(target_folder.join("app.jar")).unwrap();

        // Maven project in subfolder
        let maven_project = root_folder.path().join("maven-app");
        std::fs::create_dir(&maven_project).unwrap();
        let maven_target = maven_project.join("target");
        std::fs::create_dir(&maven_target).unwrap();
        std::fs::File::create(maven_project.join("pom.xml")).unwrap();
        std::fs::File::create(maven_target.join("classes.jar")).unwrap();

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
        assert!(!maven_target.exists());
        assert!(root_folder.path().join("pom.xml").exists());
        assert!(maven_project.join("pom.xml").exists());
    }
}
