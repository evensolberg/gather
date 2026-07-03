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

/// Pre-flight existence check: report all missing source paths before processing.
///
/// Iterates over `sources` and collects every path that does not exist on disk.
/// When `stop_on_error` is `true` and `dry_run` is `false`, returns an error
/// listing all missing paths so the user sees the complete picture in one
/// message before any files are moved or copied.  In all other cases the
/// function returns `Ok(())` without emitting any output; per-file feedback for
/// missing sources is handled by `process_file` when the copy or move fails.
///
/// # Returns
///
/// - `Ok(())` when all paths exist, or when the caller has opted out of hard
///   errors (`stop_on_error = false`, or `dry_run = true`).
/// - `Err(...)` when one or more paths are absent and `stop_on_error` is `true`
///   and `dry_run` is `false`.
pub fn validate_sources(sources: &[&str], opts: &ProcessOptions) -> anyhow::Result<()> {
    // Only meaningful in hard-error, non-dry-run mode.  The caller guards this,
    // but the check is kept here too so the function is self-contained for tests.
    if !opts.stop_on_error || opts.dry_run {
        return Ok(());
    }

    // Use try_exists() rather than exists() so that OS errors such as "permission
    // denied" surface as immediate hard errors instead of being silently treated
    // as "not found".  exists() returns false for *any* metadata failure; only
    // Ok(false) from try_exists() means the file is genuinely absent.
    //
    // Note: a TOCTOU race exists between this check and the actual fs::copy /
    // fs::rename in process_file.  A file deleted between the two points will
    // produce a false-clean pre-flight followed by a mid-run copy error.  This
    // is inherent in any check-then-act design and is acceptable for the
    // single-user interactive use case this tool targets.
    let mut missing: Vec<&str> = Vec::new();
    for &s in sources.iter() {
        match std::path::Path::new(s).try_exists() {
            Ok(true) => {}  // file exists — nothing to do
            Ok(false) => missing.push(s),
            Err(err) => {
                // An OS-level error while probing existence (e.g. permission
                // denied) will almost certainly prevent the copy/move from
                // succeeding too; surface it immediately with context.
                return Err(anyhow::Error::from(err))
                    .with_context(|| format!("Unable to access source file '{s}'"));
            }
        }
    }

    if missing.is_empty() {
        return Ok(());
    }

    // Include every missing path in the error message so it appears on stderr
    // via the `eprintln!` in main(), giving the user the full picture before any
    // files are moved or copied.
    anyhow::bail!(
        "{} source file(s) not found:\n  {}\nHalting.",
        missing.len(),
        missing.join("\n  ")
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
        // Guard against inaccessible sources: a file absent or unreadable at
        // dry-run time would not be copied in a real run either (TOCTOU
        // notwithstanding).  Use try_exists() so permission errors are not
        // silently misreported as "not found".  Use println! so the notice is
        // always visible, matching the arrow line which also bypasses the logger
        // (-q does not suppress it).
        match std::path::Path::new(source).try_exists() {
            Ok(false) => {
                println!("  {source} (not found — would be skipped)");
                return Ok(false);
            }
            Err(_) => {
                println!("  {source} (not accessible — would be skipped)");
                return Ok(false);
            }
            Ok(true) => {} // file exists — show the preview arrow below
        }
        println!("  {source} {arrow} {target_display}");
        return Ok(true);
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
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        assert!(
            validate_sources(&sources, &opts).is_ok(),
            "all paths exist — should return Ok"
        );
    }

    #[test]
    fn validate_sources_empty_slice_returns_ok() {
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        assert!(
            validate_sources(&[], &opts).is_ok(),
            "empty source list should return Ok"
        );
    }

    #[test]
    fn validate_sources_missing_with_stop_on_error_returns_err() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let absent = dir.path().join("no_such_file.txt");
        let sources = [absent.to_str().expect("utf-8")];
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: true,
            show_detail_info: false,
        };
        assert!(
            validate_sources(&sources, &opts).is_err(),
            "missing path + stop_on_error=true should return Err"
        );
    }

    #[test]
    fn validate_sources_missing_without_stop_on_error_returns_ok() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let absent = dir.path().join("no_such_file.txt");
        let sources = [absent.to_str().expect("utf-8")];
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: false,
            show_detail_info: false,
        };
        assert!(
            validate_sources(&sources, &opts).is_ok(),
            "missing path + stop_on_error=false should return Ok (per-file handling deferred to process_file)"
        );
    }

    #[test]
    fn validate_sources_multiple_missing_all_halt_on_stop_on_error() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let a = dir.path().join("missing_a.txt");
        let b = dir.path().join("missing_b.txt");
        let sources = [a.to_str().expect("utf-8"), b.to_str().expect("utf-8")];
        let opts = ProcessOptions {
            dry_run: false,
            move_files: false,
            stop_on_error: true,
            show_detail_info: false,
        };
        let result = validate_sources(&sources, &opts);
        assert!(
            result.is_err(),
            "multiple missing paths + stop_on_error=true should return Err"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("2 source file(s) not found"),
            "error message should contain '2 source file(s) not found'; got: {msg}"
        );
    }

    #[test]
    fn validate_sources_dry_run_missing_with_stop_on_error_returns_ok() {
        // dry-run is non-destructive — missing files should warn but not abort.
        let dir = tempfile::tempdir().expect("create temp dir");
        let absent = dir.path().join("no_such_file.txt");
        let sources = [absent.to_str().expect("utf-8")];
        let opts = ProcessOptions {
            dry_run: true,
            move_files: false,
            stop_on_error: true,
            show_detail_info: false,
        };
        assert!(
            validate_sources(&sources, &opts).is_ok(),
            "dry-run + stop_on_error=true + missing path should still return Ok"
        );
    }
}
