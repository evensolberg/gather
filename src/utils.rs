use clap::parser::ValueSource;
use env_logger::{Builder, Target};
use log::LevelFilter;

pub fn check_directory(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let td_metadata = std::fs::metadata(target);
    match td_metadata {
        Ok(td_md) => {
            if !td_md.is_dir() {
                return Err("Specified target is not a directory. Unable to proceed.".into());
            }
            log::debug!("Specified target is a directory. Procceeding.");
        }
        Err(err) => {
            let error_message = format!("Target: {err}");
            return Err(error_message.into());
        }
    }

    Ok(())
}

pub fn log_build(cli_args: &clap::ArgMatches) -> Builder {
    // create a log builder
    let mut logbuilder = Builder::new();

    // Figure out what log level to use.
    if cli_args.value_source("quiet") == Some(ValueSource::CommandLine) {
        logbuilder.filter_level(LevelFilter::Off);
    } else {
        match cli_args.get_count("debug") {
            0 => logbuilder.filter_level(LevelFilter::Info),
            1 => logbuilder.filter_level(LevelFilter::Debug),
            _ => logbuilder.filter_level(LevelFilter::Trace),
        };
    }

    // Initialize logging
    logbuilder.target(Target::Stdout).init();

    // return the log builder
    logbuilder
}
