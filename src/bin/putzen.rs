use std::convert::TryFrom;
use std::ffi::OsStr;
use std::io::Result;
use std::path::PathBuf;

use argh::FromArgs;
use globset::{Glob, GlobSet, GlobSetBuilder};
use jwalk::Parallelism;

use putzen_cli::{
    DecisionContext, DoCleanUp, DryRunCleaner, FileToFolderMatch, Folder, FolderProcessed,
    HumanReadable, IsFolderToRemove, NiceInteractiveDecider, NoOpObserver, ProperCleaner,
    RunObserver,
};

#[cfg(feature = "highscore-board")]
use putzen_cli::HighscoreObserver;

/// Static glob pattern used when neither `-a` nor `--hidden` is given.
const DEFAULT_HIDDEN_GLOB: &str = ".worktrees";
/// Static glob pattern used for `-a` / `--dive-into-hidden-folders`.
const ALL_HIDDEN_GLOB: &str = "*";

/// Parse a single glob pattern. Returns a stringified error including the
/// offending input so CLI users see what they typed.
fn parse_glob(s: &str) -> std::result::Result<Glob, String> {
    Glob::new(s).map_err(|e| format!("invalid glob `{s}`: {e}"))
}

/// Cheap first-char heuristic: a glob can only match a hidden basename
/// (which always starts with `.`) if its pattern starts with `.` or with a
/// metacharacter that could expand to one (`*`, `?`, `[`, `{`). Anything
/// else — e.g. `worktrees` — is guaranteed to never match.
fn pattern_can_match_hidden(pattern: &str) -> bool {
    matches!(pattern.chars().next(), Some('.' | '*' | '?' | '[' | '{'))
}

/// Decides whether a hidden directory (basename starts with `.`) is allowed
/// to be entered by the walker. Built once per run from CLI args.
struct HiddenPolicy {
    no_hidden: bool,
    matcher: GlobSet,
}

impl HiddenPolicy {
    fn new(no_hidden: bool, globs: &[Glob]) -> std::result::Result<Self, String> {
        let mut b = GlobSetBuilder::new();
        for g in globs {
            b.add(g.clone());
        }
        let matcher = b
            .build()
            .map_err(|e| format!("failed to build glob set: {e}"))?;
        Ok(Self { no_hidden, matcher })
    }

    /// Should the walker enter this hidden basename?
    /// Caller is responsible for only passing hidden names (starting with `.`).
    fn allows_hidden(&self, name: &OsStr) -> bool {
        if self.no_hidden {
            return false;
        }
        self.matcher.is_match(name)
    }

    /// Build a policy from parsed CLI args. Enforces the mutual-exclusion
    /// rules and applies the default `.worktrees` pattern when no
    /// `--hidden` and no `-a` was passed.
    fn from_args(args: &PutzenCliArgs) -> std::result::Result<Self, String> {
        let dash_a = args.dive_into_hidden_folders;
        let no_hidden = args.no_hidden;
        let hidden_given = !args.hidden.is_empty();

        if no_hidden && hidden_given {
            return Err("`--no-hidden` and `--hidden` are mutually exclusive".into());
        }
        if no_hidden && dash_a {
            return Err("`--no-hidden` and `-a` are mutually exclusive".into());
        }
        if dash_a && hidden_given {
            return Err("`-a` and `--hidden` are mutually exclusive".into());
        }

        if no_hidden {
            return Self::new(true, &[]);
        }

        // Resolve the active set of globs:
        //   -a       -> ["*"]
        //   --hidden -> user list (warn on patterns that can't match a hidden name)
        //   neither  -> [".worktrees"]
        let owned: Vec<Glob> = if dash_a {
            vec![parse_glob(ALL_HIDDEN_GLOB).expect("static glob must parse")]
        } else if hidden_given {
            for g in &args.hidden {
                let pat = g.glob();
                if !pattern_can_match_hidden(pat) {
                    eprintln!(
                        "warning: --hidden `{pat}` does not start with `.`, `*`, `?`, `[`, or `{{` \
                         — hidden basenames always start with `.`, so this pattern will never match"
                    );
                }
            }
            args.hidden.clone()
        } else {
            vec![parse_glob(DEFAULT_HIDDEN_GLOB).expect("static glob must parse")]
        };

        Self::new(false, &owned)
    }
}

/// all supported this to clean up
static FOLDER_TO_CLEANUP: [FileToFolderMatch; 3] = [
    FileToFolderMatch::new("Cargo.toml", "target"),
    FileToFolderMatch::new("package.json", "node_modules"),
    FileToFolderMatch::new("CMakeLists.txt", "build"),
];

#[derive(FromArgs)]
/// help keeping your disk clean of build and dependency artifacts
///
/// Hidden directories are normally skipped, except for `.worktrees`
/// (so colocated git worktrees are cleaned alongside the main checkout).
/// Use `--hidden <GLOB>` to override the list, `--no-hidden` to turn it
/// off entirely, or `-a` to descend into every hidden dir.
///
/// Examples:
///     putzen                              # descends into `.worktrees` by default
///     putzen --hidden '.{worktrees,jj}'  # one glob, two hidden dirs
///     putzen --hidden '.work*'            # any hidden dir starting with `.work`
///     putzen -a                           # every hidden dir (== '*')
///     putzen --no-hidden                  # skip all hidden dirs (legacy)
struct PutzenCliArgs {
    /// show the version number
    #[argh(switch, short = 'v')]
    version: bool,

    /// show the stored highscore board and exit
    #[cfg(feature = "highscore-board")]
    #[argh(switch)]
    scores: bool,

    /// dry-run will never delete anything, good for simulations
    #[argh(switch, short = 'd')]
    dry_run: bool,

    /// switch to say yes to all questions
    #[argh(switch, short = 'y')]
    yes_to_all: bool,

    /// follow symbolic links
    #[argh(switch, short = 'L')]
    follow: bool,

    /// include every hidden directory (== --hidden '*')
    #[argh(switch, short = 'a')]
    dive_into_hidden_folders: bool,

    /// skip every hidden directory (overrides the default `.worktrees`)
    #[argh(switch)]
    no_hidden: bool,

    /// glob of hidden directories to descend into (repeatable). Match is
    /// against the full basename including the leading dot, e.g.
    /// `.worktrees`, `.{worktrees,jj}`, `.work*`. Default: `.worktrees`.
    #[argh(option, from_str_fn(parse_glob))]
    hidden: Vec<Glob>,

    /// path where to start with disk clean up.
    #[argh(positional, default = "PathBuf::from(\".\")")]
    folder: PathBuf,
}

fn main() -> Result<()> {
    let args: PutzenCliArgs = argh::from_env();
    if args.version {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    #[cfg(feature = "highscore-board")]
    if args.scores {
        let highscores = putzen_cli::Highscores::load()?;
        println!("{}", putzen_cli::render_board(&highscores));
        return Ok(());
    }
    visit_path(&args)
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

    let mut observer: Box<dyn RunObserver> = if !args.dry_run {
        #[cfg(feature = "highscore-board")]
        {
            Box::new(HighscoreObserver::load()?)
        }
        #[cfg(not(feature = "highscore-board"))]
        {
            Box::new(NoOpObserver)
        }
    } else {
        Box::new(NoOpObserver)
    };

    let hidden_policy = HiddenPolicy::from_args(args)
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))?;

    ctx.println(format!("Start cleaning at {}", folder.display()));
    for folder in jwalk::WalkDirGeneric::<((), Option<Folder>)>::new(folder)
        .skip_hidden(false)
        .follow_links(args.follow)
        .parallelism(Parallelism::RayonNewPool(8))
        .process_read_dir(move |depth, _, _, children| {
            // 1. drop hidden children disallowed by the policy.
            // depth=None is the virtual root call (parent of the starting dir);
            // we must NOT filter those children or we'd block the starting dir itself.
            if depth.is_some() {
                children.retain(|dir_entry_result| {
                    let Ok(dir) = dir_entry_result else {
                        return true;
                    };
                    let name = dir.file_name();
                    // byte-level check: works for non-UTF-8 names too, and `.` is always ASCII
                    let is_hidden = name.as_encoded_bytes().first() == Some(&b'.');
                    if !is_hidden {
                        return true;
                    }
                    hidden_policy.allows_hidden(name)
                });
            }

            // 2. keep only directories
            children.retain(|dir_entry_result| {
                dir_entry_result
                    .as_ref()
                    .map(|dir| dir.path().is_dir())
                    .unwrap_or(false)
            });

            // 3. existing build-artefact marking
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
            let result = folder.accept(&ctx, rule, &*cleaner, &mut decider, &mut *observer);
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

    if let Some(medals) = observer.on_run_complete(amount_cleaned as u64) {
        println!("{medals}");
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
            #[cfg(feature = "highscore-board")]
            scores: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: false,
            no_hidden: false,
            hidden: Vec::new(),
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
    fn parse_glob_accepts_valid_pattern_with_dot() {
        let g = parse_glob(".worktrees").expect("should parse");
        assert_eq!(g.glob(), ".worktrees");
    }

    #[test]
    fn parse_glob_accepts_brace_expansion() {
        let g = parse_glob(".{worktrees,jj}").expect("should parse");
        assert_eq!(g.glob(), ".{worktrees,jj}");
    }

    #[test]
    fn parse_glob_accepts_wildcard() {
        parse_glob("*").expect("should parse wildcard");
        parse_glob(".work*").expect("should parse prefix wildcard");
    }

    #[test]
    fn pattern_can_match_hidden_heuristic() {
        assert!(pattern_can_match_hidden(".worktrees"));
        assert!(pattern_can_match_hidden(".{worktrees,jj}"));
        assert!(pattern_can_match_hidden(".work*"));
        assert!(pattern_can_match_hidden("*"));
        assert!(pattern_can_match_hidden("?orktrees"));
        assert!(pattern_can_match_hidden("[.a]worktrees"));
        assert!(pattern_can_match_hidden("{.a,.b}"));
        // anything not starting with `.` or a metachar cannot match a hidden basename
        assert!(!pattern_can_match_hidden("worktrees"));
        assert!(!pattern_can_match_hidden(""));
    }

    #[test]
    fn parse_glob_rejects_invalid_pattern() {
        let err = parse_glob("[unterminated").expect_err("should fail");
        // error message includes the offending input so users can self-diagnose
        assert!(err.contains("[unterminated"), "got: {err}");
    }

    fn policy(globs: &[&str]) -> HiddenPolicy {
        let compiled: Vec<Glob> = globs.iter().map(|g| parse_glob(g).unwrap()).collect();
        HiddenPolicy::new(false, &compiled).unwrap()
    }

    #[test]
    fn policy_default_worktrees_only() {
        let p = policy(&[".worktrees"]);
        assert!(p.allows_hidden(".worktrees".as_ref()));
        assert!(!p.allows_hidden(".git".as_ref()));
        assert!(!p.allows_hidden(".cache".as_ref()));
    }

    #[test]
    fn policy_brace_expansion_two_dirs() {
        let p = policy(&[".{worktrees,jj}"]);
        assert!(p.allows_hidden(".worktrees".as_ref()));
        assert!(p.allows_hidden(".jj".as_ref()));
        assert!(!p.allows_hidden(".pijul".as_ref()));
    }

    #[test]
    fn policy_star_matches_all_hidden() {
        let p = policy(&["*"]);
        assert!(p.allows_hidden(".worktrees".as_ref()));
        assert!(p.allows_hidden(".git".as_ref()));
        assert!(p.allows_hidden(".cache".as_ref()));
    }

    #[test]
    fn policy_pattern_without_dot_matches_nothing_hidden() {
        // documented footgun: pattern omits the leading dot, so it can
        // never match a hidden basename (which always starts with `.`)
        let p = policy(&["worktrees"]);
        assert!(!p.allows_hidden(".worktrees".as_ref()));
    }

    #[test]
    fn policy_no_hidden_rejects_everything() {
        let compiled: Vec<Glob> = vec![parse_glob(".worktrees").unwrap()];
        let p = HiddenPolicy::new(true, &compiled).unwrap();
        assert!(!p.allows_hidden(".worktrees".as_ref()));
        assert!(!p.allows_hidden(".anything".as_ref()));
    }

    fn args_from(input: &[&str]) -> std::result::Result<PutzenCliArgs, argh::EarlyExit> {
        PutzenCliArgs::from_args(&["putzen"], input)
    }

    #[test]
    fn from_args_default_includes_worktrees_only() {
        let args = args_from(&[]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(policy.allows_hidden(".worktrees".as_ref()));
        assert!(!policy.allows_hidden(".git".as_ref()));
    }

    #[test]
    fn from_args_dash_a_includes_everything_hidden() {
        let args = args_from(&["-a"]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(policy.allows_hidden(".worktrees".as_ref()));
        assert!(policy.allows_hidden(".git".as_ref()));
        assert!(policy.allows_hidden(".whatever".as_ref()));
    }

    #[test]
    fn from_args_no_hidden_rejects_everything() {
        let args = args_from(&["--no-hidden"]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(!policy.allows_hidden(".worktrees".as_ref()));
        assert!(!policy.allows_hidden(".anything".as_ref()));
    }

    #[test]
    fn from_args_hidden_overrides_default() {
        let args = args_from(&["--hidden", ".git"]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(policy.allows_hidden(".git".as_ref()));
        // default `.worktrees` is replaced, not merged
        assert!(!policy.allows_hidden(".worktrees".as_ref()));
    }

    #[test]
    fn from_args_hidden_repeatable() {
        let args = args_from(&["--hidden", ".git", "--hidden", ".cache"]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(policy.allows_hidden(".git".as_ref()));
        assert!(policy.allows_hidden(".cache".as_ref()));
        assert!(!policy.allows_hidden(".worktrees".as_ref()));
    }

    #[test]
    fn from_args_brace_expansion_one_flag_two_dirs() {
        let args = args_from(&["--hidden", ".{worktrees,jj}"]).unwrap();
        let policy = HiddenPolicy::from_args(&args).unwrap();
        assert!(policy.allows_hidden(".worktrees".as_ref()));
        assert!(policy.allows_hidden(".jj".as_ref()));
    }

    #[test]
    fn from_args_invalid_glob_errors_at_parse_time() {
        // argh's from_str_fn surfaces the error at argument-parse time
        let result = args_from(&["--hidden", "[bad"]);
        let early_exit = match result {
            Err(e) => e,
            Ok(_) => panic!("expected parse error for invalid glob"),
        };
        let msg = format!("{early_exit:?}");
        assert!(
            msg.contains("[bad"),
            "error should mention offending input: {msg}"
        );
    }

    #[test]
    fn from_args_no_hidden_conflicts_with_hidden() {
        let args = args_from(&["--no-hidden", "--hidden", ".git"]).unwrap();
        let result = HiddenPolicy::from_args(&args);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected conflict error"),
        };
        assert!(err.contains("--no-hidden"), "got: {err}");
        assert!(err.contains("--hidden"), "got: {err}");
    }

    #[test]
    fn from_args_no_hidden_conflicts_with_dash_a() {
        let args = args_from(&["--no-hidden", "-a"]).unwrap();
        let result = HiddenPolicy::from_args(&args);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected conflict error"),
        };
        assert!(err.contains("--no-hidden"), "got: {err}");
        assert!(err.contains("-a"), "got: {err}");
    }

    #[test]
    fn from_args_dash_a_conflicts_with_hidden() {
        let args = args_from(&["-a", "--hidden", ".git"]).unwrap();
        let result = HiddenPolicy::from_args(&args);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected conflict error"),
        };
        assert!(err.contains("-a"), "got: {err}");
        assert!(err.contains("--hidden"), "got: {err}");
    }

    #[test]
    fn default_run_descends_into_dot_worktrees_but_not_dot_git() {
        let root = tempfile::TempDir::new().unwrap();

        // .worktrees/wt1/target  — should be cleaned by default
        let wt_target = root.path().join(".worktrees").join("wt1").join("target");
        std::fs::create_dir_all(&wt_target).unwrap();
        std::fs::File::create(
            root.path()
                .join(".worktrees")
                .join("wt1")
                .join("Cargo.toml"),
        )
        .unwrap();
        std::fs::File::create(wt_target.join("artefact")).unwrap();

        // .git/target  — should NOT be touched (hidden, not in default include set)
        let git_target = root.path().join(".git").join("target");
        std::fs::create_dir_all(&git_target).unwrap();
        std::fs::File::create(root.path().join(".git").join("Cargo.toml")).unwrap();
        std::fs::File::create(git_target.join("artefact")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            #[cfg(feature = "highscore-board")]
            scores: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: false,
            no_hidden: false,
            hidden: Vec::new(),
            folder: root.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(
            !wt_target.exists(),
            ".worktrees/wt1/target should have been cleaned"
        );
        assert!(git_target.exists(), ".git/target must NOT be touched");
    }

    #[test]
    fn no_hidden_skips_dot_worktrees() {
        let root = tempfile::TempDir::new().unwrap();
        let wt_target = root.path().join(".worktrees").join("wt1").join("target");
        std::fs::create_dir_all(&wt_target).unwrap();
        std::fs::File::create(
            root.path()
                .join(".worktrees")
                .join("wt1")
                .join("Cargo.toml"),
        )
        .unwrap();
        std::fs::File::create(wt_target.join("artefact")).unwrap();

        let args = PutzenCliArgs {
            version: false,
            #[cfg(feature = "highscore-board")]
            scores: false,
            dry_run: false,
            yes_to_all: true,
            follow: false,
            dive_into_hidden_folders: false,
            no_hidden: true,
            hidden: Vec::new(),
            folder: root.path().to_path_buf(),
        };

        visit_path(&args).unwrap();

        assert!(
            wt_target.exists(),
            "--no-hidden must leave .worktrees/wt1/target untouched"
        );
    }
}
