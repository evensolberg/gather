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

#[cfg(test)]
mod tests {
    use super::{check_directory, log_level, process_file, ProcessOptions};
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
        assert_eq!(
            result.unwrap(),
            true,
            "dry_run --move should return Ok(true)"
        );
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
        assert_eq!(
            result.unwrap(),
            true,
            "successful copy should return Ok(true)"
        );
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
        assert_eq!(
            result.unwrap(),
            true,
            "successful move should return Ok(true)"
        );
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
}
