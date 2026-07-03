# Anyhow Error Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `Box<dyn Error>` throughout with `anyhow::Error` so error call sites can attach human-readable context via `.with_context()` / `anyhow::bail!`.

**Architecture:** Add `anyhow = "1"` as a dependency. Update the two files that deal in errors (`src/utils.rs` and `src/main.rs`) — changing return-type signatures, swapping `.into()` for `anyhow::bail!`, and using `.with_context()` in `check_directory` to embed the target path in the OS error. No behaviour change visible to end-users; all existing tests must remain green.

**Tech Stack:** Rust 2021 edition · `anyhow 1.x`

## Global Constraints

- No behaviour changes — existing tests must remain green.
- `clippy` must produce zero warnings (`cargo clippy -- -D warnings`).
- `cargo fmt` before every commit.
- Commit message format: `type(scope): description` with `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Run `/opt/homebrew/bin/git-mit es` before every commit if the binary is present.

---

### Task 1: Migrate `Box<dyn Error>` to `anyhow::Error`

**Files:**
- Modify: `Cargo.toml` (add dependency)
- Modify: `src/utils.rs` (update `check_directory` signature + error construction)
- Modify: `src/main.rs` (update `run()` signature + error constructions)

**Interfaces:**
- Produces:
  ```rust
  // utils.rs
  pub fn check_directory(target: &str) -> anyhow::Result<()>

  // main.rs
  fn run() -> anyhow::Result<()>
  ```

---

- [ ] **Step 1: Add `anyhow` to `Cargo.toml`**

In `Cargo.toml`, add `anyhow` to `[dependencies]` (keep alphabetical order):

```toml
[dependencies]
anyhow = "1"
clap = { version = "4.6.1", features = ["cargo", "env", "wrap_help"] }
env_logger = "0.11.10"
log = "0.4.32"
```

- [ ] **Step 2: Update `src/utils.rs`**

Replace the entire file content with the following (the only changes are: add `use anyhow::Context;`, change `check_directory`'s return type, and update its two error-construction sites):

```rust
use anyhow::Context as _;
use env_logger::{Builder, Target};
use log::LevelFilter;

/// Verify that the target is a directory.
///
/// # Arguments
///
/// - `target: &str` - a string containing the path to whatever we want to check.
///
/// # Returns
///
/// - `anyhow::Result<()>` - returns an empty `Ok()` if it is a directory, or an
///   error (with the target path embedded) if not.
pub fn check_directory(target: &str) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(target)
        .with_context(|| format!("Target directory '{target}'"))?;
    if !metadata.is_dir() {
        anyhow::bail!("Specified target is not a directory. Unable to proceed.");
    }
    log::debug!("Specified target is a directory. Proceeding.");

    Ok(())
}

/// Determine the `LevelFilter` from parsed flag values.
///
/// `quiet = true` suppresses all log output (`Off`). Otherwise `debug_count`
/// selects the level: 0 → `Info`, 1 → `Debug`, 2+ → `Trace`.
fn log_level(quiet: bool, debug_count: u8) -> LevelFilter {
    if quiet {
        LevelFilter::Off
    } else {
        match debug_count {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

/// Build a logging configuration based on CLI input.
pub fn log_build(cli_args: &clap::ArgMatches) {
    // Route all log output to stdout so it shares the same fd as the
    // println!-based summary output (see main.rs print_summary block).
    // Both streams write to the same fd; the logger uses its own internal
    // buffer while println! goes through Rust's LineWriter (line-flushed).
    Builder::new()
        .filter_level(log_level(
            cli_args.get_flag("quiet"),
            cli_args.get_count("debug"),
        ))
        .target(Target::Stdout)
        .init();
}

#[cfg(test)]
mod tests {
    use super::{check_directory, log_level};
    use log::LevelFilter;

    // ---------------------------------------------------------------------------
    // check_directory
    // ---------------------------------------------------------------------------

    #[test]
    fn check_directory_accepts_real_temp_dir() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        assert!(
            check_directory(dir.path().to_str().expect("non-UTF-8 temp path")).is_ok(),
            "expected Ok for a real directory"
        );
    }

    #[test]
    fn check_directory_rejects_file_path() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("dummy.txt");
        std::fs::write(&file_path, b"data").expect("failed to write temp file");
        let result = check_directory(file_path.to_str().expect("non-UTF-8 temp path"));
        assert!(
            result.is_err(),
            "expected Err for a file path, not a directory"
        );
    }

    #[test]
    fn check_directory_rejects_nonexistent_path() {
        // Join a never-created child name onto an existing tempdir — the child
        // path is guaranteed absent without needing to drop the parent first.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let absent = dir.path().join("does-not-exist");
        let result = check_directory(absent.to_str().expect("non-UTF-8 temp path"));
        assert!(result.is_err(), "expected Err for a nonexistent path");
    }

    // ---------------------------------------------------------------------------
    // log_level
    // ---------------------------------------------------------------------------

    #[test]
    fn log_level_quiet_returns_off() {
        assert_eq!(log_level(true, 0), LevelFilter::Off);
    }

    #[test]
    fn log_level_no_flags_returns_info() {
        assert_eq!(log_level(false, 0), LevelFilter::Info);
    }

    #[test]
    fn log_level_one_debug_flag_returns_debug() {
        assert_eq!(log_level(false, 1), LevelFilter::Debug);
    }

    #[test]
    fn log_level_two_debug_flags_returns_trace() {
        assert_eq!(log_level(false, 2), LevelFilter::Trace);
    }

    #[test]
    fn log_level_three_debug_flags_also_returns_trace() {
        // Confirms the wildcard arm covers all values above 1, not just exactly 2.
        assert_eq!(log_level(false, 3), LevelFilter::Trace);
    }
}
```

- [ ] **Step 3: Update `src/main.rs`**

Replace the entire file content with the following (changes: remove `use std::error::Error`, change `run()` return type, swap `Err(format!(...).into())` for `anyhow::bail!`):

```rust
mod cli;
mod utils;

use std::path::Path;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// This is where the magic happens.
fn run() -> anyhow::Result<()> {
    // Set up the command line. Ref https://docs.rs/clap for details.
    let cli_args = cli::build();

    // Set up logging
    utils::log_build(&cli_args);

    // create a list of the files to gather
    let sources = cli_args
        .get_many::<String>("read")
        .unwrap_or_default()
        .map(String::as_str);
    log::debug!("files_to_gather: {sources:?}");

    // Verify that the target exists and that it is a directory
    let target_dir = cli_args.get_one::<String>("target").expect(
        "default_value('.') guarantees target is always present — this is a clap bug if None",
    );
    log::trace!("target_dir: {target_dir:?}");
    utils::check_directory(target_dir)?;

    let move_files = cli_args.get_flag("move");
    let stop_on_error = cli_args.get_flag("stop");
    let show_detail_info = !cli_args.get_flag("detail-off");
    let dry_run = cli_args.get_flag("dry-run");
    let print_summary = cli_args.get_flag("summary");
    log::debug!("move_files: {move_files}, stop_on_error: {stop_on_error}, show_detail_info: {show_detail_info}, dry_run: {dry_run}, print_summary: {print_summary}");

    if dry_run {
        // Write directly to stdout so the banner is never silenced by -q/--quiet.
        // The quiet flag sets LevelFilter::Off; even log::error! is filtered out.
        println!("Starting dry-run.");
    }

    let mut total_file_count: usize = 0;
    let mut processed_file_count: usize = 0;
    let mut skipped_file_count: usize = 0;

    // Gather files
    for source in sources {
        total_file_count += 1;

        // Paths ending in "/" or ".." have no filename component — treat like any other error.
        let Some(file_name) = Path::new(source).file_name() else {
            if stop_on_error {
                anyhow::bail!("Error: Invalid filename in path: {source}. Halting.");
            }
            log::warn!("Invalid filename in path: {source}. Continuing.");
            skipped_file_count += 1;
            continue;
        };

        let new_filename = Path::new(target_dir).join(file_name);
        let target = new_filename.display();

        if dry_run {
            // Write directly to stdout so previews are never silenced by -q/--quiet.
            if move_files {
                println!("  {source} --> {target}");
            } else {
                println!("  {source} ==> {target}");
            }
            processed_file_count += 1;
        } else if move_files {
            log::debug!("Moving {source} to {target}");
            match std::fs::rename(source, &new_filename) {
                Ok(()) => {
                    if show_detail_info {
                        log::info!("  {source} --> {target}");
                    }
                    processed_file_count += 1;
                }
                Err(err) => {
                    if stop_on_error {
                        anyhow::bail!(
                            "Error: {err}. Unable to move {source} to {target}. Halting."
                        );
                    }
                    log::warn!("Unable to move {source} to {target}. Continuing.");
                    skipped_file_count += 1;
                }
            }
        } else {
            log::debug!("Copying {source} to {target}");
            match std::fs::copy(source, &new_filename) {
                Ok(_) => {
                    if show_detail_info {
                        log::info!("  {source} ==> {target}");
                    }
                    processed_file_count += 1;
                }
                Err(err) => {
                    if stop_on_error {
                        anyhow::bail!(
                            "Error: {err}. Unable to copy {source} to {target}. Halting."
                        );
                    }
                    log::warn!("Unable to copy {source} to {target}. Continuing.");
                    skipped_file_count += 1;
                }
            }
        } // if dry_run
    } // for source

    if print_summary {
        // Write directly to stdout so the summary is never silenced by -q/--quiet.
        println!("Total files examined:        {total_file_count:5}");
        if move_files {
            println!("Files moved:                 {processed_file_count:5}");
        } else {
            println!("Files copied:                {processed_file_count:5}");
        }
        println!("Files skipped due to errors: {skipped_file_count:5}");
    }

    Ok(())
} // fn run()

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// The actual executable function that gets called when the program is invoked.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 4: Run all tests**

```
cd /Volumes/SSD/Source/Rust/gather
cargo test 2>&1
```

Expected: all tests **PASS**.

- [ ] **Step 5: Check clippy and formatting**

```
cd /Volumes/SSD/Source/Rust/gather
cargo fmt && cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings, zero errors.

- [ ] **Step 6: Commit**

```bash
cd /Volumes/SSD/Source/Rust/gather
/opt/homebrew/bin/git-mit es 2>/dev/null || true
git add Cargo.toml Cargo.lock src/utils.rs src/main.rs
git commit -m "$(cat <<'EOF'
feat(error): replace Box<dyn Error> with anyhow (gtr-jz8)

Add anyhow = "1" and migrate all error return types to anyhow::Result.
Swap Err(format!(...).into()) for anyhow::bail! and use .with_context()
in check_directory to embed the target path in the OS error message.
No behaviour changes; all existing tests pass.

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 7: Close the crumb**

```bash
/Users/evensolberg/.cargo/bin/crumbs close gtr-jz8
git add .crumbs/replace-box-dyn-error-with-anyhow-error-for-richer-error-con.md
git commit -m "$(cat <<'EOF'
chore(crumbs): close gtr-jz8 — anyhow migration complete

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```
