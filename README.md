<div align="center">
 <img src="https://github.com/sassman/putzen-rs/blob/main/resources/logo.png?raw=true" width="256" height="256">
 <h1><strong>Putzen</strong></h1>

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![crates.io](https://img.shields.io/crates/v/putzen-cli.svg)](https://crates.io/crates/putzen-cli)
[![dependency status](https://deps.rs/repo/github/sassman/putzen-rs/status.svg)](https://deps.rs/repo/github/sassman/putzen-rs)
[![Build Status](https://github.com/sassman/putzen-rs/workflows/Build/badge.svg)](https://github.com/sassman/putzen-rs/actions?query=branch%3Amain+workflow%3ABuild+)

"putzen" is German and means cleaning. It helps keeping your disk clean of build and dependency artifacts safely.

![demo](resources/demo.gif)

</div>

### New: `putzen caches`

Interactive TUI for cleaning user-level tool caches (`~/.cargo`, `~/.npm`, `~/.cache/huggingface`, …). Browse, mark, drill in, delete — in one screen.

![caches demo](resources/caches-demo.gif)

[Higher-quality MP4](resources/caches-demo.mp4)

## About 

In short, putzen solves the problem of cleaning up build or dependency artifacts.
It does so by a simple "File" -> "Folder" rule. If the "File" and "Folder" is present, it cleans "Folder"

It also does all this fast, means in parallel (if the filesystem supports it).

### Supported Artifacts

putzen supports cleaning artifacts for:

| type       | file that is checked | folder that is cleaned |
|------------|----------------------|------------------------|
| rust       | Cargo.toml           | target                 |
| javascript | package.json         | node_modules           |
| CMake      | CMakeLists.txt       | build                  |

furthermore, it does also support:
- It can do run a dry-run (`-d`)
- Interactive asking for deletion
- Sums up the space that will be freed

## Quick Start

### Install

### On Linux as snap

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/putzen)

- installation [for Linux Mint](https://snapcraft.io/install/putzen/mint)
- installation [for Arch Linux](https://snapcraft.io/install/putzen/arch)

*TL;DR:*
```sh
sudo snap install putzen
```

### With cargo

To install the `putzen`, you just need to run

```sh
cargo install putzen-cli
```

**Note** the binary is called `putzen` (without `-cli`)

to verify if the installation was successful, you can run `which putzen` that should output similar to

```sh
$HOME/.cargo/bin/putzen
```

### Usage

```sh
$ putzen --help

Usage: putzen [-v] [--scores] [-d] [-y] [-L] [-a] [--no-hidden] [--hidden <hidden...>] [--] [<folder>]

help keeping your disk clean of build and dependency artifacts

Positional Arguments:
  folder            path where to start with disk clean up.

Options:
  -v, --version     show the version number
  --scores          show the stored highscore board and exit
  -d, --dry-run     dry-run will never delete anything, good for simulations
  -y, --yes-to-all  switch to say yes to all questions
  -L, --follow      follow symbolic links
  -a, --dive-into-hidden-folders
                    include every hidden directory (== --hidden '*')
  --no-hidden       skip every hidden directory (overrides the default
                    `.worktrees`)
  --hidden          glob of hidden directories to descend into (repeatable).
                    Match is against the full basename including the leading
                    dot, e.g. `.worktrees`, `.{worktrees,jj}`, `.work*`.
                    Default: `.worktrees`.
  --help, help      display usage information
```

For the interactive cache-cleaning TUI, run `putzen caches`:

```sh
$ putzen caches --help

Usage: putzen caches [--root <root...>] [--floor <floor>] [--dry-run] [-y]

interactive cleanup of user-level cache directories

Options:
  --root            scan root (repeatable). When given, REPLACES the built-in
                    defaults.
  --floor           caches whose newest file is younger than this are flagged
                    ACTIVE
  --dry-run         dry run: never delete, just show what would happen
  -y, --yes         skip the deletion confirmation modal
  --help, help      display usage information
```

### Hidden directories

`putzen` skips hidden directories by default, **except for `.worktrees`** —
projects that colocate git worktrees inside the repo get cleaned in one run.

- `--hidden <GLOB>` — pick which hidden dirs to descend into (repeatable). Default: `.worktrees`. Glob is matched against the full basename including the leading dot, so write `.worktrees`, `.{worktrees,jj}`, `.work*`.
- `--no-hidden` — skip *all* hidden directories (pre-3.x behavior).
- `-a` / `--dive-into-hidden-folders` — descend into every hidden directory (equivalent to `--hidden '*'`).

These three are mutually exclusive.

### Highscores

Every putzen run earns you a little reward. The biggest single cleanup and the biggest total run ever measured are kept as a tiny gold/silver/bronze podium. Keep running it on your machine and watch your records stack up over time — show the board any time with `--scores`:

```
❯ putzen --scores

   ──── ★ SINGLE CLEANUP ★ ────
     🥇 Gold
         40.1GiB · 2026-03-14
   ────────────────────────────
     🥈 Silver
         37.9GiB · 2026-03-10
   ────────────────────────────
     🥉 Bronze
          6.5GiB · 2026-03-14
   ────────────────────────────

   ──── ★    TOTAL RUN   ★ ────
     🥇 Gold
         60.3GiB · 2026-03-14
   ────────────────────────────
     🥈 Silver
         44.6GiB · 2026-03-10
   ────────────────────────────
     🥉 Bronze
         19.6GiB · 2026-04-03
   ────────────────────────────
```

### Caches

`putzen caches` scans known tool-cache locations under `$HOME` (cargo, npm, pnpm, yarn, bun, uv, pip, huggingface, gradle, ivy, …) and presents them as a ranked list. You browse with arrow keys, drill into a cache to see its children, mark anything to delete with `Space`, and confirm with `d` + `y`.

Pass `--root <dir>` to scan a tree of your choosing instead of the built-in defaults.

#### The score

Each cache gets a single number that ranks "how much disk this is wasting *right now*":

```
score = size_MiB × age_days
```

- **size_MiB** — total size of the cache directory.
- **age_days** — days since its newest file was last touched.

The intuition: a 5 GiB cache you used yesterday is probably still useful; a 200 MiB cache untouched for a year is dead weight, and the score keeps it visible. The right-pane heatmap bar is `score / max_score` — the heaviest cache in your visible set is full-red, everything else scales down through orange to green.

The `--floor <duration>` flag (e.g. `--floor 14d`) marks caches younger than that threshold as **ACTIVE** — marking them for deletion needs an extra confirmation modal so a fresh cache you're still using doesn't disappear by accident. Default floor: 7 days.

## Alternative Projects

- [kondo](https://github.com/tbillington/kondo)

## License

- **[GNU GPL v3 license](https://www.gnu.org/licenses/gpl-3.0)**
- Copyright 2019 - 2026 © [Sven Kanoldt](https://d34dl0ck.me)
- Logo - [Clean icons created by photo3idea_studio - Flaticon](https://www.flaticon.com/free-icons/clean)
