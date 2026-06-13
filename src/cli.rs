use clap::{Arg, ArgAction, ArgMatches, Command};

pub fn build() -> ArgMatches {
    build_command().get_matches()
}

fn build_command() -> Command {
    Command::new(clap::crate_name!())
        .about(clap::crate_description!())
        .version(clap::crate_version!())
        // .author(clap::crate_authors!("\n"))
        .long_about("Gathers files from directories and subdirectories into a target directory.")
        .arg(
            Arg::new("read")
                .value_name("FILE(S)")
                .help("One or more file(s) to process. Wildcards and multiple_occurrences files (e.g. 2019*.pdf 2020*.pdf) are supported. Use ** glob to recurse (i.e. **/*.pdf). Note: Case sensitive.")
                .required(true)
                .num_args(1..)
                .action(ArgAction::Append)
        )
        .arg(
            Arg::new("target")
                .short('t')
                .long("target")
                .value_name("TARGET")
                .help("The target directory into which files are to be gathered. Defaults to the current directory.")
                .num_args(1)
                .default_value(".")
                .action(ArgAction::Set)
        )
        .arg( // Move rather than copy files
            Arg::new("move")
                .short('m')
                .long("move")
                .help("Move files instead of copying them.")
                .action(ArgAction::SetTrue)
        )
        .arg( // Stop on error
            Arg::new("stop")
                .short('s')
                .long("stop-on-error")
                .help("Stop on error. If this flag isn't set, the application will attempt to continue in case of error.")
                .action(ArgAction::SetTrue)
        )
        .arg( // Dry-run
            Arg::new("dry-run")
                .short('n')
                .long("dry-run")
                .help("Iterate through the files and produce output without actually processing anything.")
                .action(ArgAction::SetTrue)
        )
        .arg( // Hidden debug parameter
            Arg::new("debug")
                .short('d')
                .long("debug")
                .help("Output debug information as we go. Supply it twice for trace-level logs.")
                .hide(true)
                .env("GATHER_DEBUG")
                .num_args(0)
                .action(ArgAction::Count)
        )
        .arg( // Don't print any information
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress all output except errors. Dry-run previews (-n) and explicit summaries (-p) always appear regardless of this flag.")
                .action(ArgAction::SetTrue)
        )
        .arg( // Print summary information
            Arg::new("summary")
                .short('p')
                .long("print-summary")
                .help("Print summary information about the number of files gathered.")
                .action(ArgAction::SetTrue)
        )
        .arg( // Don't show detail information
            Arg::new("detail-off")
                .short('o')
                .long("detail-off")
                .help("Don't print detailed information about each file processed.")
                .action(ArgAction::SetTrue)
        )
}

#[cfg(test)]
mod tests {
    use super::build_command;

    #[test]
    fn target_defaults_to_current_directory_when_omitted() {
        let matches = build_command()
            .try_get_matches_from(["gather", "file.txt"])
            .expect("parsing should succeed even without an explicit target");
        assert_eq!(
            matches.get_one::<String>("target").map(String::as_str),
            Some(".")
        );
    }

    #[test]
    fn target_uses_provided_directory() {
        let matches = build_command()
            .try_get_matches_from(["gather", "file.txt", "--target", "/tmp/out"])
            .expect("parsing should succeed with an explicit target via --target");
        assert_eq!(
            matches.get_one::<String>("target").map(String::as_str),
            Some("/tmp/out")
        );
    }

    #[test]
    fn short_flag_sets_target() {
        let matches = build_command()
            .try_get_matches_from(["gather", "-t", "/tmp/out", "file.txt"])
            .expect("parsing should succeed with -t flag");
        assert_eq!(
            matches.get_one::<String>("target").map(String::as_str),
            Some("/tmp/out")
        );
    }

    #[test]
    fn bare_path_without_target_flag_goes_to_read_arg() {
        // Without -t/--target, a trailing path like /dest is consumed by the
        // greedy `read` arg — not silently adopted as the target directory.
        let matches = build_command()
            .try_get_matches_from(["gather", "file.txt", "/dest"])
            .expect("parsing should succeed — /dest goes to read, not target");
        let read_values: Vec<&str> = matches
            .get_many::<String>("read")
            .unwrap()
            .map(String::as_str)
            .collect();
        assert!(
            read_values.contains(&"/dest"),
            "expected /dest in read args, got {read_values:?}"
        );
        assert_eq!(
            matches.get_one::<String>("target").map(String::as_str),
            Some("."),
            "target should be the default '.' when -t is omitted"
        );
    }
}
