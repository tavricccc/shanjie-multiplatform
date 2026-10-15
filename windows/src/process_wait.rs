//! Bounded process waits for the manager's Shell/TSF window-owning thread.
use crate::profile::Result;
use std::{
    io,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{GetExitCodeProcess, WaitForSingleObject},
    UI::WindowsAndMessaging::*,
};

fn timeout_error() -> Box<dyn std::error::Error> {
    io::Error::new(io::ErrorKind::TimedOut,
        "等待管理員工具結束已逾時；它可能仍在執行，請確認安裝視窗及紀錄，勿重複啟動安裝。程式未被強制終止。"
    ).into()
}

fn wait_ready(handle: HANDLE, timeout: Duration) -> Result<()> {
    if handle.is_invalid() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "管理員工具沒有提供有效的程序控制代碼。",
        )
        .into());
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("等待期限過長。")?;
    loop {
        // Give process completion priority even if messages arrive continuously.
        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 => return Ok(()),
            WAIT_FAILED => return Err(io::Error::last_os_error().into()),
            WAIT_TIMEOUT => {}
            _ => return Err("程序等待回傳非預期狀態。".into()),
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timeout_error());
        }
        let milliseconds = remaining.as_millis().max(1).min(u32::MAX as u128 - 1) as u32;
        let status = unsafe {
            MsgWaitForMultipleObjectsEx(
                Some(&[handle]),
                milliseconds,
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if status == WAIT_OBJECT_0 {
            return Ok(());
        }
        if status == WAIT_FAILED {
            return Err(io::Error::last_os_error().into());
        }
        if status == WAIT_TIMEOUT {
            continue;
        }
        if status.0 != WAIT_OBJECT_0.0 + 1 {
            return Err("視窗訊息等待回傳非預期狀態。".into());
        }
        // A bounded batch prevents a busy queue from defeating the deadline.
        for _ in 0..64 {
            if Instant::now() >= deadline {
                return Err(timeout_error());
            }
            let mut message = MSG::default();
            if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                break;
            }
            if message.message == WM_QUIT {
                unsafe {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "等待收到結束訊息；管理員工具可能仍在執行，請勿重複啟動安裝。",
                )
                .into());
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

/// The caller owns `handle`. Timeout/quit never terminates the child process.
pub fn wait_for_exit(handle: HANDLE, timeout: Duration) -> Result<u32> {
    wait_ready(handle, timeout)?;
    let mut code = 0;
    unsafe {
        GetExitCodeProcess(handle, &mut code)?;
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::Command,
    };
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::*,
            System::{LibraryLoader::GetModuleHandleW, Threading::*},
        },
    };
    struct Event(HANDLE);
    impl Event {
        fn new() -> Self {
            Self(unsafe { CreateEventW(None, true, false, None).unwrap() })
        }
    }
    impl Drop for Event {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    #[test]
    fn real_child_exit_status_is_preserved() {
        let system = std::env::var_os("SystemRoot").unwrap();
        let mut child = Command::new(std::path::PathBuf::from(system).join("System32").join("cmd.exe"))
            .args(["/d", "/c"])
            .raw_arg("exit 7")
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()
            .unwrap();
        assert_eq!(
            wait_for_exit(HANDLE(child.as_raw_handle()), Duration::from_secs(5)).unwrap(),
            7
        );
        assert_eq!(child.wait().unwrap().code(), Some(7));
    }

    #[test]
    fn pending_wait_times_out_without_signalling_or_closing_handle() {
        let event = Event::new();
        let started = Instant::now();
        let error = wait_ready(event.0, Duration::from_millis(30)).unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(unsafe { WaitForSingleObject(event.0, 0) }, WAIT_TIMEOUT);
    }

    #[test]
    fn invalid_handle_is_rejected() {
        assert!(wait_for_exit(HANDLE::default(), Duration::from_secs(1)).is_err());
    }

    unsafe extern "system" fn probe(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if message == WM_APP + 73 {
            LRESULT(73)
        } else {
            DefWindowProcW(window, message, wp, lp)
        }
    }

    #[test]
    fn hidden_test_window_answers_sent_messages_during_wait() {
        unsafe {
            let event = Event::new();
            let module = GetModuleHandleW(None).unwrap();
            let class = crate::wide(&format!("ShanjieWaitProbe-{}", GetCurrentThreadId()));
            let wc = WNDCLASSW {
                lpfnWndProc: Some(probe),
                hInstance: HINSTANCE(module.0),
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let window = CreateWindowExW(
                Default::default(),
                wc.lpszClassName,
                PCWSTR::null(),
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(wc.hInstance),
                None,
            )
            .unwrap();
            let window_bits = window.0 as usize;
            let event_bits = event.0 .0 as usize;
            let sender = std::thread::spawn(move || {
                let mut response = 0;
                let sent = SendMessageTimeoutW(
                    HWND(window_bits as _),
                    WM_APP + 73,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    1000,
                    Some(&mut response),
                );
                SetEvent(HANDLE(event_bits as _)).unwrap();
                (sent.0, response)
            });
            let waited = wait_ready(event.0, Duration::from_secs(3));
            let reply = sender.join().unwrap();
            DestroyWindow(window).unwrap();
            UnregisterClassW(wc.lpszClassName, Some(wc.hInstance)).unwrap();
            waited.unwrap();
            assert_ne!(reply.0, 0);
            assert_eq!(reply.1, 73);
        }
    }

    #[test]
    fn quit_message_is_preserved_and_stops_waiting() {
        unsafe {
            let event = Event::new();
            PostQuitMessage(19);
            let error = wait_ready(event.0, Duration::from_secs(1)).unwrap_err();
            assert_eq!(
                error.downcast_ref::<io::Error>().unwrap().kind(),
                io::ErrorKind::Interrupted
            );
            let mut message = MSG::default();
            assert!(PeekMessageW(&mut message, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool());
            assert_eq!(message.wParam.0, 19);
        }
    }
}
