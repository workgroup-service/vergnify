// @author Cursor

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn locate(binaries: &[&str]) -> Option<PathBuf> {
    let mut dirs = env::split_paths(&env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    if let Some(home) = home_dir() {
        dirs.extend(version_manager_dirs(&home));
    }
    dirs.extend(system_fallback_dirs());
    locate_in(binaries, &dirs)
}

pub fn locate_in(binaries: &[&str], dirs: &[PathBuf]) -> Option<PathBuf> {
    for binary in binaries {
        for dir in dirs {
            if let Some(path) = candidate(dir, binary) {
                return Some(path);
            }
        }
    }
    None
}

pub fn version_manager_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join(".pyenv/shims"),
        home.join(".asdf/shims"),
        home.join(".local/share/mise/shims"),
        home.join(".mise/shims"),
        home.join("Library/Application Support/mise/shims"),
        home.join(".local/share/fnm/aliases/default/bin"),
        home.join(".fnm/aliases/default/bin"),
        home.join("Library/Application Support/fnm/aliases/default/bin"),
        home.join(".cargo/bin"),
    ];
    if let Some(nvm) = nvm_default_bin(home) {
        dirs.push(nvm);
    }
    dirs
}

fn nvm_default_bin(home: &Path) -> Option<PathBuf> {
    let nvm = home.join(".nvm");
    let alias = fs::read_to_string(nvm.join("alias/default")).ok()?;
    let alias = alias.trim();
    if alias.is_empty() {
        return None;
    }
    let version =
        if alias.starts_with('v') && alias.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
            alias.to_string()
        } else {
            fs::read_to_string(nvm.join("alias").join(alias))
                .ok()?
                .trim()
                .to_string()
        };
    let bin = nvm.join("versions/node").join(version).join("bin");
    bin.is_dir().then_some(bin)
}

fn system_fallback_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ]
}

fn candidate(dir: &Path, name: &str) -> Option<PathBuf> {
    for file_name in executable_names(name) {
        let path = dir.join(file_name);
        if is_runnable(&path) {
            return Some(path);
        }
    }
    None
}

fn executable_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![format!("{name}.exe"), name.to_string()]
    } else {
        vec![name.to_string()]
    }
}

fn is_runnable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_executable_in_the_given_dirs() {
        let dir = std::env::temp_dir().join(format!("vergnify-locate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("node");
        fs::write(&bin, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&bin, perms).unwrap();
        }

        assert_eq!(locate_in(&["node"], &[dir.clone()]), Some(bin));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolves_nvm_default_alias() {
        let home = std::env::temp_dir().join(format!("vergnify-nvm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let bin = home.join(".nvm/versions/node/v20.11.0/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(home.join(".nvm/alias")).unwrap();
        fs::write(home.join(".nvm/alias/default"), "v20.11.0\n").unwrap();

        assert!(version_manager_dirs(&home).contains(&bin));
        let _ = fs::remove_dir_all(&home);
    }
}
