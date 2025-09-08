use std::time::{Duration, Instant};

/// Trait for high-precision timers
pub trait Timer: Clone + Send + Sync {
    type Timestamp: Copy + Clone + Send + Sync;
    fn now(&self) -> Self::Timestamp;
    fn elapsed(&self, ts: Self::Timestamp) -> Duration;
    fn sleep(&self, d: Duration);
    fn record_frame(&mut self, d: Duration);
    fn calibration_stats(&self) -> CalibrationStats;
}

#[derive(Debug, Clone)]
pub struct CalibrationStats {
    pub average_frame_time_ns: f64,
    pub jitter_ns: f64,
    pub min_frame_time_ns: f64,
    pub max_frame_time_ns: f64,
    pub effective_fps: f64,
}

#[derive(Debug, Clone)]
pub struct HighPrecisionTimer {
    pub start: Instant,
    pub frame_times: Vec<Duration>,
    pub max_samples: usize,
}

impl Timer for HighPrecisionTimer {
    type Timestamp = u64;
    fn now(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }
    fn elapsed(&self, ts: u64) -> Duration {
        Duration::from_nanos(self.now().saturating_sub(ts))
    }
    fn sleep(&self, d: Duration) {
        self.high_precision_sleep(d)
    }
    fn record_frame(&mut self, d: Duration) {
        if self.frame_times.len() >= self.max_samples {
            self.frame_times.remove(0);
        }
        self.frame_times.push(d);
    }
    fn calibration_stats(&self) -> CalibrationStats {
        let times: Vec<f64> = self
            .frame_times
            .iter()
            .map(|d| d.as_nanos() as f64)
            .collect();
        if times.is_empty() {
            return CalibrationStats {
                average_frame_time_ns: 0.0,
                jitter_ns: 0.0,
                min_frame_time_ns: 0.0,
                max_frame_time_ns: 0.0,
                effective_fps: 0.0,
            };
        }
        let sum: f64 = times.iter().sum();
        let avg = sum / times.len() as f64;
        let var = times.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / times.len() as f64;
        let jitter = var.sqrt();
        let min = *times
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let max = *times
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        CalibrationStats {
            average_frame_time_ns: avg,
            jitter_ns: jitter,
            min_frame_time_ns: min,
            max_frame_time_ns: max,
            effective_fps: if avg > 0.0 { 1e9 / avg } else { 0.0 },
        }
    }
}

impl HighPrecisionTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            frame_times: Vec::with_capacity(1000),
            max_samples: 1000,
        }
    }
    pub fn high_precision_sleep(&self, duration: Duration) {
        #[cfg(target_os = "windows")]
        self.windows_sleep(duration);
        #[cfg(target_os = "linux")]
        self.linux_sleep(duration);
        #[cfg(target_os = "macos")]
        self.macos_sleep(duration);
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        std::thread::sleep(duration);
    }
    // ... (include your platform-specific implementations exactly)
    #[cfg(target_os = "windows")]
    fn windows_sleep(&self, duration: Duration) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{
            CreateWaitableTimerW, SetWaitableTimer, WaitForSingleObject,
        };

        unsafe {
            let timer = CreateWaitableTimerW(None, true, None).unwrap();

            let intervals = -(duration.as_nanos() as i64 / 100);

            let mut due_time = FILETIME {
                dwLowDateTime: intervals as u32,
                dwHighDateTime: (intervals >> 32) as u32,
            };

            if SetWaitableTimer(timer, &due_time, 0, None, None, false).as_bool() {
                WaitForSingleObject(timer, u32::MAX);
            }

            CloseHandle(timer);
        }
    }

    #[cfg(target_os = "linux")]
    fn linux_sleep(&self, duration: Duration) {
        use libc::{clock_nanosleep, timespec, CLOCK_MONOTONIC};

        let req = timespec {
            tv_sec: duration.as_secs() as libc::time_t,
            tv_nsec: duration.subsec_nanos() as libc::c_long,
        };

        unsafe {
            clock_nanosleep(CLOCK_MONOTONIC, 0, &req, std::ptr::null_mut());
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_sleep(&self, duration: Duration) {
        use mach::mach_time::{mach_absolute_time, mach_timebase_info, mach_timebase_info_data_t};
        use std::thread;

        if duration.as_nanos() < 100_000 {
            unsafe {
                let start = mach_absolute_time();
                let mut timebase = mach_timebase_info_data_t { numer: 0, denom: 0 };
                mach_timebase_info(&mut timebase);

                let target_ticks =
                    duration.as_nanos() as u64 * timebase.denom as u64 / timebase.numer as u64;

                while mach_absolute_time() - start < target_ticks {
                    std::hint::spin_loop();
                }
            }
        } else {
            thread::sleep(duration);
        }
    }
}

impl Default for HighPrecisionTimer {
    fn default() -> Self {
        Self::new()
    }
}
