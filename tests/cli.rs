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

/// A nonexistent target directory triggers an error event whose properties are
/// jointly produced and cannot vary independently: exit code 1, error on stderr
/// (not stdout), and no wrapping quote characters in the message.
/// All three are asserted from a single process spawn.
///
/// Regression guard: a naïve `fn main() -> Result<(), Box<dyn Error>>` produces
/// `Error: "message"` via `{:?}` (Debug); a naïve `Termination` path routes to
/// the wrong stream.  This test catches both.
#[test]
fn bad_target_error_path_is_correct() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let bad_target = tmp.path().join("no_such_subdir"); // guaranteed absent
    let output = Command::new(GATHER)
        .arg("somefile.txt")
        .arg("-t")
        .arg(&bad_target)
        .output()
        .expect("failed to run gather");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for a missing target directory, got {:?}",
        output.status,
    );
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr, got nothing"
    );
    assert!(
        output.stdout.is_empty(),
        "expected stdout to be empty on error, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains('"'),
        "error message must not contain quote characters; got:\n{stderr}"
    );
}

/// Helper: run an error-path test and return the captured output.
/// Sets up a valid target directory, passes a nonexistent source file,
/// and optionally enables --move; with --stop-on-error the error propagates
/// to main() so the full error-formatting path is exercised.
fn run_gather_copy_or_move_error(use_move: bool) -> std::process::Output {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");
    let mut cmd = Command::new(GATHER);
    cmd.arg("--stop-on-error");
    if use_move {
        cmd.arg("--move");
    }
    cmd.arg("gather_test_nosuchfile_abc123.txt") // well-formed name but absent on disk
        .arg("-t")
        .arg(dst)
        .output()
        .expect("failed to run gather")
}

/// Error messages from a failed copy (the `--stop-on-error` path) must contain
/// no quote characters, route to stderr (not stdout), and produce exit code 1.
/// Guards that the io::Error Display embedded in the copy-error format string is
/// quote-free and that the error stream is correct.
#[test]
fn copy_error_message_contains_no_quotes() {
    let output = run_gather_copy_or_move_error(false);
    let stderr = String::from_utf8_lossy(&output.stderr);
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
        output.stdout.is_empty(),
        "expected stdout to be empty on copy error, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains('"'),
        "copy error message must not contain quote characters; got:\n{stderr}"
    );
}

/// Error messages from a failed move (--move --stop-on-error path) must also
/// contain no quote characters.  The move path uses std::fs::rename, which
/// can produce different io::Error variants than fs::copy; this test guards
/// the rename Display is also quote-free.
#[test]
fn move_error_message_contains_no_quotes() {
    let output = run_gather_copy_or_move_error(true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for a move error with --stop-on-error, got {:?}",
        output.status,
    );
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr, got nothing"
    );
    assert!(
        output.stdout.is_empty(),
        "expected stdout to be empty on move error, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains('"'),
        "move error message must not contain quote characters; got:\n{stderr}"
    );
}

// -------------------------------------------------------------------
// gtr-wek — pre-flight existence validation before the processing loop
// -------------------------------------------------------------------

/// With `--stop-on-error`, a missing source file must cause exit 1 with an
/// error message on stderr *before* any other file is processed.  The real
/// file must NOT be copied — the pre-flight pass aborts before any I/O.
#[test]
fn preflight_missing_source_stop_on_error_exits_nonzero() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    // One real file + one absent file (tempdir-scoped to guarantee absence).
    let real = tmp.path().join("real.txt");
    fs::write(&real, b"data").expect("write real file");
    let missing = tmp.path().join("missing.txt"); // intentionally never created

    let output = Command::new(GATHER)
        .arg("--stop-on-error")
        .arg(real.to_str().unwrap())
        .arg(missing.to_str().unwrap())
        .arg("-t")
        .arg(&dst)
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(
        output.status.code(),
        Some(0),
        "expected non-zero exit when a source is missing with --stop-on-error; got 0\nstderr: {stderr}"
    );
    // The fatal error must name the missing path and appear on stderr.
    assert!(
        stderr.contains(missing.to_str().unwrap()),
        "expected missing path in stderr; got:\n{stderr}"
    );
    assert!(
        stderr.contains("not found"),
        "expected 'not found' in stderr; got:\n{stderr}"
    );
    // No normal processing output should appear on stdout.
    assert!(
        stdout.is_empty(),
        "expected stdout to be empty when pre-flight aborts; got:\n{stdout}"
    );
    assert!(
        !dst.join("real.txt").exists(),
        "real.txt must NOT be copied when pre-flight aborts before processing"
    );
}

/// Without `--stop-on-error`, a missing source file should produce a warning
/// on stdout but the process must still exit 0 and process the existing files.
#[test]
fn preflight_missing_source_without_stop_on_error_continues() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    let real = tmp.path().join("real.txt");
    fs::write(&real, b"data").expect("write real file");
    let missing = tmp.path().join("missing.txt"); // intentionally never created

    let output = Command::new(GATHER)
        .arg(real.to_str().unwrap())
        .arg(missing.to_str().unwrap())
        .arg("-t")
        .arg(&dst)
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected exit 0 (continue mode) for missing source without --stop-on-error; got {:?}\nstderr: {stderr}",
        output.status,
    );
    // The warning must mention the missing path and "not found" on stdout
    // (the logger is configured with Target::Stdout).
    assert!(
        stdout.contains(missing.to_str().unwrap()),
        "expected missing path in stdout warning; got:\n{stdout}"
    );
    assert!(
        stdout.contains("not found"),
        "expected 'not found' in stdout warning; got:\n{stdout}"
    );
    // Nothing should have been written to stderr in continue mode.
    assert!(
        stderr.is_empty(),
        "expected stderr to be empty in continue mode; got:\n{stderr}"
    );
    // The real file must have been copied despite the missing one.
    assert!(
        dst.join("real.txt").exists(),
        "real.txt must be copied even when another source is missing"
    );
}

/// With `--stop-on-error` and multiple missing files, ALL missing paths must
/// be reported in the pre-flight pass (not just the first one).
#[test]
fn preflight_multiple_missing_all_reported_with_stop_on_error() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    // Tempdir-scoped paths guarantee absence without relying on the working directory.
    let missing_a = tmp.path().join("absent_alpha.txt"); // intentionally never created
    let missing_b = tmp.path().join("absent_beta.txt"); // intentionally never created

    let output = Command::new(GATHER)
        .arg("--stop-on-error")
        .arg(missing_a.to_str().unwrap())
        .arg(missing_b.to_str().unwrap())
        .arg("-t")
        .arg(&dst)
        .output()
        .expect("failed to run gather");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Both missing absolute paths must appear somewhere in stderr output.
    assert!(
        stderr.contains(missing_a.to_str().unwrap()),
        "stderr must mention the first missing file; got:\n{stderr}"
    );
    assert!(
        stderr.contains(missing_b.to_str().unwrap()),
        "stderr must mention the second missing file; got:\n{stderr}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "expected non-zero exit when sources are missing with --stop-on-error"
    );
}

/// `--dry-run` with a missing source file must print a "(not found)" notice on
/// stdout so the user knows which files would be skipped — the summary alone
/// ("Files skipped due to errors: 1") provides no filename.
#[test]
fn dry_run_missing_source_shows_not_found_notice() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    // Tempdir-scoped path guarantees absence without relying on the working directory.
    let missing = tmp.path().join("absent_source.txt"); // intentionally never created
    let missing_str = missing.to_str().unwrap();

    let stdout = run_gather(&["--dry-run", missing_str, "-t", dst.to_str().unwrap()]);

    assert!(
        stdout.contains(missing_str),
        "expected the missing absolute path to appear in dry-run stdout; got:\n{stdout}"
    );
    assert!(
        stdout.contains("not found"),
        "expected a 'not found' notice in dry-run stdout for a missing source; got:\n{stdout}"
    );
}

/// `--dry-run` combined with `--stop-on-error` must still produce the preview
/// output and exit 0 — dry-run is a best-effort preview regardless of --stop.
/// validate_sources is intentionally skipped in dry-run mode; per-file "(not
/// found — would be skipped)" notices are shown instead of aborting.
#[test]
fn dry_run_with_stop_on_error_still_shows_preview() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");

    let real = tmp.path().join("real.txt");
    fs::write(&real, b"data").expect("write real file");
    let missing = tmp.path().join("missing.txt"); // intentionally never created

    let output = Command::new(GATHER)
        .arg("--dry-run")
        .arg("--stop-on-error")
        .arg(real.to_str().unwrap())
        .arg(missing.to_str().unwrap())
        .arg("-t")
        .arg(&dst)
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Dry-run must exit 0 even with --stop-on-error.
    assert!(
        output.status.success(),
        "expected exit 0 (dry-run is best-effort preview); got {:?}
stderr: {stderr}",
        output.status,
    );
    // The missing file must appear in the preview with a "(not found)" notice.
    assert!(
        stdout.contains("not found"),
        "expected '(not found)' in dry-run stdout for missing source; got:
{stdout}"
    );
    // No files should have been created in the destination.
    assert!(
        !dst.join("real.txt").exists(),
        "dry-run must not copy any files"
    );
}

// -------------------------------------------------------------------
// gtr-1a5 — --serial / -1 end-to-end integration tests
// -------------------------------------------------------------------

/// `--serial` with a single file must copy it to the target directory.
/// Smoke test: confirms the flag is accepted and processing runs.
#[test]
fn serial_copy_single_file() {
    let (_guard, src, dst) = setup_tmp("serial_single");
    run_gather(&["--serial", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        dst.join("sample.txt").exists(),
        "--serial must copy the file to the target directory"
    );
    assert_eq!(
        fs::read(dst.join("sample.txt")).unwrap(),
        b"hello",
        "--serial must preserve file content"
    );
}

/// `-1` (the short alias for `--serial`) must behave identically.
#[test]
fn serial_short_flag_copies_file() {
    let (_guard, src, dst) = setup_tmp("serial_short");
    run_gather(&["-1", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        dst.join("sample.txt").exists(),
        "-1 (--serial short alias) must copy the file to the target directory"
    );
}

/// `--serial` with multiple source files must copy all of them.
#[test]
fn serial_copy_multiple_files() {
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("gather_test_serial_multi_{pid}"));
    let dst = root.join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");
    let a = root.join("alpha.txt");
    let b = root.join("beta.txt");
    let c = root.join("gamma.txt");
    fs::write(&a, b"aaa").unwrap();
    fs::write(&b, b"bbb").unwrap();
    fs::write(&c, b"ccc").unwrap();
    let _guard = TempGuard(root);

    run_gather(&[
        "--serial",
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        c.to_str().unwrap(),
        "-t",
        dst.to_str().unwrap(),
    ]);

    assert!(dst.join("alpha.txt").exists(), "alpha.txt must be copied");
    assert!(dst.join("beta.txt").exists(), "beta.txt must be copied");
    assert!(dst.join("gamma.txt").exists(), "gamma.txt must be copied");
}

/// `--serial --move` must move the file (source gone, target present).
#[test]
fn serial_move_removes_source() {
    let (_guard, src, dst) = setup_tmp("serial_move");
    let src_str = src.to_str().unwrap();
    run_gather(&[
        "--serial",
        "--move",
        src_str,
        "-t",
        dst.to_str().unwrap(),
    ]);
    assert!(
        dst.join("sample.txt").exists(),
        "--serial --move must create the file at the target"
    );
    assert!(
        !src.exists(),
        "--serial --move must remove the source file"
    );
}

/// `--serial` with two sources sharing a basename must preserve both files:
/// the second must be renamed to `sample_1.txt`, not overwrite the first.
/// This verifies the serial loop's in-order determinism — the first source
/// wins the base name and the second is suffixed.
#[test]
fn serial_collision_preserves_both_files() {
    let pid = std::process::id();
    let root = std::env::temp_dir().join(format!("gather_test_serial_coll_{pid}"));
    let src_a = root.join("dir_a");
    let src_b = root.join("dir_b");
    let dst = root.join("dst");
    fs::create_dir_all(&src_a).unwrap();
    fs::create_dir_all(&src_b).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(src_a.join("sample.txt"), b"from-a").unwrap();
    fs::write(src_b.join("sample.txt"), b"from-b").unwrap();
    let _guard = TempGuard(root);

    run_gather(&[
        "--serial",
        src_a.join("sample.txt").to_str().unwrap(),
        src_b.join("sample.txt").to_str().unwrap(),
        "-t",
        dst.to_str().unwrap(),
    ]);

    let first = fs::read(dst.join("sample.txt")).expect("sample.txt must exist");
    assert_eq!(first, b"from-a", "first source must keep the base name");

    let renamed = dst.join("sample_1.txt");
    assert!(renamed.exists(), "second source must be renamed to sample_1.txt");
    let second = fs::read(&renamed).unwrap();
    assert_eq!(second, b"from-b", "second source content must be preserved");
}

/// `--serial -p` (with print-summary) must count processed files correctly.
#[test]
fn serial_print_summary_counts_correctly() {
    let (_guard, src, dst) = setup_tmp("serial_summary");
    let stdout = run_gather(&[
        "--serial",
        "-p",
        src.to_str().unwrap(),
        "-t",
        dst.to_str().unwrap(),
    ]);
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary header in stdout; got:\n{stdout}"
    );
    assert!(
        stdout.contains("    1"),
        "expected count of 1 in summary; got:\n{stdout}"
    );
}
