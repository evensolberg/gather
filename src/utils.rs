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

/// Determine the log level from CLI arguments.
///
/// Quiet mode suppresses all log output (`Off`). Otherwise the number of
/// `-d`/`--debug` flags selects the level: 0 → `Info`, 1 → `Debug`, 2+ → `Trace`.
fn log_level(cli_args: &clap::ArgMatches) -> LevelFilter {
    if cli_args.get_flag("quiet") {
        LevelFilter::Off
    } else {
        match cli_args.get_count("debug") {
            0 => LevelFilter::Info,
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    }
}

/// Build a logging configuration based on CLI input.
pub fn log_build(cli_args: &clap::ArgMatches) -> Builder {
    // create a log builder
    let mut logbuilder = Builder::new();

    logbuilder.filter_level(log_level(cli_args));

    // Route all log output to stdout so it shares the same fd as the
    // println!-based summary output (see main.rs print_summary block).
    // Both streams write to the same fd; the logger uses its own internal
    // buffer while println! goes through Rust's LineWriter (line-flushed).
    logbuilder.target(Target::Stdout).init();

    // return the log builder
    logbuilder
}

#[cfg(test)]
mod tests {
    use super::{check_directory, log_level};
    use log::LevelFilter;

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    /// Build minimal `ArgMatches` that only expose the `quiet` and `debug` flags,
    /// without triggering the real CLI parser or touching the global logger.
    fn make_matches(quiet: bool, debug_count: u8) -> clap::ArgMatches {
        let mut argv: Vec<&str> = vec!["prog"];
        if quiet {
            argv.push("-q");
        }
        for _ in 0..debug_count {
            argv.push("-d");
        }
        clap::Command::new("prog")
            .arg(
                clap::Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("debug")
                    .short('d')
                    .long("debug")
                    .env("GATHER_DEBUG")
                    .action(clap::ArgAction::Count),
            )
            .try_get_matches_from(argv)
            .unwrap()
    }

    // ---------------------------------------------------------------------------
    // check_directory
    // ---------------------------------------------------------------------------

    #[test]
    fn check_directory_accepts_real_temp_dir() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        assert!(
            check_directory(dir.path().to_str().unwrap()).is_ok(),
            "expected Ok for a real directory"
        );
    }

    #[test]
    fn check_directory_rejects_file_path() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file_path = dir.path().join("dummy.txt");
        std::fs::write(&file_path, b"data").expect("failed to write temp file");
        let result = check_directory(file_path.to_str().unwrap());
        assert!(result.is_err(), "expected Err for a file path, not a directory");
    }

    #[test]
    fn check_directory_rejects_nonexistent_path() {
        let result = check_directory("/this/path/does/not/exist/anywhere/gather-test");
        assert!(result.is_err(), "expected Err for a nonexistent path");
    }

    // ---------------------------------------------------------------------------
    // log_level
    // ---------------------------------------------------------------------------

    #[test]
    fn log_level_quiet_returns_off() {
        let matches = make_matches(true, 0);
        assert_eq!(log_level(&matches), LevelFilter::Off);
    }

    #[test]
    fn log_level_no_flags_returns_info() {
        // Guard against GATHER_DEBUG being set in the caller's environment, which
        // would cause the env-bound debug arg to increment the count automatically.
        unsafe { std::env::remove_var("GATHER_DEBUG") };
        let matches = make_matches(false, 0);
        assert_eq!(log_level(&matches), LevelFilter::Info);
    }

    #[test]
    fn log_level_one_debug_flag_returns_debug() {
        let matches = make_matches(false, 1);
        assert_eq!(log_level(&matches), LevelFilter::Debug);
    }

    #[test]
    fn log_level_two_debug_flags_returns_trace() {
        let matches = make_matches(false, 2);
        assert_eq!(log_level(&matches), LevelFilter::Trace);
    }

    #[test]
    fn log_level_three_debug_flags_also_returns_trace() {
        // Confirms the wildcard arm covers all values above 1, not just exactly 2.
        let matches = make_matches(false, 3);
        assert_eq!(log_level(&matches), LevelFilter::Trace);
    }
}
