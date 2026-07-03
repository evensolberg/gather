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
/// - `Result<(), Box<dyn std::error::Error>>` - returns an empty `Ok()` if it is a directory, or an error if not.
pub fn check_directory(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = std::fs::metadata(target).map_err(|e| format!("Target: {e}"))?;
    if !metadata.is_dir() {
        return Err("Specified target is not a directory. Unable to proceed.".into());
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
