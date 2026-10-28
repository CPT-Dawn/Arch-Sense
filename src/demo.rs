//! Demo mode: the full UI on a simulated Predator laptop.
//!
//! The laptop is a [`FakeMachine`] in a temp directory, so every control
//! writes into that tree and nothing else on the host is read or written.
//! A background thread keeps rewriting the sensor files so the gauges move.
//! The directory is deleted when the [`Demo`] is dropped.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::app::App;
use crate::fixture::FakeMachine;
use crate::keyboard::RecordingTransport;
use crate::lighting::{self, Colour, LightingBackend, Rgb};
use crate::machine::MachineRoot;
use crate::theme::ColorMode;

/// The model the demo laptop reports.
pub const MODEL: &str = "Predator PH16-71";
/// How often the sensors move.
const DRIFT_INTERVAL: Duration = Duration::from_millis(700);

/// What the sensors read at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readings {
    pub cpu_celsius: u32,
    pub gpu_celsius: u32,
    pub cpu_fan_rpm: u32,
    pub gpu_fan_rpm: u32,
    pub battery_percent: u8,
    pub battery_microwatts: u64,
}

impl Readings {
    /// The readings after `tick` drift steps: slow load swells with quicker
    /// wobble on top, fans following the temperatures, the battery sagging.
    fn at(tick: u64) -> Self {
        let t = tick as f64;
        let swell = (t / 14.0).sin();
        let cpu = 66.0 + 14.0 * swell + 3.0 * (t / 2.3).sin();
        let gpu = 58.0 + 10.0 * (t / 17.0 + 1.2).sin() + 2.0 * (t / 1.9).cos();
        let rpm = |celsius: f64, wobble: f64| (1500.0 + (celsius - 45.0) * 85.0 + wobble) as u32;
        Self {
            cpu_celsius: cpu.round() as u32,
            gpu_celsius: gpu.round() as u32,
            cpu_fan_rpm: rpm(cpu, 40.0 * (t / 3.1).sin()),
            gpu_fan_rpm: rpm(gpu, 40.0 * (t / 2.7).cos()),
            battery_percent: 78u8.saturating_sub((tick / 40) as u8).max(5),
            battery_microwatts: (24.0e6 + 9.0e6 * swell + 1.5e6 * (t / 1.7).sin()) as u64,
        }
    }
}

/// A running simulated laptop.
pub struct Demo {
    fake: FakeMachine,
    tick: u64,
}

impl Demo {
    /// Builds the laptop: Predator Sense, drifting sensors, a USB HID keyboard
    /// with a Wave already saved so the keyboard preview has something to do.
    pub fn start() -> Result<Self> {
        let r = Readings::at(0);
        let fake = FakeMachine::builder()
            .predator_sense()
            .sys_vendor("Acer")
            .product_name(MODEL)
            .turbo(false)
            .intel_cpu(r.cpu_celsius)
            .nvidia_hwmon_gpu(r.gpu_celsius)
            .acer_sensors(r.cpu_celsius, r.gpu_celsius, r.cpu_fan_rpm, r.gpu_fan_rpm)
            .battery(r.battery_percent, "Discharging", r.battery_microwatts)
            .usb_hid_keyboard()
            // A healthy install, so the Doctor examines this laptop and not the host.
            .gcc_kernel()
            .dkms_built()
            .secure_boot(false)
            .login(&["dawn"], &[1000, 966])
            .build()?;

        let backend = LightingBackend::UsbHid;
        let mut lighting = backend.default_lighting();
        lighting.effect = backend.effect("Wave").unwrap_or(lighting.effect);
        lighting.colours = vec![Colour::Rgb(Rgb::new(0, 200, 255))];
        lighting.brightness = 80;
        lighting.speed = 40;
        lighting::saved::save(&fake.machine(), &lighting)?;

        Ok(Self { fake, tick: 0 })
    }

    pub fn fake(&self) -> &FakeMachine {
        &self.fake
    }

    pub fn machine(&self) -> MachineRoot {
        self.fake.machine()
    }

    /// What the sensor files currently say.
    pub fn readings(&self) -> Readings {
        Readings::at(self.tick)
    }

    /// Moves the sensors one step by rewriting their files.
    pub fn drift(&mut self) {
        self.tick += 1;
        write_sensors(&self.machine(), &Readings::at(self.tick));
    }
}

fn write_sensors(machine: &MachineRoot, r: &Readings) {
    let hwmon = machine.hwmon_dir();
    let milli = |celsius: u32| (celsius * 1000).to_string();
    let writes = [
        ("hwmon0", "temp1_input", milli(r.cpu_celsius)),
        ("hwmon1", "temp1_input", milli(r.gpu_celsius)),
        ("hwmon2", "temp1_input", milli(r.cpu_celsius)),
        ("hwmon2", "temp2_input", milli(r.gpu_celsius)),
        ("hwmon2", "fan1_input", r.cpu_fan_rpm.to_string()),
        ("hwmon2", "fan2_input", r.gpu_fan_rpm.to_string()),
    ];
    for (chip, file, value) in writes {
        replace(&hwmon.join(chip).join(file), &value);
    }
    let bat = machine.power_supply_dir().join("BAT1");
    replace(&bat.join("capacity"), &r.battery_percent.to_string());
    replace(&bat.join("power_now"), &r.battery_microwatts.to_string());
}

/// Swaps the file's contents in one rename, so a reader never sees it half-written.
fn replace(path: &Path, contents: &str) {
    let staging = path.with_extension("tmp");
    if fs::write(&staging, contents).is_ok() {
        let _ = fs::rename(staging, path);
    }
}

/// Runs the full UI on a simulated laptop and cleans up afterwards, whether
/// the user quits, hits Ctrl-C (raw mode delivers it as a key) or the app panics.
pub fn run() -> Result<()> {
    let mut demo = Demo::start()?;
    let machine = demo.machine();
    let root: PathBuf = demo.fake().path().to_path_buf();
    let guard = Arc::new(Mutex::new(Some(root)));
    remove_on_panic(Arc::clone(&guard));

    let stop = Arc::new(AtomicBool::new(false));
    let drifter = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(DRIFT_INTERVAL);
                demo.drift();
            }
            demo
        })
    };

    let mut app = App::new(machine, RecordingTransport::default())?;
    app.set_color_mode(ColorMode::detect(|name| std::env::var(name).ok()));
    let terminal = ratatui::init();
    let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture);
    let result = app.run(terminal); // the app drops inside, flushing its last write
    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();

    stop.store(true, Ordering::Relaxed);
    let demo = drifter.join(); // dropping it deletes the tree
    *guard.lock().unwrap_or_else(PoisonError::into_inner) = None;
    drop(demo);
    result
}

/// Release builds abort on panic and skip destructors, so delete the tree in a hook.
fn remove_on_panic(root: Arc<Mutex<Option<PathBuf>>>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        if let Some(root) = root.lock().unwrap_or_else(PoisonError::into_inner).take() {
            let _ = fs::remove_dir_all(root);
        }
        previous(info);
    }));
}
