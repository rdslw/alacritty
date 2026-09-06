# Local vte patch

This directory contains the build and test sources from the published `vte 0.15.0`
crate, based on commit `3b3da71c34cc1256c7e20981cf03f8eb95e08ffc`.
The original MIT and Apache licenses are included.

The OSC 7 changes are in `src/ansi.rs`: the default no-op
`Handler::set_current_directory` callback and OSC 7 dispatch. Dispatch preserves
literal semicolons and accepts only UTF-8. Reports at the parser's parameter limit
are ignored because their contents may have been truncated; encode semicolons as
`%3B` to avoid that limit. A second patch, OSC 133 prompt markers, is described
in `OSC133.md`.

This copy is selected by the workspace's `[patch.crates-io]` entry. Replace it
with a released vte dependency once that API is available upstream. Keeping the
patch here avoids a second parser or a dependency on an unpublished Git branch.

To test the parser separately:

```sh
cargo test --manifest-path vendor/vte/Cargo.toml --features ansi,serde
```

## Upstreaming

`alacritty_terminal` is published on crates.io, and its `Term::current_directory`
API needs this patched parser. The `[patch.crates-io]` entry only applies inside
this workspace, so consumers of the published crate would build against the
unpatched vte and fail. Do not publish `alacritty_terminal` from this tree. An
Alacritty pull request cannot depend on this directory either; Alacritty only
carries temporary git patches for validation, with a TODO to remove them before
a release.

Steps to move the feature upstream:

1. Check the Alacritty issue tracker for a maintainer position on OSC 7 before
   investing in the pull requests. None was found on 2026-09-05.
2. Open a pull request against https://github.com/alacritty/vte with the two
   hunks in `src/ansi.rs` described above, a `CHANGELOG.md` entry, and a parser
   test using the `MockHandler` in the `ansi.rs` test module. Cover BEL and ST
   termination, a literal semicolon, invalid UTF-8, and the parameter-limit
   rejection. The Alacritty tests in `alacritty_terminal/src/term/mod.rs`
   (`osc7_*`) show the expected behavior.
3. Wait for a vte release that contains the change.
4. In `alacritty_terminal/Cargo.toml`, raise the `vte` version to that release.
   In the root `Cargo.toml`, remove `exclude = ["vendor/vte"]` and the `vte`
   line under `[patch.crates-io]`. Delete this directory and run
   `cargo update -p vte` so `Cargo.lock` records the registry checksum again.
5. Open the Alacritty pull request with the remaining changes: the terminal
   state and tests in `alacritty_terminal`, `alacritty/src/working_directory.rs`,
   the daemon, CLI, and event changes, both changelogs, `docs/features.md`, and
   the man pages. Follow `CONTRIBUTING.md`: run `cargo test`, and check its note
   on the `alacritty_terminal` version. State the design decisions in the PR
   description: the report wins over process inspection (see the comment on
   `working_directory::resolve`), host matching without lookups, Windows drive
   paths only, and the reset and empty-report semantics from the man page.
