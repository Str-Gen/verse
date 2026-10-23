//! CPU-dispatched operations on image bytes. Dependencies select supported SIMD
//! instructions at runtime and retain portable fallbacks.
use image::RgbaImage;
use sha2::{Digest, Sha256};

pub(crate) fn encode(bytes: &[u8]) -> String {
    base64_simd::STANDARD.encode_to_string(bytes)
}

pub(crate) fn decode(encoded: &str) -> Result<Vec<u8>, base64_simd::Error> {
    base64_simd::STANDARD.decode_to_vec(encoded)
}

pub(crate) fn possibly_animated_png(bytes: &[u8]) -> bool {
    memchr::memmem::find(bytes, b"acTL").is_some()
}

pub(crate) fn fingerprint(pixels: &RgbaImage) -> [u8; 32] {
    let mut smallest = [u8::MAX; 32];
    for (horizontal, vertical) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut hash = Sha256::new();
        hash.update(pixels.width().to_le_bytes());
        hash.update(pixels.height().to_le_bytes());
        if pixels.width() != 0 && pixels.height() != 0 {
            let row_bytes = pixels.width() as usize * 4;
            let mut reversed = if horizontal {
                vec![0; row_bytes]
            } else {
                Vec::new()
            };
            for y in 0..pixels.height() {
                let py = if vertical { pixels.height() - 1 - y } else { y };
                let start = py as usize * row_bytes;
                let row = &pixels.as_raw()[start..start + row_bytes];
                if horizontal {
                    for (output, pixel) in reversed
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(row.as_chunks::<4>().0.iter().rev())
                    {
                        output.copy_from_slice(pixel);
                    }
                    hash.update(&reversed);
                } else {
                    hash.update(row);
                }
            }
        }
        let candidate: [u8; 32] = hash.finalize().into();
        smallest = smallest.min(candidate);
    }
    smallest
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    #[test]
    fn base64_matches_scalar_for_tails_alignments_and_binary_data()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes: Vec<u8> = (0..8192).map(|i| ((i * 73 + i / 13) % 256) as u8).collect();
        for offset in 0..32 {
            for length in (0..160).chain([255, 256, 257, 1023, 4096]) {
                let input = &bytes[offset..offset + length];
                let encoded = encode(input);
                assert_eq!(encoded, STANDARD.encode(input));
                assert_eq!(decode(&encoded)?, input);
            }
        }
        Ok(())
    }

    #[test]
    fn base64_rejects_noncanonical_or_corrupt_input() {
        for input in [
            "A", "AA", "AAA", "A===", "=AAA", "AA=A", "AAAA=", "AAAA\n", "AA-_", "AB==", "AAB=",
            "é==",
        ] {
            assert!(
                STANDARD.decode(input).is_err(),
                "reference accepted {input:?}"
            );
            assert!(decode(input).is_err(), "accepted {input:?}");
        }
        let encoded = encode(&[255; 256]);
        for at in 0..encoded.len() {
            let mut invalid = encoded.clone().into_bytes();
            invalid[at] = b'!';
            let input = String::from_utf8_lossy(&invalid);
            assert!(decode(&input).is_err());
        }
    }

    #[test]
    fn animation_scan_matches_scalar_at_all_lane_boundaries() {
        for length in 0..260 {
            let mut bytes = vec![0xff; length];
            assert!(!possibly_animated_png(&bytes));
            for start in 0..length.saturating_sub(3) {
                bytes[start..start + 4].copy_from_slice(b"acTL");
                assert_eq!(
                    possibly_animated_png(&bytes),
                    bytes.windows(4).any(|w| w == b"acTL")
                );
                bytes[start..start + 4].fill(0xff);
            }
        }
    }

    #[test]
    fn fingerprints_preserve_pixel_order_dimensions_and_mirrors() {
        for (width, height) in [
            (0, 0),
            (0, 3),
            (3, 0),
            (1, 1),
            (1, 17),
            (17, 1),
            (7, 13),
            (32, 33),
            (65, 67),
        ] {
            let pixels = RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([
                    x.wrapping_mul(73) as u8,
                    y.wrapping_mul(31) as u8,
                    (x ^ y) as u8,
                    x.wrapping_add(y).wrapping_mul(17) as u8,
                ])
            });
            let expected = reference_fingerprint(&pixels);
            assert_eq!(fingerprint(&pixels), expected);
            assert_eq!(
                fingerprint(&image::imageops::flip_horizontal(&pixels)),
                expected
            );
            assert_eq!(
                fingerprint(&image::imageops::flip_vertical(&pixels)),
                expected
            );
        }
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
}
