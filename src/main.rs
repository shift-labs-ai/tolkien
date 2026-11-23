//! `tolkien`: print the tokens of a text, one per line.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use tolkien::matchers::{LINE_TERMINATION, WHITE_SPACE};
use tolkien::Lexer;

const USAGE: &str = "\
Print the tokens of a text, one per line: kind, start, end, text.

Usage: tolkien [OPTIONS] [TEXT]...

With no TEXT, reads standard input.

Options:
  -s, --skip-space   Leave out WHITE_SPACE and LINE_TERMINATION tokens
  -h, --help         Print this help
  -V, --version      Print the version
";

fn main() -> ExitCode {
    let mut skip_space = false;
    let mut words = Vec::new();
    let mut options = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--" if options => options = false,
            "-s" | "--skip-space" if options => skip_space = true,
            "-h" | "--help" if options => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" if options => {
                println!("tolkien {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            option if options && option.starts_with('-') && option.len() > 1 => {
                eprintln!("tolkien: unknown option {option}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => words.push(arg),
        }
    }

    let text = if words.is_empty() {
        let mut input = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut input) {
            eprintln!("tolkien: cannot read standard input: {error}");
            return ExitCode::FAILURE;
        }
        input
    } else {
        words.join(" ")
    };

    let lexer = Lexer::default();
    let mut out = io::BufWriter::new(io::stdout().lock());
    for token in lexer.tokens(&text) {
        if skip_space && matches!(token.kind, WHITE_SPACE | LINE_TERMINATION) {
            continue;
        }
        let line = writeln!(
            out,
            "{}\t{}\t{}\t{:?}",
            token.kind, token.start, token.end, token.text
        );
        if line.is_err() {
            // The reader closed the pipe, as `head` does.
            break;
        }
    }
    ExitCode::SUCCESS
}
