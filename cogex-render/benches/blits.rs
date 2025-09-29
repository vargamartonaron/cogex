use cogex_core::{ArrowDirection, StimulusType};
use cogex_render::{Renderer as _, SkiaRenderer};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

fn harness(width: u32, height: u32) -> SkiaRenderer {
    let mut r = SkiaRenderer::new(width, height, 40);
    // Prewarm cached assets that benches will use
    r.blit_cached(0, (width as f32 * 0.5, height as f32 * 0.5)); // Welcome
    r.blit_cached(8, (width as f32 * 0.5, height as f32 * 0.5)); // FixationH
    r.blit_cached(9, (width as f32 * 0.5, height as f32 * 0.5)); // FixationV
    r.blit_cached(5, (200.0, 200.0)); // CircleStim
    r.blit_cached(6, (200.0, 200.0)); // RectStim
    r.blit_cached(7, (200.0, 200.0)); // ArrowStim
    r
}

pub fn bench_blits(c: &mut Criterion) {
    let mut g = c.benchmark_group("blit_cached");
    g.sample_size(100);

    g.bench_function("fixation_center", |b| {
        b.iter_batched(
            || harness(1280, 720),
            |mut r| {
                let center = (640.0, 360.0);
                r.blit_cached(8, black_box(center));
                r.blit_cached(9, black_box(center));
            },
            BatchSize::SmallInput,
        )
    });

    g.bench_function("circle_left", |b| {
        b.iter_batched(
            || harness(1280, 720),
            |mut r| {
                let pos = (440.0, 360.0);
                r.blit_cached(5, black_box(pos));
            },
            BatchSize::SmallInput,
        )
    });

    g.bench_function("arrow_right", |b| {
        b.iter_batched(
            || harness(1280, 720),
            |mut r| {
                let pos = (840.0, 360.0);
                r.blit_cached(7, black_box(pos));
            },
            BatchSize::SmallInput,
        )
    });

    g.finish();
}

criterion_group!(benches, bench_blits);
criterion_main!(benches);
