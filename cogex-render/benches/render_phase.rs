use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use cogex_render::{PhaseRenderer as _, Renderer as _, SkiaRenderer};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use pprof::criterion::{Output, PProfProfiler};

// ---- MockPhase ----
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MockPhase {
    welcome: bool,
    calib: bool,
    practice: bool,
    experiment: bool,
    allows_input: bool,
}

impl Phase for MockPhase {
    fn is_welcome(&self) -> bool {
        self.welcome
    }
    fn requires_calibration(&self) -> bool {
        self.calib
    }
    fn is_practice(&self) -> bool {
        self.practice
    }
    fn is_experiment(&self) -> bool {
        self.experiment
    }
    fn next(&self) -> Option<Self> {
        None
    }
    fn allows_input(&self) -> bool {
        self.allows_input
    }
}

// ---- Benchmark ----
pub fn bench_render_phase(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_phase");
    group.sample_size(30);
    group.measurement_time(std::time::Duration::from_secs(15));

    // Shared harness (setup outside measured region)
    let mut renderer = SkiaRenderer::new(1280, 720, 40);
    let phase = MockPhase {
        welcome: false,
        calib: false,
        practice: true,
        experiment: false,
        allows_input: false,
    };

    // Warm-up: load text and raster caches once
    let warm = StimulusType::Circle {
        radius: 40.0,
        color: [255, 255, 255, 255],
    };
    let _ = renderer.render_phase(
        &phase,
        Some((&warm, (640.0, 360.0))),
        Some(&TrialState::Stimulus),
        Some((10, 40)),
    );

    // Benchmark 1: fixation
    group.bench_function("fixation_center", |b| {
        b.iter(|| {
            let _ =
                renderer.render_phase(&phase, None, Some(&TrialState::Fixation), Some((10, 40)));
            black_box(());
        });
    });

    // Benchmark 2: rectangle stimulus
    let rect_stim = StimulusType::Rectangle {
        width: 80.0,
        height: 60.0,
        color: [0, 255, 0, 255],
    };
    group.bench_function("stimulus_rectangle", |b| {
        b.iter(|| {
            let _ = renderer.render_phase(
                &phase,
                Some((&rect_stim, (540.0, 360.0))),
                Some(&TrialState::Stimulus),
                Some((10, 40)),
            );
            black_box(());
        });
    });

    // Benchmark 3: response arrow
    let arrow_stim = StimulusType::Arrow {
        direction: ArrowDirection::Right,
        size: 60.0,
        color: [0, 0, 255, 255],
    };
    group.bench_function("response_arrow_right", |b| {
        b.iter(|| {
            let _ = renderer.render_phase(
                &phase,
                Some((&arrow_stim, (740.0, 360.0))),
                Some(&TrialState::Response),
                Some((10, 40)),
            );
            black_box(());
        });
    });

    group.finish();
}

// ---- Criterion group ----
criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
        .measurement_time(std::time::Duration::from_secs(15));
    targets = bench_render_phase
}

criterion_main!(benches);
