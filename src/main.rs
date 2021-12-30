use clap::{App, Arg}; // Command line
use std::error::Error;
use std::path::Path;

// Logging
use env_logger::{Builder, Target};
use log::LevelFilter;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// This is where the magic happens.
fn run() -> Result<(), Box<dyn Error>> {
    // Set up the command line. Ref https://docs.rs/clap for details.
    let cli_args = App::new(clap::crate_name!())
        .about(clap::crate_description!())
        .version(clap::crate_version!())
        // .author(clap::crate_authors!("\n"))
        .long_about("Gathers files from directories and subdirectories into a target directory.")
        .arg(
            Arg::with_name("read")
                .value_name("FILE(S)")
                .help("One or more file(s) to process. Wildcards and multiple files (e.g. 2019*.pdf 2020*.pdf) are supported. Use ** glob to recurse (i.e. **/*.pdf). Note: Case sensitive.")
                .takes_value(true)
                .required(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("target")
                .value_name("TARGET")
                .help("The target directory into which files are to be gathered.")
                .takes_value(true)
                .required(true)
                .last(true)
                .multiple(false),
        )
        .arg( // Move rather than copy files
            Arg::with_name("move")
                .short("m")
                .long("move")
                .multiple(true)
                .help("Move files instead of copying them.")
                .takes_value(false)
                .hidden(false),
        )
        .arg( // Stop on error
            Arg::with_name("stop")
                .short("s")
                .long("stop-on-error")
                .multiple(true)
                .help("Stop on error. If this flag isn't set, the application will attempt to continue in case of error.")
                .takes_value(false)
                .hidden(false),
        )
        .arg( // Dry-run
            Arg::with_name("dry-run")
                .short("r")
                .long("dry-run")
                .multiple(false)
                .help("Iterate through the files and produce output without actually processing anything.")
                .takes_value(false)
        )
        .arg( // Hidden debug parameter
            Arg::with_name("debug")
                .short("d")
                .long("debug")
                .multiple(true)
                .help("Output debug information as we go. Supply it twice for trace-level logs.")
                .takes_value(false)
                .hidden(false),
        )
        .arg( // Don't print any information
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .multiple(false)
                .help("Don't produce any output except errors while working.")
                .takes_value(false)
        )
        .arg( // Print summary information
            Arg::with_name("summary")
                .short("p")
                .long("print-summary")
                .multiple(false)
                .help("Print summary information about the number of files gathered.")
                .takes_value(false)
        )
        .arg( // Don't show detail information
            Arg::with_name("detail-off")
                .short("o")
                .long("detail-off")
                .multiple(false)
                .help("Don't print detailed information about each file processed.")
                .takes_value(false)
        )
        .get_matches();

    // create a log builder
    let mut logbuilder = Builder::new();

    // Figure out what log level to use.
    if cli_args.is_present("quiet") {
        logbuilder.filter_level(LevelFilter::Off);
    } else {
        match cli_args.occurrences_of("debug") {
            0 => logbuilder.filter_level(LevelFilter::Info),
            1 => logbuilder.filter_level(LevelFilter::Debug),
            _ => logbuilder.filter_level(LevelFilter::Trace),
        };
    }

    // Initialize logging
    logbuilder.target(Target::Stdout).init();

    // create a list of the files to gather
    let files_to_gather = cli_args.values_of("read").unwrap();
    log::debug!("files_to_gather: {:?}", files_to_gather);

    // Verify that the target exists and that it is a directory
    let target_dir = cli_args.value_of("target").unwrap();
    log::trace!("target_dir: {:?}", target_dir);
    let td_metadata = std::fs::metadata(&target_dir);
    match td_metadata {
        Ok(td_md) => {
            if !td_md.is_dir() {
                return Err("Specified target is not a directory. Unable to proceed.".into());
            } else {
                log::debug!("Specified target is a directory. Procceeding.");
            }
        }
        Err(err) => {
            let error_message = format!("Target: {}", err);
            return Err(error_message.into());
        }
    }

    let move_files = cli_args.is_present("move");
    if move_files {
        log::debug!("Move flag set. Gathering files by moving.");
    } else {
        log::debug!("Move flag not set. Gathering files by copying.");
    }

    let stop_on_error = cli_args.is_present("stop");
    if stop_on_error {
        log::debug!("Stop on error flag set. Will stop if errors occur.");
    } else {
        log::debug!("Stop on error flag not set. Will attempt to continue in case of errors.");
    }

    let show_detail_info = !cli_args.is_present("detail-off");
    let dry_run = cli_args.is_present("dry-run");
    if dry_run {
        log::info!("Dry-run starting.");
    }

    let mut total_file_count: usize = 0;
    let mut processed_file_count: usize = 0;
    let mut skipped_file_count: usize = 0;

    // Gather files
    for filename in files_to_gather {
        let new_filename =
            Path::new(target_dir).join(Path::new(&filename).file_name().unwrap_or_default());
        let targetfile = new_filename.as_path();

        total_file_count += 1;

        if dry_run {
            if move_files {
                log::info!("  {} ==> {}", filename, targetfile.to_str().unwrap());
                processed_file_count += 1;
            } else {
                log::info!("  {} --> {}", filename, targetfile.to_str().unwrap());
                processed_file_count += 1;
            }
        } else {
            if move_files {
                log::debug!("Moving file {} to {}", filename, targetfile.display());
                match std::fs::rename(&filename, targetfile) {
                    Ok(_) => {
                        if show_detail_info {
                            log::info!("  {} ==> {}", filename, targetfile.to_str().unwrap());
                        }
                        processed_file_count += 1;
                    }
                    Err(err) => {
                        if stop_on_error {
                            return Err(format!(
                                "Error: {}. Unable to move file {} to {}. Halting.",
                                err,
                                filename,
                                targetfile.to_str().unwrap()
                            )
                            .into());
                        } else {
                            log::warn!(
                                "Unable to move file {} to {}. Continuing.",
                                filename,
                                targetfile.to_str().unwrap()
                            );
                            skipped_file_count += 1;
                        }
                    }
                }
            } else {
                // Copy files
                log::debug!("Copying file {} to {}", &filename, &targetfile.display());
                match std::fs::copy(&filename, targetfile) {
                    Ok(_) => {
                        if show_detail_info {
                            log::info!("  {} --> {}", filename, targetfile.to_str().unwrap());
                        }
                        processed_file_count += 1;
                    }
                    Err(err) => {
                        if stop_on_error {
                            return Err(format!(
                                "Error: {}. Unable to copy file {} to {}. Halting.",
                                err,
                                filename,
                                targetfile.to_str().unwrap()
                            )
                            .into());
                        } else {
                            log::warn!(
                                "Unable to copy file {} to {}. Continuing.",
                                filename,
                                targetfile.to_str().unwrap()
                            );
                            skipped_file_count += 1;
                        }
                    }
                }
            } // if move_files
        } // if dry_run
    } // for filename

    // Print summary information
    if cli_args.is_present("summary") {
        log::info!("Total files examined:        {:5}", total_file_count);
        if move_files {
            log::info!("Files moved:                 {:5}", processed_file_count);
        } else {
            log::info!("Files copied:                {:5}", processed_file_count);
        }
        log::info!("Files skipped due to errors: {:5}", skipped_file_count);
    }

    // Everything is a-okay in the end
    Ok(())
} // fn run()

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// The actual executable function that gets called when the program in invoked.
fn main() {
    std::process::exit(match run() {
        Ok(_) => 0, // everying is hunky dory - exit with code 0 (success)
        Err(err) => {
            log::error!("{}", err.to_string().replace("\"", ""));
            1 // exit with a non-zero return code, indicating a problem
        }
    });
}
