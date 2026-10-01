//! Win32 boundary. Windows owns windows; the message loop owns their boxed state.
//! Callbacks never free state, and unwind panics never cross the system ABI.
mod options;
mod render;

use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use stillterm_engine::{Engine, StepClock, Theme};
use stillterm_windows::{invocation::Mode, settings::Settings};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub fn show_error(text: &str) {
    // SAFETY: null owner is allowed; both UTF-16 strings live through the call.
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide("Stillterm").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
fn settings_path() -> Result<PathBuf, String> {
    let base =
        std::env::var_os("APPDATA").ok_or("Windows application-data folder is unavailable")?;
    Ok(PathBuf::from(base)
        .join("Stillterm")
        .join("screensaver.toml"))
}
fn load_settings() -> Result<Settings, String> {
    let path = settings_path()?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings::default());
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut text = String::new();
    file.take(65_537)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Settings::parse(&text)
}

struct State {
    settings: Settings,
    theme: Theme,
    engine: Option<Engine>,
    surface: Option<render::Surface>,
    clock: StepClock,
    last: Instant,
    started: Instant,
    initial_cursor: POINT,
    parent: HWND,
    fullscreen: bool,
    ready: bool,
    suspended: bool,
    dpi: u32,
}
impl State {
    fn new(settings: Settings, parent: HWND, fullscreen: bool) -> Result<Self, String> {
        let (theme, _) = settings.validate()?;
        let mut initial_cursor = POINT { x: 0, y: 0 };
        // SAFETY: writes to a live stack point.
        unsafe {
            GetCursorPos(&mut initial_cursor);
        }
        Ok(Self {
            settings,
            theme,
            engine: None,
            surface: None,
            clock: StepClock::default(),
            last: Instant::now(),
            started: Instant::now(),
            initial_cursor,
            parent,
            fullscreen,
            ready: false,
            suspended: false,
            dpi: 0,
        })
    }
    fn release(&mut self) {
        self.engine = None;
        self.surface = None;
        self.clock.reset();
        self.last = Instant::now();
    }
    fn tick(&mut self, window: HWND) -> Result<bool, String> {
        // SAFETY: the callback's window is live; output rectangles are writable.
        unsafe {
            if self.suspended || IsWindowVisible(window) == 0 || IsIconic(window) != 0 {
                self.release();
                return Ok(false);
            }
            let mut rect: RECT = std::mem::zeroed();
            if GetClientRect(window, &mut rect) == 0 {
                return Err("Cannot read drawing area".into());
            }
            let width = rect.right;
            let height = rect.bottom;
            if width <= 0 || height <= 0 {
                self.release();
                return Ok(false);
            }
            let dpi = GetDpiForWindow(window).max(96);
            if self.dpi != dpi
                || self
                    .surface
                    .as_ref()
                    .is_none_or(|s| s.width != width || s.height != height)
            {
                let scale = if self.fullscreen {
                    1.0
                } else {
                    (f64::from(width) / 800.0)
                        .min(f64::from(height) / 500.0)
                        .min(1.0)
                };
                let font = (f64::from(self.settings.font_size) * f64::from(dpi) / 96.0 * scale)
                    .max(7.0) as i32;
                let surface = render::Surface::new(width, height, font)?;
                if let Some(engine) = &mut self.engine {
                    engine.resize(surface.size);
                } else {
                    self.engine = Some(Engine::rain(
                        surface.size,
                        self.settings.validate()?.1,
                        self.settings.seed,
                    ));
                }
                self.surface = Some(surface);
                self.dpi = dpi;
                self.clock.reset();
                self.last = Instant::now();
            }
        }
        let now = Instant::now();
        let engine = self.engine.as_mut().ok_or("Missing animation state")?;
        for _ in 0..self.clock.advance(now.duration_since(self.last)) {
            engine.step();
        }
        self.last = now;
        Ok(self
            .surface
            .as_mut()
            .ok_or("Missing display surface")?
            .update(engine.frame(), self.theme))
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Windows supplies callback parameters. State lives until the owner
    // destroys every window after GetMessage returns; it is never dropped here.
    unsafe {
        std::panic::catch_unwind(|| window_message(hwnd, msg, w, l)).unwrap_or_else(|_| {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            PostQuitMessage(1);
            0
        })
    }
}
unsafe fn window_message(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: creation data points to a stable Box<State> owned by run_animation.
    // Mutable state borrows end before calls that can synchronously reenter it.
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            return 1;
        }
        if msg == WM_ERASEBKGND {
            return 1;
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if state.is_null() {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        match msg {
            WM_TIMER => {
                if !(*state).parent.is_null() {
                    let parent = (*state).parent;
                    if IsWindow(parent) == 0 {
                        PostQuitMessage(0);
                        return 0;
                    }
                    let mut rect: RECT = std::mem::zeroed();
                    if GetClientRect(parent, &mut rect) == 0 {
                        PostQuitMessage(0);
                        return 0;
                    }
                    let mut current: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut current);
                    if current.right != rect.right || current.bottom != rect.bottom {
                        SetWindowPos(
                            hwnd,
                            null_mut(),
                            0,
                            0,
                            rect.right,
                            rect.bottom,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        );
                    }
                }
                match (*state).tick(hwnd) {
                    Ok(true) => {
                        InvalidateRect(hwnd, null(), 0);
                    }
                    Ok(false) => {}
                    Err(_) => {
                        PostQuitMessage(1);
                    }
                }
                0
            }
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                if let Some(surface) = &(*state).surface {
                    surface.paint(dc);
                } else {
                    FillRect(dc, &paint.rcPaint, GetStockObject(BLACK_BRUSH) as HBRUSH);
                }
                EndPaint(hwnd, &paint);
                0
            }
            WM_SIZE => {
                (*state).surface = None;
                (*state).clock.reset();
                0
            }
            WM_DPICHANGED => {
                let rect = *(l as *const RECT);
                (*state).surface = None;
                if (*state).fullscreen {
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                }
                0
            }
            WM_POWERBROADCAST => {
                if w == PBT_APMSUSPEND as usize {
                    (*state).suspended = true;
                    (*state).release();
                }
                if w == PBT_APMRESUMEAUTOMATIC as usize || w == PBT_APMRESUMESUSPEND as usize {
                    (*state).suspended = false;
                    (*state).last = Instant::now();
                }
                1
            }
            WM_DISPLAYCHANGE if (*state).fullscreen => {
                PostQuitMessage(0);
                0
            }
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
            | WM_MOUSEWHEEL
                if (*state).fullscreen && (*state).ready =>
            {
                PostQuitMessage(0);
                0
            }
            WM_MOUSEMOVE if (*state).fullscreen && (*state).ready => {
                let mut point: POINT = std::mem::zeroed();
                if GetCursorPos(&mut point) != 0
                    && (*state).started.elapsed() > Duration::from_millis(300)
                {
                    let dx = i64::from(point.x) - i64::from((*state).initial_cursor.x);
                    let dy = i64::from(point.y) - i64::from((*state).initial_cursor.y);
                    if dx.abs() > 3 || dy.abs() > 3 {
                        PostQuitMessage(0);
                    }
                }
                0
            }
            WM_ACTIVATEAPP if w == 0 && (*state).fullscreen && (*state).ready => {
                PostQuitMessage(0);
                0
            }
            WM_SETCURSOR if (*state).fullscreen => {
                SetCursor(null_mut());
                1
            }
            WM_CLOSE => {
                PostQuitMessage(0);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                (*state).release();
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

unsafe extern "system" fn monitor_rect(_: HMONITOR, _: HDC, rect: *mut RECT, data: LPARAM) -> i32 {
    // SAFETY: synchronous EnumDisplayMonitors callback; data points to its live Vec.
    unsafe {
        (*(data as *mut Vec<RECT>)).push(*rect);
    }
    1
}

pub fn run(mode: Mode) -> Result<(), String> {
    // SAFETY: choose DPI context before creating windows. Embedded preview must
    // match its foreign parent to avoid Windows' cross-process DPI mismatch rules.
    unsafe {
        if let Mode::Preview(parent) = mode {
            let parent = parent.get() as HWND;
            if IsWindow(parent) == 0 {
                return Err("Preview parent no longer exists".into());
            }
            SetThreadDpiAwarenessContext(GetWindowDpiAwarenessContext(parent));
        } else {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }
    if let Mode::Configure(parent) = mode {
        return options::run(
            parent.map_or(null_mut(), |p| p.get() as HWND),
            load_settings(),
        );
    }
    // A corrupt preferences file must not show a modal error over the saver.
    run_animation(mode, load_settings().unwrap_or_default())
}
fn run_animation(mode: Mode, settings: Settings) -> Result<(), String> {
    // SAFETY: all window creation/dispatch/destruction occurs on this thread.
    // Boxes remain alive through DestroyWindow; cleanup also runs on errors.
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = wide("Stillterm.ScreenSaver");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
            ..std::mem::zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("Cannot register screensaver window".into());
        }
        let fullscreen = matches!(mode, Mode::Fullscreen);
        let parent = if let Mode::Preview(p) = mode {
            p.get() as HWND
        } else {
            null_mut()
        };
        let mut rects: Vec<RECT> = vec![];
        if fullscreen {
            EnumDisplayMonitors(
                null_mut(),
                null(),
                Some(monitor_rect),
                &mut rects as *mut _ as isize,
            );
        } else {
            let mut rect = std::mem::zeroed();
            if GetClientRect(parent, &mut rect) != 0 {
                rects.push(rect);
            }
        }
        let mut windows: Vec<(HWND, Box<State>)> = vec![];
        let result = (|| {
            if rects.is_empty() {
                return Err("No display surface available".into());
            }
            for rect in rects {
                let mut state = Box::new(State::new(settings.clone(), parent, fullscreen)?);
                let style = if fullscreen { WS_POPUP } else { WS_CHILD };
                let ex_style = if fullscreen {
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW
                } else {
                    0
                };
                let hwnd = CreateWindowExW(
                    ex_style,
                    class.as_ptr(),
                    wide("Stillterm").as_ptr(),
                    style,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    parent,
                    null_mut(),
                    instance,
                    (&mut *state as *mut State).cast(),
                );
                if hwnd.is_null() {
                    return Err("Cannot create screensaver window".into());
                }
                windows.push((hwnd, state));
                ShowWindow(hwnd, SW_SHOWNA);
                if SetTimer(hwnd, 1, 1000_u32.div_ceil(u32::from(settings.fps)), None) == 0 {
                    return Err("Cannot start animation timer".into());
                }
            }
            if fullscreen {
                SetForegroundWindow(windows[0].0);
            }
            for (_, state) in &mut windows {
                state.ready = true;
            }
            let mut message: MSG = std::mem::zeroed();
            loop {
                let status = GetMessageW(&mut message, null_mut(), 0, 0);
                if status == -1 {
                    return Err("Windows message loop failed".into());
                }
                if status == 0 {
                    return if message.wParam == 0 {
                        Ok(())
                    } else {
                        Err("Screensaver stopped after a rendering error".into())
                    };
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        })();
        for (hwnd, _) in &windows {
            if IsWindow(*hwnd) != 0 {
                DestroyWindow(*hwnd);
            }
        }
        drop(windows);
        UnregisterClassW(class.as_ptr(), instance);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_lifecycle_releases_resources_and_ignores_input() {
        // SAFETY: this test owns both windows and their boxed state on one thread.
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = wide("Stillterm.LifecycleTest");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let parent = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("Stillterm test").as_ptr(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                320,
                200,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            assert!(!parent.is_null());
            let mut state = Box::new(State::new(Settings::default(), parent, false).unwrap());
            let child = CreateWindowExW(
                0,
                class.as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                320,
                200,
                parent,
                null_mut(),
                instance,
                (&mut *state as *mut State).cast(),
            );
            assert!(!child.is_null());
            state.ready = true;
            SendMessageW(child, WM_TIMER, 1, 0);
            assert!(state.surface.is_some() && state.engine.is_some());
            let initial_size = state.engine.as_ref().unwrap().size();
            SendMessageW(child, WM_KEYDOWN, 27, 0);
            SendMessageW(child, WM_LBUTTONDOWN, 0, 0);
            // WM_QUIT is synthesized at low priority. Drain ordinary messages
            // as the real loop does; filtering only for WM_QUIT can miss it.
            let drain = || {
                let mut message: MSG = std::mem::zeroed();
                let mut quit = false;
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if message.message == WM_QUIT {
                        quit = true;
                    } else {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                quit
            };
            assert!(!drain(), "Preview input must not exit the saver process");
            SetWindowPos(
                parent,
                null_mut(),
                0,
                0,
                600,
                350,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            SendMessageW(child, WM_TIMER, 1, 0);
            assert_ne!(state.engine.as_ref().unwrap().size(), initial_size);
            SendMessageW(child, WM_POWERBROADCAST, PBT_APMSUSPEND as usize, 0);
            SendMessageW(child, WM_TIMER, 1, 0);
            assert!(state.engine.is_none() && state.surface.is_none());
            SendMessageW(child, WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, 0);
            SendMessageW(child, WM_TIMER, 1, 0);
            assert!(state.engine.is_some());
            ShowWindow(parent, SW_HIDE);
            SendMessageW(child, WM_TIMER, 1, 0);
            assert!(state.engine.is_none() && state.surface.is_none());
            DestroyWindow(parent);
            assert_eq!(IsWindow(child), 0);
            assert!(state.engine.is_none());
            assert!(drain(), "Parent destruction must end the preview loop");
            UnregisterClassW(class.as_ptr(), instance);
        }
    }
}
