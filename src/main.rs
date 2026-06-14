mod cli;
mod utils;

use std::error::Error;
use std::path::Path;

// Logging

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// This is where the magic happens.
fn run() -> Result<(), Box<dyn Error>> {
    // Set up the command line. Ref https://docs.rs/clap for details.
    let cli_args = cli::build();

    // Set up logging
    utils::log_build(&cli_args);

    // create a list of the files to gather
    let sources = cli_args
        .get_many::<String>("read")
        .unwrap_or_default()
        .map(std::string::String::as_str);
    log::debug!("files_to_gather: {sources:?}");

    // Verify that the target exists and that it is a directory
    let target_dir = cli_args.get_one::<String>("target").expect(
        "default_value('.') guarantees target is always present — this is a clap bug if None",
    );
    log::trace!("target_dir: {target_dir:?}");
    utils::check_directory(target_dir)?;

    let move_files = cli_args.get_flag("move");
    let stop_on_error = cli_args.get_flag("stop");
    let show_detail_info = !cli_args.get_flag("detail-off");
    let dry_run = cli_args.get_flag("dry-run");
    let print_summary = cli_args.get_flag("summary");
    log::debug!("move_files: {move_files}, stop_on_error: {stop_on_error}, show_detail_info: {show_detail_info}, dry_run: {dry_run}, print_summary: {print_summary}");

    if dry_run {
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
            if stop_on_error {
                return Err(format!("Error: Invalid filename in path: {source}. Halting.").into());
            }
            log::warn!("Invalid filename in path: {source}. Continuing.");
            skipped_file_count += 1;
            continue;
        };

        let new_filename = Path::new(target_dir).join(file_name);
        let target = new_filename.display();

        if dry_run {
            // Write directly to stdout so previews are never silenced by -q/--quiet.
            // Same reasoning as the banner above and the print_summary block below.
            if move_files {
                println!("  {source} --> {target}");
            } else {
                println!("  {source} ==> {target}");
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

    if print_summary {
        // Write directly to stdout so the summary is never silenced by -q/--quiet.
        // The quiet flag sets LevelFilter::Off; if we used log::info! or log::error!
        // here they would be filtered out when the two flags are combined.
        println!("Total files examined:        {total_file_count:5}");
        if move_files {
            println!("Files moved:                 {processed_file_count:5}");
        } else {
            println!("Files copied:                {processed_file_count:5}");
        }
        println!("Files skipped due to errors: {skipped_file_count:5}");
    }

    Ok(())
} // fn run()

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// The actual executable function that gets called when the program in invoked.
fn main() {
    std::process::exit(match run() {
        Ok(()) => 0, // everything is hunky dory - exit with code 0 (success)
        Err(err) => {
            // Use eprintln! rather than log::error! so fatal errors are always
            // visible even when -q/--quiet sets LevelFilter::Off.
            eprintln!("{}", err.to_string().replace('\"', ""));
            1 // exit with a non-zero return code, indicating a problem
        }
    });
}
