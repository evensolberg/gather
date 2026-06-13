/// Integration tests for the gather CLI.
///
/// These tests compile and run the binary directly to verify end-to-end behaviour
/// without mocking any internals.
use std::fs;
use std::process::Command;

// Path to the compiled binary, resolved by Cargo at test-build time.
const GATHER: &str = env!("CARGO_BIN_EXE_gather");

/// Returns (src_file, dest_dir) paths inside a fresh temp directory.
/// The caller must remove the directory when done.
fn setup_tmp(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("gather_test_{tag}"));
    let dst = root.join("dst");
    fs::create_dir_all(&dst).expect("create dst dir");
    let src = root.join("sample.txt");
    fs::write(&src, b"hello").expect("write sample file");
    (src, dst)
}

// -------------------------------------------------------------------
// Bug: gtr-b6p — quiet mode silently defeats --print-summary/-p flag
// -------------------------------------------------------------------

/// When `-q` and `-p` are combined, the summary MUST still appear on
/// stdout.  Before the fix this test fails because `log::info!` is
/// silenced by `LevelFilter::Error`.
#[test]
fn quiet_and_print_summary_both_show_summary() {
    let (src, dst) = setup_tmp("qp");

    let output = Command::new(GATHER)
        .args([
            "-q",
            "-p",
            src.to_str().unwrap(),
            "-t",
            dst.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);

    fs::remove_dir_all(src.parent().unwrap()).ok();

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout when -q -p combined, got:\n{stdout}"
    );
}

/// Without `-q`, `-p` alone must also show the summary (regression guard).
#[test]
fn print_summary_without_quiet_shows_summary() {
    let (src, dst) = setup_tmp("p_only");

    let output = Command::new(GATHER)
        .args(["-p", src.to_str().unwrap(), "-t", dst.to_str().unwrap()])
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);

    fs::remove_dir_all(src.parent().unwrap()).ok();

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}",
        output.status
    );
    assert!(
        stdout.contains("Total files examined:"),
        "expected summary in stdout for -p alone, got:\n{stdout}"
    );
}

/// `-q` without `-p` must NOT print any summary — quiet is still quiet.
#[test]
fn quiet_without_print_summary_suppresses_output() {
    let (src, dst) = setup_tmp("q_only");

    let output = Command::new(GATHER)
        .args(["-q", src.to_str().unwrap(), "-t", dst.to_str().unwrap()])
        .output()
        .expect("failed to run gather");

    let stdout = String::from_utf8_lossy(&output.stdout);

    fs::remove_dir_all(src.parent().unwrap()).ok();

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}",
        output.status
    );
    assert!(
        !stdout.contains("Total files examined:"),
        "summary must not appear in stdout when only -q is given, got:\n{stdout}"
    );
}
