//! Compare image byte operations with their pre-SIMD implementations.
//! Run with cargo run -p reader-core --release --example profile_image_bytes.
#[path = "../src/image_bytes.rs"]
mod image_bytes;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::RgbaImage;
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn compare<T: PartialEq + std::fmt::Debug>(
    name: &str,
    iterations: usize,
    before: impl Fn() -> T,
    after: impl Fn() -> T,
) {
    assert_eq!(before(), after());
    let mut old = Vec::new();
    let mut new = Vec::new();
    for round in 0..7 {
        for newer in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let start = Instant::now();
            for _ in 0..iterations {
                if newer {
                    black_box(after());
                } else {
                    black_box(before());
                }
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;
            if newer {
                new.push(ms);
            } else {
                old.push(ms);
            }
        }
    }
    old.sort_by(f64::total_cmp);
    new.sort_by(f64::total_cmp);
    println!(
        "{name}: before_ms={:.6} [{:.6},{:.6}] after_ms={:.6} [{:.6},{:.6}] ratio={:.2} iterations={iterations} rounds=7",
        old[3],
        old[0],
        old[6],
        new[3],
        new[0],
        new[6],
        old[3] / new[3]
    );
}

fn main() {
    for size in [64, 4096, 1024 * 1024] {
        let bytes: Vec<u8> = (0..size).map(|i| (i * 73 % 256) as u8).collect();
        let encoded = STANDARD.encode(&bytes);
        let iterations = (16 * 1024 * 1024 / size).min(10000);
        compare(
            &format!("encode/{size}"),
            iterations,
            || STANDARD.encode(black_box(&bytes)),
            || image_bytes::encode(black_box(&bytes)),
        );
        compare(
            &format!("decode/{size}"),
            iterations,
            || STANDARD.decode(black_box(&encoded)).ok(),
            || image_bytes::decode(black_box(&encoded)).ok(),
        );
        compare(
            &format!("scan/{size}"),
            iterations,
            || black_box(&bytes).windows(4).any(|w| w == b"acTL"),
            || image_bytes::possibly_animated_png(black_box(&bytes)),
        );
    }
    let pixels = RgbaImage::from_fn(512, 512, |x, y| {
        image::Rgba([x as u8, y as u8, (x ^ y) as u8, 255])
    });
    compare(
        "fingerprint/512x512",
        8,
        || reference_fingerprint(black_box(&pixels)),
        || image_bytes::fingerprint(black_box(&pixels)),
    );
}

fn reference_fingerprint(pixels: &RgbaImage) -> [u8; 32] {
    let mut smallest = [u8::MAX; 32];
    for (horizontal, vertical) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut hash = Sha256::new();
        hash.update(pixels.width().to_le_bytes());
        hash.update(pixels.height().to_le_bytes());
        for y in 0..pixels.height() {
            for x in 0..pixels.width() {
                let px = if horizontal {
                    pixels.width() - 1 - x
                } else {
                    x
                };
                let py = if vertical { pixels.height() - 1 - y } else { y };
                hash.update(pixels.get_pixel(px, py).0);
            }
        }
        let candidate: [u8; 32] = hash.finalize().into();
        smallest = smallest.min(candidate);
    }
    smallest
}
