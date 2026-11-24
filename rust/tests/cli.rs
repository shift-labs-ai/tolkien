//! The `tolkien` command.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn tolkien(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tolkien"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the tolkien binary runs");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("the command reads its input");
    child.wait_with_output().expect("the command exits")
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("output is UTF-8")
}

#[test]
fn prints_one_token_per_line_of_a_text() {
    let output = tolkien(&["--text", "Mail dana@acme.com"], "");
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "WORD\t0\t4\t\"Mail\"\nWHITE_SPACE\t4\t5\t\" \"\nLIKE_EMAIL\t5\t18\t\"dana@acme.com\"\n"
    );
}

#[test]
fn reads_standard_input_and_skips_space() {
    for args in [&["--skip-space"][..], &["-s", "-"]] {
        let output = tolkien(args, "a b\nc\n");
        assert!(output.status.success());
        assert_eq!(
            stdout(&output),
            "WORD\t0\t1\t\"a\"\nWORD\t2\t3\t\"b\"\nWORD\t4\t5\t\"c\"\n"
        );
    }
}

#[test]
fn reads_a_file() {
    let file = std::env::temp_dir().join(format!("tolkien-cli-{}.txt", std::process::id()));
    std::fs::write(&file, "a 1").expect("the file is written");
    let path = file.to_str().expect("the path is UTF-8");
    let output = tolkien(&["-s", path], "");
    std::fs::remove_file(&file).expect("the file is removed");
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "WORD\t0\t1\t\"a\"\nLIKE_NUMBER\t2\t3\t\"1\"\n"
    );

    let missing = tolkien(&["no-such-file.txt"], "");
    assert_eq!(missing.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.starts_with("tolkien: cannot read no-such-file.txt"),
        "{stderr}"
    );
}

#[test]
fn takes_a_file_or_a_text_and_one_file() {
    for args in [
        &["-t", "a", "file.txt"][..],
        &["a.txt", "b.txt"],
        &["--text"],
    ] {
        let output = tolkien(args, "");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn adds_the_catalog_when_asked() {
    let plain = tolkien(&["-s", "-t", "ping 192.168.1.20"], "");
    assert_eq!(
        stdout(&plain),
        "WORD\t0\t4\t\"ping\"\nLIKE_NUMBER\t5\t17\t\"192.168.1.20\"\n"
    );
    for flag in ["-c", "--catalog"] {
        let output = tolkien(&[flag, "-s", "-t", "ping 192.168.1.20"], "");
        assert!(output.status.success());
        assert_eq!(
            stdout(&output),
            "WORD\t0\t4\t\"ping\"\nIPV4\t5\t17\t\"192.168.1.20\"\n"
        );
    }
}

#[test]
fn takes_a_name_after_a_double_dash_as_a_file() {
    let output = tolkien(&["--", "--skip-space"], "");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("tolkien: cannot read --skip-space"),
        "{stderr}"
    );
}

#[test]
fn rejects_an_unknown_option() {
    let output = tolkien(&["--nope"], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("tolkien: unknown option --nope"),
        "{stderr}"
    );
}

#[test]
fn prints_help_and_version() {
    let help = tolkien(&["--help"], "");
    assert!(help.status.success());
    assert!(stdout(&help).contains("Usage: tolkien"));
    let version = tolkien(&["-V"], "");
    assert_eq!(
        stdout(&version),
        concat!("tolkien ", env!("CARGO_PKG_VERSION"), "\n")
    );
}
