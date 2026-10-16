//! which v2.20 -- print full path of executables.
//!
//! Rust rewrite of GNU which 2.20 as ported to Windows by the GnuWin32
//! project.  Original C source: Copyright (C) 1999, 2003, 2007, 2008 Carlo
//! Wood <carlo@gnu.org>.  This program is free software, licensed under the
//! GPL (version 3 or later); see the LICENSE file.

mod options;
mod pathsearch;
mod shell;
mod status;
mod tilde;
mod util;

use std::io::{IsTerminal, Read, Write};

use options::{parse, usage_text};
use pathsearch::Searcher;
use shell::Function;
use status::{is_dirsep, sh_get_home_dir};
use util::{last_dirsep_index, stricmp, strnicmp};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let progname = args
        .first()
        .cloned()
        .unwrap_or_else(|| "which".to_string());

    let (mut opts, mut commands) = parse(&args, &progname);

    // uidget() has no observable effect on the Windows build.

    let mut searcher = Searcher::new();

    if opts.show_dot {
        searcher.get_cwd();
    }

    if opts.show_tilde || opts.skip_tilde {
        let mut home = std::env::var("HOME").unwrap_or_else(|_| sh_get_home_dir());
        if !home.is_empty() && !is_dirsep(home.as_bytes()[home.len() - 1]) {
            home.push('\\');
        }
        opts.home = home;
    }

    if opts.skip_alias {
        opts.read_alias = false;
    }
    if opts.skip_functions {
        opts.read_functions = false;
    }

    if commands.is_empty() {
        eprint!("{}", usage_text(&progname));
        finish(-1);
    }

    // PATH, prefixed with "." by the Windows build, with a trailing ';'
    // removed.
    let mut path_list = format!(".{}", ';');
    path_list.push_str(&std::env::var("PATH").unwrap_or_default());
    if path_list.ends_with(';') {
        path_list.pop();
    }

    let mut functions: Vec<Function> = Vec::new();
    let mut function_start_type = 0i32;

    if opts.read_alias || opts.read_functions {
        let mut aliases: Vec<String> = Vec::new();

        if std::io::stdin().is_terminal() {
            let which_opts = if opts.read_functions {
                if opts.read_alias {
                    "--read-functions, --read-alias, -i"
                } else {
                    "--read-functions"
                }
            } else {
                "--read-alias, -i"
            };
            eprintln!("{}: {}: Warning: stdin is a tty.", progname, which_opts);
        }

        let lines = read_lines_preserving_newlines();
        let mut processing_aliases = opts.read_alias;

        let mut li = 0;
        while li < lines.len() {
            let buf = lines[li].clone();
            li += 1;

            let mut looks_like_function_start = false;
            let mut function_start_has_declare = false;

            if opts.read_functions {
                let b = buf.as_bytes();
                if !b.is_empty() {
                    let mut pi: isize = b.len() as isize - 1;
                    while pi > 2 && is_ascii_space(b[pi as usize]) {
                        pi -= 1;
                    }
                    let p = pi as usize;
                    if b[p] == b')' && p >= 2 && b[p - 1] == b'(' && b[p - 2] == b' ' {
                        looks_like_function_start = true;
                        function_start_has_declare = strnicmp(&buf, "declare -", 9) == 0;
                    }
                    if p > 4
                        && b[p] == b'{'
                        && b[p - 1] == b' '
                        && b[p - 2] == b')'
                        && b[p - 3] == b'('
                        && b[p - 4] == b' '
                    {
                        looks_like_function_start = true;
                        function_start_type = 1;
                        function_start_has_declare = false;
                    }
                }
            }

            if processing_aliases && !looks_like_function_start {
                // bash 2.0.5b can emit lines like "declare -fx NAME"; eat them.
                if strnicmp(&buf, "declare -", 9) == 0 {
                    continue;
                }
                aliases.push(buf);
            } else if opts.read_functions && looks_like_function_start {
                processing_aliases = false;

                let b = buf.as_bytes();
                let mut start = 0usize;
                if function_start_has_declare {
                    // Skip "declare -" and the following option word.
                    let mut pi = 9usize;
                    while pi < b.len() {
                        let c = b[pi];
                        pi += 1;
                        if c == b' ' {
                            break;
                        }
                    }
                    start = pi;
                }
                let mut pe = start;
                while pe < b.len() && b[pe] != b' ' {
                    pe += 1;
                }
                let name = String::from_utf8_lossy(&b[start..pe]).into_owned();

                let mut f = Function {
                    name,
                    lines: Vec::new(),
                };
                while li < lines.len() {
                    let l = lines[li].clone();
                    li += 1;
                    f.lines.push(l.clone());
                    if stricmp(&l, "}\n") == 0 {
                        break;
                    }
                }
                functions.push(f);
            }
        }

        if opts.read_alias {
            for a in &aliases {
                let path_list_ref = &path_list;
                let funcs = &functions;
                let opts_ref = &opts;
                shell::process_alias(
                    a,
                    &mut commands,
                    opts.read_functions,
                    opts.show_all,
                    funcs,
                    function_start_type,
                    |indent, cmd| {
                        searcher.path_search(indent, cmd, path_list_ref, opts_ref);
                    },
                );
            }
        }
    }

    let mut fail_count: i32 = 0;

    for slot in commands.iter() {
        let cmd = match slot {
            Some(c) => c.clone(),
            None => continue,
        };

        let mut found_something = false;

        if opts.read_functions && !cmd.contains('\\') {
            found_something = shell::func_search(false, &cmd, &functions, function_start_type);
        }

        if (opts.show_all || !found_something)
            && !searcher.path_search(false, &cmd, &path_list, &opts)
            && !found_something
        {
            let name = if searcher.absolute_path_given {
                last_dirsep_index(&cmd)
                    .map(|i| cmd[i + 1..].to_string())
                    .unwrap_or_else(|| cmd.clone())
            } else {
                cmd.clone()
            };
            let where_ = if searcher.absolute_path_given {
                searcher.abs_path.clone().unwrap_or_default()
            } else {
                path_list.clone()
            };
            eprintln!("{}: no {} in ({})", progname, name, where_);
            fail_count += 1;
        }
    }

    finish(fail_count);
}

/// Flush stdout and terminate with `code`, matching the C `return`/`exit`.
fn finish(code: i32) -> ! {
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

/// Read all of stdin, keeping line terminators (`\n` and any preceding `\r`),
/// the way `fgets` would.
fn read_lines_preserving_newlines() -> Vec<String> {
    let mut data = Vec::new();
    let _ = std::io::stdin().lock().read_to_end(&mut data);

    let mut lines = Vec::new();
    let mut start = 0usize;
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' {
            lines.push(String::from_utf8_lossy(&data[start..=i]).into_owned());
            start = i + 1;
        }
    }
    if start < data.len() {
        lines.push(String::from_utf8_lossy(&data[start..]).into_owned());
    }
    lines
}

/// Equivalent of C `isspace` for the ASCII range.
fn is_ascii_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}
