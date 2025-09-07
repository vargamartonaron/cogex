use std::ptr;
use std::time::{Duration, Instant};

/// Platform-specific high-precision timer implementation
/// Provides sub-millisecond precision timing for cognitive experiments
#[derive(Debug, Clone)]
pub struct HighPrecisionTimer {
    start_time: Instant,
    pub frame_times: Vec<Duration>,
    max_samples: usize,
}

pub struct TimingInfo {
    pub average_frame_time: f64, // nanoseconds
    pub jitter: f64,             // standard deviation in nanoseconds
    pub min_frame_time: f64,     // nanoseconds
    pub max_frame_time: f64,     // nanoseconds
}

impl HighPrecisionTimer {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            frame_times: Vec::with_capacity(1000),
            max_samples: 1000,
        }
    }

    /// Returns timestamp in nanoseconds from timer creation
    pub fn get_timestamp(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    /// Records frame duration to the frame_times buffer
    pub fn record_frame_time(&mut self, duration: Duration) {
        if self.frame_times.len() >= self.max_samples {
            self.frame_times.remove(0);
        }
        self.frame_times.push(duration);
    }

    /// Returns statistics computed from recorded frame durations
    pub fn get_info(&self) -> TimingInfo {
        if self.frame_times.is_empty() {
            return TimingInfo {
                average_frame_time: 0.0,
                jitter: 0.0,
                min_frame_time: 0.0,
                max_frame_time: 0.0,
            };
        }
        let times_ns: Vec<f64> = self
            .frame_times
            .iter()
            .map(|d| d.as_nanos() as f64)
            .collect();

        let avg = times_ns.iter().sum::<f64>() / times_ns.len() as f64;
        let variance =
            times_ns.iter().map(|x| (*x - avg).powi(2)).sum::<f64>() / times_ns.len() as f64;
        let stddev = variance.sqrt();
        let min_val = *times_ns
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let max_val = *times_ns
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        TimingInfo {
            average_frame_time: avg,
            jitter: stddev,
            min_frame_time: min_val,
            max_frame_time: max_val,
        }
    }

    /// High precision sleep (platform specific)
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
            clock_nanosleep(CLOCK_MONOTONIC, 0, &req, ptr::null_mut());
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
