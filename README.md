# Putzen

> "putzen" is German and means cleaning. Nobody likes "putzen". But it is necessary.

## What does putzen?

In short, putzen solves the problem of cleaning up build or dependency artifacts.
It does so by a simple "File" -> "Folder" rule. If the "File" and "Folder" is present, it cleans "Folder"

It also does all this fast, means in parallel (if the filesystem supports it).

### Features

putzen supports cleaning artifacts for:
- rust  (`Cargo.toml` -> `target`)
- js (`package.json` -> `node_modules`)
- CMake (`CMakeLists.txt` -> `build`)

It can do a dry-run, asks you for permission, summarizes size nicely

## Quick Start

### Install

To install the `putzen`, you just need to run

```bash
cargo install --force putzen-cli
```

(--force just makes it update to the latest `putzen` if it's already installed)

**Note** the binary is called `putzen` (without `-cli`)

to verify if the installation was successful, you can run `which putzen` that should output similar to

```sh
$HOME/.cargo/bin/putzen
```

### Usage

```sh
$ putzen --help

Usage: putzen <folder> [-d] [-L] [-a]

help keeping your disk clean of build and dependency artifacts

Positional Arguments:
  folder            path of where to start with disk clean up.

Options:
  -d, --dry-run     dry-run will never delete anything, good for simulations
  -L, --follow      follow symbolic links
  -a, --dive-into-hidden-folders
                    dive into hidden folders too, e.g. `.git`
  --help            display usage information
```

## Alternative Projects

- [kondo](https://github.com/tbillington/kondo)

## License

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

- **[GNU GPL v3 license](https://www.gnu.org/licenses/gpl-3.0)**
- Copyright 2019 © [Sven Assmann][me].

[me]: https://www.d34dl0ck.me
