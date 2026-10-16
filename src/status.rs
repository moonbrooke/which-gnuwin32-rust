//! File-status and path primitives ported from `bash.c` and `sys.h`.

/// `FS_EXISTS` from `bash.h`.
pub const FS_EXISTS: i32 = 0x1;
/// `FS_EXECABLE` from `bash.h`.
pub const FS_EXECABLE: i32 = 0x2;
/// `FS_DIRECTORY` from `bash.h`.
pub const FS_DIRECTORY: i32 = 0x10;

/// `IS_SLASH` from `sys.h` (Windows variant): both `/` and `\` count.
#[inline]
pub fn is_slash(c: u8) -> bool {
    c == b'/' || c == b'\\'
}

/// `IS_DIRSEP` from `sys.h`.
#[inline]
pub fn is_dirsep(c: u8) -> bool {
    is_slash(c)
}

/// `IS_ABSOLUTE` from `sys.h` (Windows variant).
///
/// Absolute means either a leading slash (`/` or `\`) or a drive letter
/// (`X:`).
pub fn is_absolute(name: &str) -> bool {
    let b = name.as_bytes();
    if b.is_empty() {
        return false;
    }
    is_slash(b[0]) || (b.len() >= 2 && b[1] == b':')
}

/// `absolute_program` as patched for the Windows build.
///
/// The original bash definition returned true when the string contained a
/// slash.  The GnuWin32 patch replaced it with `IS_ABSOLUTE`, so a relative
/// path such as `bin\foo` is *not* treated as an absolute program.  This quirk
/// is reproduced deliberately.
pub fn absolute_program(name: &str) -> bool {
    is_absolute(name)
}

/// Port of `file_status` from `bash.c`.
///
/// `WIN32SYSTEM` is 1 in the Windows build, so access is decided with
/// `access(2)`.  On Windows `_access(name, X_OK)` succeeds for any existing
/// file that is not a directory, and directories are rejected before that
/// check.
pub fn file_status(name: &str) -> i32 {
    match std::fs::metadata(name) {
        Ok(md) => {
            if md.is_dir() {
                FS_EXISTS | FS_DIRECTORY
            } else {
                // access(name, X_OK) succeeds for existing non-directories.
                FS_EXISTS | FS_EXECABLE
            }
        }
        Err(_) => 0,
    }
}

/// Port of `extract_colon_unit` from `bash.c`, adapted to the Windows path
/// separator `';'`.
///
/// Returns the next `';'` separated unit starting at `*p_index`, advancing the
/// index.  Returns an empty string for an empty unit and `None` once the end
/// of the list is reached.
pub fn extract_colon_unit(string: &str, p_index: &mut usize) -> Option<String> {
    let bytes = string.as_bytes();
    let len = bytes.len();
    if *p_index >= len {
        return None;
    }

    let mut i = *p_index;
    if i != 0 && bytes[i] == b';' {
        i += 1;
    }

    let start = i;
    while i < len && bytes[i] != b';' {
        i += 1;
    }
    *p_index = i;

    if i == start {
        if i < len {
            *p_index += 1;
        }
        Some(String::new())
    } else {
        Some(string[start..i].to_string())
    }
}

/// Port of `get_next_path_element` from `bash.c`: an empty PATH element means
/// the current directory.
pub fn next_path_element(path_list: &str, p_index: &mut usize) -> Option<String> {
    let path = extract_colon_unit(path_list, p_index)?;
    if path.is_empty() {
        Some(".".to_string())
    } else {
        Some(path)
    }
}

/// Port of `make_full_pathname` from `bash.c`.  Always joins with a backslash,
/// which is `DIR_SEPARATOR` in the Windows build.
pub fn make_full_pathname(path: &str, name: &str) -> String {
    let mut full = String::with_capacity(path.len() + 1 + name.len());
    full.push_str(path);
    full.push('\\');
    full.push_str(name);
    full
}

/// Port of the Windows build's `sh_get_home_dir` fallback.
///
/// On Windows `getpwuid` is a macro that returns NULL, so bash's
/// `get_current_user_info` leaves the home directory as `/`.
pub fn sh_get_home_dir() -> String {
    "/".to_string()
}
