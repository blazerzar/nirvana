use std::mem;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS, OpenInputDesktop,
};

pub(super) fn get_idle_seconds() -> u64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let _ = GetLastInputInfo(&mut info);
        let current = GetTickCount64();
        current.saturating_sub(info.dwTime as u64) / 1000
    }
}

pub(super) fn is_locked() -> bool {
    unsafe {
        // OpenInputDesktop fails when the workstation is locked
        let hdesk = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x0100));
        match hdesk {
            Ok(handle) => {
                let _ = CloseDesktop(handle);
                false
            }
            Err(_) => true,
        }
    }
}
