use crossterm::{cursor, terminal, ExecutableCommand};
use std::io;
/// Own only console modes changed for a running interactive operation.
pub(super) struct Session {
    entered: bool,
    #[cfg(windows)]
    modes: Vec<(isize, u32, bool)>,
}
impl Session {
    pub fn open() -> io::Result<Self> {
        let mut session = Self {
            entered: false,
            #[cfg(windows)]
            modes: Vec::new(),
        };
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Console::*;
            let input = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(input, &mut mode) == 0 {
                return Err(io::Error::last_os_error());
            }
            {
                if SetConsoleMode(
                    input,
                    (mode | ENABLE_EXTENDED_FLAGS) & !ENABLE_QUICK_EDIT_MODE,
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                session.modes.push((input as isize, mode, true));
            }
            let output = GetStdHandle(STD_ERROR_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(output, &mut mode) != 0 {
                // Crossterm can also use the native Windows backend when VT is unavailable.
                if SetConsoleMode(
                    output,
                    mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
                ) != 0
                {
                    session.modes.push((output as isize, mode, false));
                }
            }
        }
        session.entered = true;
        std::io::stderr().execute(terminal::EnterAlternateScreen)?;
        std::io::stderr().execute(cursor::Hide)?;
        Ok(session)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.entered {
            let _ = std::io::stderr()
                .execute(cursor::Show)
                .and_then(|w| w.execute(terminal::LeaveAlternateScreen));
        }
        #[cfg(windows)]
        unsafe {
            for &(handle, mode, input) in self.modes.iter().rev() {
                use windows_sys::Win32::System::Console::{SetConsoleMode, ENABLE_EXTENDED_FLAGS};
                let handle = handle as windows_sys::Win32::Foundation::HANDLE;
                // QuickEdit changes require EXTENDED_FLAGS even when the original mode lacked it.
                if input && mode & ENABLE_EXTENDED_FLAGS == 0 {
                    SetConsoleMode(handle, mode | ENABLE_EXTENDED_FLAGS);
                }
                windows_sys::Win32::System::Console::SetConsoleMode(handle, mode);
            }
        }
    }
}
