use hmail_cleaner_core::cleanup::PortResult;
use std::{fs, path::Path};

pub fn verify_data_folder(root: &Path, ini_path: &Path) -> PortResult<()> {
    if !root.is_absolute() || !ini_path.is_absolute() {
        return Err("DataFolder and hMailServer.ini paths must be absolute".into());
    }
    if !fs::metadata(ini_path)?.is_file() {
        return Err("hMailServer.ini is not a file".into());
    }
    if !fs::metadata(root)?.is_dir() {
        return Err("DataFolder is not a directory".into());
    }
    let configured = read_data_folder(ini_path)?;
    if configured.trim().is_empty() || !Path::new(&configured).is_absolute() {
        return Err("INI DataFolder must be a nonempty absolute path".into());
    }
    let actual = root.canonicalize()?;
    let expected = Path::new(&configured).canonicalize()?;
    #[cfg(windows)]
    let matches = actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy());
    #[cfg(not(windows))]
    let matches = actual == expected;
    if !matches {
        return Err("DataFolder differs from hMailServer.ini".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn read_data_folder(path: &Path) -> PortResult<String> {
    let contents = fs::read_to_string(path)?;
    let mut directories = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            directories = line.eq_ignore_ascii_case("[Directories]");
            continue;
        }
        if !directories {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("DataFolder") {
                return Ok(value.trim().trim_matches(['\'', '"']).into());
            }
        }
    }
    Err("DataFolder is missing from [Directories] in hMailServer.ini".into())
}
#[cfg(windows)]
fn read_data_folder(path: &Path) -> PortResult<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::WindowsProgramming::GetPrivateProfileStringW;
    const BUFFER_CHARACTERS: usize = 32768;
    let wide = |value: &std::ffi::OsStr| value.encode_wide().chain(Some(0)).collect::<Vec<_>>();
    let file = wide(path.as_os_str());
    let section = wide(std::ffi::OsStr::new("Directories"));
    let key = wide(std::ffi::OsStr::new("DataFolder"));
    let mut buffer = vec![0u16; BUFFER_CHARACTERS];
    // All input strings are NUL-terminated and live through the call; the buffer owns nSize elements.
    let count = unsafe {
        GetPrivateProfileStringW(
            section.as_ptr(),
            key.as_ptr(),
            std::ptr::null(),
            buffer.as_mut_ptr(),
            BUFFER_CHARACTERS as u32,
            file.as_ptr(),
        )
    } as usize;
    if count == 0 || count == BUFFER_CHARACTERS - 1 {
        return Err("INI DataFolder is missing or too long".into());
    }
    Ok(String::from_utf16(&buffer[..count])?)
}
