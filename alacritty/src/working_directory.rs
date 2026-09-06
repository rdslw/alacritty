use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::{CString, OsString};
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;

/// Resolve OSC 7 metadata only when launching a process, falling back to process inspection.
///
/// The report wins over process inspection, like WezTerm and GNOME Terminal. When OSC 133 markers
/// show a running command (`at_prompt` is `Some(false)`), process inspection comes first, like
/// kitty: the command may have changed directory or be a nested shell without the hook. Without
/// markers (`None`) a stale report cannot be detected.
pub fn resolve(
    uri: Option<&str>,
    at_prompt: Option<bool>,
    fallback: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    let reported = || {
        uri.and_then(|uri| local_path(uri, &gethostname::gethostname()))
            .filter(|path| usable_directory(path))
    };

    match at_prompt {
        Some(false) => fallback().or_else(reported),
        _ => reported().or_else(fallback),
    }
}

/// Convert a local file URI without resolving host aliases or accessing remote filesystems.
fn local_path(uri: &str, hostname: &OsStr) -> Option<PathBuf> {
    let (scheme, location) = uri.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }

    let path_start = location.find('/')?;
    let (host, path) = location.split_at(path_start);
    if !host.is_empty()
        && !host.eq_ignore_ascii_case("localhost")
        && !host.as_bytes().eq_ignore_ascii_case(hostname.as_encoded_bytes())
    {
        return None;
    }

    // Queries, fragments, backslashes, and malformed percent escapes are not file paths.
    if path.bytes().any(|byte| byte.is_ascii_control() || matches!(byte, b'?' | b'#' | b'\\'))
        || path
            .split('%')
            .skip(1)
            .any(|part| part.len() < 2 || !part.as_bytes()[..2].iter().all(u8::is_ascii_hexdigit))
    {
        return None;
    }

    let path = percent_decode_str(path).collect::<Vec<_>>();
    if path.contains(&0) || path.starts_with(b"//") {
        return None;
    }

    #[cfg(unix)]
    let path = PathBuf::from(OsString::from_vec(path));

    #[cfg(windows)]
    let path = {
        // Only local drive paths are supported; never turn a URI into a UNC or device path.
        if path.len() < 4 || !path[1].is_ascii_alphabetic() || &path[2..4] != b":/" {
            return None;
        }
        let mut path = String::from_utf8(path).ok()?;
        path.remove(0);
        PathBuf::from(path)
    };

    path.is_absolute().then_some(path)
}

/// Runs under the terminal lock; keep this limited to cheap local syscalls.
fn usable_directory(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    #[cfg(unix)]
    {
        let Ok(path) = CString::new(path.as_os_str().as_bytes()) else { return false };
        // A directory may exist while the child is unable to enter it.
        unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
    }

    #[cfg(windows)]
    true
}

#[cfg(test)]
mod tests {
    use std::fs;

    use percent_encoding::{NON_ALPHANUMERIC, percent_encode};

    use super::*;

    #[test]
    fn local_hosts_and_encoding() {
        for host in ["", "localhost", "LOCALHOST", "workstation", "WORKSTATION"] {
            #[cfg(unix)]
            let (uri_path, expected) = ("/tmp/a%20b/%C5%BC;%23%3F%25", "/tmp/a b/ż;#?%");
            #[cfg(windows)]
            let (uri_path, expected) = ("/C:/a%20b/%C5%BC;%23%25", "C:/a b/ż;#%");

            let uri = format!("FILE://{host}{uri_path}");
            assert_eq!(local_path(&uri, OsStr::new("workstation")), Some(PathBuf::from(expected)));
        }
    }

    #[test]
    fn invalid_or_remote_uris() {
        for uri in [
            "",
            "file:/tmp",
            "https://localhost/tmp",
            "file://localhost",
            "file://remote/tmp",
            "file://workstation.example/tmp",
            "file://user@localhost/tmp",
            "file://localhost:22/tmp",
            "file://127.0.0.1/tmp",
            "file://[::1]/tmp",
            "file:///tmp?query",
            "file:///tmp#fragment",
            "file:///tmp/%",
            "file:///tmp/%2",
            "file:///tmp/%GG",
            "file:///tmp/%00",
            "file:///tmp/\0",
            "file:///tmp/\n",
            "file:///tmp/\\foo",
            "file:////server/share",
            "file:///%2Fserver/share",
        ] {
            assert_eq!(local_path(uri, OsStr::new("workstation")), None, "{uri:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path() {
        assert_eq!(
            local_path("file:///tmp/%FF", OsStr::new("workstation")),
            Some(PathBuf::from(OsString::from_vec(b"/tmp/\xff".to_vec()))),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_requires_absolute_drive_path() {
        for uri in
            ["file:///tmp", "file:///C:relative", "file:///C:/%FF", "file:///%5C%5Cserver/share"]
        {
            assert_eq!(local_path(uri, OsStr::new("workstation")), None);
        }
        assert_eq!(
            local_path("file:///C:/", OsStr::new("workstation")),
            Some(PathBuf::from("C:/")),
        );
    }

    fn file_uri(path: &Path) -> String {
        #[cfg(unix)]
        let path = path.as_os_str().as_bytes();
        #[cfg(windows)]
        let path = format!("/{}", path.display()).replace('\\', "/");
        #[cfg(windows)]
        let path = path.as_bytes();

        format!("file:///{}", percent_encode(&path[1..], NON_ALPHANUMERIC))
    }

    #[test]
    fn directory_precedence_and_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let uri = file_uri(directory.path());
        for at_prompt in [None, Some(true)] {
            assert_eq!(
                resolve(Some(&uri), at_prompt, || panic!("OSC 7 should take precedence")),
                Some(directory.path().to_path_buf()),
            );
        }

        let file = directory.path().join("file");
        fs::write(&file, "").unwrap();
        let file_uri = file_uri(&file);
        assert_eq!(resolve(Some(&file_uri), None, || None), None);
        directory.close().unwrap();

        for uri in [None, Some(""), Some("file://remote/tmp"), Some(&uri), Some(&file_uri)] {
            for at_prompt in [None, Some(true), Some(false)] {
                let fallback = PathBuf::from("fallback");
                assert_eq!(resolve(uri, at_prompt, || Some(fallback.clone())), Some(fallback));
                assert_eq!(resolve(uri, at_prompt, || None), None);
            }
        }
    }

    #[test]
    fn running_command_prefers_process_inspection() {
        let directory = tempfile::tempdir().unwrap();
        let uri = file_uri(directory.path());
        let fallback = PathBuf::from("fallback");

        assert_eq!(resolve(Some(&uri), Some(false), || Some(fallback.clone())), Some(fallback));
        assert_eq!(resolve(Some(&uri), Some(false), || None), Some(directory.path().to_path_buf()));
    }

    #[cfg(unix)]
    #[test]
    fn inaccessible_directory() {
        use std::os::unix::fs::PermissionsExt;

        // Root can traverse directories regardless of their permission bits.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let uri = file_uri(directory.path());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o600)).unwrap();
        let resolved = resolve(Some(&uri), None, || None);
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(resolved, None);
    }
}
