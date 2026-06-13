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
    let stdout = run_gather(&["-q", "-p", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
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
    let stdout = run_gather(&["-n", "-p", src.to_str().unwrap(), "-t", dst.to_str().unwrap()]);
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout for -n -p combined, got:\n{stdout}"
    );
}
