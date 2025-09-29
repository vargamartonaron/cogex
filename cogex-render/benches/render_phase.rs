use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use cogex_render::{PhaseRenderer as _, Renderer as _, SkiaRenderer};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

// Minimal mock implementing only what render_phase reads
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
        unimplemented!()
    }
    fn allows_input(&self) -> bool {
        false
    }
}

fn harness() -> (SkiaRenderer, MockPhase) {
    let mut r = SkiaRenderer::new(1280, 720, 40);
    // Warm up text cache by blitting a known progress id once through render_phase
    let phase = MockPhase {
        welcome: false,
        calib: false,
        practice: true,
        experiment: false,
        allows_input: false,
    };
    let stim = Some((
        &StimulusType::Circle {
            radius: 50.0,
            color: [255, 0, 0, 255],
        },
        (640.0, 360.0),
    ));
    let _ = r.render_phase(&phase, stim, Some(&TrialState::Stimulus), Some((10, 40)));
    (r, phase)
}

pub fn bench_phase_fixation(c: &mut Criterion) {
    let mut g = c.benchmark_group("render_phase");
    g.sample_size(60);

    g.bench_function("fixation", |b| {
        b.iter_batched(
            || harness(),
            |(mut r, p)| {
                let _ = r.render_phase(
                    black_box(&p),
                    None,
                    Some(&TrialState::Fixation),
                    Some((10, 40)),
                );
            },
            BatchSize::SmallInput,
        )
    });

    g.bench_function("stimulus_only", |b| {
        b.iter_batched(
            || harness(),
            |(mut r, p)| {
                let s = StimulusType::Rectangle {
                    width: 80.0,
                    height: 60.0,
                    color: [0, 255, 0, 255],
                };
                let _ = r.render_phase(
                    black_box(&p),
                    Some((&s, (540.0, 360.0))),
                    Some(&TrialState::Stimulus),
                    Some((10, 40)),
                );
            },
            BatchSize::SmallInput,
        )
    });

    g.bench_function("response_overlay", |b| {
        b.iter_batched(
            || harness(),
            |(mut r, p)| {
                let s = StimulusType::Arrow {
                    direction: ArrowDirection::Right,
                    size: 60.0,
                    color: [0, 0, 255, 255],
                };
                let _ = r.render_phase(
                    black_box(&p),
                    Some((&s, (740.0, 360.0))),
                    Some(&TrialState::Response),
                    Some((10, 40)),
                );
            },
            BatchSize::SmallInput,
        )
    });

    g.finish();
}

criterion_group!(benches, bench_phase_fixation);
criterion_main!(benches);
