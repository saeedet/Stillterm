//! Small native settings form; preferences are replaced only after validation.
use super::*;
use std::io::Write;
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows_sys::Win32::UI::{
    Controls::EM_SETLIMITTEXT,
    Input::KeyboardAndMouse::{EnableWindow, SetFocus},
};

struct UiFont(HFONT);
impl Drop for UiFont {
    fn drop(&mut self) {
        // SAFETY: the options window and all its controls are destroyed first.
        unsafe {
            DeleteObject(self.0);
        }
    }
}

struct Form {
    theme: HWND,
    fields: Vec<HWND>,
}
const LABELS: [&str; 7] = [
    "Speed (0.1–4)",
    "Density (0–1)",
    "Brightness (0–1)",
    "Frames per second (10–60)",
    "Character size (10–48)",
    "Seed",
    "Characters (blank = preset)",
];

fn save(settings: &Settings) -> Result<(), String> {
    let text = settings.to_toml()?;
    let path = settings_path()?;
    let directory = path.parent().ok_or("Invalid settings location")?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        use std::os::windows::ffi::OsStrExt;
        let source: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: paths are terminated UTF-16 buffers and remain live during replacement.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
unsafe fn field_text(hwnd: HWND) -> String {
    // SAFETY: hwnd is one of this form's live text controls; buffer size is bounded.
    unsafe {
        let mut buffer = vec![0u16; 2049];
        let length = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }
}
unsafe fn read_form(form: &Form) -> Result<Settings, String> {
    // SAFETY: all controls remain alive for the duration of WM_COMMAND.
    unsafe {
        let fields: Vec<String> = form.fields.iter().map(|h| field_text(*h)).collect();
        let number = |i: usize| {
            fields[i]
                .trim()
                .parse::<f64>()
                .map(Some)
                .map_err(|_| format!("Invalid {}", LABELS[i]))
        };
        let settings = Settings {
            theme: if SendMessageW(form.theme, CB_GETCURSEL, 0, 0) == 1 {
                "matrix"
            } else {
                "monochrome"
            }
            .into(),
            speed: number(0)?,
            density: number(1)?,
            intensity: number(2)?,
            fps: fields[3]
                .trim()
                .parse()
                .map_err(|_| "Invalid frames per second")?,
            font_size: fields[4]
                .trim()
                .parse()
                .map_err(|_| "Invalid character size")?,
            seed: fields[5]
                .trim()
                .parse()
                .map_err(|_| "Seed must be a nonnegative whole number")?,
            characters: fields[6].clone(),
        };
        settings.validate()?;
        Ok(settings)
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: owner keeps Form alive until DestroyWindow returns. Catch all unwinds.
    unsafe {
        std::panic::catch_unwind(|| message(hwnd, msg, w, l)).unwrap_or_else(|_| {
            PostQuitMessage(1);
            0
        })
    }
}
unsafe fn message(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: pointers are installed only from this form's synchronous CreateWindowEx.
    // No borrowed mutable form state is held across reentrant window operations.
    unsafe {
        if msg == WM_NCCREATE {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (*(l as *const CREATESTRUCTW)).lpCreateParams as isize,
            );
            return 1;
        }
        let form = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Form;
        match msg {
            WM_COMMAND if !form.is_null() => {
                let id = w & 0xffff;
                let notification = (w >> 16) & 0xffff;
                match id {
                    1 => match read_form(&*form).and_then(|s| save(&s)) {
                        Ok(()) => {
                            PostMessageW(hwnd, WM_CLOSE, 0, 0);
                        }
                        Err(error) => {
                            MessageBoxW(
                                hwnd,
                                wide(&error).as_ptr(),
                                wide("Check settings").as_ptr(),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    },
                    2 => {
                        PostMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                    10 if notification == CBN_SELCHANGE as usize => {
                        let theme = if SendMessageW((*form).theme, CB_GETCURSEL, 0, 0) == 1 {
                            Theme::Matrix
                        } else {
                            Theme::Monochrome
                        };
                        let preset = theme.rain_defaults();
                        for (index, value) in [preset.speed, preset.density, preset.intensity]
                            .iter()
                            .enumerate()
                        {
                            SetWindowTextW(
                                (&(*form).fields)[index],
                                wide(&value.to_string()).as_ptr(),
                            );
                        }
                        SetWindowTextW((&(*form).fields)[6], wide("").as_ptr());
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, msg, w, l)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

pub fn run(owner: HWND, settings: Result<Settings, String>) -> Result<(), String> {
    // SAFETY: controls, their parent, and Form stay on this thread. The owner is
    // validated before use and reenabled on every return from the modal loop.
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
        let scale = GetDpiForSystem().max(96) as i32;
        let px = |n: i32| n * scale / 96;
        let owner = if IsWindow(owner) != 0 {
            owner
        } else {
            null_mut()
        };
        let font = UiFont(CreateFontW(
            -px(14),
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            ANTIALIASED_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide("Segoe UI").as_ptr(),
        ));
        if font.0.is_null() {
            return Err("Cannot create settings font".into());
        }
        let instance = GetModuleHandleW(null());
        let class = wide("Stillterm.Options");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Cannot register settings window".into());
        }
        let mut form = Box::new(Form {
            theme: null_mut(),
            fields: vec![],
        });
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Stillterm Settings").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            px(530),
            px(430),
            owner,
            null_mut(),
            instance,
            (&mut *form as *mut Form).cast(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class.as_ptr(), instance);
            return Err("Cannot create settings window".into());
        }
        let result = (|| {
            let control = |kind: &str,
                           text: &str,
                           style: u32,
                           id: usize,
                           x,
                           y,
                           width,
                           height|
             -> Result<HWND, String> {
                let handle = CreateWindowExW(
                    0,
                    wide(kind).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    px(x),
                    px(y),
                    px(width),
                    px(height),
                    hwnd,
                    id as HMENU,
                    instance,
                    null(),
                );
                if handle.is_null() {
                    return Err("Cannot create settings control".into());
                }
                SendMessageW(handle, WM_SETFONT, font.0 as usize, 1);
                Ok(handle)
            };
            let initial_error = settings.as_ref().err().cloned();
            let settings = settings.unwrap_or_default();
            let preset = settings.validate()?.0.rain_defaults();
            control("STATIC", "Theme", 0, 0, 20, 22, 200, 24)?;
            form.theme = control(
                "COMBOBOX",
                "",
                WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                10,
                225,
                18,
                270,
                150,
            )?;
            for theme in ["Monochrome", "Matrix"] {
                SendMessageW(form.theme, CB_ADDSTRING, 0, wide(theme).as_ptr() as isize);
            }
            SendMessageW(
                form.theme,
                CB_SETCURSEL,
                usize::from(settings.theme == "matrix"),
                0,
            );
            let values = [
                settings.speed.unwrap_or(preset.speed).to_string(),
                settings.density.unwrap_or(preset.density).to_string(),
                settings.intensity.unwrap_or(preset.intensity).to_string(),
                settings.fps.to_string(),
                settings.font_size.to_string(),
                settings.seed.to_string(),
                settings.characters,
            ];
            for (i, label) in LABELS.iter().enumerate() {
                let y = 58 + i as i32 * 36;
                control("STATIC", label, 0, 0, 20, y + 3, 205, 24)?;
                let field = control(
                    "EDIT",
                    &values[i],
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    20 + i,
                    225,
                    y,
                    270,
                    26,
                )?;
                SendMessageW(field, EM_SETLIMITTEXT, if i == 6 { 1024 } else { 32 }, 0);
                form.fields.push(field);
            }
            control(
                "BUTTON",
                "Save",
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                1,
                310,
                330,
                85,
                30,
            )?;
            control("BUTTON", "Cancel", WS_TABSTOP, 2, 410, 330, 85, 30)?;
            if !owner.is_null() {
                EnableWindow(owner, 0);
            }
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            SetFocus(form.theme);
            if let Some(error) = initial_error {
                MessageBoxW(
                    hwnd,
                    wide(&format!(
                        "Saved settings could not be loaded. Showing defaults.\n\n{error}"
                    ))
                    .as_ptr(),
                    wide("Stillterm Settings").as_ptr(),
                    MB_OK | MB_ICONWARNING,
                );
            }
            let mut message = std::mem::zeroed();
            loop {
                let status = GetMessageW(&mut message, null_mut(), 0, 0);
                if status == -1 {
                    return Err("Settings message loop failed".into());
                }
                if status == 0 {
                    return Ok(());
                }
                if IsDialogMessageW(hwnd, &message) == 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        })();
        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
        if !owner.is_null() {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
        UnregisterClassW(class.as_ptr(), instance);
        result
    }
}
