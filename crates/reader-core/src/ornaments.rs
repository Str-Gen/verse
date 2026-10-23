use crate::image_bytes;
use crate::{Error, ImageEvidence, Limits, Resource, Result, archive::Archive};
use image::{ImageFormat, ImageReader};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

/// Unknown assets remain substantive until inspected evidence establishes otherwise.
pub(crate) fn classification<'a>(evidence: &'a [ImageEvidence], resource: &str) -> &'a str {
    evidence
        .iter()
        .find(|item| item.resource == resource)
        .map_or("substantive", |item| item.classification.as_str())
}

fn raster_extension(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "tif" | "tiff"
        )
    })
}

fn media_type(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Avif => Some("image/avif"),
        ImageFormat::Bmp => Some("image/bmp"),
        ImageFormat::Tiff => Some("image/tiff"),
        _ => None,
    }
}

fn reader<'a>(
    bytes: &'a [u8],
    format: ImageFormat,
    limits: &Limits,
) -> ImageReader<Cursor<&'a [u8]>> {
    let mut decoder_limits = image::Limits::default();
    let max_dimension = u32::try_from(limits.image_pixels).unwrap_or(u32::MAX);
    decoder_limits.max_image_width = Some(max_dimension);
    decoder_limits.max_image_height = Some(max_dimension);
    decoder_limits.max_alloc = Some(limits.image_pixels.saturating_mul(16));
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(decoder_limits);
    reader
}

pub(crate) fn inspect(
    archive: &Archive,
    limits: &Limits,
    occurrences: &[(String, bool)],
) -> Result<(Vec<ImageEvidence>, Vec<Resource>)> {
    let mut resources = Vec::new();
    let evidence = inspect_to(archive, limits, occurrences, |resource| {
        resources.push(resource);
        Ok(())
    })?;
    Ok((evidence, resources))
}

pub(crate) fn inspect_to(
    archive: &Archive,
    limits: &Limits,
    occurrences: &[(String, bool)],
    mut resource_sink: impl FnMut(Resource) -> Result<()>,
) -> Result<Vec<ImageEvidence>> {
    let paths: BTreeSet<&str> = archive
        .files
        .keys()
        .filter(|p| raster_extension(p))
        .map(String::as_str)
        .chain(occurrences.iter().map(|(p, _)| p.as_str()))
        .collect();
    let mut evidence = Vec::new();
    let mut groups: BTreeMap<[u8; 32], Vec<usize>> = BTreeMap::new();
    for path in paths {
        let exact_occurrences = occurrences.iter().filter(|(p, _)| p == path).count();
        let all_decorative = exact_occurrences > 0
            && occurrences
                .iter()
                .filter(|(p, _)| p == path)
                .all(|(_, decorative)| *decorative);
        let mut item = ImageEvidence {
            resource: path.to_owned(),
            width: 0,
            height: 0,
            exact_occurrences,
            mirrored_or_identical_assets: 0,
            classification: "unsupported".into(),
            reason:
                "Missing, invalid, or unsupported raster resource; preserve a visible placeholder."
                    .into(),
        };
        let Some(bytes) = archive.read(path)? else {
            evidence.push(item);
            continue;
        };
        let Ok(format) = image::guess_format(&bytes) else {
            evidence.push(item);
            continue;
        };
        let Some(media_type) = media_type(format) else {
            evidence.push(item);
            continue;
        };
        let dimensions = reader(&bytes, format, limits).into_dimensions();
        let (width, height) = match dimensions {
            Ok(dimensions) => dimensions,
            Err(image::ImageError::Limits(_)) => {
                return Err(Error::Limit("image decoder dimensions"));
            }
            Err(_) => {
                evidence.push(item);
                continue;
            }
        };
        if u64::from(width) * u64::from(height) > limits.image_pixels {
            return Err(Error::ImageTooLarge {
                resource: path.into(),
                width,
                height,
                max_pixels: limits.image_pixels,
            });
        }
        let decoded = match reader(&bytes, format, limits).decode() {
            Ok(image) => image.into_rgba8(),
            Err(image::ImageError::Limits(_)) => {
                return Err(Error::Limit("image decoder allocation"));
            }
            Err(_) => {
                evidence.push(item);
                continue;
            }
        };
        item.width = width;
        item.height = height;
        let small = (width <= 512 && height <= 128) || (width <= 128 && height <= 512);
        // Animation evidence covers only the first frame. A conservative PNG chunk-name
        // check can retain a static image unnecessarily, but cannot justify skipping one.
        let possibly_animated = matches!(format, ImageFormat::Gif | ImageFormat::WebP)
            || (format == ImageFormat::Png && image_bytes::possibly_animated_png(&bytes));
        if all_decorative && small && !possibly_animated {
            item.classification = "ornament".into();
            item.reason = "Every occurrence has explicit decorative semantics and empty alt text; dimensions are bounded. Repetition alone does not justify skipping.".into();
        } else {
            item.classification = "substantive".into();
            item.reason = if possibly_animated {
                "Only the first frame was inspected; animation may contain substantive content."
            } else {
                "Decoration is not established for every occurrence; retain the image and pause playback. Repetition and empty alt text alone are insufficient."
            }.into();
        }
        groups
            .entry(image_bytes::fingerprint(&decoded))
            .or_default()
            .push(evidence.len());
        // Both frontends render these formats directly. Normalize other decoded
        // formats to PNG so rendering does not depend on installed image plugins.
        let (render_media, render_bytes) = if matches!(
            format,
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
        ) {
            (media_type, std::borrow::Cow::Borrowed(bytes.as_slice()))
        } else {
            let mut output = Cursor::new(Vec::new());
            decoded
                .write_to(&mut output, ImageFormat::Png)
                .map_err(|error| {
                    Error::Invalid(format!("could not prepare image {path}: {error}"))
                })?;
            ("image/png", std::borrow::Cow::Owned(output.into_inner()))
        };
        resource_sink(Resource {
            id: path.to_owned(),
            media_type: render_media.into(),
            data_url: format!(
                "data:{render_media};base64,{}",
                image_bytes::encode(&render_bytes)
            ),
        })?;
        evidence.push(item);
    }
    for indices in groups.values() {
        for &index in indices {
            evidence[index].mirrored_or_identical_assets = indices.len();
        }
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::path::Path;

    #[test]
    fn repeated_and_mirrored_images_do_not_silently_disappear() -> Result<()> {
        let limits = Limits::default();
        let archive = Archive::open(Path::new("fixtures/complex.epub"), &limits)?;
        let occurrences = vec![
            ("EPUB/ornament.png".into(), false),
            ("EPUB/ornament.png".into(), false),
            ("EPUB/ornament-mirror.png".into(), false),
        ];
        let (evidence, resources) = inspect(&archive, &limits, &occurrences)?;
        assert_eq!(evidence.len(), 4);
        assert_eq!(resources.len(), 4);
        assert_eq!(
            classification(&evidence, "EPUB/ornament.png"),
            "substantive"
        );
        assert_eq!(classification(&evidence, "unseen.png"), "substantive");
        let original = evidence
            .iter()
            .find(|e| e.resource == "EPUB/ornament.png")
            .ok_or_else(|| Error::Invalid("missing test evidence".into()))?;
        assert_eq!(original.exact_occurrences, 2);
        assert_eq!(original.mirrored_or_identical_assets, 2);
        let explicitly_decorative = vec![("EPUB/ornament.png".into(), true)];
        let (decorated, _) = inspect(&archive, &limits, &explicitly_decorative)?;
        assert_eq!(classification(&decorated, "EPUB/ornament.png"), "ornament");
        assert_eq!(
            classification(&decorated, "EPUB/uncertain.png"),
            "substantive"
        );
        Ok(())
    }

    #[test]
    fn invalid_assets_and_pixel_limits_are_observable() -> Result<()> {
        let mut limits = Limits::default();
        let mut archive = Archive::open(Path::new("fixtures/complex.epub"), &limits)?;
        archive
            .files
            .insert("bad.png".into(), b"broken image".to_vec());
        let (evidence, resources) = inspect(&archive, &limits, &[("absent.png".into(), true)])?;
        assert_eq!(classification(&evidence, "bad.png"), "unsupported");
        assert_eq!(classification(&evidence, "absent.png"), "unsupported");
        assert!(
            !resources
                .iter()
                .any(|r| r.id == "bad.png" || r.id == "absent.png")
        );
        limits.image_pixels = 10;
        assert!(matches!(
            inspect(&archive, &limits, &[]),
            Err(Error::ImageTooLarge { .. }) | Err(Error::Limit(_))
        ));
        Ok(())
    }
    #[test]
    fn supported_formats_decode_and_export_frontend_compatible_images() -> Result<()> {
        let limits = Limits::default();
        let mut archive = Archive::open(Path::new("fixtures/complex.epub"), &limits)?;
        archive.files.clear();
        for (name, format) in [
            ("pixels.png", ImageFormat::Png),
            ("pixels.jpg", ImageFormat::Jpeg),
            ("pixels.gif", ImageFormat::Gif),
            ("pixels.webp", ImageFormat::WebP),
            ("pixels.bmp", ImageFormat::Bmp),
            ("pixels.tiff", ImageFormat::Tiff),
        ] {
            let mut output = Cursor::new(Vec::new());
            image::RgbImage::from_pixel(4, 3, image::Rgb([53, 108, 165]))
                .write_to(&mut output, format)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            archive.files.insert(name.into(), output.into_inner());
        }
        archive.files.insert(
            "pixels.avif".into(),
            include_bytes!("../fixtures/pixels.avif").to_vec(),
        );
        let (evidence, resources) = inspect(&archive, &limits, &[])?;
        assert_eq!(resources.len(), 7);
        assert!(
            evidence
                .iter()
                .all(|e| e.width == 4 && e.height == 3 && e.classification == "substantive")
        );
        for resource in resources {
            assert!(matches!(
                resource.media_type.as_str(),
                "image/png" | "image/jpeg" | "image/webp"
            ));
            let (_, encoded) = resource
                .data_url
                .split_once(',')
                .ok_or_else(|| Error::Invalid("missing data URL".into()))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| Error::Invalid(e.to_string()))?;
            let image =
                image::load_from_memory(&bytes).map_err(|e| Error::Invalid(e.to_string()))?;
            assert_eq!((image.width(), image.height()), (4, 3));
        }
        Ok(())
    }

    #[test]
    fn image_limit_is_per_image_and_inclusive() -> Result<()> {
        let mut limits = Limits {
            image_pixels: 12,
            ..Limits::default()
        };
        let mut archive = Archive::open(Path::new("fixtures/complex.epub"), &limits)?;
        archive.files.clear();
        let mut output = Cursor::new(Vec::new());
        image::RgbaImage::new(4, 3)
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        for i in 0..10 {
            archive
                .files
                .insert(format!("image-{i}.png"), output.get_ref().clone());
        }
        let (_, resources) = inspect(&archive, &limits, &[])?;
        assert_eq!(resources.len(), 10);
        limits.image_pixels = 11;
        let error = inspect(&archive, &limits, &[])
            .err()
            .ok_or_else(|| Error::Invalid("oversized image accepted".into()))?;
        assert!(matches!(
            error,
            Error::ImageTooLarge {
                width: 4,
                height: 3,
                max_pixels: 11,
                ..
            }
        ));
        Ok(())
    }
}
