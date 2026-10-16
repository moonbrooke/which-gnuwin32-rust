//! Command-line handling: a small `getopt_long` emulation plus the usage and
//! version text from `which.c`.

use std::io::IsTerminal;

/// Program version, matching `VERSION` in `config.h`.
pub const VERSION: &str = "2.20";

/// Parsed command-line options, mirroring the static variables in `which.c`.
#[derive(Debug, Default, Clone)]
pub struct Options {
    pub skip_dot: bool,
    pub skip_tilde: bool,
    pub skip_alias: bool,
    pub read_alias: bool,
    pub show_dot: bool,
    pub show_tilde: bool,
    pub show_all: bool,
    pub tty_only: bool,
    pub skip_functions: bool,
    pub read_functions: bool,
    /// `HOME` with a trailing directory separator, set only when
    /// `--show-tilde` or `--skip-tilde` is in effect.
    pub home: String,
}

/// Identifiers for the long options, in the order they appear in `longopts`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LongId {
    Help,
    Version,
    SkipDot,
    SkipTilde,
    ShowDot,
    ShowTilde,
    TtyOnly,
    SkipAlias,
    ReadFunctions,
    SkipFunctions,
    All,
    ReadAlias,
}

const LONGS: &[(&str, LongId)] = &[
    ("help", LongId::Help),
    ("version", LongId::Version),
    ("skip-dot", LongId::SkipDot),
    ("skip-tilde", LongId::SkipTilde),
    ("show-dot", LongId::ShowDot),
    ("show-tilde", LongId::ShowTilde),
    ("tty-only", LongId::TtyOnly),
    ("all", LongId::All),
    ("read-alias", LongId::ReadAlias),
    ("skip-alias", LongId::SkipAlias),
    ("read-functions", LongId::ReadFunctions),
    ("skip-functions", LongId::SkipFunctions),
];

/// Result of matching a possibly-abbreviated long option name.
enum LongMatch {
    Found(usize),
    Ambiguous,
    None,
}

/// `getopt_long`'s long-option lookup: an exact match wins, otherwise a unique
/// prefix match is accepted and any ambiguous prefix is rejected.
fn match_long(name: &str) -> LongMatch {
    let mut first: Option<usize> = None;
    let mut ambig = false;
    for (i, (n, _)) in LONGS.iter().enumerate() {
        if n.starts_with(name) {
            if n.len() == name.len() {
                return LongMatch::Found(i);
            }
            if first.is_none() {
                first = Some(i);
            } else {
                ambig = true;
            }
        }
    }
    if ambig {
        LongMatch::Ambiguous
    } else if let Some(i) = first {
        LongMatch::Found(i)
    } else {
        LongMatch::None
    }
}

/// `geteuid()` from the Windows build's `sys.h`.
fn geteuid() -> u32 {
    37
}

/// `superuser` from `which.c`.
const SUPERUSER: u32 = 0;

/// `print_usage`.
///
/// The string literal is intentionally unindented so that the two leading
/// spaces of each option line are preserved exactly.
pub fn usage_text(progname: &str) -> String {
    format!(
"Usage: {progname} [options] [--] COMMAND [...]
Write the full path of COMMAND(s) to standard output.

  --version, -[vV] Print version and exit successfully.
  --help,          Print this help and exit successfully.
  --skip-dot       Skip directories in PATH that start with a dot.
  --skip-tilde     Skip directories in PATH that start with a tilde.
  --show-dot       Don't expand a dot to current directory in output.
  --show-tilde     Output a tilde for HOME directory for non-root.
  --tty-only       Stop processing options on the right if not on tty.
  --all, -a        Print all matches in PATH, not just the first
  --read-alias, -i Read list of aliases from stdin.
  --skip-alias     Ignore option --read-alias; don't read stdin.
  --read-functions Read shell functions from stdin.
  --skip-functions Ignore option --read-functions; don't read stdin.

Recommended use is to write the output of (alias; declare -f) to standard
input, so that which can show aliases and shell functions. See which(1) for
examples.

If the options --read-alias and/or --read-functions are specified then the
output can be a full alias or function definition, optionally followed by
the full path of each command used inside of those.

Report bugs to <which-bugs@gnu.org>.
"
    )
}

/// `print_version`.
pub fn version_text() -> String {
    format!(
"GNU which v{VERSION}, Copyright (C) 1999 - 2008 Carlo Wood.
GNU which comes with ABSOLUTELY NO WARRANTY;
This program is free software; your freedom to use, change
and distribute this program is protected by the GPL.
"
    )
}

/// Emulate `getopt_long` for `which`, returning the resulting options and the
/// permuted list of command arguments (as `Option<String>` because aliases may
/// consume an argument by setting it to `None`).
///
/// `--help`, `--version`, `-v` and `-V` print and exit directly, matching the
/// C control flow.
pub fn parse(args: &[String], progname: &str) -> (Options, Vec<Option<String>>) {
    let mut opts = Options::default();
    let mut commands: Vec<Option<String>> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            i += 1;
            commands.extend(args[i..].iter().cloned().map(Some));
            break;
        }

        if !arg.starts_with('-') || arg == "-" {
            commands.push(Some(arg.clone()));
            i += 1;
            continue;
        }

        if let Some(body) = arg.strip_prefix("--") {
            // Long option.  Split off any "=value" part.
            let (name, has_value) = match body.find('=') {
                Some(pos) => (&body[..pos], true),
                None => (body, false),
            };

            match match_long(name) {
                LongMatch::Found(idx) => {
                    let id = LONGS[idx].1;
                    if has_value {
                        // None of the long options take an argument.
                        eprintln!(
                            "{}: option `--{}' doesn't allow an argument",
                            progname, LONGS[idx].0
                        );
                    } else {
                        apply(id, &mut opts, progname);
                    }
                }
                LongMatch::Ambiguous => {
                    eprintln!("{}: option `{}' is ambiguous", progname, arg);
                }
                LongMatch::None => {
                    eprintln!("{}: unrecognized option `--{}'", progname, body);
                }
            }
            i += 1;
            continue;
        }

        // Short option cluster.
        for &c in arg.as_bytes()[1..].iter() {
            match c {
                b'a' => opts.show_all = true,
                b'i' => opts.read_alias = true,
                b'v' | b'V' => {
                    print!("{}", version_text());
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("{}: invalid option -- {}", progname, c as char);
                }
            }
        }
        i += 1;
    }

    (opts, commands)
}

/// Apply a long option, matching the body of the `switch` in `which.c`.
fn apply(id: LongId, opts: &mut Options, progname: &str) {
    match id {
        LongId::Help => {
            print!("{}", usage_text(progname));
            std::process::exit(0);
        }
        LongId::Version => {
            print!("{}", version_text());
            std::process::exit(0);
        }
        LongId::SkipDot => opts.skip_dot = !opts.tty_only,
        LongId::SkipTilde => opts.skip_tilde = !opts.tty_only,
        LongId::ShowDot => opts.show_dot = !opts.tty_only,
        LongId::ShowTilde => opts.show_tilde = !opts.tty_only && geteuid() != SUPERUSER,
        LongId::TtyOnly => opts.tty_only = !std::io::stdout().is_terminal(),
        LongId::SkipAlias => opts.skip_alias = true,
        LongId::ReadFunctions => opts.read_functions = true,
        LongId::SkipFunctions => opts.skip_functions = true,
        LongId::All => opts.show_all = true,
        LongId::ReadAlias => opts.read_alias = true,
    }
}
