use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use pprof::criterion::{Output, PProfProfiler};
use std::time::Duration;

use cogex_core::{ArrowDirection, StimulusType};
use cogex_render::{Renderer as _, SkiaRenderer};

/// Initialize renderer and prewarm cached assets so that subsequent blits are fast and realistic.
fn prepare_renderer(width: u32, height: u32) -> SkiaRenderer {
    let mut r = SkiaRenderer::new(width, height, 40);

    // Prewarm static cache entries used by benchmarks.
    let center = (width as f32 * 0.5, height as f32 * 0.5);
    r.blit_cached(0, center); // Welcome
    r.blit_cached(8, center); // FixationCross
    r.blit_cached(5, (200.0, 200.0)); // CircleStim
    r.blit_cached(6, (200.0, 200.0)); // RectStim
    r.blit_cached(7, (200.0, 200.0)); // ArrowStim

    r
}

/// Benchmarks the `blit_cached` function across several cache indices and positions.
pub fn bench_blit_cached(c: &mut Criterion) {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    let mut group = c.benchmark_group("blit_cached");

    // Tweak parameters for profiling and stable results.
    group
        .sample_size(50)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(2));

    // Prepare one renderer and reuse across iterations for stable memory/cache conditions.
    group.bench_function("fixation_center", |b| {
        let mut renderer = prepare_renderer(WIDTH, HEIGHT);
        let pos = (640.0, 360.0);
        b.iter(|| {
            renderer.blit_cached(8, black_box(pos));
            black_box(());
        });
    });

    group.bench_function("circle_left", |b| {
        let mut renderer = prepare_renderer(WIDTH, HEIGHT);
        let pos = (440.0, 360.0);
        b.iter(|| {
            renderer.blit_cached(5, black_box(pos));
            black_box(());
        });
    });

    group.bench_function("arrow_right", |b| {
        let mut renderer = prepare_renderer(WIDTH, HEIGHT);
        let pos = (840.0, 360.0);
        b.iter(|| {
            renderer.blit_cached(7, black_box(pos));
            black_box(());
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
        .confidence_level(0.95)
        .noise_threshold(0.02)
        .significance_level(0.05);
    targets = bench_blit_cached
}

criterion_main!(benches);
