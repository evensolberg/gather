# Extract `process_file` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the `dry_run / move / copy` per-file dispatch block from `run()` into a standalone, unit-testable `process_file` function.

**Architecture:** Add a `ProcessOptions` struct and `pub fn process_file` to `src/utils.rs` (alongside the existing `check_directory` helper). `run()` in `src/main.rs` constructs a `ProcessOptions`, then calls `utils::process_file` inside the for-loop instead of the inline if/else chain. No new files; no new dependencies (anyhow already added by gtr-jz8).

**Tech Stack:** Rust 2021 edition · `anyhow 1.x` (already a dependency) · `tempfile` (already a dev-dependency) for unit-test fixtures · `cargo test` (unit + integration)

## Global Constraints

- All existing tests must remain green — this is a pure refactoring, no behaviour changes.
- No new crate dependencies.
- `clippy` must produce zero warnings (`cargo clippy -- -D warnings`).
- `cargo fmt` before every commit.
- Commit message format: `type(scope): description` with `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Run `/opt/homebrew/bin/git-mit es` before every commit if the binary is present.

---

### Task 1: Add `ProcessOptions` and `process_file` to `utils.rs`

**Files:**
- Modify: `src/utils.rs` (add struct + function + unit tests)

**Interfaces:**
- Produces:
  ```rust
  pub struct ProcessOptions {
      pub dry_run: bool,
      pub move_files: bool,
      pub stop_on_error: bool,
      pub show_detail_info: bool,
  }

  pub fn process_file(
      source: &str,
      target: &std::path::Path,
      opts: &ProcessOptions,
  ) -> anyhow::Result<bool>
  ```
  `Ok(true)` = file was processed; `Ok(false)` = file was skipped (soft error); `Err(...)` = hard stop.

---

- [ ] **Step 1: Write the failing unit tests**

Add the following block at the **end** of the `#[cfg(test)]` module in `src/utils.rs`
(after the existing `log_level_*` tests, before the closing `}`):

```rust
// ---------------------------------------------------------------------------
// process_file
// ---------------------------------------------------------------------------

#[test]
fn process_file_dry_run_copy_returns_ok_true_without_creating_file() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("in.txt");
    std::fs::write(&src, b"data").expect("write src");
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: true,
        move_files: false,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(result.unwrap(), true, "dry_run should return Ok(true)");
    assert!(!tgt.exists(), "dry_run must not create the target file");
    assert!(src.exists(), "dry_run must not remove the source file");
}

#[test]
fn process_file_dry_run_move_returns_ok_true_without_moving_file() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("in.txt");
    std::fs::write(&src, b"data").expect("write src");
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: true,
        move_files: true,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(result.unwrap(), true, "dry_run --move should return Ok(true)");
    assert!(!tgt.exists(), "dry_run --move must not create the target file");
    assert!(src.exists(), "dry_run --move must not remove the source file");
}

#[test]
fn process_file_copy_success_creates_file_and_keeps_source() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("in.txt");
    std::fs::write(&src, b"hello").expect("write src");
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: false,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(result.unwrap(), true, "successful copy should return Ok(true)");
    assert!(tgt.exists(), "copy must create the target file");
    assert!(src.exists(), "copy must not remove the source file");
}

#[test]
fn process_file_move_success_creates_target_and_removes_source() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("in.txt");
    std::fs::write(&src, b"hello").expect("write src");
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: true,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(result.unwrap(), true, "successful move should return Ok(true)");
    assert!(tgt.exists(), "move must create the target file");
    assert!(!src.exists(), "move must remove the source file");
}

#[test]
fn process_file_copy_missing_source_soft_error_returns_ok_false() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("no_such_file.txt"); // intentionally absent
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: false,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(
        result.unwrap(),
        false,
        "missing source + stop_on_error=false should return Ok(false)"
    );
}

#[test]
fn process_file_copy_missing_source_hard_error_returns_err() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("no_such_file.txt"); // intentionally absent
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: false,
        stop_on_error: true,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert!(
        result.is_err(),
        "missing source + stop_on_error=true should return Err"
    );
}

#[test]
fn process_file_move_missing_source_soft_error_returns_ok_false() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("no_such_file.txt"); // intentionally absent
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: true,
        stop_on_error: false,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert_eq!(
        result.unwrap(),
        false,
        "missing source (move) + stop_on_error=false should return Ok(false)"
    );
}

#[test]
fn process_file_move_missing_source_hard_error_returns_err() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let src = dir.path().join("no_such_file.txt"); // intentionally absent
    let tgt = dir.path().join("out.txt");
    let opts = ProcessOptions {
        dry_run: false,
        move_files: true,
        stop_on_error: true,
        show_detail_info: false,
    };
    let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
    assert!(
        result.is_err(),
        "missing source (move) + stop_on_error=true should return Err"
    );
}
```

Also update the `use super::` line at the top of the test module to import the new items:

```rust
use super::{check_directory, log_level, process_file, ProcessOptions};
```

- [ ] **Step 2: Run tests to confirm they fail**

```
cd /Volumes/SSD/Source/Rust/gather
cargo test process_file 2>&1 | head -30
```

Expected: compilation error — `process_file` and `ProcessOptions` not yet defined.

- [ ] **Step 3: Implement `ProcessOptions` and `process_file` in `src/utils.rs`**

Add the following **before** the `#[cfg(test)]` block (i.e., after the closing `}` of `log_build`):

```rust
/// Options that control per-file processing behaviour, collected from CLI flags.
pub struct ProcessOptions {
    pub dry_run: bool,
    pub move_files: bool,
    pub stop_on_error: bool,
    pub show_detail_info: bool,
}

/// Process a single source file: dry-run preview, copy, or move.
///
/// # Returns
///
/// - `Ok(true)`  — the file was processed (copied or moved).
/// - `Ok(false)` — the operation failed but `stop_on_error` is `false`;
///   the caller should count this as a skipped file and continue.
/// - `Err(...)`  — the operation failed and `stop_on_error` is `true`;
///   the caller should propagate the error and halt.
pub fn process_file(
    source: &str,
    target: &std::path::Path,
    opts: &ProcessOptions,
) -> anyhow::Result<bool> {
    let target_display = target.display();

    if opts.dry_run {
        if opts.move_files {
            println!("  {source} --> {target_display}");
        } else {
            println!("  {source} ==> {target_display}");
        }
        return Ok(true);
    }

    if opts.move_files {
        log::debug!("Moving {source} to {target_display}");
        match std::fs::rename(source, target) {
            Ok(()) => {
                if opts.show_detail_info {
                    log::info!("  {source} --> {target_display}");
                }
                Ok(true)
            }
            Err(err) => {
                if opts.stop_on_error {
                    anyhow::bail!(
                        "Error: {err}. Unable to move {source} to {target_display}. Halting."
                    );
                }
                log::warn!("Unable to move {source} to {target_display}. Continuing.");
                Ok(false)
            }
        }
    } else {
        log::debug!("Copying {source} to {target_display}");
        match std::fs::copy(source, target) {
            Ok(_) => {
                if opts.show_detail_info {
                    log::info!("  {source} ==> {target_display}");
                }
                Ok(true)
            }
            Err(err) => {
                if opts.stop_on_error {
                    anyhow::bail!(
                        "Error: {err}. Unable to copy {source} to {target_display}. Halting."
                    );
                }
                log::warn!("Unable to copy {source} to {target_display}. Continuing.");
                Ok(false)
            }
        }
    }
}
```

- [ ] **Step 4: Run the new unit tests**

```
cd /Volumes/SSD/Source/Rust/gather
cargo test process_file -- --nocapture 2>&1
```

Expected: all 8 `process_file_*` tests **PASS**.

- [ ] **Step 5: Run the full test suite to confirm nothing regressed**

```
cd /Volumes/SSD/Source/Rust/gather
cargo test 2>&1
```

Expected: all existing tests still **PASS**.

- [ ] **Step 6: Check clippy and formatting**

```
cd /Volumes/SSD/Source/Rust/gather
cargo fmt && cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings, zero errors.

---

### Task 2: Refactor `run()` in `main.rs` to call `utils::process_file`

**Files:**
- Modify: `src/main.rs` (replace the inline if/else chain with `ProcessOptions` + `utils::process_file`)

**Interfaces:**
- Consumes (from Task 1):
  ```rust
  utils::ProcessOptions { dry_run, move_files, stop_on_error, show_detail_info }
  utils::process_file(source: &str, target: &Path, opts: &ProcessOptions)
      -> anyhow::Result<bool>
  ```

---

- [ ] **Step 1: Replace the per-file dispatch block in `run()`**

Replace the complete body of `run()` in `src/main.rs` with:

```rust
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

    let opts = utils::ProcessOptions {
        dry_run: cli_args.get_flag("dry-run"),
        move_files: cli_args.get_flag("move"),
        stop_on_error: cli_args.get_flag("stop"),
        show_detail_info: !cli_args.get_flag("detail-off"),
    };
    let print_summary = cli_args.get_flag("summary");
    log::debug!(
        "move_files: {}, stop_on_error: {}, show_detail_info: {}, dry_run: {}, print_summary: {}",
        opts.move_files,
        opts.stop_on_error,
        opts.show_detail_info,
        opts.dry_run,
        print_summary
    );

    if opts.dry_run {
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
            if opts.stop_on_error {
                anyhow::bail!("Error: Invalid filename in path: {source}. Halting.");
            }
            log::warn!("Invalid filename in path: {source}. Continuing.");
            skipped_file_count += 1;
            continue;
        };

        let new_filename = Path::new(target_dir).join(file_name);

        match utils::process_file(source, &new_filename, &opts)? {
            true => processed_file_count += 1,
            false => skipped_file_count += 1,
        }
    } // for source

    if print_summary {
        // Write directly to stdout so the summary is never silenced by -q/--quiet.
        println!("Total files examined:        {total_file_count:5}");
        if opts.move_files {
            println!("Files moved:                 {processed_file_count:5}");
        } else {
            println!("Files copied:                {processed_file_count:5}");
        }
        println!("Files skipped due to errors: {skipped_file_count:5}");
    }

    Ok(())
} // fn run()
```

- [ ] **Step 2: Run the full test suite**

```
cd /Volumes/SSD/Source/Rust/gather
cargo test 2>&1
```

Expected: **all tests PASS** — unit tests in `utils.rs` and all integration tests in `tests/cli.rs`.

- [ ] **Step 3: Check clippy and formatting**

```
cd /Volumes/SSD/Source/Rust/gather
cargo fmt && cargo clippy -- -D warnings 2>&1
```

Expected: zero warnings, zero errors.

- [ ] **Step 4: Commit**

```bash
cd /Volumes/SSD/Source/Rust/gather
/opt/homebrew/bin/git-mit es 2>/dev/null || true
git add src/utils.rs src/main.rs
git commit -m "$(cat <<'EOF'
refactor(main): extract per-file dispatch into process_file (gtr-3aa)

Add ProcessOptions struct and process_file() to utils.rs so the
dry-run / move / copy logic is independently unit-testable. run()
now constructs ProcessOptions from CLI flags and delegates per-file
work to utils::process_file, returning Ok(true/false) or Err for
the hard-stop case. No behaviour changes; all existing tests pass.

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 5: Close the crumb**

```bash
/Users/evensolberg/.cargo/bin/crumbs close gtr-3aa
git add .crumbs/extract-per-file-processing-logic-into-a-dedicated-function.md
git commit -m "$(cat <<'EOF'
chore(crumbs): close gtr-3aa — process_file extracted

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

---

## Self-Review

**Spec coverage:**
- ✅ Extract `dry_run / move / copy` block → `process_file` in Task 1 + Task 2
- ✅ `ProcessOptions` struct with all four fields: `dry_run`, `move_files`, `stop_on_error`, `show_detail_info`
- ✅ Return type `anyhow::Result<bool>`: `Ok(true)` processed, `Ok(false)` skipped, `Err` hard stop
- ✅ Unit tests for `process_file` in `utils.rs` (8 cases)
- ✅ `run()` refactored to use `utils::process_file`
- ✅ All existing integration tests still exercised
- ✅ Uses anyhow throughout (no `Box<dyn Error>` — assumes gtr-jz8 applied first)

**Placeholder scan:** No TBDs, no vague steps, all code blocks are complete.

**Type consistency:** `ProcessOptions` defined in Task 1 Step 3 matches its use in Task 2 Step 1 field-for-field. `process_file` signature is identical in tests (Task 1 Step 1), implementation (Task 1 Step 3), and call-site (Task 2 Step 1). All error returns use `anyhow::bail!` consistently.
