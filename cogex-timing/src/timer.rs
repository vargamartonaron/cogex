use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Trait for high-precision timers
pub trait Timer: Clone + Send + Sync {
    type Timestamp: Copy + Clone + Send + Sync;
    fn now(&self) -> Self::Timestamp;
    fn elapsed(&self, ts: Self::Timestamp) -> Duration;
    fn sleep(&self, d: Duration);
    fn frame_count(&self) -> u64;
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
    start: Instant,
    frame_times: VecDeque<f64>, // nanoseconds
    capacity: usize,
    // Welford's running stats
    count: usize,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl HighPrecisionTimer {
    pub fn new() -> Self {
        let capacity = 1000;
        Self {
            start: Instant::now(),
            frame_times: VecDeque::with_capacity(capacity),
            capacity,
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: f64::INFINITY,
            max: 0.0,
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

    fn frame_count(&self) -> u64 {
        self.count as u64
    }

    fn record_frame(&mut self, d: Duration) {
        let sample = d.as_nanos() as f64;

        // Evict oldest if full
        if self.frame_times.len() == self.capacity {
            self.frame_times.pop_front();
            // Reset stats and recompute for simplicity
            self.count = 0;
            self.mean = 0.0;
            self.m2 = 0.0;
            self.min = f64::INFINITY;
            self.max = 0.0;
            for &old in &self.frame_times {
                self.count += 1;
                let delta = old - self.mean;
                self.mean += delta / self.count as f64;
                let delta2 = old - self.mean;
                self.m2 += delta * delta2;
                self.min = self.min.min(old);
                self.max = self.max.max(old);
            }
        }

        // Add new sample
        self.frame_times.push_back(sample);
        self.count += 1;
        let delta = sample - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = sample - self.mean;
        self.m2 += delta * delta2;
        self.min = self.min.min(sample);
        self.max = self.max.max(sample);
    }

    fn calibration_stats(&self) -> CalibrationStats {
        let avg = if self.count > 0 { self.mean } else { 0.0 };
        let jitter = if self.count > 1 {
            (self.m2 / self.count as f64).sqrt()
        } else {
            0.0
        };
        CalibrationStats {
            average_frame_time_ns: avg,
            jitter_ns: jitter,
            min_frame_time_ns: if self.min.is_finite() { self.min } else { 0.0 },
            max_frame_time_ns: self.max,
            effective_fps: if avg > 0.0 { 1e9 / avg } else { 0.0 },
        }
    }
}
