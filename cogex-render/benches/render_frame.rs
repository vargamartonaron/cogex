use std::time::Duration;

use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use cogex_render::{PhaseRenderer as _, SkiaRenderer};
use cogex_timing::HighPrecisionTimer;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use pprof::criterion::{Output, PProfProfiler};

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

fn harness() -> (
    SkiaRenderer,
    MockPhase,
    Vec<u8>,
    HighPrecisionTimer,
    StimulusType,
) {
    let width = 1280u32;
    let height = 720u32;
    let r = SkiaRenderer::new(width, height, 40);
    let phase = MockPhase {
        practice: true,
        experiment: false,
    };
    let fb = vec![0u8; (width * height * 4) as usize];
    let timer = HighPrecisionTimer::new();
    let s = StimulusType::Arrow {
        direction: ArrowDirection::Left,
        size: 60.0,
        color: [0, 0, 255, 255],
    };
    (r, phase, fb, timer, s)
}

pub fn bench_frame_response(c: &mut Criterion) {
    let mut g = c.benchmark_group("render_frame");
    g.sample_size(30)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(15));

    g.bench_function("response_frame", |b| {
        let (mut r, p, mut fb, mut t, s) = harness();
        b.iter(|| {
            let _stats = r.render_frame(
                &p,
                Some((&s, (740.0, 360.0))),
                Some(&TrialState::Response),
                Some((10, 40)),
                &mut fb,
                &mut t,
            );
        })
    });

    g.finish();
}

criterion_group! {
    name=benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = bench_frame_response
}

criterion_main!(benches);
