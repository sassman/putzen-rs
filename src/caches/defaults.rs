//! Catalogue of default cache directories scanned by `putzen caches`.
//!
//! To add a cache directory:
//!   1. Add one `///` line above a HOME-relative (or absolute) path literal.
//!   2. Place it in the right block — see contribution rules in the spec.
//!   3. Open a PR.
//!
//! The `///` is the runtime label *and* the contributor-facing doc.

#[macro_export]
macro_rules! roots {
    //—— base case
    (@build [$($acc:tt)*]) => { &[ $($acc)* ] };

    //—— ERROR: two or more `///` lines on the same entry
    (@build [$($acc:tt)*]
        #[doc = $_a:literal] #[doc = $_b:literal] $path:literal $(, $($rest:tt)*)?
    ) => {
        compile_error!(concat!(
            "putzen: cache root \"", $path, "\" has multiple `///` lines. ",
            "Use exactly one — extend prose with plain `//` instead."
        ));
    };

    //—— happy path
    (@build [$($acc:tt)*]
        #[doc = $label:literal] $path:literal $(, $($rest:tt)*)?
    ) => {
        roots!(@build [
            $($acc)*
            $crate::caches::defaults::DefaultRoot {
                label: $crate::caches::defaults::strip_leading_spaces($label),
                path:  $path,
            },
        ] $($($rest)*)?)
    };

    //—— ERROR: path literal without preceding `///`
    (@build [$($acc:tt)*] $path:literal $(, $($rest:tt)*)?) => {
        compile_error!(concat!(
            "putzen: cache root \"", $path, "\" is missing its `///` label. ",
            "Add a one-line `///` doc comment above this entry."
        ));
    };

    //—— entrypoint (must be last so `@build` doesn't match it)
    ( $($t:tt)* ) => { roots!(@build [] $($t)*) };
}

pub struct DefaultRoot {
    pub label: &'static str,
    pub path:  &'static str, // HOME-relative unless it begins with '/'
}

/// Strip leading ASCII spaces from a `///` doc string at compile time.
///
/// Rust desugars `/// foo` to `#[doc = " foo"]` (one leading space).
/// This const helper lets the `roots!` macro normalise the label in a
/// `const` context, since `str::trim_start_matches` is not `const fn`.
///
/// Implementation detail of the [`roots!`] macro — not intended for direct use.
pub const fn strip_leading_spaces(s: &'static str) -> &'static str {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    // Safe split at a known-ASCII boundary.
    s.split_at(i).1
}

/// `roots!` requires a `///` doc comment above every path literal.
///
/// ```compile_fail
/// const _: &[putzen_cli::caches::defaults::DefaultRoot] = putzen_cli::roots![
///     ".a",
/// ];
/// ```
pub const fn _missing_doc_check() {}

/// `roots!` allows at most one `///` line per entry.
///
/// ```compile_fail
/// const _: &[putzen_cli::caches::defaults::DefaultRoot] = putzen_cli::roots![
///     /// a
///     /// b
///     ".x",
/// ];
/// ```
pub const fn _multi_doc_check() {}

// Cross-platform cache paths (resolve via ~ on every OS).
pub const SEEDS: &[DefaultRoot] = roots![
    // ── Rust ────────────────────────────────────────────────────────
    /// cargo packaged crates
    ".cargo/registry/cache",
    /// cargo extracted sources
    ".cargo/registry/src",
    /// cargo registry index
    ".cargo/registry/index",
    /// cargo git checkouts
    ".cargo/git/checkouts",
    /// cargo git bare repos
    ".cargo/git/db",
    // ── Go / JVM / .NET package caches ──────────────────────────────
    /// go modules
    "go/pkg/mod",
    /// maven local repo
    ".m2/repository",
    /// gradle caches
    ".gradle/caches",
    /// gradle wrapper distributions
    ".gradle/wrapper/dists",
    /// ivy cache
    ".ivy2/cache",
    /// sbt boot
    ".sbt/boot",
    /// NuGet global packages
    ".nuget/packages",
    // ── Other language pkg caches ───────────────────────────────────
    /// Hex (Elixir) packages
    ".hex/packages",
    /// opam download cache
    ".opam/download-cache",
    /// Clojure gitlibs
    ".gitlibs",
    /// Cabal packages (Haskell)
    ".cabal/packages",
    // ── ML / LLM model caches ───────────────────────────────────────
    /// Ollama models
    ".ollama/models",
    /// triton compile cache
    ".triton/cache",
    /// CUDA NVRTC compute cache
    ".nv/ComputeCache",
];

#[cfg(target_family = "unix")]
pub const SEEDS_OS: &[DefaultRoot] = roots![
    /// XDG cache home
    ".cache",
    /// macOS per-app caches
    "Library/Caches",
    /// Xcode DerivedData
    "Library/Developer/Xcode/DerivedData",
    /// Xcode Archives
    "Library/Developer/Xcode/Archives",
    /// iOS DeviceSupport
    "Library/Developer/Xcode/iOS DeviceSupport",
    /// CoreSimulator caches
    "Library/Developer/CoreSimulator/Caches",
    /// npm
    ".npm",
    /// yarn cache
    ".yarn/cache",
    /// bun install cache
    ".bun/install/cache",
    /// pnpm store (legacy dotfile path)
    ".pnpm-store",
    /// sccache (Linux XDG)
    ".cache/sccache",
    /// sccache (macOS, via Mozilla `directories` crate)
    "Library/Caches/Mozilla.sccache",
];

#[cfg(target_family = "windows")]
pub const SEEDS_OS: &[DefaultRoot] = roots![
    /// WinINet shared cache
    "AppData/Local/Microsoft/Windows/INetCache",
    /// Edge browser cache
    "AppData/Local/Microsoft/Edge/User Data/Default/Cache",
    /// Chrome browser cache
    "AppData/Local/Google/Chrome/User Data/Default/Cache",
    /// npm
    "AppData/Roaming/npm-cache",
    /// yarn
    "AppData/Local/Yarn/Cache",
    /// pnpm store
    "AppData/Local/pnpm",
    /// pip wheel cache
    "AppData/Local/pip/Cache",
    /// uv cache
    "AppData/Local/uv/cache",
    /// HuggingFace hub
    "AppData/Local/huggingface",
    /// go build cache
    "AppData/Local/go-build",
    /// JetBrains caches
    "AppData/Local/JetBrains",
    /// VSCode CachedData
    "AppData/Roaming/Code/CachedData",
    /// sccache
    "AppData/Local/Mozilla/sccache",
];

pub fn defaults() -> impl Iterator<Item = &'static DefaultRoot> {
    SEEDS.iter().chain(SEEDS_OS.iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_emits_label_and_path() {
        const SAMPLE: &[DefaultRoot] = roots![
            /// cargo registry
            ".cargo/registry/cache",
        ];
        assert_eq!(SAMPLE.len(), 1);
        assert_eq!(SAMPLE[0].label, "cargo registry");
        assert_eq!(SAMPLE[0].path,  ".cargo/registry/cache");
    }

    #[test]
    fn macro_emits_multiple_entries() {
        const SAMPLE: &[DefaultRoot] = roots![
            /// alpha
            ".a",
            /// beta
            ".b",
            /// gamma
            ".c",
        ];
        let labels: Vec<_> = SAMPLE.iter().map(|r| r.label).collect();
        let paths:  Vec<_> = SAMPLE.iter().map(|r| r.path).collect();
        assert_eq!(labels, ["alpha", "beta", "gamma"]);
        assert_eq!(paths,  [".a", ".b", ".c"]);
    }

    #[test]
    fn label_has_no_leading_space() {
        const SAMPLE: &[DefaultRoot] = roots![
            /// no leading space
            ".x",
        ];
        assert_eq!(SAMPLE[0].label, "no leading space");
        assert!(!SAMPLE[0].label.starts_with(' '));
    }

    #[test]
    fn defaults_contains_cargo_registry() {
        assert!(defaults().any(|r| r.path == ".cargo/registry/cache"));
    }

    #[test]
    fn defaults_has_no_empty_labels() {
        for r in defaults() {
            assert!(!r.label.is_empty(), "empty label for {}", r.path);
        }
    }

    #[test]
    fn defaults_paths_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for r in defaults() {
            assert!(seen.insert(r.path), "duplicate path {}", r.path);
        }
    }
}
