mod cli;
mod utils;

use rayon::prelude::*;

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// This is where the magic happens.
fn run() -> anyhow::Result<()> {
    // Set up the command line. Ref https://docs.rs/clap for details.
    let cli_args = cli::build();

    // Set up logging
    utils::log_build(&cli_args);

    // Collect source paths into a Vec so we can run the pre-flight existence
    // check over the whole list before touching any files.
    let sources: Vec<&str> = cli_args
        .get_many::<String>("read")
        .unwrap_or_default()
        .map(String::as_str)
        .collect();
    log::debug!("files_to_gather: {sources:?}");

    // Verify that the target exists and that it is a directory
    let target_dir = cli_args.get_one::<String>("target").expect(
        "default_value('.') guarantees target is always present — this is a clap bug if None",
    );
    log::trace!("target_dir: {target_dir:?}");
    utils::check_directory(target_dir)?;

    let opts = utils::ProcessOptions {
        dry_run: cli_args.get_flag("dry-run"),
        move_files: cli_args.get_flag("move"),
        stop_on_error: cli_args.get_flag("stop"),
        show_detail_info: !cli_args.get_flag("detail-off"),
    };
    let print_summary = cli_args.get_flag("summary");
    let serial = cli_args.get_flag("serial");
    log::debug!(
        "dry_run: {}, move_files: {}, stop_on_error: {}, show_detail_info: {}, print_summary: {}, serial: {}",
        opts.dry_run,
        opts.move_files,
        opts.stop_on_error,
        opts.show_detail_info,
        print_summary,
        serial,
    );

    if opts.dry_run {
        // Write directly to stdout so the banner is never silenced by -q/--quiet.
        // The quiet flag sets LevelFilter::Off; even log::error! is filtered out.
        println!("Starting dry-run.");
    }

    // Pre-flight: abort if any source path is absent, inaccessible, or not a
    // regular file before touching any files.  Dry-run intentionally skips
    // this — dry-run is a best-effort preview ("what would happen?") and
    // should show per-file notices rather than aborting, even with --stop.
    if opts.stop_on_error && !opts.dry_run {
        utils::validate_sources(&sources)?;
    }

    // Process files in parallel by default, serially when --serial/-1 is set.
    //
    // Serial path (including dry-run):
    //   A plain for-loop with ? inside so --stop-on-error short-circuits on the
    //   first error without touching remaining files.  Dry-run always runs serially
    //   so preview lines appear in input order, which matters for auditing an
    //   ordered file list.
    //
    // Parallel path:
    //   par_iter().collect() dispatches all workers concurrently.  collect::<Vec<Result>>
    //   is eager and non-short-circuiting, so --stop-on-error cannot abort in-flight
    //   operations; it surfaces the first Err after all workers complete.  For true
    //   halt-on-first-error semantics combine --serial with --stop.
    // Every source produces exactly one outcome, so total is known up front.
    let total_file_count = sources.len();
    let (processed_file_count, skipped_file_count) = if serial || opts.dry_run {
        let mut processed = 0;
        let mut skipped = 0;
        for &source in &sources {
            if utils::process_source(source, target_dir, &opts)? {
                processed += 1;
            } else {
                skipped += 1;
            }
        }
        (processed, skipped)
    } else {
        let results: Vec<anyhow::Result<bool>> = sources
            .par_iter()
            .map(|&source| utils::process_source(source, target_dir, &opts))
            .collect();
        let mut processed = 0;
        let mut skipped = 0;
        for result in results {
            if result? {
                processed += 1;
            } else {
                skipped += 1;
            }
        }
        (processed, skipped)
    };

    if print_summary {
        // Write directly to stdout so the summary is never silenced by -q/--quiet.
        // The quiet flag sets LevelFilter::Off; if we used log::info! or log::error!
        // here they would be filtered out when the two flags are combined.
        println!("Total files examined:        {total_file_count:5}");
        if opts.move_files {
            println!("Files moved:                 {processed_file_count:5}");
        } else {
            println!("Files copied:                {processed_file_count:5}");
        }
        println!("Files skipped due to errors: {skipped_file_count:5}");
    }

    Ok(())
} // fn run()

//////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// The actual executable function that gets called when the program is invoked.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // Use eprintln! so fatal errors always appear on stderr regardless of
            // the logger's filter level (which -q/--quiet sets to LevelFilter::Off).
            // {err} uses Display — no spurious quote characters around the message.
            // Returning ExitCode::FAILURE (rather than calling process::exit) lets
            // destructors and buffered log flushes run on the error path too.
            eprintln!("{err}");
            std::process::ExitCode::FAILURE
        }
    }
}
