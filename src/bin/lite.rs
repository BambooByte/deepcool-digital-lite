#![windows_subsystem = "windows"]

use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use hidapi::HidApi;
use slint::ComponentHandle;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_MANAGER_CONNECT,
    SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_STATUS,
};
use windows_sys::Win32::System::Threading::CreateMutexW;

slint::include_modules!();

const VID: u16 = 0x3633;
const PID: u16 = 0x0016;
const TASK_NAME: &str = "DeepCoolDigitalLite";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const INSTANCE_MUTEX: &str = "Local\\DeepCoolDigitalLite";

struct SingleInstance(HANDLE);

impl SingleInstance {
    fn acquire(name: &str) -> io::Result<Option<Self>> {
        let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle == 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn apply_native_title_bar(ui: &LiteWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_SHARED,
        SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let dark = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|value| value == 0)
        .unwrap_or(false);
    let handle = ui.window().window_handle();
    let Ok(handle) = handle.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get();
    let dark_value = i32::from(dark);
    let caption_color: u32 = if dark { 0x001c1c1c } else { 0x00fafafa };
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let resource = 100usize as *const u16;
        let small = LoadImageW(
            module,
            resource,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        );
        let big = LoadImageW(
            module,
            resource,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_SHARED,
        );
        if small != 0 {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small);
        }
        if big != 0 {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, big);
        }
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&dark_value as *const i32).cast(),
            std::mem::size_of_val(&dark_value) as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            (&caption_color as *const u32).cast(),
            std::mem::size_of_val(&caption_color) as u32,
        );
    }
}

fn schedule_native_title_bar(ui: &LiteWindow) {
    for delay in [1, 100, 300] {
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(delay), move || {
            if let Some(ui) = weak.upgrade() {
                apply_native_title_bar(&ui);
            }
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Cpu = 0,
    Gpu = 1,
    Mix = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CpuDetail {
    Fan = 0,
    Clock = 1,
    Auto = 2,
}

impl CpuDetail {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Clock,
            2 => Self::Auto,
            _ => Self::Fan,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Fan => "FAN",
            Self::Clock => "CLOCK",
            Self::Auto => "AUTO",
        }
    }
}

#[derive(Clone)]
struct MachineInfo {
    cpu: String,
    gpu: String,
    memory: String,
}

impl Default for MachineInfo {
    fn default() -> Self {
        Self {
            cpu: "Unavailable".into(),
            gpu: "Unavailable".into(),
            memory: "Unavailable".into(),
        }
    }
}

impl Mode {
    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Gpu,
            2 => Self::Mix,
            _ => Self::Cpu,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Mix => "MIX",
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = SingleInstance::acquire(INSTANCE_MUTEX)? else {
        return Ok(());
    };
    // Recreate the software-rendered window after hiding it to the tray.
    std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1");
    let initial_mode = load_mode();
    let initial_interval = load_interval();
    let initial_detail = load_detail();
    let mode = Arc::new(AtomicU8::new(initial_mode as u8));
    let interval = Arc::new(AtomicU8::new(initial_interval));
    let detail = Arc::new(AtomicU8::new(initial_detail as u8));
    let running = Arc::new(AtomicBool::new(true));
    let status = Arc::new(Mutex::new("Connecting to case display...".to_string()));
    let info = Arc::new(Mutex::new(load_machine_info()));
    let worker = {
        let mode = mode.clone();
        let interval = interval.clone();
        let detail = detail.clone();
        let running = running.clone();
        let status = status.clone();
        let info = info.clone();
        thread::spawn(move || run_display(mode, interval, detail, running, status, info))
    };

    let ui = LiteWindow::new()?;
    let tray = LiteTray::new()?;
    ui.set_selected_mode(initial_mode as i32);
    ui.set_selected_interval(initial_interval as i32);
    ui.set_selected_detail(initial_detail as i32);
    ui.set_startup_enabled(startup_enabled());

    let weak = ui.as_weak();
    tray.on_open_settings(move || {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.show();
            schedule_native_title_bar(&ui);
        }
    });
    tray.on_exit_app(|| {
        let _ = slint::quit_event_loop();
    });
    ui.window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    let weak = ui.as_weak();
    let mode_for_ui = mode.clone();
    ui.on_mode_changed(move |index| {
        let selected = Mode::from_index(index as usize);
        mode_for_ui.store(selected as u8, Ordering::Relaxed);
        if let Err(error) = save_mode(selected) {
            if let Some(ui) = weak.upgrade() {
                ui.set_error_message(error.to_string().into());
            }
        }
    });

    let weak = ui.as_weak();
    let interval_for_ui = interval.clone();
    ui.on_interval_changed(move |seconds| {
        let seconds = seconds as u8;
        interval_for_ui.store(seconds, Ordering::Relaxed);
        if let Err(error) = save_interval(seconds) {
            if let Some(ui) = weak.upgrade() {
                ui.set_error_message(error.to_string().into());
            }
        }
    });

    let weak = ui.as_weak();
    let detail_for_ui = detail.clone();
    ui.on_detail_changed(move |index| {
        let selected = CpuDetail::from_index(index as usize);
        detail_for_ui.store(selected as u8, Ordering::Relaxed);
        if let Err(error) = save_detail(selected) {
            if let Some(ui) = weak.upgrade() {
                ui.set_error_message(error.to_string().into());
            }
        }
    });

    let weak = ui.as_weak();
    ui.on_startup_changed(move |enabled| {
        if let Err(error) = set_startup(enabled) {
            if let Some(ui) = weak.upgrade() {
                ui.set_startup_enabled(startup_enabled());
                ui.set_error_message(error.to_string().into());
            }
        }
    });

    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            if let Some(ui) = weak.upgrade() {
                if let Ok(message) = status.lock() {
                    ui.set_status_message(message.clone().into());
                }
                if let Ok(info) = info.lock() {
                    ui.set_cpu_name(info.cpu.clone().into());
                    ui.set_gpu_name(info.gpu.clone().into());
                    ui.set_memory_info(info.memory.clone().into());
                }
            }
        },
    );
    tray.show()?;
    if !std::env::args().any(|arg| arg == "--minimized") {
        ui.show()?;
        schedule_native_title_bar(&ui);
    }
    let result = slint::run_event_loop();
    running.store(false, Ordering::Relaxed);
    let _ = worker.join();
    result?;
    Ok(())
}
fn settings_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default())
        .join("DeepCoolDigitalLite")
        .join(name)
}
fn load_mode() -> Mode {
    match fs::read_to_string(settings_path("mode.txt")).as_deref() {
        Ok("GPU") => Mode::Gpu,
        Ok("MIX") => Mode::Mix,
        _ => Mode::Cpu,
    }
}
fn save_mode(mode: Mode) -> io::Result<()> {
    let path = settings_path("mode.txt");
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, mode.label())
}
fn load_interval() -> u8 {
    match fs::read_to_string(settings_path("interval.txt"))
        .as_deref()
        .map(str::trim)
    {
        Ok("7") => 7,
        _ => 5,
    }
}
fn save_interval(seconds: u8) -> io::Result<()> {
    let path = settings_path("interval.txt");
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, seconds.to_string())
}
fn load_detail() -> CpuDetail {
    match fs::read_to_string(settings_path("detail.txt")).as_deref() {
        Ok("CLOCK") => CpuDetail::Clock,
        Ok("AUTO") => CpuDetail::Auto,
        _ => CpuDetail::Fan,
    }
}
fn save_detail(detail: CpuDetail) -> io::Result<()> {
    let path = settings_path("detail.txt");
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, detail.label())
}
fn startup_enabled() -> bool {
    use std::os::windows::process::CommandExt;
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
fn set_startup(enabled: bool) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("schtasks");
    if enabled {
        let exe = std::env::current_exe()?;
        cmd.args([
            "/Create",
            "/F",
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
            "/TN",
            TASK_NAME,
            "/TR",
            &format!("\"{}\" --minimized", exe.display()),
        ]);
    } else {
        cmd.args(["/Delete", "/F", "/TN", TASK_NAME]);
    }
    let output = cmd.creation_flags(CREATE_NO_WINDOW).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn set_status(status: &Mutex<String>, message: impl Into<String>) {
    if let Ok(mut current) = status.lock() {
        *current = message.into();
    }
}

fn official_service_running() -> bool {
    let name: Vec<u16> = "Deep Cool Display Service"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let manager = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if manager == 0 {
            return false;
        }
        let service = OpenServiceW(manager, name.as_ptr(), SERVICE_QUERY_STATUS);
        let mut state = std::mem::zeroed::<SERVICE_STATUS>();
        let running = service != 0
            && QueryServiceStatus(service, &mut state) != 0
            && state.dwCurrentState == SERVICE_RUNNING;
        if service != 0 {
            CloseServiceHandle(service);
        }
        CloseServiceHandle(manager);
        running
    }
}

fn bridge_path() -> io::Result<PathBuf> {
    let packaged = std::env::current_exe()?
        .parent()
        .unwrap()
        .join("sensor-bridge")
        .join("SensorBridge.exe");
    if packaged.exists() {
        return Ok(packaged);
    }
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("sensor-bridge")
        .join("SensorBridge.exe"))
}

fn load_machine_info() -> MachineInfo {
    use std::os::windows::process::CommandExt;
    bridge_path()
        .ok()
        .and_then(|path| {
            Command::new(path)
                .arg("--info")
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .ok()
        })
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|line| {
            parse_info(line.trim_end()).map(|(cpu, gpu, memory)| MachineInfo {
                cpu: cpu.into(),
                gpu: gpu.into(),
                memory: memory.into(),
            })
        })
        .unwrap_or_default()
}

fn run_display(
    mode: Arc<AtomicU8>,
    interval: Arc<AtomicU8>,
    detail: Arc<AtomicU8>,
    running: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    info: Arc<Mutex<MachineInfo>>,
) {
    while running.load(Ordering::Relaxed) {
        if official_service_running() {
            set_status(
                &status,
                "Stop the official DeepCool service to use the screen",
            );
            thread::sleep(Duration::from_secs(2));
            continue;
        }
        if let Err(error) = display_session(&mode, &interval, &detail, &running, &status, &info) {
            set_status(&status, format!("Display unavailable: {error}"));
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn display_session(
    mode: &AtomicU8,
    interval: &AtomicU8,
    detail: &AtomicU8,
    running: &AtomicBool,
    status: &Mutex<String>,
    info: &Mutex<MachineInfo>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::windows::process::CommandExt;
    let device = HidApi::new()?.open(VID, PID)?;
    let mut child = Command::new(bridge_path()?)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .spawn()?;
    let reader = BufReader::new(child.stdout.take().unwrap());
    let mut mix_start = Instant::now();
    let mut last_settings = (
        Mode::from_index(mode.load(Ordering::Relaxed) as usize),
        interval.load(Ordering::Relaxed),
        CpuDetail::from_index(detail.load(Ordering::Relaxed) as usize),
    );
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for line in reader.lines() {
            if !running.load(Ordering::Relaxed) || official_service_running() {
                break;
            }
            let line = line?;
            if let Some((cpu, gpu, memory)) = parse_info(&line) {
                if let Ok(mut info) = info.lock() {
                    *info = MachineInfo {
                        cpu: cpu.into(),
                        gpu: gpu.into(),
                        memory: memory.into(),
                    };
                }
                continue;
            }
            let values = parse_values(&line)?;
            let selected = Mode::from_index(mode.load(Ordering::Relaxed) as usize);
            let seconds = interval.load(Ordering::Relaxed);
            let selected_detail = CpuDetail::from_index(detail.load(Ordering::Relaxed) as usize);
            if (selected, seconds, selected_detail) != last_settings {
                mix_start = Instant::now();
                last_settings = (selected, seconds, selected_detail);
            }
            let elapsed = mix_start.elapsed().as_secs();
            let page = page_for(selected, elapsed, seconds);
            let cpu_detail = detail_for(selected, selected_detail, elapsed, seconds);
            if page == Mode::Gpu && !gpu_available(&values) {
                set_status(
                    status,
                    "GPU sensor unavailable (temperature, usage or clock)",
                );
                continue;
            }
            if page == Mode::Cpu
                && (values[..3].iter().any(Option::is_none)
                    || (cpu_detail == CpuDetail::Fan && values[3].is_none())
                    || (cpu_detail == CpuDetail::Clock && values[8].is_none()))
            {
                set_status(status, format!("CPU sensor unavailable ({cpu_detail:?})"));
                continue;
            }
            device.write(&display_packet(&values, page, cpu_detail))?;
            if selected != Mode::Cpu && values[5].is_none() {
                set_status(
                    status,
                    format!("{selected:?} · {page:?} page · GPU power unavailable (0 W shown)"),
                );
            } else {
                set_status(
                    status,
                    format!("Connected · {selected:?} · showing {page:?}"),
                );
            }
        }
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result?;
    Err("sensor bridge stopped".into())
}

type Sensors = [Option<f32>; 9];
fn gpu_available(values: &Sensors) -> bool {
    [values[4], values[6], values[7]]
        .iter()
        .all(Option::is_some)
}
fn page_for(selected: Mode, elapsed_secs: u64, interval: u8) -> Mode {
    match selected {
        Mode::Mix if (elapsed_secs / interval as u64) % 2 == 1 => Mode::Gpu,
        Mode::Mix => Mode::Cpu,
        other => other,
    }
}
fn detail_for(mode: Mode, selected: CpuDetail, elapsed_secs: u64, interval: u8) -> CpuDetail {
    if selected != CpuDetail::Auto {
        return selected;
    }
    let slot = elapsed_secs
        / if mode == Mode::Mix {
            interval as u64
        } else {
            5
        };
    let cpu_turn = if mode == Mode::Mix { slot / 2 } else { slot };
    if cpu_turn % 2 == 0 {
        CpuDetail::Fan
    } else {
        CpuDetail::Clock
    }
}
fn parse_info(line: &str) -> Option<(&str, &str, &str)> {
    let mut parts = line.strip_prefix("INFO\t")?.split('\t');
    let fields = (parts.next()?, parts.next()?, parts.next()?);
    parts.next().is_none().then_some(fields)
}
fn parse_values(line: &str) -> Result<Sensors, &'static str> {
    let values: Vec<Option<f32>> = line
        .trim()
        .split(',')
        .map(|value| value.parse::<f32>().ok())
        .collect();
    values.try_into().map_err(|_| "expected nine sensor values")
}

fn display_packet(values: &Sensors, page: Mode, cpu_detail: CpuDetail) -> [u8; 64] {
    let mut packet = [0u8; 64];
    packet[..6].copy_from_slice(&[16, 104, 1, 6, 35, 1]);
    if page == Mode::Gpu {
        packet[6] = 4;
        packet[19..21].copy_from_slice(&word(values[5]).to_be_bytes());
        packet[21..25].copy_from_slice(&values[4].unwrap_or(0.0).to_be_bytes());
        packet[25] = percent(values[6]);
        packet[26..28].copy_from_slice(&word(values[7]).to_be_bytes());
    } else {
        packet[6] = if cpu_detail == CpuDetail::Clock { 2 } else { 3 };
        packet[7..9].copy_from_slice(&word(values[1]).to_be_bytes());
        packet[10..14].copy_from_slice(&values[0].unwrap_or(0.0).to_be_bytes());
        packet[14] = percent(values[2]);
        if cpu_detail == CpuDetail::Clock {
            packet[15..17].copy_from_slice(&word(values[8]).to_be_bytes());
        } else {
            packet[17..19].copy_from_slice(&word(values[3]).to_be_bytes());
        }
    }
    packet[40] = packet[1..=39]
        .iter()
        .map(|&value| value as u16)
        .sum::<u16>() as u8;
    packet[41] = 22;
    packet
}
fn word(value: Option<f32>) -> u16 {
    value.unwrap_or(0.0).round().clamp(0.0, u16::MAX as f32) as u16
}
fn percent(value: Option<f32>) -> u8 {
    value.unwrap_or(0.0).round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_mutex_allows_only_one_instance() {
        let name = format!("Local\\DeepCoolDigitalLiteTest-{}", std::process::id());
        let first = SingleInstance::acquire(&name).unwrap();
        assert!(first.is_some());
        assert!(SingleInstance::acquire(&name).unwrap().is_none());
    }

    #[test]
    fn packets_cover_cpu_and_gpu() {
        let values = parse_values("51,34.9,2.8,836,37,150,10,1845,5200").unwrap();
        let cpu = display_packet(&values, Mode::Cpu, CpuDetail::Fan);
        let clock = display_packet(&values, Mode::Cpu, CpuDetail::Clock);
        let gpu = display_packet(&values, Mode::Gpu, CpuDetail::Fan);
        assert_eq!(
            (cpu[6], cpu[14], u16::from_be_bytes([cpu[17], cpu[18]])),
            (3, 3, 836)
        );
        assert_eq!(
            (clock[6], u16::from_be_bytes([clock[15], clock[16]])),
            (2, 5200)
        );
        assert_eq!(
            (gpu[6], gpu[25], u16::from_be_bytes([gpu[26], gpu[27]])),
            (4, 10, 1845)
        );
        assert_eq!(
            gpu[40],
            gpu[1..=39].iter().map(|&v| v as u16).sum::<u16>() as u8
        );
    }

    #[test]
    fn machine_info_header_is_not_a_sensor_sample() {
        assert_eq!(
            parse_info("INFO\tIntel Core i7\tNVIDIA RTX\t32 GB"),
            Some(("Intel Core i7", "NVIDIA RTX", "32 GB"))
        );
        assert_eq!(parse_info("51,34.9,2.8,836,37,150,10,1845"), None);
    }

    #[test]
    fn mix_and_cpu_detail_follow_selected_interval() {
        for interval in [5, 7] {
            let interval = interval as u64;
            assert_eq!(page_for(Mode::Mix, interval - 1, interval as u8), Mode::Cpu);
            assert_eq!(page_for(Mode::Mix, interval, interval as u8), Mode::Gpu);
            assert_eq!(page_for(Mode::Mix, interval * 2, interval as u8), Mode::Cpu);
            assert_eq!(
                detail_for(Mode::Mix, CpuDetail::Auto, interval * 2, interval as u8),
                CpuDetail::Clock
            );
            assert_eq!(
                detail_for(Mode::Mix, CpuDetail::Auto, interval * 4, interval as u8),
                CpuDetail::Fan
            );
        }
        assert_eq!(detail_for(Mode::Cpu, CpuDetail::Auto, 4, 7), CpuDetail::Fan);
        assert_eq!(
            detail_for(Mode::Cpu, CpuDetail::Auto, 5, 7),
            CpuDetail::Clock
        );
    }

    #[test]
    fn gpu_page_uses_zero_only_when_power_is_unavailable() {
        let values = parse_values("51,34.9,2.8,836,35.1,missing,8,1815,5200").unwrap();
        assert!(gpu_available(&values));
        let packet = display_packet(&values, Mode::Gpu, CpuDetail::Fan);
        assert_eq!(packet[6], 4);
        assert_eq!(u16::from_be_bytes([packet[19], packet[20]]), 0);
        assert_eq!(f32::from_be_bytes(packet[21..25].try_into().unwrap()), 35.1);
        assert_eq!(packet[25], 8);
        assert_eq!(u16::from_be_bytes([packet[26], packet[27]]), 1815);
    }
}
