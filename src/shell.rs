//! Alias and shell-function handling ported from `which.c`.

use crate::util::{stricmp, strnicmp};

/// A shell function definition read from stdin.
pub struct Function {
    pub name: String,
    pub lines: Vec<String>,
}

/// `func_search`: print the definition of `cmd` if a matching function exists.
pub fn func_search(
    indent: bool,
    cmd: &str,
    functions: &[Function],
    function_start_type: i32,
) -> bool {
    for f in functions {
        if stricmp(&f.name, cmd) == 0 {
            if indent {
                print!("\t");
            }
            if function_start_type == 1 {
                println!("{} () {{", cmd);
            } else {
                println!("{} ()", cmd);
            }
            for line in &f.lines {
                if indent {
                    print!("\t");
                }
                print!("{}", line);
            }
            return true;
        }
    }
    false
}

/// `process_alias`: if `text` defines an alias for one of the commands, print
/// the alias definition followed by a PATH search for each command it invokes.
///
/// `path_search` is invoked as `path_search(indent, command)`.
pub fn process_alias<F>(
    text: &str,
    commands: &mut [Option<String>],
    read_functions: bool,
    show_all: bool,
    functions: &[Function],
    function_start_type: i32,
    mut path_search: F,
) where
    F: FnMut(bool, &str),
{
    let bytes = text.as_bytes();
    let mut pi = 0usize;

    while pi < bytes.len() && (bytes[pi] == b' ' || bytes[pi] == b'\t') {
        pi += 1;
    }
    if strnicmp(&text[pi..], "alias", 5) == 0 {
        pi += 5;
    }
    while pi < bytes.len() && (bytes[pi] == b' ' || bytes[pi] == b'\t') {
        pi += 1;
    }

    let name_start = pi;
    while pi < bytes.len() && bytes[pi] != b' ' && bytes[pi] != b'\t' && bytes[pi] != b'=' {
        pi += 1;
    }
    let len = pi - name_start;
    let name = &text[name_start..pi];

    for slot in commands.iter_mut() {
        let matches = match slot.as_deref() {
            Some(arg) => len == arg.len() && strnicmp(arg, name, len) == 0,
            None => false,
        };
        if !matches {
            continue;
        }

        print!("{}", text);

        if !show_all {
            *slot = None;
        }

        while pi < bytes.len() && (bytes[pi] == b' ' || bytes[pi] == b'\t') {
            pi += 1;
        }
        if pi < bytes.len() && bytes[pi] == b'=' {
            pi += 1;
        }
        while pi < bytes.len() && (bytes[pi] == b' ' || bytes[pi] == b'\t') {
            pi += 1;
        }

        let mut q: u8 = 0;
        if pi < bytes.len() && (bytes[pi] == b'"' || bytes[pi] == b'\'') {
            q = bytes[pi];
            pi += 1;
        }

        loop {
            let mut found = false;

            while pi < bytes.len() && (bytes[pi] == b' ' || bytes[pi] == b'\t') {
                pi += 1;
            }

            let cmd_start = pi;
            while pi < bytes.len()
                && bytes[pi] != b' '
                && bytes[pi] != b'\t'
                && bytes[pi] != q
                && bytes[pi] != b'|'
                && bytes[pi] != b'&'
            {
                pi += 1;
            }
            let cmd = &text[cmd_start..pi];

            let cmd_matches = match slot.as_deref() {
                Some(arg) => stricmp(cmd, arg) == 0,
                None => false,
            };
            if cmd_matches {
                *slot = None;
            }

            if read_functions && !cmd.contains('\\') {
                found = func_search(true, cmd, functions, function_start_type);
            }
            if show_all || !found {
                path_search(true, cmd);
            }

            // Skip separators, treating `||` and `&&` as separators too.
            while pi < bytes.len() {
                let c = bytes[pi];
                let nxt = bytes.get(pi + 1).copied().unwrap_or(0);
                let cond1 = c != b'|' || nxt == b'|';
                let cond2 = c != b'&' || nxt == b'&';
                if cond1 && cond2 {
                    pi += 1;
                } else {
                    break;
                }
            }

            if pi >= bytes.len() {
                break;
            }
            pi += 1;
        }

        break;
    }
}
