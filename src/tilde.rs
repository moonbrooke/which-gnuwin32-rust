//! Tilde expansion ported from `tilde/tilde.c`.
//!
//! Only the behaviour that `which.c` relies on is needed, but the general
//! scanning algorithm is reproduced faithfully (including the default
//! prefixes `" ~"`/`"\t~"` and suffixes `" "`/`"\n"`).

use crate::status::is_dirsep;

/// `default_prefixes` from `tilde.c`: whitespace followed by a tilde.
const DEFAULT_PREFIXES: [&str; 2] = [" ~", "\t~"];

/// `default_suffixes` from `tilde.c`.
const DEFAULT_SUFFIXES: [u8; 2] = [b' ', b'\n'];

/// `tilde_find_prefix`: index of the `~` that starts an expansion.
fn find_prefix(string: &str) -> usize {
    let bytes = string.as_bytes();
    if bytes.is_empty() || bytes[0] == b'~' {
        return 0;
    }
    let string_len = bytes.len();
    for i in 0..string_len {
        for prefix in DEFAULT_PREFIXES {
            let pb = prefix.as_bytes();
            if i + pb.len() <= string_len && &bytes[i..i + pb.len()] == pb {
                return i + pb.len() - 1;
            }
        }
    }
    string_len
}

/// `tilde_find_suffix`: index of the character that ends the tilde word.
fn find_suffix(string: &str) -> usize {
    let bytes = string.as_bytes();
    let string_len = bytes.len();
    for i in 0..string_len {
        if is_dirsep(bytes[i]) {
            return i;
        }
        if DEFAULT_SUFFIXES.contains(&bytes[i]) {
            return i;
        }
    }
    string_len
}

/// `glue_prefix_and_suffix`.
fn glue_prefix_and_suffix(prefix: &str, suffix: &str, suffind: usize) -> String {
    let plen = if prefix.is_empty() { 0 } else { prefix.len() };
    let mut ret = String::with_capacity(plen + (suffix.len() - suffind));
    if plen > 0 {
        ret.push_str(prefix);
    }
    ret.push_str(&suffix[suffind..]);
    ret
}

/// `tilde_expand_word`.
///
/// A leading `~` or `~/` expands to `$HOME` (or the home-directory fallback).
/// `~user` never expands on the Windows build because the password-database
/// lookups (`getpwnam`) are macros returning NULL and the patched
/// `isolate_tilde_prefix` produces an empty username; the input is returned
/// unchanged.
fn expand_word(filename: &str, home: &str) -> String {
    if !filename.starts_with('~') {
        return filename.to_string();
    }

    let bytes = filename.as_bytes();
    if bytes.len() == 1 || is_dirsep(bytes[1]) {
        let expansion = std::env::var("HOME").unwrap_or_else(|_| home.to_string());
        return glue_prefix_and_suffix(&expansion, filename, 1);
    }

    filename.to_string()
}

/// `tilde_expand`: expand the tilde word(s) in `string`.
///
/// `home` is the value used when `$HOME` is unset (the Windows fallback `/`).
pub fn tilde_expand(string: &str, home: &str) -> String {
    let mut result = String::new();
    let mut s = string;

    loop {
        let start = find_prefix(s);
        result.push_str(&s[..start]);
        s = &s[start..];

        let end = find_suffix(s);
        if start == 0 && end == 0 {
            break;
        }

        let tilde_word = &s[..end];
        s = &s[end..];

        let expansion = expand_word(tilde_word, home);

        let elen = expansion.len();
        let first_is_sep = expansion.as_bytes().first().map_or(false, |&c| is_dirsep(c));
        let next_is_sep = s.as_bytes().first().map_or(false, |&c| is_dirsep(c));

        // Mirrors the Cygwin/Win32 fix in tilde.c.
        if elen > 1 || !first_is_sep || next_is_sep {
            result.push_str(&expansion);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_without_tilde_are_unchanged() {
        assert_eq!(tilde_expand("plain/path", "/home"), "plain/path");
        assert_eq!(tilde_expand("", "/home"), "");
    }

    #[test]
    fn user_tilde_is_not_expanded() {
        // getpwnam always fails on the Windows build.
        assert_eq!(tilde_expand("~user/x", "/home"), "~user/x");
    }

    #[test]
    fn tilde_uses_home_fallback_when_env_unset() {
        if std::env::var_os("HOME").is_none() {
            assert_eq!(tilde_expand("~/x", "/home"), "/home/x");
            assert_eq!(tilde_expand("~", "/home"), "/home");
        }
    }
}
