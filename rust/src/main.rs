//! `tolkien`: print the tokens of a text, one per line.
//!
//! The command of the crate, for work on the lexer itself. It takes the
//! arguments the npm package's command takes, of which it has the few a
//! developer needs; `src/main.ts` is the one with every option.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use tolkien::matchers::{LINE_TERMINATION, WHITE_SPACE};
use tolkien::{catalog, Lexer};

const USAGE: &str = "\
Print the tokens of a text, one per line: kind, start, end, text.

Usage: tolkien [OPTIONS] [FILE]

With no FILE, or with -, reads standard input. Start and end are byte
offsets.

Options:
  -t, --text <TEXT>  Lex this text instead of a file
  -c, --catalog      Add every matcher of the catalog: addresses, secrets,
                     card and phone numbers, units and more
  -s, --skip-space   Leave out WHITE_SPACE and LINE_TERMINATION tokens
  -h, --help         Print this help
  -V, --version      Print the version
";

fn main() -> ExitCode {
    let mut skip_space = false;
    let mut with_catalog = false;
    let mut given = None;
    let mut source = None;
    let mut options = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" if options => options = false,
            "-t" | "--text" if options => {
                given = args.next();
                if given.is_none() {
                    eprintln!("tolkien: {arg} needs a text\n\n{USAGE}");
                    return ExitCode::from(2);
                }
            }
            "-c" | "--catalog" if options => with_catalog = true,
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
            _ if source.is_none() => source = Some(arg),
            _ => {
                eprintln!("tolkien: more than one file\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

    let text = match (given, source.as_deref()) {
        (Some(_), Some(_)) => {
            eprintln!("tolkien: give a file or --text, not both\n\n{USAGE}");
            return ExitCode::from(2);
        }
        (Some(text), None) => text,
        (None, None | Some("-")) => {
            let mut input = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut input) {
                eprintln!("tolkien: cannot read standard input: {error}");
                return ExitCode::FAILURE;
            }
            input
        }
        (None, Some(file)) => match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("tolkien: cannot read {file}: {error}");
                return ExitCode::FAILURE;
            }
        },
    };

    let lexer = if with_catalog {
        Lexer::with(catalog::all()).expect("the catalog forms a valid lexer")
    } else {
        Lexer::default()
    };
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
