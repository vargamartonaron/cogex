use anyhow::Result;
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use std::time::Instant;
use tiny_skia::Pixmap;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Fullscreen, Window, WindowId},
};
mod experiment;
mod renderer;
mod timer;
use experiment::{ExperimentPhase, ExperimentState};
use renderer::ExperimentRenderer;

pub struct CognitiveExperiment {
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    experiment_state: ExperimentState,
    renderer: Option<ExperimentRenderer>,
    current_size: Option<winit::dpi::PhysicalSize<u32>>,
    scale_factor: f64,
    refresh_rate: Option<f64>,
}

impl Default for CognitiveExperiment {
    fn default() -> Self {
        Self {
            window: None,
            pixels: None,
            experiment_state: ExperimentState::new(),
            renderer: None,
            current_size: None,
            scale_factor: 1.0,
            refresh_rate: None,
        }
    }
}

impl ApplicationHandler for CognitiveExperiment {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let primary_monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .expect("No monitor available");

        self.refresh_rate = primary_monitor
            .refresh_rate_millihertz()
            .map(|rate| rate as f64 / 1000.0);

        let window_attributes = Window::default_attributes()
            .with_title("Cognitive Experiment")
            .with_fullscreen(Some(Fullscreen::Borderless(Some(primary_monitor.clone()))))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

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

        let window_ref: &'static Window = Box::leak(Box::new(Arc::clone(&window)));
        let surface_texture =
            SurfaceTexture::new(physical_size.width, physical_size.height, window_ref);

        self.pixels = Some(
            Pixels::new(physical_size.width, physical_size.height, surface_texture)
                .expect("Failed to create pixel buffer"),
        );

        self.renderer = Some(ExperimentRenderer::new(
            physical_size.width,
            physical_size.height,
        ));
        self.window = Some(window);

        println!("Cognitive Experiment Started - Beginning Calibration Phase");

        if let Some(window) = &self.window {
            window.set_cursor_visible(false);
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.cleanup_and_exit(event_loop);
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
                self.update_experiment();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    self.handle_input(event.physical_key, event_loop);
                }
            }
            WindowEvent::Resized(new_size) => {
                self.handle_resize(new_size);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor;
                if let Some(window) = &self.window {
                    self.handle_resize(window.inner_size());
                }
            }
            _ => {}
        }
    }
}

impl CognitiveExperiment {
    fn render(&mut self) -> Result<()> {
        let Some(current_size) = self.current_size else {
            return Ok(());
        };
        if let (Some(pixels), Some(renderer)) = (&mut self.pixels, &mut self.renderer) {
            let start_time = Instant::now();

            let mut pixmap = Pixmap::new(current_size.width, current_size.height)
                .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap"))?;

            renderer.render_frame(&mut pixmap, &self.experiment_state)?;

            let frame = pixels.frame_mut();
            frame.copy_from_slice(pixmap.data());

            pixels.render()?;

            // Record frame timing in ExperimentState's HighPrecisionTimer directly
            let elapsed = start_time.elapsed();
            self.experiment_state.timer.record_frame_time(elapsed);
        }
        Ok(())
    }

    fn update_experiment(&mut self) {
        // Handle phase-specific logic
        match self.experiment_state.phase {
            ExperimentPhase::Welcome => {
                // Wait for user to press Space to start calibration
            }
            ExperimentPhase::Calibration => {
                // If enough frames collected, complete calibration
                if self.experiment_state.timer.frame_times.len() >= 300
                    && !self.experiment_state.calibrated
                {
                    self.experiment_state.apply_calibration();
                    self.experiment_state.advance_practice();
                    self.experiment_state.calibrated = true;
                }
                // Calibration updates (if any) can be placed here
            }
            ExperimentPhase::Practice => {
                self.experiment_state.update_trial();
                if self.experiment_state.practice_done() {
                    self.experiment_state.advance_experiment();
                }
            }
            ExperimentPhase::Experiment => {
                self.experiment_state.update_trial();
                if self.experiment_state.experiment_done() {
                    self.experiment_state.advance_debrief();
                }
            }
            ExperimentPhase::Debrief => {
                // Debrief phase logic/stalling here
            }
        }
    }

    fn handle_input(&mut self, key: winit::keyboard::PhysicalKey, event_loop: &ActiveEventLoop) {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if let PhysicalKey::Code(keycode) = key {
            match keycode {
                KeyCode::Space => {
                    match self.experiment_state.phase {
                        ExperimentPhase::Welcome => {
                            self.experiment_state.advance_calibration();
                        }
                        ExperimentPhase::Calibration => {
                            // optionally allow skipping calibration
                            if !self.experiment_state.calibrated {
                                self.experiment_state.apply_calibration();
                                self.experiment_state.advance_practice();
                                self.experiment_state.calibrated = true;
                            }
                        }
                        ExperimentPhase::Practice | ExperimentPhase::Experiment => {
                            self.experiment_state.record_response();
                        }
                        ExperimentPhase::Debrief => {
                            self.cleanup_and_exit(event_loop);
                        }
                    }
                }
                KeyCode::Escape => {
                    self.cleanup_and_exit(event_loop);
                }
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
        if let Some(renderer) = &mut self.renderer {
            *renderer = ExperimentRenderer::new(new_size.width, new_size.height);
        }
        println!("Display resized to: {}×{}", new_size.width, new_size.height);
    }

    fn cleanup_and_exit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.set_cursor_visible(true);
        }

        println!("\nExperiment completed.");
        println!(
            "Calibration data samples: {}",
            self.experiment_state.timer.frame_times.len()
        );
        println!("Results saved. Thank you!");

        event_loop.exit();
    }
}

fn main() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        unsafe {
            winapi::um::timeapi::timeBeginPeriod(1);
        }
    }

    let event_loop = EventLoop::new()?;
    let mut app = CognitiveExperiment::default();

    println!("=== COGNITIVE EXPERIMENT APPLICATION ===");
    println!("Platform: {}", std::env::consts::OS);
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Press SPACE to start calibration or ESC to exit.\n");

    event_loop.run_app(&mut app)?;

    #[cfg(target_os = "windows")]
    {
        unsafe {
            winapi::um::timeapi::timeEndPeriod(1);
        }
    }

    Ok(())
}
