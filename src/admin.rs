//! Elevation, DPI awareness and console setup.
//!
//! NetTrack needs administrator rights for two things: enabling the ETW
//! kernel network providers, and reading per-connection TCP statistics for
//! processes owned by other users.
//!
//! Rather than depending on an embedded application manifest (which would
//! need a resource compiler at build time), the process asks for elevation
//! itself at start-up: if it is not already elevated, it relaunches through
//! the shell's `runas` verb and hands over to the elevated copy.

use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows::Win32::Globalization::CP_UTF8;
use windows::Win32::System::Console::{SetConsoleCP, SetConsoleOutputCP};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Whether the current process is running elevated.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            size,
            &mut size,
        )
        .is_ok();

        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// What happened when elevation was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// The process is already elevated.
    AlreadyElevated,
    /// An elevated copy was started; the current process should exit.
    Relaunched,
    /// Elevation was refused or failed; the process continues with reduced
    /// functionality.
    Failed,
}

/// Relaunch the process elevated if it is not already.
///
/// Returns [`Elevation::Relaunched`] when a new elevated process was started,
/// in which case the caller should exit immediately.
pub fn ensure_elevated() -> Elevation {
    if is_elevated() {
        return Elevation::AlreadyElevated;
    }

    let Ok(exe) = std::env::current_exe() else {
        return Elevation::Failed;
    };
    let exe = exe.to_string_lossy().to_string();

    // Pass the original arguments through so the relaunch behaves the same.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let params = args.join(" ");

    let verb: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
    let file: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
    let params: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            None,
            windows::core::PCWSTR(verb.as_ptr()),
            windows::core::PCWSTR(file.as_ptr()),
            windows::core::PCWSTR(params.as_ptr()),
            None,
            SW_SHOWNORMAL,
        )
    };

    // A result greater than 32 means success.
    if result.0 as isize > 32 {
        Elevation::Relaunched
    } else {
        Elevation::Failed
    }
}

/// Make the process per-monitor DPI aware so the layout stays crisp on
/// high density displays.
///
/// This has to happen before anything is drawn.
pub fn set_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Switch the console to UTF-8 so that the box drawing characters and the
/// country flags render correctly.
pub fn set_utf8_codepage() {
    unsafe {
        let _ = SetConsoleOutputCP(CP_UTF8);
        let _ = SetConsoleCP(CP_UTF8);
    }
}

/// Describe what is unavailable when the process is not elevated.
pub fn degraded_features() -> &'static str {
    if is_elevated() {
        ""
    } else {
        "not elevated: live UDP peers and cross-user process details are unavailable"
    }
}

/// A banner line shown in the UI when not elevated.
pub fn elevation_banner() -> Option<(&'static str, &'static str)> {
    if is_elevated() {
        None
    } else {
        Some((
            "NOT ELEVATED",
            "Run as Administrator to see UDP peers (YouTube/QUIC), connect/disconnect events,\nand per-connection TCP stats for processes owned by other users.",
        ))
    }
}

/// The handle of the running instance.
pub fn instance() -> HMODULE {
    unsafe { GetModuleHandleW(None).unwrap_or_default() }
}
