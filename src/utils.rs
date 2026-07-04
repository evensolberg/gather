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

#[cfg(test)]
mod tests {
    use super::{check_directory, log_level, process_file, validate_sources, ProcessOptions};
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
    fn validate_sources_missing_with_stop_on_error_returns_err() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let absent = dir.path().join("no_such_file.txt");
        let sources = [absent.to_str().expect("utf-8")];
        assert!(
            validate_sources(&sources).is_err(),
            "missing path should return Err"
        );
    }


    #[test]
    fn validate_sources_multiple_missing_all_halt_on_stop_on_error() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let a = dir.path().join("missing_a.txt");
        let b = dir.path().join("missing_b.txt");
        let sources = [a.to_str().expect("utf-8"), b.to_str().expect("utf-8")];
        let result = validate_sources(&sources);
        assert!(
            result.is_err(),
            "multiple missing paths + stop_on_error=true should return Err"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("2 source path(s) cannot be processed"),
            "error message should contain '2 source path(s) cannot be processed'; got: {msg}"
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

}
