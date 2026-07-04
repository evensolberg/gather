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
    let metadata =
        std::fs::metadata(target).with_context(|| format!("Target directory '{target}'"))?;
    if !metadata.is_dir() {
        anyhow::bail!("Target '{target}' is not a directory. Unable to proceed.");
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

/// Options that control per-file processing behaviour, collected from CLI flags.
#[derive(Debug)]
pub struct ProcessOptions {
    pub dry_run: bool,
    pub move_files: bool,
    pub stop_on_error: bool,
    pub show_detail_info: bool,
}

/// Pre-flight existence check: verify all source paths can be processed.
///
/// Iterates over `sources` and calls `std::fs::metadata()` on each path to
/// check existence, accessibility, and file type in one syscall.  Returns an
/// error listing every problematic path so the user sees the complete picture
/// in one message before any files are moved or copied.
///
/// OS-level errors (e.g. permission denied) are collected into the same batch
/// message rather than causing an immediate return, so all unreachable paths
/// are reported together.
///
/// # Returns
///
/// - `Ok(())` when all source paths are regular files.
/// - `Err(...)` when one or more paths are absent, inaccessible, or not a
///   regular file.
pub fn validate_sources(sources: &[&str]) -> anyhow::Result<()> {
    // Collect every problematic path so the user sees the full picture before
    // any files are moved or copied.  Use metadata() for a single syscall that
    // checks existence, accessibility, and file type together — the same check
    // that process_file uses so pre-flight and real-run are consistent.
    //
    // Note: a TOCTOU race exists between this check and the actual fs::copy /
    // fs::rename in process_file.  A file deleted between the two points will
    // produce a false-clean pre-flight followed by a mid-run copy error.  This
    // is inherent in any check-then-act design and is acceptable for the
    // single-user interactive use case this tool targets.
    let mut problems: Vec<String> = Vec::new();
    for &s in sources {
        // metadata() follows symlinks (matching fs::copy behaviour).
        // NotFound → genuinely absent; other Err → OS error (e.g. permission
        // denied); Ok + !is_file() → exists but is a directory, pipe, etc.
        match std::fs::metadata(s) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                problems.push(format!("{s} (not found)"));
            }
            Err(err) => {
                // OS-level error: add inline so the batch message shows all
                // issues rather than stopping at the first one.
                problems.push(format!("{s} ({err})"));
            }
            Ok(meta) if !meta.is_file() => {
                problems.push(format!("{s} (not a regular file)"));
            }
            Ok(_) => {} // regular file — nothing to do
        }
    }

    if problems.is_empty() {
        return Ok(());
    }

    // Include every problem path in the error message so it appears on stderr
    // via the `eprintln!` in main(), giving the user the full picture before
    // any files are moved or copied.
    anyhow::bail!(
        "{} source path(s) cannot be processed:\n  {}\nHalting.",
        problems.len(),
        problems.join("\n  ")
    );
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
    let (verb, op, arrow) = if opts.move_files {
        ("Moving", "move", "-->")
    } else {
        ("Copying", "copy", "==>")
    };

    if opts.dry_run {
        // Use metadata() to check existence, accessibility, and file type in
        // one stat call.  metadata() follows symlinks (matching fs::copy
        // behaviour).  Dry-run is inherently best-effort — a file that passes
        // here could still fail on the real run (e.g. no read permission) — but
        // this covers the common cases the user needs to know about up front.
        // Use println! so notices are always visible regardless of -q/--quiet.
        match std::fs::metadata(source) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                println!("  {source} (not found — would be skipped)");
                return Ok(false);
            }
            Err(err) => {
                println!("  {source} (not accessible: {err} — would be skipped)");
                return Ok(false);
            }
            Ok(meta) if !meta.is_file() => {
                println!("  {source} (not a regular file — would be skipped)");
                return Ok(false);
            }
            Ok(_) => {} // regular file — show the preview arrow below
        }
        println!("  {source} {arrow} {target_display}");
        return Ok(true);
    }

    // Guard: check source exists and is a regular file before operating on it.
    // Without this, --move mode would silently succeed for a directory on Unix
    // (fs::rename is directory-aware), contradicting the dry-run preview and
    // the tool's file-only semantics.  All error arms match the corresponding
    // dry-run messages so the two modes report the same conditions consistently.
    match std::fs::metadata(source) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // Absent file — match dry-run "(not found — would be skipped)".
            if opts.stop_on_error {
                anyhow::bail!("'{source}' not found");
            }
            log::warn!("'{source}' not found. Skipping.");
            return Ok(false);
        }
        Err(err) => {
            // Inaccessible (e.g. permission denied on stat) — report now so
            // the message matches the dry-run "(not accessible: …)" output.
            if opts.stop_on_error {
                return Err(err)
                    .with_context(|| format!("'{source}' is not accessible"));
            }
            log::warn!("'{source}' is not accessible ({err}). Skipping.");
            return Ok(false);
        }
        Ok(meta) if !meta.is_file() => {
            if opts.stop_on_error {
                anyhow::bail!("'{source}' is not a regular file");
            }
            log::warn!("'{source}' is not a regular file. Skipping.");
            return Ok(false);
        }
        Ok(_) => {} // regular file — proceed to copy/rename
    }

    log::debug!("{verb} {source} to {target_display}");
    let result: Result<(), std::io::Error> = if opts.move_files {
        std::fs::rename(source, target)
    } else {
        std::fs::copy(source, target).map(|_| ())
    };

    match result {
        Ok(()) => {
            if opts.show_detail_info {
                log::info!("  {source} {arrow} {target_display}");
            }
            Ok(true)
        }
        Err(err) => {
            if opts.stop_on_error {
                return Err(err)
                    .with_context(|| format!("Unable to {op} '{source}' to '{target_display}'"));
            }
            log::warn!("Unable to {op} {source} to {target_display}. Continuing.");
            Ok(false)
        }
    }
}

/// Build a collision-free target path.
///
/// Returns `{dir}/{file_name}` when that path does not already exist and is
/// not already present in `claimed`.  Otherwise appends `_N` before the
/// extension (`report_1.pdf`, `report_2.pdf`, …) until a free path is found.
/// The search is bounded by `u32::MAX` — effectively infinite for any real
/// file system.
///
/// The `claimed` set lets callers track paths that have been "virtually
/// allocated" during a dry-run pass (where no files are written to disk).
/// Pass an empty set for normal (non-dry-run) operation, where the real
/// filesystem is the source of truth.
///
/// Note: there is an inherent TOCTOU race between the existence check here
/// and the subsequent write in [`process_file`].  Two parallel workers
/// processing files with the same basename could both observe the same
/// unoccupied path, then race to write it.  This is accepted for the
/// single-user interactive use-case this tool targets; running with
/// `--serial` eliminates the race entirely.
fn resolve_unique_target(
    dir: &std::path::Path,
    file_name: &std::ffi::OsStr,
    claimed: Option<&std::collections::HashSet<std::path::PathBuf>>,
) -> std::path::PathBuf {
    // A path is free when the filesystem does not have it AND it is absent
    // from the caller-supplied claimed set (if provided).  Passing None
    // skips the claimed check and incurs no allocation — the common case
    // for non-dry-run operation where the real filesystem is authoritative.
    let is_free =
        |p: &std::path::PathBuf| !p.exists() && claimed.is_none_or(|c| !c.contains(p));

    let base = dir.join(file_name);
    if is_free(&base) {
        return base;
    }

    let p = std::path::Path::new(file_name);
    // file_stem() returns None only for the empty string, which is already
    // rejected by the file_name() guard in process_source before we get here.
    let stem = p.file_stem().unwrap_or(file_name).to_string_lossy();
    let ext = p.extension().map(|e| e.to_string_lossy());

    for n in 1_u32.. {
        let new_name = match &ext {
            Some(e) => format!("{stem}_{n}.{e}"),
            None => format!("{stem}_{n}"),
        };
        let candidate = dir.join(new_name);
        if is_free(&candidate) {
            return candidate;
        }
    }
    // u32 exhausted — unreachable on any real file system.
    unreachable!(
        "u32 suffix range exhausted for '{}'",
        file_name.to_string_lossy()
    )
}

/// Process one source path: resolve a usable filename, build a collision-free
/// target path, then delegate to [`process_file`].
///
/// Isolating this logic from the main loop makes it callable from both a
/// serial iterator and a parallel Rayon iterator without duplicating the
/// filename-extraction guard.
///
/// When the computed target path already exists (two sources share the same
/// basename), [`resolve_unique_target`] appends a numeric suffix so both
/// files are preserved rather than the second silently overwriting the first.
/// A warning is emitted whenever the name changes.
///
/// ## Dry-run collision tracking
///
/// Because dry-run never writes files to disk, the real filesystem cannot
/// detect within-pass collisions.  Callers in dry-run mode must supply a
/// `claimed` set; `process_source` consults the set when resolving the target
/// and registers the chosen path so the next call sees it as occupied.  Pass
/// `None` for normal (non-dry-run) operation — the filesystem is then the
/// sole source of truth.
///
/// # Returns
///
/// - `Ok(true)`  — the file was processed (copied or moved).
/// - `Ok(false)` — the path was skipped (invalid filename or soft file error).
/// - `Err(...)`  — a hard error occurred and `stop_on_error` is `true`.
pub fn process_source(
    source: &str,
    target_dir: &str,
    opts: &ProcessOptions,
    claimed: Option<&mut std::collections::HashSet<std::path::PathBuf>>,
) -> anyhow::Result<bool> {
    let Some(file_name) = std::path::Path::new(source).file_name() else {
        if opts.stop_on_error {
            anyhow::bail!("Invalid filename in path: '{source}'. Halting.");
        }
        log::warn!("Invalid filename in path: '{source}'. Continuing.");
        return Ok(false);
    };

    // Resolve a collision-free target path.  In dry-run mode the caller
    // provides a `claimed` set so collisions are detected without disk writes.
    // claimed.as_deref() converts Option<&mut HashSet> → Option<&HashSet>,
    // passing None when there is no set (non-dry-run) — no allocation needed.
    let target_path = resolve_unique_target(
        std::path::Path::new(target_dir),
        file_name,
        claimed.as_deref(),
    );

    // Register the chosen path so subsequent calls in the same dry-run pass
    // see it as occupied (claimed is None in non-dry-run mode, so this is a no-op).
    if let Some(c) = claimed {
        c.insert(target_path.clone());
    }

    // Warn when the target name was changed to avoid a silent overwrite.
    // Emit only the filename (not the full path) to keep the message scannable.
    // resolve_unique_target always joins a name onto the directory, so the
    // target always has a file_name; the guard filters out the no-change case.
    let final_name = target_path.file_name();
    if final_name != Some(file_name) {
        let renamed = final_name.unwrap_or(file_name).to_string_lossy();
        log::warn!("Name collision: '{source}' written as '{renamed}' to avoid overwriting an existing file.");
    }

    process_file(source, &target_path, opts)
}

#[cfg(test)]
mod tests {
    use super::{
        check_directory, log_level, process_file, process_source, resolve_unique_target,
        validate_sources, ProcessOptions,
    };
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

    // ---------------------------------------------------------------------------
    // process_file
    // ---------------------------------------------------------------------------

    #[test]
    fn process_file_directory_source_soft_error_returns_ok_false() {
        // A directory passed as a source must be rejected with Ok(false) rather
        // than silently moved (fs::rename on a directory succeeds on Unix and
        // would move the whole tree, contradicting the dry-run preview).
        let dir = tempfile::tempdir().expect("create temp dir");
        let src_dir = tempfile::tempdir().expect("create source dir");
        let tgt = dir.path().join("out.txt");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        let result = process_file(src_dir.path().to_str().expect("utf-8"), &tgt, &opts);
        assert!(
            !result.expect("should return Ok, not Err, in soft-error mode"),
            "directory source (copy, soft-error) should return Ok(false)"
        );
        assert!(!tgt.exists(), "no target should be created for a directory source");
    }

    #[test]
    fn process_file_directory_source_move_soft_error_returns_ok_false() {
        // Move mode is the critical case: fs::rename on a directory silently
        // succeeds on Unix — without the is_file() guard the entire directory
        // would be relocated to the target, which is never what the user wants.
        let dir = tempfile::tempdir().expect("create temp dir");
        let src_dir = tempfile::tempdir().expect("create source dir");
        let tgt = dir.path().join("out.txt");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: true,
            stop_on_error: false,
            show_detail_info: false,
        };
        let result = process_file(src_dir.path().to_str().expect("utf-8"), &tgt, &opts);
        assert!(
            !result.expect("should return Ok, not Err, in soft-error mode"),
            "directory source (move, soft-error) should return Ok(false)"
        );
        assert!(
            src_dir.path().exists(),
            "source directory must NOT be moved/renamed"
        );
    }

    #[test]
    fn process_file_directory_source_hard_error_returns_err() {
        // With stop_on_error the is_file() guard must bail rather than returning Ok(false).
        let dir = tempfile::tempdir().expect("create temp dir");
        let src_dir = tempfile::tempdir().expect("create source dir");
        let tgt = dir.path().join("out.txt");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: true,
            show_detail_info: false,
        };
        let result = process_file(src_dir.path().to_str().expect("utf-8"), &tgt, &opts);
        assert!(
            result.is_err(),
            "directory source + stop_on_error=true should return Err"
        );
    }

    #[test]
    fn process_file_dry_run_missing_source_returns_ok_false_without_creating_file() {
        // A missing source in dry-run must return Ok(false) and print a "(not found)"
        // notice instead of the success-looking "==>" arrow, which would give a false-safe
        // picture of what the real run would do.
        let dir = tempfile::tempdir().expect("create temp dir");
        let src = dir.path().join("no_such_file.txt"); // intentionally absent
        let tgt = dir.path().join("out.txt");
        let opts = ProcessOptions {
            dry_run: true,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        let result = process_file(src.to_str().expect("utf-8"), &tgt, &opts);
        assert!(
            !result.unwrap(),
            "dry_run + missing source should return Ok(false)"
        );
        assert!(!tgt.exists(), "dry_run must not create the target file");
    }

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
        assert!(result.unwrap(), "dry_run should return Ok(true)");
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
        assert!(result.unwrap(), "dry_run --move should return Ok(true)");
        assert!(
            !tgt.exists(),
            "dry_run --move must not create the target file"
        );
        assert!(
            src.exists(),
            "dry_run --move must not remove the source file"
        );
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
        assert!(result.unwrap(), "successful copy should return Ok(true)");
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
        assert!(result.unwrap(), "successful move should return Ok(true)");
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
        assert!(
            !result.unwrap(),
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
        assert!(
            !result.unwrap(),
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

    // ---------------------------------------------------------------------------
    // validate_sources
    // ---------------------------------------------------------------------------

    #[test]
    fn validate_sources_all_exist_returns_ok() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let f1 = dir.path().join("a.txt");
        let f2 = dir.path().join("b.txt");
        std::fs::write(&f1, b"").expect("write f1");
        std::fs::write(&f2, b"").expect("write f2");
        let sources = [
            f1.to_str().expect("utf-8"),
            f2.to_str().expect("utf-8"),
        ];
        assert!(
            validate_sources(&sources).is_ok(),
            "all paths exist — should return Ok(())"
        );
    }

    #[test]
    fn validate_sources_empty_slice_returns_ok() {
        assert!(
            validate_sources(&[]).is_ok(),
            "empty source list should return Ok(())"
        );
    }

    #[test]
    fn validate_sources_missing_returns_err() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let absent = dir.path().join("no_such_file.txt");
        let sources = [absent.to_str().expect("utf-8")];
        assert!(
            validate_sources(&sources).is_err(),
            "missing path should return Err"
        );
    }


    #[test]
    fn validate_sources_multiple_missing_reports_all() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let a = dir.path().join("missing_a.txt");
        let b = dir.path().join("missing_b.txt");
        let sources = [a.to_str().expect("utf-8"), b.to_str().expect("utf-8")];
        let result = validate_sources(&sources);
        assert!(result.is_err(), "multiple missing paths should return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("2 source path(s) cannot be processed"),
            "error message should contain count; got: {msg}"
        );
        // Both individual paths must be mentioned so the user sees the full picture.
        assert!(
            msg.contains(a.to_str().expect("utf-8")),
            "error message must mention the first missing path; got: {msg}"
        );
        assert!(
            msg.contains(b.to_str().expect("utf-8")),
            "error message must mention the second missing path; got: {msg}"
        );
    }

    #[test]
    fn validate_sources_directory_source_reports_not_regular_file() {
        // A directory that exists should be rejected by the pre-flight check with
        // "(not a regular file)" — not silently passed as "found" — so the user
        // sees the problem before any other files are moved or copied.
        let dir = tempfile::tempdir().expect("create temp dir");
        let sources = [dir.path().to_str().expect("utf-8")];
        let result = validate_sources(&sources);
        assert!(result.is_err(), "a directory source + stop_on_error=true should return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not a regular file"),
            "error message should mention 'not a regular file'; got: {msg}"
        );
    }

    // ---------------------------------------------------------------------------
    // process_source
    // ---------------------------------------------------------------------------

    #[test]
    fn process_source_invalid_path_soft_error_returns_ok_false() {
        // A path whose last component is ".." has no usable filename.
        // In soft-error mode this must return Ok(false) (logging a warning) rather than panic.
        let dir = tempfile::tempdir().expect("create temp dir");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        // "somedir/.." has no file_name() component.
        let bad_path = format!("{}/..",&dir.path().display());
        let result = process_source(&bad_path, dir.path().to_str().expect("utf-8"), &opts, None);
        assert!(
            !result.expect("soft-error invalid path should return Ok, not Err"),
            "invalid path in soft-error mode should return Ok(false)"
        );
    }

    #[test]
    fn process_source_invalid_path_hard_error_returns_err() {
        // With stop_on_error the invalid-path case must bail rather than Ok(false).
        let dir = tempfile::tempdir().expect("create temp dir");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: true,
            show_detail_info: false,
        };
        let bad_path = format!("{}/..",&dir.path().display());
        let result = process_source(&bad_path, dir.path().to_str().expect("utf-8"), &opts, None);
        assert!(result.is_err(), "invalid path + stop_on_error=true should return Err");
    }

    #[test]
    fn process_source_valid_file_copy_succeeds() {
        // process_source with a normal file delegates to process_file and copies it.
        let dir = tempfile::tempdir().expect("create temp dir");
        let src = dir.path().join("in.txt");
        std::fs::write(&src, b"hello").expect("write src");
        let target_dir = tempfile::tempdir().expect("create target dir");
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        let result = process_source(
            src.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            None,
        );
        assert!(result.expect("valid copy should return Ok"), "valid copy should return Ok(true)");
        assert!(target_dir.path().join("in.txt").exists(), "copy must create target file");
        assert!(src.exists(), "copy must not remove source file");
    }

    // ---------------------------------------------------------------------------
    // resolve_unique_target
    // ---------------------------------------------------------------------------

    #[test]
    fn resolve_unique_target_returns_base_when_no_collision() {
        // When the target path does not exist, the original name is returned as-is.
        let dir = tempfile::tempdir().expect("create temp dir");
        let name = std::ffi::OsStr::new("report.pdf");
        let result = resolve_unique_target(dir.path(), name, None);
        assert_eq!(
            result,
            dir.path().join("report.pdf"),
            "no collision: should return the unmodified basename"
        );
    }

    #[test]
    fn resolve_unique_target_suffix_1_when_base_exists() {
        // When report.pdf already occupies the target, the next candidate is report_1.pdf.
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("report.pdf"), b"original").expect("write");
        let name = std::ffi::OsStr::new("report.pdf");
        let result = resolve_unique_target(dir.path(), name, None);
        assert_eq!(
            result,
            dir.path().join("report_1.pdf"),
            "single collision: should return <stem>_1.<ext>"
        );
    }

    #[test]
    fn resolve_unique_target_skips_taken_suffixes() {
        // When both report.pdf and report_1.pdf exist, the function must skip to report_2.pdf.
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("report.pdf"), b"a").expect("write base");
        std::fs::write(dir.path().join("report_1.pdf"), b"b").expect("write _1");
        let name = std::ffi::OsStr::new("report.pdf");
        let result = resolve_unique_target(dir.path(), name, None);
        assert_eq!(
            result,
            dir.path().join("report_2.pdf"),
            "two collisions: should return <stem>_2.<ext>"
        );
    }

    #[test]
    fn resolve_unique_target_no_extension_appends_suffix() {
        // Files without an extension (e.g. Makefile) must be suffixed as Makefile_1.
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("Makefile"), b"x").expect("write");
        let name = std::ffi::OsStr::new("Makefile");
        let result = resolve_unique_target(dir.path(), name, None);
        assert_eq!(
            result,
            dir.path().join("Makefile_1"),
            "extension-less file: should return <name>_1"
        );
    }

    #[test]
    fn resolve_unique_target_respects_claimed_without_disk_collision() {
        // report.pdf does NOT exist on disk, but it is present in the claimed
        // set.  The function must treat it as occupied and return report_1.pdf.
        // This exercises the claimed-set path independently of the filesystem.
        let dir = tempfile::tempdir().expect("create temp dir");
        let mut claimed = std::collections::HashSet::new();
        claimed.insert(dir.path().join("report.pdf"));
        let name = std::ffi::OsStr::new("report.pdf");
        let result = resolve_unique_target(dir.path(), name, Some(&claimed));
        assert_eq!(
            result,
            dir.path().join("report_1.pdf"),
            "path absent on disk but in claimed: should return <stem>_1.<ext>"
        );
    }

    // ---------------------------------------------------------------------------
    // process_source — collision avoidance (integration)
    // ---------------------------------------------------------------------------

    #[test]
    fn process_source_dry_run_collision_shows_distinct_targets() {
        // In dry-run mode no files are written to disk, so resolve_unique_target
        // would see an empty target for every call and predict the same path for
        // both colliding sources.  The caller must supply a `claimed` set so that
        // successive calls within the same pass claim distinct target paths.
        let src_dir_a = tempfile::tempdir().expect("create src dir a");
        let src_dir_b = tempfile::tempdir().expect("create src dir b");
        let target_dir = tempfile::tempdir().expect("create target dir");

        let src_a = src_dir_a.path().join("report.pdf");
        let src_b = src_dir_b.path().join("report.pdf");
        std::fs::write(&src_a, b"a").expect("write src a");
        std::fs::write(&src_b, b"b").expect("write src b");

        let opts = ProcessOptions {
            dry_run: true,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };

        let mut claimed = std::collections::HashSet::new();

        let result_a = process_source(
            src_a.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            Some(&mut claimed),
        );
        assert!(
            result_a.expect("first dry-run should succeed"),
            "first dry-run: Ok(true)"
        );

        let result_b = process_source(
            src_b.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            Some(&mut claimed),
        );
        assert!(
            result_b.expect("second dry-run should succeed"),
            "second dry-run: Ok(true)"
        );

        // No files must be created (dry-run guarantee).
        assert!(
            !target_dir.path().join("report.pdf").exists(),
            "dry-run must not create report.pdf"
        );
        assert!(
            !target_dir.path().join("report_1.pdf").exists(),
            "dry-run must not create report_1.pdf"
        );

        // Both predicted target paths must be registered in the claimed set.
        let base = target_dir.path().join("report.pdf");
        let renamed = target_dir.path().join("report_1.pdf");
        assert!(
            claimed.contains(&base),
            "claimed must contain the first predicted target"
        );
        assert!(
            claimed.contains(&renamed),
            "claimed must contain the second (renamed) predicted target"
        );
    }

    #[test]
    fn process_source_collision_keeps_both_files() {
        // When two source files share the same basename (common when gathering
        // from multiple directories), both must be preserved in the target.
        // The second must NOT overwrite the first; it must be renamed <stem>_1.<ext>.
        let src_dir_a = tempfile::tempdir().expect("create src dir a");
        let src_dir_b = tempfile::tempdir().expect("create src dir b");
        let target_dir = tempfile::tempdir().expect("create target dir");

        let src_a = src_dir_a.path().join("report.pdf");
        let src_b = src_dir_b.path().join("report.pdf");
        std::fs::write(&src_a, b"content-a").expect("write src a");
        std::fs::write(&src_b, b"content-b").expect("write src b");

        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };

        // First file lands as report.pdf.
        let result_a = process_source(
            src_a.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            None,
        );
        assert!(
            result_a.expect("first copy should not error"),
            "first copy should return Ok(true)"
        );

        // Second file must be written as report_1.pdf, not overwriting report.pdf.
        let result_b = process_source(
            src_b.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            None,
        );
        assert!(
            result_b.expect("second copy should not error"),
            "second copy should return Ok(true)"
        );

        // Original file must be intact.
        let first = std::fs::read(target_dir.path().join("report.pdf"))
            .expect("report.pdf must exist");
        assert_eq!(first, b"content-a", "first file must not be overwritten");

        // Renamed copy must contain the second source's content.
        let renamed = target_dir.path().join("report_1.pdf");
        assert!(renamed.exists(), "second copy must be renamed to report_1.pdf");
        let second = std::fs::read(&renamed).expect("report_1.pdf must be readable");
        assert_eq!(second, b"content-b", "second file content must be preserved");
    }

    #[test]
    fn process_source_collision_move_mode_keeps_both_files() {
        // Collision avoidance must also work in move mode: the second source must
        // be moved to a suffixed path, the first must remain at its original name,
        // and both source files must be absent afterwards (moved, not copied).
        let src_dir_a = tempfile::tempdir().expect("create src dir a");
        let src_dir_b = tempfile::tempdir().expect("create src dir b");
        let target_dir = tempfile::tempdir().expect("create target dir");

        let src_a = src_dir_a.path().join("report.pdf");
        let src_b = src_dir_b.path().join("report.pdf");
        std::fs::write(&src_a, b"content-a").expect("write src a");
        std::fs::write(&src_b, b"content-b").expect("write src b");

        let opts = ProcessOptions {
            dry_run: false,
            move_files: true,
            stop_on_error: false,
            show_detail_info: false,
        };

        let result_a = process_source(
            src_a.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            None,
        );
        assert!(
            result_a.expect("first move should not error"),
            "first move should return Ok(true)"
        );

        let result_b = process_source(
            src_b.to_str().expect("utf-8"),
            target_dir.path().to_str().expect("utf-8"),
            &opts,
            None,
        );
        assert!(
            result_b.expect("second move should not error"),
            "second move should return Ok(true)"
        );

        // Both source files must have been removed (moved).
        assert!(!src_a.exists(), "first source must be gone after move");
        assert!(!src_b.exists(), "second source must be gone after move");

        // Both targets must exist with correct content.
        let first = std::fs::read(target_dir.path().join("report.pdf"))
            .expect("report.pdf must exist");
        assert_eq!(first, b"content-a", "first target must not be overwritten");

        let renamed = target_dir.path().join("report_1.pdf");
        assert!(renamed.exists(), "second move must produce report_1.pdf");
        let second = std::fs::read(&renamed).expect("report_1.pdf must be readable");
        assert_eq!(second, b"content-b", "second target content must be preserved");
    }

}
