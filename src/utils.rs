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

/// Build a logging configuration based on CLI input.
pub fn log_build(cli_args: &clap::ArgMatches) -> Builder {
    // create a log builder
    let mut logbuilder = Builder::new();

    // Figure out what log level to use.
    if cli_args.get_flag("quiet") {
        logbuilder.filter_level(LevelFilter::Error);
    } else {
        match cli_args.get_count("debug") {
            0 => logbuilder.filter_level(LevelFilter::Info),
            1 => logbuilder.filter_level(LevelFilter::Debug),
            _ => logbuilder.filter_level(LevelFilter::Trace),
        };
    }

    // Route all log output to stdout so it shares the same fd as the
    // println!-based summary output (see main.rs print_summary block).
    // Both streams write to the same fd; the logger uses its own internal
    // buffer while println! goes through Rust's LineWriter (line-flushed).
    logbuilder.target(Target::Stdout).init();

    // return the log builder
    logbuilder
}
