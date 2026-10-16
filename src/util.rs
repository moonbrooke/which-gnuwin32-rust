//! Small string helpers that mirror the Win32 CRT functions used by the
//! original C source (`stricmp`, `strnicmp`) plus a directory-separator
//! search that matches the port's `strrchr(..., DIR_SEPARATOR)`.

/// Case-insensitive comparison, equivalent to the Win32 `_stricmp`/`stricmp`.
///
/// Only ASCII letters are folded, which matches the behaviour for the paths
/// and identifiers this program deals with.
pub fn stricmp(a: &str, b: &str) -> i32 {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let mut i = 0;
    loop {
        let ca = ab.get(i).copied().unwrap_or(0);
        let cb = bb.get(i).copied().unwrap_or(0);
        let la = ca.to_ascii_lowercase();
        let lb = cb.to_ascii_lowercase();
        if la != lb {
            return if la < lb { -1 } else { 1 };
        }
        if ca == 0 {
            return 0;
        }
        i += 1;
    }
}

/// Case-insensitive comparison of at most `n` bytes, equivalent to `_strnicmp`.
///
/// Returns 0 when the first `n` bytes compare equal (the C function returns 0
/// for `n == 0` as well).
pub fn strnicmp(a: &str, b: &str, n: usize) -> i32 {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let mut i = 0;
    while i < n {
        let ca = ab.get(i).copied().unwrap_or(0);
        let cb = bb.get(i).copied().unwrap_or(0);
        let la = ca.to_ascii_lowercase();
        let lb = cb.to_ascii_lowercase();
        if la != lb {
            return if la < lb { -1 } else { 1 };
        }
        if ca == 0 {
            return 0;
        }
        i += 1;
    }
    0
}

/// Index of the last directory separator.
///
/// The Windows port uses `strrchr(path, DIR_SEPARATOR)` with `DIR_SEPARATOR`
/// equal to `'\\'`.  For paths that use forward slashes (where the C original
/// would dereference a NULL pointer) this falls back to `'/'` so the program
/// degrades gracefully instead of crashing or invoking undefined behaviour.
pub fn last_dirsep_index(s: &str) -> Option<usize> {
    s.rfind('\\').or_else(|| s.rfind('/'))
}
