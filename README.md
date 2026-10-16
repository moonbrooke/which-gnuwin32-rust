# which (Rust)

A Rust rewrite of **GNU which v2.20** as ported to Windows by the
[GnuWin32](http://gnuwin32.sourceforge.net) project.

This crate is a faithful, dependency-free reimplementation that targets the **Windows** behaviour of that port: `;`-separated `PATH`, `\` as the output directory
separator, a leading `.` prepended to `PATH`, `PATHEXT` extension probing, and
the Windows-build quirks described below.

## Behaviour

`which` prints the full path of the command(s) it is given, searching the
directories listed in `%PATH%` using the same algorithm as `bash`:

- PATH is searched with `.` prepended automatically.  An empty PATH element
  means the current directory.
- When a command has no extension, `%PATHEXT%` (default `.com;.exe;.bat;.cmd`)
  is probed.
- Directories are never reported as executables.
- Tilde components (`~`, `~/…`) in PATH elements are expanded from `%HOME%`.
- Found paths are lexically normalised (`.` and `..` are resolved).

### Options

```
--all, -a           Print all matching executables in PATH, not just the first.
--read-alias, -i    Read aliases from stdin, reporting matching ones on stdout.
--skip-alias        Ignore option --read-alias; don't read stdin.
--read-functions    Read shell functions from stdin, reporting matching ones.
--skip-functions    Ignore option --read-functions; don't read stdin.
--skip-dot          Skip directories in PATH that start with a dot.
--skip-tilde        Skip directories in PATH that start with a tilde and
                    executables that reside in the HOME directory.
--show-dot          Print "./programname" for a match in the current directory.
--show-tilde        Print "~/..." for a match in HOME (ignored for root).
--tty-only          Stop processing options to the right if stdout is not a tty.
--version, -v, -V   Print version information and exit successfully.
--help              Print usage information and exit successfully.
```

The recommended usage mirrors GNU which:

```
(alias; declare -f) | which --tty-only --read-alias --read-functions --show-tilde --show-dot $@
```

## Build

```
cargo build --release
```

The resulting binary is `target\release\which.exe`.

## Tests

```
cargo test
```

Unit tests cover path normalisation and tilde expansion; integration tests in
`tests/cli.rs` exercise PATH/PATHEXT lookup, `--all`, `--show-dot`,
`--show-tilde`, `--skip-dot`, option permutation, the `--` terminator,
`--read-alias`, `--read-functions` and error reporting.

## Faithful quirks preserved

These match the GnuWin32 Windows build, even where they differ from upstream
Unix `which`:

- `absolute_program` is `IS_ABSOLUTE` (the GnuWin32 patch), so a relative path
  such as `dir\prog` is *not* treated as an absolute program; it is looked up
  by searching PATH.
- `~user` never expands: the password-database lookup returns NULL on Windows.
- `%HOME%` falls back to `/` when unset.
- `--show-tilde` is always enabled for non-root (the port reports `euid == 37`).
- `show-dot`/`show-tilde` output uses `\` and keeps any original separators.

Where the C code would invoke undefined behaviour (for example an absolute
forward-slash path whose directory part contains no `\`, where the original
dereferences a NULL pointer), this implementation degrades gracefully instead
of crashing.

## License

GPL-3.0-or-later.  The original program is Copyright (C) 1999, 2003, 2007,
2008 Carlo Wood; `bash.c` and `tilde/tilde.c` are derived from GNU Bash and the
GNU Readline tilde library.  See `LICENSE`.
