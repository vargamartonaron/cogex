use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use cogex_render::{PhaseRenderer as _, SkiaRenderer};
use cogex_timing::HighPrecisionTimer;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MockPhase {
    practice: bool,
    experiment: bool,
}
impl Phase for MockPhase {
    fn is_welcome(&self) -> bool {
        false
    }
    fn requires_calibration(&self) -> bool {
        false
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
        unimplemented!()
    }
}

fn harness() -> (SkiaRenderer, MockPhase, Vec<u8>, HighPrecisionTimer) {
    let width = 1280u32;
    let height = 720u32;
    let r = SkiaRenderer::new(width, height, 40);
    let phase = MockPhase {
        practice: true,
        experiment: false,
    };
    let fb = vec![0u8; (width * height * 4) as usize];
    let timer = HighPrecisionTimer::new();
    (r, phase, fb, timer)
}

pub fn bench_frame_response(c: &mut Criterion) {
    let mut g = c.benchmark_group("render_frame");
    g.sample_size(40);

    g.bench_function("response_frame", |b| {
        b.iter_batched(
            || harness(),
            |(mut r, p, mut fb, mut t)| {
                let stim = StimulusType::Arrow {
                    direction: ArrowDirection::Right,
                    size: 60.0,
                    color: [0, 0, 255, 255],
                };
                let _stats = r.render_frame(
                    &p,
                    Some((&stim, (740.0, 360.0))),
                    Some(&TrialState::Response),
                    Some((10, 40)),
                    &mut fb,
                    &mut t,
                );
                black_box(_stats);
            },
            BatchSize::SmallInput,
        )
    });

    g.finish();
}

criterion_group!(benches, bench_frame_response);
criterion_main!(benches);
