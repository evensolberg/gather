mod cli;
mod utils;

use clap::parser::ValueSource;
use std::error::Error;
use std::path::Path;

// Logging

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// This is where the magic happens.
fn run() -> Result<(), Box<dyn Error>> {
    // Set up the command line. Ref https://docs.rs/clap for details.
    let cli_args = cli::build();

    // Set up logging
    let _logbuilder = utils::log_build(&cli_args);

    // create a list of the files to gather
    let sources = cli_args
        .get_many::<String>("read")
        .unwrap_or_default()
        .map(std::string::String::as_str);
    log::debug!("files_to_gather: {sources:?}");

    // Verify that the target exists and that it is a directory
    let target_dir = cli_args
        .get_one::<String>("target")
        .ok_or("target argument is missing — this is a bug; default_value should guarantee it")?;
    log::trace!("target_dir: {target_dir:?}");
    utils::check_directory(target_dir)?;

    let move_files = cli_args.value_source("move") == Some(ValueSource::CommandLine);
    let stop_on_error = cli_args.value_source("stop") == Some(ValueSource::CommandLine);
    let show_detail_info = cli_args.value_source("detail-off") != Some(ValueSource::CommandLine);
    let dry_run = cli_args.value_source("dry-run") == Some(ValueSource::CommandLine);
    log::debug!("move_files: {move_files}, stop_on_error: {stop_on_error}, show_detail_info: {show_detail_info}, dry_run: {dry_run}");

    if dry_run {
        log::info!("Starting dry-run.");
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
                return Err(
                    format!("Error: Invalid filename in path: {source}. Halting.").into(),
                );
            }
            log::warn!("Invalid filename in path: {source}. Continuing.");
            skipped_file_count += 1;
            continue;
        };

        let new_filename = Path::new(target_dir).join(file_name);
        let target = new_filename.display();

        if dry_run {
            if move_files {
                log::info!("  {source} --> {target}");
            } else {
                log::info!("  {source} ==> {target}");
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
                        return Err(format!(
                            "Error: {err}. Unable to move {source} to {target}. Halting."
                        )
                        .into());
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
                        return Err(format!(
                            "Error: {err}. Unable to copy {source} to {target}. Halting."
                        )
                        .into());
                    }
                    log::warn!("Unable to copy {source} to {target}. Continuing.");
                    skipped_file_count += 1;
                }
            }
        } // if dry_run
    } // for filename

    if cli_args.value_source("summary") == Some(ValueSource::CommandLine) {
        log::info!("Total files examined:        {total_file_count:5}");
        if move_files {
            log::info!("Files moved:                 {processed_file_count:5}");
        } else {
            log::info!("Files copied:                {processed_file_count:5}");
        }
        log::info!("Files skipped due to errors: {skipped_file_count:5}");
    }

    Ok(())
} // fn run()

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// The actual executable function that gets called when the program in invoked.
fn main() {
    std::process::exit(match run() {
        Ok(()) => 0, // everything is hunky dory - exit with code 0 (success)
        Err(err) => {
            log::error!("{}", err.to_string().replace('\"', ""));
            1 // exit with a non-zero return code, indicating a problem
        }
    });
}
