//! Integration tests for the `which` binary.
//!
//! Each test builds a small throw-away directory tree and runs the compiled
//! binary against it with a controlled `PATH`/`PATHEXT`/`HOME`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_which")
}

/// A unique, empty temporary directory for one test.
fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut d = std::env::temp_dir();
    d.push(format!(
        "which-rust-test-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"x").unwrap();
}

/// Run the binary in `cwd` with `PATH` set, a deterministic `PATHEXT`, and the
/// given arguments.
fn run(cwd: &Path, path: &str, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .env("PATH", path)
        .env("PATHEXT", ".exe;.cmd")
        .output()
        .unwrap()
}

/// Like `run` but feeds `stdin`.
fn run_stdin(cwd: &Path, path: &str, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(bin())
        .args(args)
        .current_dir(cwd)
        .env("PATH", path)
        .env("PATHEXT", ".exe;.cmd")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn version_is_exact() {
    let o = Command::new(bin()).arg("--version").output().unwrap();
    assert_eq!(
        out(&o),
        "GNU which v2.20, Copyright (C) 1999 - 2008 Carlo Wood.\n\
GNU which comes with ABSOLUTELY NO WARRANTY;\n\
This program is free software; your freedom to use, change\n\
and distribute this program is protected by the GPL.\n"
    );
    assert!(o.status.success());
}

#[test]
fn short_version_flags() {
    for flag in ["-v", "-V"] {
        let o = Command::new(bin()).arg(flag).output().unwrap();
        assert!(out(&o).starts_with("GNU which v2.20,"));
        assert!(o.status.success());
    }
}

#[test]
fn help_has_indented_options() {
    let o = Command::new(bin()).arg("--help").output().unwrap();
    let text = out(&o);
    assert!(text.starts_with("Usage: "));
    assert!(text.contains("\n  --skip-dot       Skip directories"));
    assert!(text.contains("\n  --read-functions Read shell functions from stdin."));
    assert!(text.ends_with("Report bugs to <which-bugs@gnu.org>.\n"));
    assert!(o.status.success());
}

#[test]
fn finds_first_match_with_extension() {
    let root = temp_dir("first");
    let bin1 = root.join("bin1");
    let bin2 = root.join("bin2");
    touch(&bin1.join("foo.exe"));
    touch(&bin2.join("foo.cmd"));

    let path = format!("{};{}", bin1.display(), bin2.display());
    let cwd = root.clone();
    let o = run(&cwd, &path, &["foo"]);

    assert_eq!(out(&o), format!("{}\\foo.exe\n", bin1.display()));
    assert!(o.status.success());
}

#[test]
fn all_flag_prints_every_match() {
    let root = temp_dir("all");
    let bin1 = root.join("bin1");
    let bin2 = root.join("bin2");
    touch(&bin1.join("foo.exe"));
    touch(&bin2.join("foo.cmd"));

    let path = format!("{};{}", bin1.display(), bin2.display());
    let o = run(&root, &path, &["-a", "foo"]);

    assert_eq!(
        out(&o),
        format!(
            "{}\\foo.exe\n{}\\foo.cmd\n",
            bin1.display(),
            bin2.display()
        )
    );
}

#[test]
fn missing_command_reports_failure() {
    let root = temp_dir("missing");
    let bin1 = root.join("bin1");
    fs::create_dir_all(&bin1).unwrap();

    let path = format!("{}", bin1.display());
    let o = run(&root, &path, &["nope"]);

    assert_eq!(o.status.code(), Some(1));
    let expected = format!(
        "which: no nope in (.;{})",
        bin1.display()
    );
    // progname is the full path of the binary, so match on the suffix.
    assert!(
        err(&o).ends_with(&format!(": no nope in (.;{})\n", bin1.display())),
        "stderr was: {:?} (expected to end with {:?})",
        err(&o),
        expected
    );
}

#[test]
fn show_dot_for_cwd_match() {
    let root = temp_dir("showdot");
    touch(&root.join("foo.exe"));
    let path = root.join("bin1").display().to_string();

    let o = run(&root, &path, &["--show-dot", "foo"]);
    assert_eq!(out(&o), ".\\foo.exe\n");
    assert!(o.status.success());
}

#[test]
fn show_tilde_for_home_match() {
    let root = temp_dir("showtilde");
    touch(&root.join("foo.exe"));

    let o = Command::new(bin())
        .args(["--show-tilde", "foo"])
        .current_dir(&root)
        .env("PATH", root.join("bin1").display().to_string())
        .env("PATHEXT", ".exe;.cmd")
        .env("HOME", &root)
        .output()
        .unwrap();

    assert_eq!(out(&o), "~\\foo.exe\n");
    assert!(o.status.success());
}

#[test]
fn skip_dot_ignores_current_directory() {
    let root = temp_dir("skipdot");
    let bin1 = root.join("bin1");
    touch(&root.join("foo.exe"));
    touch(&bin1.join("foo.exe"));

    let path = format!("{}", bin1.display());
    let o = run(&root, &path, &["--skip-dot", "foo"]);
    assert_eq!(out(&o), format!("{}\\foo.exe\n", bin1.display()));
}

#[test]
fn directories_are_not_executables() {
    let root = temp_dir("dirs");
    let bin1 = root.join("bin1");
    fs::create_dir_all(bin1.join("adir")).unwrap();

    let path = bin1.display().to_string();
    let o = run(&root, &path, &["adir"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("no adir in ("));
}

#[test]
fn option_permutation_and_terminator() {
    let root = temp_dir("perm");
    let bin1 = root.join("bin1");
    touch(&bin1.join("foo.exe"));
    let path = bin1.display().to_string();

    // Options after the argument are still recognised.
    let a = run(&root, &path, &["foo", "--all"]);
    let b = run(&root, &path, &["--all", "foo"]);
    assert_eq!(out(&a), out(&b));

    // After `--`, everything is a command.
    let c = run(&root, &path, &["--", "--version"]);
    assert_eq!(c.status.code(), Some(1));
    assert!(err(&c).contains("no --version in ("));
}

#[test]
fn read_alias_prints_definition_and_paths() {
    let root = temp_dir("alias");
    let bin1 = root.join("bin1");
    for n in ["echo", "cat", "sort"] {
        touch(&bin1.join(format!("{}.exe", n)));
    }
    let path = bin1.display().to_string();

    let input = "alias test2='echo test2 | cat | sort'\n";
    let o = run_stdin(&root, &path, &["--read-alias", "test2"], input);

    assert_eq!(
        out(&o),
        format!(
            "alias test2='echo test2 | cat | sort'\n\t{}\\echo.exe\n\t{}\\cat.exe\n\t{}\\sort.exe\n",
            bin1.display(),
            bin1.display(),
            bin1.display()
        )
    );
}

#[test]
fn read_functions_prints_definition() {
    let root = temp_dir("funcs");
    let bin1 = root.join("bin1");
    fs::create_dir_all(&bin1).unwrap();
    let path = bin1.display().to_string();

    let input = "foo ()\n{\n  echo hi\n}\n";
    let o = run_stdin(&root, &path, &["--read-functions", "foo"], input);
    assert_eq!(out(&o), "foo ()\n{\n  echo hi\n}\n");

    // bash `declare -f` style.
    let input = "declare -fx bar ()\n{\n  echo yo\n}\n";
    let o = run_stdin(&root, &path, &["--read-functions", "bar"], input);
    assert_eq!(out(&o), "bar ()\n{\n  echo yo\n}\n");

    // zsh style.
    let input = "baz () {\n  echo zz\n}\n";
    let o = run_stdin(&root, &path, &["--read-functions", "baz"], input);
    assert_eq!(out(&o), "baz () {\n  echo zz\n}\n");
}

#[test]
fn no_arguments_fails() {
    let root = temp_dir("noargs");
    let o = run(&root, "C:\\", &[]);
    assert!(!o.status.success());
    assert!(err(&o).starts_with("Usage: "));
}
