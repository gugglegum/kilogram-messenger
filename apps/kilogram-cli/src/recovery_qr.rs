use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use image::{
    ColorType, ImageEncoder, ImageFormat, ImageReader, Limits, Luma, codecs::png::PngEncoder,
};
use qrcode::{EcLevel, QrCode, Version};
use tempfile::NamedTempFile;

use crate::recovery_link::MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES;

const HISTORY_RECOVERY_LINK_PREFIX: &str = "kilogram://history-recovery/v1/";
const QR_MODULE_PIXELS: u32 = 4;
const MAX_QR_IMAGE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_QR_IMAGE_DIMENSION: u32 = 4_096;
const MAX_QR_IMAGE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryQrRenderReport {
    pub qr_version: i16,
    pub module_count: usize,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub png_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryQrDecodeReport {
    pub payload: String,
    pub image_width: u32,
    pub image_height: u32,
    pub image_bytes: u64,
    pub image_format: &'static str,
}

pub(crate) fn render_recovery_link_qr_png(
    payload: &str,
    output: &Path,
) -> Result<RecoveryQrRenderReport> {
    render_bounded_ascii_qr_png(
        payload,
        output,
        HISTORY_RECOVERY_LINK_PREFIX,
        MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
        "history recovery link",
    )
}

pub(crate) fn render_bounded_ascii_qr_png(
    payload: &str,
    output: &Path,
    expected_prefix: &str,
    max_payload_bytes: usize,
    label: &str,
) -> Result<RecoveryQrRenderReport> {
    validate_payload_shape(payload, expected_prefix, max_payload_bytes, label)?;
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::L)
        .with_context(|| format!("encode {label} as QR"))?;
    let qr_version = match code.version() {
        Version::Normal(version) => version,
        Version::Micro(_) => bail!("{label} unexpectedly encoded as Micro QR"),
    };
    let module_count = code.width();
    let image = code
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(QR_MODULE_PIXELS, QR_MODULE_PIXELS)
        .build();
    let pixel_width = image.width();
    let pixel_height = image.height();
    ensure!(
        pixel_width <= MAX_QR_IMAGE_DIMENSION && pixel_height <= MAX_QR_IMAGE_DIMENSION,
        "rendered {label} QR exceeds the image dimension limit"
    );

    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            pixel_width,
            pixel_height,
            ColorType::L8.into(),
        )
        .with_context(|| format!("encode {label} QR as PNG"))?;
    ensure!(
        png.len() as u64 <= MAX_QR_IMAGE_FILE_BYTES,
        "rendered {label} QR exceeds the PNG size limit"
    );
    persist_noclobber(output, &png)?;

    Ok(RecoveryQrRenderReport {
        qr_version,
        module_count,
        pixel_width,
        pixel_height,
        png_bytes: png.len(),
    })
}

pub(crate) fn decode_recovery_link_qr_image(path: &Path) -> Result<RecoveryQrDecodeReport> {
    decode_bounded_ascii_qr_image(
        path,
        HISTORY_RECOVERY_LINK_PREFIX,
        MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES,
        "history recovery link",
    )
}

pub(crate) fn decode_bounded_ascii_qr_image(
    path: &Path,
    expected_prefix: &str,
    max_payload_bytes: usize,
    label: &str,
) -> Result<RecoveryQrDecodeReport> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("read {label} QR metadata from {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "{label} QR input is not a regular non-symlink file"
    );
    ensure!(metadata.len() > 0, "{label} QR image is empty");
    ensure!(
        metadata.len() <= MAX_QR_IMAGE_FILE_BYTES,
        "{label} QR image exceeds the 16 MiB file limit"
    );

    let file =
        File::open(path).with_context(|| format!("open {label} QR image {}", path.display()))?;
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .with_context(|| format!("detect {label} QR image format"))?;
    let image_format = match reader.format() {
        Some(ImageFormat::Png) => "png",
        Some(ImageFormat::Jpeg) => "jpeg",
        Some(other) => bail!("unsupported recovery QR image format: {other:?}"),
        None => bail!("recovery QR image format could not be detected"),
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_QR_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_QR_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_QR_IMAGE_ALLOC_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .with_context(|| format!("decode bounded {label} QR image"))?;
    let image_width = image.width();
    let image_height = image.height();
    let mut prepared = rqrr::PreparedImage::prepare(image.to_luma8());
    let grids = prepared.detect_grids();
    ensure!(
        grids.len() == 1,
        "{label} QR image must contain exactly one QR code; detected {}",
        grids.len()
    );
    let (_, payload) = grids[0]
        .decode()
        .with_context(|| format!("decode {label} QR payload"))?;
    validate_payload_shape(&payload, expected_prefix, max_payload_bytes, label)?;

    Ok(RecoveryQrDecodeReport {
        payload,
        image_width,
        image_height,
        image_bytes: metadata.len(),
        image_format,
    })
}

fn validate_payload_shape(
    payload: &str,
    expected_prefix: &str,
    max_payload_bytes: usize,
    label: &str,
) -> Result<()> {
    ensure!(payload.is_ascii(), "{label} QR payload is not ASCII");
    ensure!(
        payload.len() <= max_payload_bytes,
        "{label} QR payload exceeds its size limit"
    );
    ensure!(
        payload.starts_with(expected_prefix),
        "{label} QR payload has an unsupported URI prefix"
    );
    Ok(())
}

fn persist_noclobber(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create recovery QR directory {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary recovery QR in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .context("write temporary recovery QR")?;
    temporary
        .as_file()
        .sync_all()
        .context("flush temporary recovery QR")?;
    match temporary.persist_noclobber(path) {
        Ok(file) => file
            .sync_all()
            .with_context(|| format!("flush recovery QR {}", path.display())),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("recovery QR output already exists: {}", path.display())
        }
        Err(error) => {
            Err(error.error).with_context(|| format!("publish recovery QR {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, codecs::jpeg::JpegEncoder, imageops::replace};

    #[test]
    fn recovery_qr_round_trips_and_is_noclobber() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("recovery.png");
        let payload = format!("{HISTORY_RECOVERY_LINK_PREFIX}{}", "A".repeat(1_100));
        let rendered = render_recovery_link_qr_png(&payload, &path)?;
        assert!(rendered.qr_version > 0);
        assert!(rendered.module_count > 0);
        assert!(rendered.png_bytes > 0);

        let decoded = decode_recovery_link_qr_image(&path)?;
        assert_eq!(decoded.payload, payload);
        assert_eq!(decoded.image_format, "png");
        assert_eq!(decoded.image_width, rendered.pixel_width);
        assert_eq!(decoded.image_height, rendered.pixel_height);
        assert!(render_recovery_link_qr_png(&decoded.payload, &path).is_err());
        Ok(())
    }

    #[test]
    fn recovery_qr_rejects_wrong_prefix_and_oversized_payload() -> Result<()> {
        let directory = tempfile::tempdir()?;
        assert!(
            render_recovery_link_qr_png("https://example.com", &directory.path().join("a.png"))
                .is_err()
        );
        let oversized = format!(
            "{HISTORY_RECOVERY_LINK_PREFIX}{}",
            "A".repeat(MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES)
        );
        assert!(render_recovery_link_qr_png(&oversized, &directory.path().join("b.png")).is_err());
        Ok(())
    }

    #[test]
    fn maximum_recovery_link_fits_one_qr_and_decodes() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("maximum.png");
        let payload = format!(
            "{HISTORY_RECOVERY_LINK_PREFIX}{}",
            "a".repeat(MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES - HISTORY_RECOVERY_LINK_PREFIX.len())
        );
        assert_eq!(payload.len(), MAX_HISTORY_RECOVERY_LINK_TEXT_BYTES);
        let rendered = render_recovery_link_qr_png(&payload, &path)?;
        assert_eq!(rendered.qr_version, 40);
        assert_eq!(decode_recovery_link_qr_image(&path)?.payload, payload);
        Ok(())
    }

    #[test]
    fn recovery_qr_decoder_enforces_file_and_dimension_bounds() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let large_file = directory.path().join("large.png");
        File::create(&large_file)?.set_len(MAX_QR_IMAGE_FILE_BYTES + 1)?;
        assert!(decode_recovery_link_qr_image(&large_file).is_err());

        let wide_image = directory.path().join("wide.png");
        let pixels = vec![255_u8; (MAX_QR_IMAGE_DIMENSION + 1) as usize];
        let mut png = Vec::new();
        PngEncoder::new(&mut png).write_image(
            &pixels,
            MAX_QR_IMAGE_DIMENSION + 1,
            1,
            ColorType::L8.into(),
        )?;
        fs::write(&wide_image, png)?;
        assert!(decode_recovery_link_qr_image(&wide_image).is_err());
        Ok(())
    }

    #[test]
    fn recovery_qr_accepts_jpeg_and_rejects_ambiguous_images() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let payload = format!("{HISTORY_RECOVERY_LINK_PREFIX}{}", "a".repeat(800));
        let first_path = directory.path().join("first.png");
        let second_path = directory.path().join("second.png");
        render_recovery_link_qr_png(&payload, &first_path)?;
        render_recovery_link_qr_png(&payload, &second_path)?;
        let first = image::open(&first_path)?.to_luma8();
        let second = image::open(&second_path)?.to_luma8();

        let jpeg_path = directory.path().join("recovery.jpg");
        let mut jpeg_file = File::create(&jpeg_path)?;
        JpegEncoder::new_with_quality(&mut jpeg_file, 95).encode_image(&first)?;
        jpeg_file.sync_all()?;
        let jpeg = decode_recovery_link_qr_image(&jpeg_path)?;
        assert_eq!(jpeg.image_format, "jpeg");
        assert_eq!(jpeg.payload, payload);

        let gap = 32;
        let mut ambiguous = GrayImage::from_pixel(
            first.width() + gap + second.width(),
            first.height().max(second.height()),
            Luma([255]),
        );
        replace(&mut ambiguous, &first, 0, 0);
        replace(&mut ambiguous, &second, i64::from(first.width() + gap), 0);
        let ambiguous_path = directory.path().join("ambiguous.png");
        ambiguous.save_with_format(&ambiguous_path, ImageFormat::Png)?;
        assert!(decode_recovery_link_qr_image(&ambiguous_path).is_err());
        Ok(())
    }
}
