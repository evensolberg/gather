/// Integration tests for the gather CLI.
///
/// These tests compile and run the binary directly to verify end-to-end behaviour
/// without mocking any internals.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// Path to the compiled binary, resolved by Cargo at test-build time.
const GATHER: &str = env!("CARGO_BIN_EXE_gather");

/// RAII guard: removes its directory tree when dropped.
/// This ensures cleanup runs even if a test panics mid-assertion.
struct TempGuard(PathBuf);

impl Drop for TempGuard {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// Creates a unique temp directory for this test run, writes a single
/// `sample.txt` source file, and prepares a `dst/` sub-directory.
///
/// The directory name includes the PID so concurrent `cargo test`
/// invocations on the same machine (e.g. CI matrix retries) cannot
/// collide on the same path. Within a single process the tag strings
/// are already distinct.
///
/// Returns `(guard, src_file, dst_dir)`. The directory is removed
/// automatically when `guard` is dropped.
fn setup_tmp(tag: &str) -> (TempGuard, PathBuf, PathBuf) {
    // PID differentiates across concurrent cargo test invocations on the
    // same machine. Within a single process, test tags ("qp", "p_only",
    // "q_only", "dry_p") are already distinct.
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("gather_test_{tag}_{pid}"));
    let dst = root.join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");
    let src = root.join("sample.txt");
    fs::write(&src, b"hello").expect("write sample file");
    (TempGuard(root), src, dst)
}

/// Runs `gather` with the given extra args, asserting that it exits 0.
/// Returns the decoded stdout content.
///
/// The `assert!(status.success())` check is centralised here so all tests
/// get consistent failure messages that include stderr, preventing drift
/// between test bodies.
fn run_gather(args: &[&str]) -> String {
    let output = Command::new(GATHER)
        .args(args)
        .output()
        .expect("failed to run gather");
    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// -------------------------------------------------------------------
// Bug: gtr-b6p — quiet mode silently defeats --print-summary/-p flag
// -------------------------------------------------------------------

/// When `-q` and `-p` are combined, the summary MUST still appear on
/// stdout.  Before the fix this test fails because `log::info!` is
/// silenced by `LevelFilter::Error`.
#[test]
fn quiet_and_print_summary_both_show_summary() {
    let (_guard, src, dst) = setup_tmp("qp");
    let stdout = run_gather(&[
        "-q",
        "-p",
        src.to_str().unwrap(),
        "-t",
        dst.to_str().unwrap(),
    ]);
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout when -q -p combined, got:\n{stdout}"
    );
}

/// Without `-q`, `-p` alone must also show the summary (regression guard).
#[test]
fn print_summary_without_quiet_shows_summary() {
    let (_guard, src, dst) = setup_tmp("p_only");
    let stdout = run_gather(&["-p", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout for -p alone, got:\n{stdout}"
    );
}

/// `-q` without `-p` must NOT print any summary — quiet is still quiet.
#[test]
fn quiet_without_print_summary_suppresses_output() {
    let (_guard, src, dst) = setup_tmp("q_only");
    let stdout = run_gather(&["-q", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        stdout.is_empty(),
        "stdout must be empty when only -q is given (no -p), got:\n{stdout}"
    );
}

/// `--dry-run` combined with `-p` must still print the summary.
/// (dry-run does not touch files but the counts are real.)
#[test]
fn dry_run_with_print_summary_shows_summary() {
    let (_guard, src, dst) = setup_tmp("dry_p");
    let stdout = run_gather(&[
        "-n",
        "-p",
        src.to_str().unwrap(),
        "-t",
        dst.to_str().unwrap(),
    ]);
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout for -n -p combined, got:\n{stdout}"
    );
}

// -------------------------------------------------------------------
// Bug: gtr-bdh — dry-run output silenced by -q (same root cause as gtr-b6p)
// -------------------------------------------------------------------

/// `--dry-run` alone must print the "Starting dry-run." banner on stdout.
/// Regression guard: verifies the baseline before the -q interaction test.
#[test]
fn dry_run_shows_banner() {
    let (_guard, src, dst) = setup_tmp("dry_banner");
    let stdout = run_gather(&["-n", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        stdout.contains("Starting dry-run."),
        "expected dry-run banner in stdout for -n, got:\n{stdout}"
    );
}

/// `--dry-run` alone must show per-file copy-preview lines on stdout,
/// including the actual source path.
/// Regression guard: verifies the baseline before the -q interaction test.
#[test]
fn dry_run_shows_file_preview() {
    let (_guard, src, dst) = setup_tmp("dry_file");
    let src_str = src.to_str().unwrap();
    let dst_str = dst.to_str().unwrap();
    let stdout = run_gather(&["-n", src_str, "-t", dst_str]);
    assert!(
        stdout.contains("==>") && stdout.contains(src_str) && stdout.contains(dst_str),
        "expected copy-preview '{src_str} ==> {dst_str}' in stdout for -n, got:\n{stdout}"
    );
}

/// `--dry-run --move` must show the move-preview `-->` arrow on stdout,
/// including the actual source path.
/// Regression guard for the move path.
#[test]
fn dry_run_move_shows_move_preview() {
    let (_guard, src, dst) = setup_tmp("dry_move");
    let src_str = src.to_str().unwrap();
    let dst_str = dst.to_str().unwrap();
    let stdout = run_gather(&["-n", "--move", src_str, "-t", dst_str]);
    assert!(
        stdout.contains("-->") && stdout.contains(src_str) && stdout.contains(dst_str),
        "expected move-preview '{src_str} --> {dst_str}' in stdout for -n --move, got:\n{stdout}"
    );
}

/// `-q` combined with `--dry-run` must STILL show the dry-run banner and
/// per-file preview on stdout.  Before the fix the `log::info!` calls in the
/// dry-run paths are silenced by `LevelFilter::Error`, producing zero output
/// and making `-n -q` completely useless.
#[test]
fn quiet_and_dry_run_still_shows_preview() {
    let (_guard, src, dst) = setup_tmp("dry_q");
    let src_str = src.to_str().unwrap();
    let dst_str = dst.to_str().unwrap();
    let stdout = run_gather(&["-n", "-q", src_str, "-t", dst_str]);
    assert!(
        stdout.contains("Starting dry-run."),
        "expected dry-run banner when -n -q combined, got:\n{stdout}"
    );
    assert!(
        stdout.contains("==>") && stdout.contains(src_str) && stdout.contains(dst_str),
        "expected file preview '{src_str} ==> {dst_str}' when -n -q combined, got:\n{stdout}"
    );
}

/// `-q` combined with `--dry-run --move` must STILL show the `-->` move-preview
/// on stdout.  Regression guard for the move arm of the dry-run block.
#[test]
fn quiet_and_dry_run_move_still_shows_preview() {
    let (_guard, src, dst) = setup_tmp("dry_qm");
    let src_str = src.to_str().unwrap();
    let dst_str = dst.to_str().unwrap();
    let stdout = run_gather(&["-n", "-q", "--move", src_str, "-t", dst_str]);
    assert!(
        stdout.contains("Starting dry-run."),
        "expected dry-run banner when -n -q --move combined, got:\n{stdout}"
    );
    assert!(
        stdout.contains("-->") && stdout.contains(src_str) && stdout.contains(dst_str),
        "expected move preview '{src_str} --> {dst_str}' when -n -q --move combined, got:\n{stdout}"
    );
}

// -------------------------------------------------------------------
// gtr-6ug / gtr-bmr — modernise main(): error-path behaviour guards
// -------------------------------------------------------------------

/// Returns a `PathBuf` that is guaranteed not to exist: a named subdirectory
/// inside a freshly-created `TempDir`.  The `TempDir` handle must be kept
/// alive for the duration of the test (RAII drop removes the parent).
///
/// Using temp-dir infrastructure (rather than a hard-coded absolute path)
/// ensures the path is absent on every machine without relying on filesystem
/// conventions that could be violated on CI.
fn absent_path(tmp: &tempfile::TempDir) -> std::path::PathBuf {
    tmp.path().join("no_such_subdir")
}

/// A nonexistent target directory must cause gather to exit with code 1.
/// Regression guard: ensures the modernised `main()` preserves the non-zero exit.
#[test]
fn bad_target_exits_with_code_one() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output = Command::new(GATHER)
        .arg("somefile.txt")
        .arg("-t")
        .arg(absent_path(&tmp)) // PathBuf: avoids to_str().unwrap() on non-UTF-8 paths
        .output()
        .expect("failed to run gather");
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for a missing target directory, got {:?}",
        output.status,
    );
}

/// Error messages must appear on stderr, not stdout, and the process must exit 1.
/// Regression guard: a naïve `fn main() -> Result<…>` could route errors to
/// the wrong stream if the runtime's Termination impl is used incorrectly.
#[test]
fn error_message_goes_to_stderr() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output = Command::new(GATHER)
        .arg("somefile.txt")
        .arg("-t")
        .arg(absent_path(&tmp))
        .output()
        .expect("failed to run gather");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 on error, got {:?}",
        output.status,
    );
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr, got nothing"
    );
    assert!(
        stdout.is_empty(),
        "expected stdout to be empty on error, got:\n{stdout}"
    );
}

/// Error messages from `check_directory` must not contain wrapping quote characters.
/// Regression guard: `fn main() -> Result<(), Box<dyn Error>>` uses `{:?}`
/// (Debug) which wraps String-backed errors in quotes — this test catches that.
#[test]
fn error_message_contains_no_quotes() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let output = Command::new(GATHER)
        .arg("somefile.txt")
        .arg("-t")
        .arg(absent_path(&tmp))
        .output()
        .expect("failed to run gather");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr, got nothing"
    );
    assert!(
        !stderr.contains('"'),
        "check_directory error must not contain quote characters; got:\n{stderr}"
    );
}

/// Error messages from a failed copy (the `--stop-on-error` path) must also
/// contain no quote characters.  The copy/move error format embeds an `io::Error`
/// via Display; this test guards that the Display representation is quote-free.
#[test]
fn copy_error_message_contains_no_quotes() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    // Use a valid target dir so check_directory passes and the copy loop runs.
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    let output = Command::new(GATHER)
        .arg("--stop-on-error")
        // A well-formed filename (file_name() is Some) but absent on disk,
        // so fs::copy fails and — with --stop-on-error — the error reaches main().
        .arg("gather_test_nosuchfile_abc123.txt")
        .arg("-t")
        .arg(&dst) // PathBuf: avoids to_str().unwrap() on non-UTF-8 paths
        .output()
        .expect("failed to run gather");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for a copy error with --stop-on-error, got {:?}",
        output.status,
    );
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr, got nothing"
    );
    assert!(
        !stderr.contains('"'),
        "copy error message must not contain quote characters; got:\n{stderr}"
    );
}
