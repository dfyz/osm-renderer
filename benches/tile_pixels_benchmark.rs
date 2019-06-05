use criterion::{black_box, criterion_group, criterion_main, Criterion};
use renderer::draw::tile_pixels::TilePixels;
use renderer::mapcss::color::Color;

fn tile_pixels_benchmark_1(c: &mut Criterion) {
    c.bench_function("create TilePixels, scale = 1", |b| {
        b.iter(|| {
            TilePixels::new(
                black_box(1),
                &Some(Color {
                    r: 0xe3,
                    g: 0xe1,
                    b: 0xd2,
                }),
            )
        })
    });
}

fn tile_pixels_benchmark_2(c: &mut Criterion) {
    c.bench_function("create TilePixels, scale = 2", |b| {
        b.iter(|| {
            TilePixels::new(
                black_box(2),
                &Some(Color {
                    r: 0xe3,
                    g: 0xe1,
                    b: 0xd2,
                }),
            )
        })
    });
}

criterion_group!(tile_pixels_benches, tile_pixels_benchmark_1, tile_pixels_benchmark_2);
criterion_main!(tile_pixels_benches);
