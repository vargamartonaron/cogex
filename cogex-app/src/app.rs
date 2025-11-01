use anyhow::Result;
use cogex_core::{Phase, StandardPhase, StimulusType};
use cogex_experiment::{ExperimentConfig, ExperimentEvent, ExperimentStateMachine};
use cogex_render::{render::FrameStats, SkiaRenderer};
use cogex_timing::{HighPrecisionTimer, Timer};
use pixels::{Pixels, SurfaceTexture};
use rand::rngs::ThreadRng;
use std::sync::Arc;
use tiny_skia::Pixmap;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Fullscreen, Icon, Window, WindowId},
};

pub struct App {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    experiment: ExperimentStateMachine<StandardPhase, StimulusType, HighPrecisionTimer, ThreadRng>,
    renderer: Option<SkiaRenderer>,
    canvas: Option<Pixmap>,
    icon: Icon,
    current_size: Option<PhysicalSize<u32>>,
    scale_factor: f64,
    refresh_rate: Option<f64>,

    should_exit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = ExperimentConfig::<StandardPhase>::default();
        let timer = HighPrecisionTimer::new();
        let rng = rand::rng();
        let experiment = ExperimentStateMachine::new(config, timer, rng);
        let icon = Self::load_icon(include_bytes!("../../assets/icon.png"));

        Ok(Self {
            window: None,
            pixels: None,
            experiment,
            renderer: None,
            canvas: None,
            icon: icon,
            current_size: None,
            scale_factor: 1.0,
            refresh_rate: None,
            should_exit: false,
        })
    }

    pub fn run(mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                winapi::um::timeapi::timeBeginPeriod(1);
            }
        }

        let event_loop = EventLoop::new()?;
        println!("=== COGNITIVE EXPERIMENT APPLICATION ===");
        println!("Platform: {}", std::env::consts::OS);
        println!("Architecture: {}", std::env::consts::ARCH);
        println!("Press SPACE to start or ESC to exit.\n");

        let result = event_loop.run_app(&mut self);

        #[cfg(target_os = "windows")]
        {
            unsafe {
                winapi::um::timeapi::timeEndPeriod(1);
            }
        }

        result.map_err(Into::into)
    }

    fn create_window_and_surface(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let primary_monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .ok_or_else(|| anyhow::anyhow!("No monitor available"))?;

        self.refresh_rate = primary_monitor
            .refresh_rate_millihertz()
            .map(|rate| rate as f64 / 1000.0);

        let window_attributes = Window::default_attributes()
            .with_title("Cogex")
            .with_fullscreen(Some(Fullscreen::Borderless(Some(primary_monitor.clone()))))
            .with_resizable(false)
            .with_window_icon(Some(self.icon.clone()));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let physical_size = window.inner_size();
        let scale_factor = window.scale_factor();

        self.current_size = Some(physical_size);
        self.scale_factor = scale_factor;

        println!("Display Configuration:");
        println!(
            "  Physical size: {}×{}",
            physical_size.width, physical_size.height
        );
        println!("  Scale factor: {:.2}", scale_factor);
        if let Some(refresh_rate) = self.refresh_rate {
            println!("  Refresh rate: {:.1} Hz", refresh_rate);
        }

        let surface_texture =
            SurfaceTexture::new(physical_size.width, physical_size.height, window.clone());

        self.pixels = Some(Pixels::new(
            physical_size.width,
            physical_size.height,
            surface_texture,
        )?);

        self.canvas = Pixmap::new(physical_size.width, physical_size.height);
        self.renderer = Some(SkiaRenderer::new(
            physical_size.width,
            physical_size.height,
            self.experiment.config.experiment_trials,
        ));

        window.set_cursor_visible(false);
        window.request_redraw();

        self.window = Some(window);

        Ok(())
    }

    fn render(&mut self) -> anyhow::Result<()> {
        let pix = self.pixels.as_mut().unwrap();
        let renderer = self.renderer.as_mut().unwrap();

        let phase = self.experiment.current_phase();
        let stim = self.experiment.current_stimulus();
        let ts = self.experiment.current_trial_state();
        let prog = self.experiment.trial_progress();
        let mut timer = self.experiment.timer.clone();

        let frame = pix.frame_mut();

        let stats: FrameStats = renderer.render_frame(phase, stim, ts, prog, frame, &mut timer)?;
        let now = timer.now();
        pix.render()?;
        let elapsed = timer.elapsed(now);

        // if self.experiment.phase.requires_calibration() && self.experiment.timer.frame_count() < 300
        {
            self.window.as_ref().unwrap().request_redraw(); // schedule next VSync‐driven render
        }

        println!(
            "outer: {:.3}ms,clear {:.3}ms, phase {:.3}ms, copy {:.3}ms, total {:.3}ms, dirty count {:.3}",
            elapsed.as_secs_f64() * 1e3,
            stats.clear.as_secs_f64() * 1e3,
            stats.phase.as_secs_f64() * 1e3,
            stats.copy.as_secs_f64() * 1e3,
            stats.total.as_secs_f64() * 1e3,
            stats.dirty_count,
        );

        self.experiment
            .handle_event(ExperimentEvent::CalibrationComplete);

        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        let events = self.experiment.update();
        for event in events {
            self.experiment.handle_event(event);
        }
        Ok(())
    }

    fn handle_input(&mut self, key: winit::keyboard::PhysicalKey, event_loop: &ActiveEventLoop) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if let PhysicalKey::Code(k) = key {
            match k {
                KeyCode::Space => {
                    if self.experiment.current_phase().is_welcome() {
                        self.experiment.handle_event(ExperimentEvent::SpacePressed);
                    }
                    if self.experiment.current_phase().allows_input() {
                        self.experiment
                            .handle_event(ExperimentEvent::ResponseReceived);
                    }
                }
                KeyCode::Escape => self.cleanup_and_exit(event_loop),
                _ => {}
            }
        }
    }

    fn handle_resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.current_size = Some(new_size);
        if let Some(pixels) = &mut self.pixels {
            if let Err(e) = pixels.resize_surface(new_size.width, new_size.height) {
                eprintln!("Failed to resize surface: {}", e);
            }
            if let Err(e) = pixels.resize_buffer(new_size.width, new_size.height) {
                eprintln!("Failed to resize buffer: {}", e);
            }
        }
        self.renderer
            .as_mut()
            .unwrap()
            .resize(new_size.width, new_size.height);
        println!("Display resized to: {}×{}", new_size.width, new_size.height);
    }
    fn cleanup_and_exit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.set_cursor_visible(true);
        }

        println!("\nExperiment completed.");
        println!("Results saved. Thank you!");

        self.should_exit = true;
        event_loop.exit();
    }

    fn load_icon(bytes: &[u8]) -> Icon {
        let (icon_rgba, icon_width, icon_height) = {
            let image = image::load_from_memory(bytes).unwrap().into_rgba8();
            let (width, height) = image.dimensions();
            let rgba = image.into_raw();
            (rgba, width, height)
        };
        Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().expect("Failed to create application")
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            if let Err(e) = self.create_window_and_surface(event_loop) {
                eprintln!("Failed to create window and surface: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.cleanup_and_exit(event_loop),
            WindowEvent::RedrawRequested => {
                self.render().unwrap();
                self.update().unwrap();
                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                self.handle_input(event.physical_key, event_loop);
            }
            WindowEvent::Resized(sz) => self.handle_resize(sz),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor;
                if let Some(window) = &self.window {
                    self.handle_resize(window.inner_size());
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.should_exit {
            event_loop.exit();
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        println!("Application resources cleaned up");
    }
}
