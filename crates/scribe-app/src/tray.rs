//! Windows tray icon with a cross-platform no-op facade.
//!
//! The win32 implementation lives on a dedicated background thread that owns
//! a message-only window (`HWND_MESSAGE`) with its own WndProc; the thread
//! touches ONLY tray state (icon registration, popup menu, balloon). All
//! application interaction crosses a single `mpsc` channel drained by the
//! GPUI-side timer loop in `window.rs`. No panics across FFI: every call is
//! checked and logged, and tray startup failure degrades to `None` (the app
//! simply runs without a tray icon).

use std::sync::mpsc;

/// Events the tray thread pushes for the GPUI drain loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrayEvent {
    Open,
    CheckUpdates,
    BalloonClicked,
    Quit,
}

const MENU_OPEN: u32 = 1;
const MENU_CHECK_UPDATES: u32 = 2;
const MENU_QUIT: u32 = 3;

/// Maps a tray popup-menu command id to an application event (pure; tested).
pub(crate) fn menu_command_event(command: u32) -> Option<TrayEvent> {
    match command {
        MENU_OPEN => Some(TrayEvent::Open),
        MENU_CHECK_UPDATES => Some(TrayEvent::CheckUpdates),
        MENU_QUIT => Some(TrayEvent::Quit),
        _ => None,
    }
}

#[cfg(windows)]
pub(crate) use imp::{TrayHandle, window_is_hidden};

#[cfg(not(windows))]
pub(crate) use stub::{TrayHandle, window_is_hidden};

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NIN_BALLOONUSERCLICK, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
        GetMessageW, GetWindowLongPtrW, HWND_MESSAGE, IsIconic, MF_SEPARATOR, MF_STRING,
        PostMessageW, RegisterClassExW, SetForegroundWindow, SetWindowLongPtrW, TPM_NONOTIFY,
        TPM_RETURNCMD, TrackPopupMenu, TranslateMessage, WM_APP, WM_DESTROY, WM_LBUTTONUP,
        WM_NCCREATE, WM_NULL, WM_RBUTTONUP, WNDCLASSEXW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{CREATESTRUCTW, MSG};

    const TRAY_ID: u32 = 1;
    const TRAY_CALLBACK: u32 = WM_APP + 1;
    const TRAY_BALLOON: u32 = WM_APP + 2;
    const TRAY_SHUTDOWN: u32 = WM_APP + 3;
    const CLASS_NAME: &[u16] = &wide("ScribeTrayWindow");
    const ICON_BYTES: &[u8] = include_bytes!("../../../assets/scribe-icon-v2.ico");

    const fn wide(text: &str) -> [u16; 32] {
        let bytes = text.as_bytes();
        let mut out = [0u16; 32];
        let mut index = 0;
        while index < bytes.len() && index < 31 {
            out[index] = bytes[index] as u16;
            index += 1;
        }
        out
    }

    fn wide_string(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn write_wide(target: &mut [u16], text: &str) {
        for (slot, value) in target
            .iter_mut()
            .zip(text.encode_utf16().chain(std::iter::once(0)))
        {
            *slot = value;
        }
    }

    fn log_tray(message: &str) {
        eprintln!("scribe tray: {message}");
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_NCCREATE => {
                let create = lparam as *const CREATESTRUCTW;
                if create.is_null() {
                    return 0;
                }
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize)
                };
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
            WM_DESTROY => {
                let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
                if pointer != 0 {
                    drop(unsafe { Box::from_raw(pointer as *mut mpsc::Sender<TrayEvent>) });
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
                }
                0
            }
            TRAY_CALLBACK => {
                match lparam as u32 {
                    WM_LBUTTONUP => send_event(hwnd, TrayEvent::Open),
                    WM_RBUTTONUP => show_menu(hwnd),
                    NIN_BALLOONUSERCLICK => send_event(hwnd, TrayEvent::BalloonClicked),
                    _ => {}
                }
                0
            }
            TRAY_BALLOON => {
                show_balloon(hwnd, wparam);
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn sender(hwnd: HWND) -> Option<&'static mpsc::Sender<TrayEvent>> {
        let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
        if pointer == 0 {
            return None;
        }
        Some(unsafe { &*(pointer as *const mpsc::Sender<TrayEvent>) })
    }

    fn send_event(hwnd: HWND, event: TrayEvent) {
        if let Some(sender) = sender(hwnd) {
            let _ = sender.send(event);
        }
    }

    fn show_menu(hwnd: HWND) {
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                log_tray("CreatePopupMenu failed");
                return;
            }
            let open = wide_string("Open Scribe");
            let check = wide_string("Check for updates now");
            let quit = wide_string("Quit");
            AppendMenuW(menu, MF_STRING, MENU_OPEN as usize, open.as_ptr());
            AppendMenuW(menu, MF_STRING, MENU_CHECK_UPDATES as usize, check.as_ptr());
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, MENU_QUIT as usize, quit.as_ptr());
            let mut point = POINT::default();
            if GetCursorPos(&mut point) == 0 {
                log_tray("GetCursorPos failed");
                DestroyMenu(menu);
                return;
            }
            SetForegroundWindow(hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
            PostMessageW(hwnd, WM_NULL, 0, 0);
            if let Some(event) = menu_command_event(command as u32) {
                send_event(hwnd, event);
            }
        }
    }

    fn show_balloon(hwnd: HWND, count: usize) {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_INFO,
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };
        write_wide(&mut data.szInfoTitle, "Scribe");
        let plural = if count == 1 { "update" } else { "updates" };
        write_wide(&mut data.szInfo, &format!("{count} {plural} available"));
        data.Anonymous.uTimeout = 8_000;
        if unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) } == 0 {
            log_tray("Shell_NotifyIconW(NIM_MODIFY) failed");
        }
    }

    fn tray_thread(events: mpsc::Sender<TrayEvent>, ready: mpsc::Sender<Result<isize, String>>) {
        unsafe {
            let instance =
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance,
                lpszClassName: CLASS_NAME.as_ptr(),
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 {
                let _ = ready.send(Err("RegisterClassExW failed".into()));
                return;
            }
            let hwnd = CreateWindowExW(
                0,
                CLASS_NAME.as_ptr(),
                CLASS_NAME.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                instance,
                Box::into_raw(Box::new(events)) as *const core::ffi::c_void,
            );
            if hwnd.is_null() {
                let _ = ready.send(Err("CreateWindowExW failed".into()));
                return;
            }
            let icon = CreateIconFromResourceEx(
                ICON_BYTES.as_ptr(),
                ICON_BYTES.len() as u32,
                1,
                0x0003_0000,
                0,
                0,
                0,
            );
            if icon.is_null() {
                let _ = ready.send(Err("CreateIconFromResourceEx failed".into()));
                DestroyWindow(hwnd);
                return;
            }
            let mut data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: TRAY_CALLBACK,
                hIcon: icon,
                ..Default::default()
            };
            write_wide(&mut data.szTip, "Scribe");
            if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                let _ = ready.send(Err("Shell_NotifyIconW(NIM_ADD) failed".into()));
                DestroyIcon(icon);
                DestroyWindow(hwnd);
                return;
            }
            if ready.send(Ok(hwnd as isize)).is_err() {
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyIcon(icon);
                DestroyWindow(hwnd);
                return;
            }
            let mut message = MSG::default();
            loop {
                let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
                if result <= 0 || message.message == TRAY_SHUTDOWN {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyIcon(icon);
            DestroyWindow(hwnd);
        }
    }

    /// Owns the tray thread; removes the icon and joins on drop.
    pub(crate) struct TrayHandle {
        hwnd: Arc<AtomicIsize>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl TrayHandle {
        pub(crate) fn start(events: mpsc::Sender<TrayEvent>) -> Option<Self> {
            let (ready_tx, ready_rx) = mpsc::channel();
            let thread = std::thread::spawn(move || tray_thread(events, ready_tx));
            let hwnd = match ready_rx.recv_timeout(Duration::from_secs(3)) {
                Ok(Ok(hwnd)) => hwnd,
                Ok(Err(error)) => {
                    log_tray(&format!("startup failed: {error}"));
                    let _ = thread.join();
                    return None;
                }
                Err(_) => {
                    log_tray("startup timed out");
                    return None;
                }
            };
            Some(Self {
                hwnd: Arc::new(AtomicIsize::new(hwnd)),
                thread: Some(thread),
            })
        }

        pub(crate) fn notify_updates(&self, count: usize) {
            let hwnd = self.hwnd.load(Ordering::Acquire);
            if hwnd != 0 {
                unsafe {
                    PostMessageW(hwnd as HWND, TRAY_BALLOON, count, 0);
                }
            }
        }
    }

    impl Drop for TrayHandle {
        fn drop(&mut self) {
            let hwnd = self.hwnd.swap(0, Ordering::AcqRel);
            if hwnd != 0 {
                unsafe {
                    PostMessageW(hwnd as HWND, TRAY_SHUTDOWN, 0, 0);
                }
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// True when the GPUI window is minimized to the taskbar (balloons only
    /// make sense while hidden).
    pub(crate) fn window_is_hidden(window: &gpui::Window) -> bool {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // NB: gpui's inherent `window_handle()` returns its own AnyWindowHandle;
        // the raw handle comes from the HasWindowHandle trait method.
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return false;
        };
        let RawWindowHandle::Win32(handle) = handle.as_ref() else {
            return false;
        };
        unsafe { IsIconic(handle.hwnd.get() as HWND) != 0 }
    }
}

#[cfg(not(windows))]
mod stub {
    use super::*;

    pub(crate) struct TrayHandle;

    impl TrayHandle {
        pub(crate) fn start(_events: mpsc::Sender<TrayEvent>) -> Option<Self> {
            None
        }

        pub(crate) fn notify_updates(&self, _count: usize) {}
    }

    pub(crate) fn window_is_hidden(_window: &gpui::Window) -> bool {
        false
    }
}
