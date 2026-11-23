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
fn prints_one_token_per_line_from_arguments() {
    let output = tolkien(&["Mail", "dana@acme.com"], "");
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "WORD\t0\t4\t\"Mail\"\nWHITE_SPACE\t4\t5\t\" \"\nLIKE_EMAIL\t5\t18\t\"dana@acme.com\"\n"
    );
}

#[test]
fn reads_standard_input_and_skips_space() {
    let output = tolkien(&["--skip-space"], "a b\nc\n");
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "WORD\t0\t1\t\"a\"\nWORD\t2\t3\t\"b\"\nWORD\t4\t5\t\"c\"\n"
    );
}

#[test]
fn adds_the_catalog_when_asked() {
    let plain = tolkien(&["-s", "ping 192.168.1.20"], "");
    assert_eq!(
        stdout(&plain),
        "WORD\t0\t4\t\"ping\"\nLIKE_NUMBER\t5\t17\t\"192.168.1.20\"\n"
    );
    for flag in ["-c", "--catalog"] {
        let output = tolkien(&[flag, "-s", "ping 192.168.1.20"], "");
        assert!(output.status.success());
        assert_eq!(
            stdout(&output),
            "WORD\t0\t4\t\"ping\"\nIPV4\t5\t17\t\"192.168.1.20\"\n"
        );
    }
}

#[test]
fn takes_text_after_a_double_dash_as_it_is() {
    let output = tolkien(&["--", "--skip-space"], "");
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        "SYMBOL\t0\t1\t\"-\"\nSYMBOL\t1\t2\t\"-\"\nWORD\t2\t12\t\"skip-space\"\n"
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
