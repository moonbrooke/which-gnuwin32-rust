//! PATH searching, path normalisation and output formatting, ported from
//! `find_command_in_path`, `path_search`, `path_clean_up` and
//! `get_current_working_directory` in `which.c`.

use crate::options::Options;
use crate::status::{
    absolute_program, file_status, is_absolute, is_dirsep, make_full_pathname, next_path_element,
    sh_get_home_dir, FS_EXECABLE, FS_EXISTS,
};
use crate::tilde::tilde_expand;
use crate::util::{last_dirsep_index, strnicmp};

/// `DEFAULT_PATHEXT` from `which.c`.
pub const DEFAULT_PATHEXT: &str = ".com;.exe;.bat;.cmd";

/// Search state that the C original keeps in file-scope globals.
pub struct Searcher {
    /// `absolute_path_given`
    pub absolute_path_given: bool,
    /// `found_path_starts_with_dot`
    pub found_path_starts_with_dot: bool,
    /// `abs_path`; holds the directory part after a search.
    pub abs_path: Option<String>,
    /// Raw `$HOME` (or its fallback), used for tilde expansion.
    raw_home: String,
    /// `cwd`, computed lazily and memoised like `get_current_working_directory`.
    cwd: Option<String>,
}

impl Searcher {
    pub fn new() -> Self {
        let raw_home = std::env::var("HOME").unwrap_or_else(|_| sh_get_home_dir());
        Searcher {
            absolute_path_given: false,
            found_path_starts_with_dot: false,
            abs_path: None,
            raw_home,
            cwd: None,
        }
    }

    /// `get_current_working_directory`: the current directory with a trailing
    /// separator.
    pub fn get_cwd(&mut self) -> String {
        if let Some(c) = &self.cwd {
            return c.clone();
        }

        let mut cwd = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        if cwd.is_none() {
            if let Ok(pwd) = std::env::var("PWD") {
                if !pwd.is_empty() {
                    cwd = Some(pwd);
                }
            }
        }

        let mut cwd = cwd.unwrap_or_default();
        if !is_absolute(&cwd) {
            eprintln!("Can't get current working directory");
            std::process::exit(-1);
        }

        if !cwd.as_bytes().last().map_or(false, |&c| is_dirsep(c)) {
            cwd.push('\\');
        }

        self.cwd = Some(cwd.clone());
        cwd
    }

    /// `find_command_in_path`.
    pub fn find_command_in_path(
        &mut self,
        name_in: &str,
        path_list_in: &str,
        path_index: &mut usize,
        opts: &Options,
    ) -> Option<String> {
        let mut name = name_in.to_string();
        let mut path_list = path_list_in.to_string();

        if !absolute_program(name_in) {
            self.absolute_path_given = false;
        } else {
            self.absolute_path_given = true;

            let abs = if !name_in.starts_with('.')
                && !is_absolute(name_in)
                && !name_in.starts_with('~')
            {
                // strcpy(abs_path, "./"); abs_path[1] = DIR_SEPARATOR;
                format!(".\\{}", name_in)
            } else {
                name_in.to_string()
            };

            // p = strrchr(abs_path, DIR_SEPARATOR); *p++ = 0; name = p;
            match last_dirsep_index(&abs) {
                Some(idx) => {
                    self.abs_path = Some(abs[..idx].to_string());
                    path_list = abs[..idx].to_string();
                    name = abs[idx + 1..].to_string();
                }
                None => {
                    // The C original would invoke undefined behaviour here
                    // (dereferencing a NULL pointer).  Degrade gracefully.
                    self.abs_path = Some(abs.clone());
                    path_list = abs;
                    name = String::new();
                }
            }
        }

        let mut found: Option<String> = None;

        while *path_index < path_list.len() {
            let mut path: String;
            if self.absolute_path_given {
                path = path_list.clone();
                *path_index = path.len();
            } else {
                match next_path_element(&path_list, path_index) {
                    Some(p) => path = p,
                    None => break,
                }
            }

            if path.starts_with('~') {
                path = tilde_expand(&path, &self.raw_home);
                if opts.skip_tilde {
                    continue;
                }
            }

            if opts.skip_dot && !is_absolute(&path) {
                continue;
            }

            self.found_path_starts_with_dot = path.starts_with('.');

            let mut full_path = make_full_pathname(&path, &name);
            let status = file_status(&full_path);
            if (status & FS_EXISTS) != 0 && (status & FS_EXECABLE) != 0 {
                found = Some(full_path);
                break;
            }

            // On MS-Windows also search for the program name with executable
            // extensions appended.
            if !name.contains('.') {
                let pathext = std::env::var("PATHEXT").unwrap_or_default();
                let pathext = if pathext.is_empty() {
                    DEFAULT_PATHEXT.to_string()
                } else {
                    pathext
                };

                let mut matched = false;
                // strtok_r skips empty tokens, so consecutive ';' collapse.
                for ext in pathext.split(';').filter(|e| !e.is_empty()) {
                    let namex = format!("{}{}", name, ext);
                    full_path = make_full_pathname(&path, &namex);
                    let status = file_status(&full_path);
                    if (status & FS_EXISTS) != 0 && (status & FS_EXECABLE) != 0 {
                        found = Some(full_path);
                        matched = true;
                        break;
                    }
                }
                if matched {
                    break;
                }
            }
        }

        found
    }

    /// `path_search`.
    pub fn path_search(
        &mut self,
        indent: bool,
        cmd: &str,
        path_list: &str,
        opts: &Options,
    ) -> bool {
        let mut found_something = false;

        if !path_list.is_empty() {
            let mut path_index: usize = 0;
            loop {
                let mut next = opts.show_all;

                let result = match self.find_command_in_path(cmd, path_list, &mut path_index, opts) {
                    Some(r) => r,
                    None => break,
                };

                let full_path = self.path_clean_up(&result);

                let in_home = (opts.show_tilde || opts.skip_tilde)
                    && strnicmp(&full_path, &opts.home, opts.home.len()) == 0;

                if indent {
                    print!("\t");
                }

                let cwd = if opts.show_dot {
                    self.get_cwd()
                } else {
                    String::new()
                };

                if !(opts.skip_tilde && in_home)
                    && opts.show_dot
                    && self.found_path_starts_with_dot
                    && strnicmp(&full_path, &cwd, cwd.len()) == 0
                {
                    println!(".\\{}", &full_path[cwd.len()..]);
                    found_something = true;
                } else if in_home {
                    if opts.skip_tilde {
                        // C prints the indent above and then `continue`s
                        // without printing the path or setting found_something.
                        next = true;
                    } else if opts.show_tilde {
                        println!("~\\{}", &full_path[opts.home.len()..]);
                        found_something = true;
                    } else {
                        println!("{}", full_path);
                        found_something = true;
                    }
                } else {
                    println!("{}", full_path);
                    found_something = true;
                }

                if !next {
                    break;
                }
            }
        }

        found_something
    }

    /// `path_clean_up`: lexical normalisation that resolves `.` and `..`
    /// components.
    pub fn path_clean_up(&mut self, path: &str) -> String {
        let p1 = path.as_bytes();
        let mut result: Vec<u8> = Vec::new();
        let mut p2: i64 = 0;

        let mut saw_slash = false;
        let mut saw_slash_dot = false;
        let mut saw_slash_dot_dot = false;

        if !is_absolute(path) {
            let cwd = self.get_cwd();
            result.extend_from_slice(cwd.as_bytes());
            p2 = cwd.len() as i64;
            saw_slash = true;
        }

        let mut idx = 0usize;
        loop {
            let c = p1.get(idx).copied().unwrap_or(0);
            let next_is_sep = idx + 1 < p1.len() && is_dirsep(p1[idx + 1]);

            if !saw_slash || !is_dirsep(c) || (idx == 1 && !next_is_sep) {
                let at = p2 as usize;
                if at == result.len() {
                    result.push(c);
                } else {
                    result[at] = c;
                }
                p2 += 1;
            }

            if saw_slash_dot && is_dirsep(c) {
                p2 = if p2 >= 2 { p2 - 2 } else { 0 };
            }

            if saw_slash_dot_dot && is_dirsep(c) {
                let mut cnt = 0;
                loop {
                    p2 -= 1;
                    if p2 < 0 {
                        return path.to_string();
                    }
                    if !is_dirsep(result[p2 as usize]) {
                        cnt += 1;
                    }
                    if cnt == 3 {
                        break;
                    }
                }
                p2 += 1;
            }

            saw_slash_dot_dot = saw_slash_dot && c == b'.';
            saw_slash_dot = saw_slash && c == b'.';
            saw_slash = is_dirsep(c);

            if c == 0 {
                break;
            }
            idx += 1;
        }

        let mut out = result;
        if p2 >= 0 && (p2 as usize) <= out.len() {
            out.truncate(p2 as usize);
        }
        while out.last() == Some(&0) {
            out.pop();
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}

impl Default for Searcher {
    fn default() -> Self {
        Searcher::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(p: &str) -> String {
        Searcher::new().path_clean_up(p)
    }

    #[test]
    fn leaves_plain_absolute_paths_untouched() {
        assert_eq!(clean("C:\\x\\y"), "C:\\x\\y");
        assert_eq!(clean("\\root\\bin"), "\\root\\bin");
    }

    #[test]
    fn collapses_dot_segments() {
        assert_eq!(clean("C:\\a\\.\\b"), "C:\\a\\b");
        assert_eq!(clean("/a/./b"), "/a/b");
    }

    #[test]
    fn collapses_duplicate_separators() {
        assert_eq!(clean("C:\\a\\\\b"), "C:\\a\\b");
    }
}
