//! Deterministic rendered-frame comparison helpers for the fidelity corpus.

use effectcraft_raster::Image;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameScore {
    pub compared_pixels: usize,
    pub mean_absolute_error: f64,
    pub max_error: f32,
}

/// Compare two premultiplied rendered frames. A size mismatch is reported as an error instead of
/// silently producing a misleading score.
pub fn compare_frames(actual: &Image, reference: &Image) -> Result<FrameScore, String> {
    if actual.width != reference.width || actual.height != reference.height {
        return Err(format!("frame size mismatch: {}x{} vs {}x{}", actual.width, actual.height, reference.width, reference.height));
    }
    let mut sum = 0.0f64;
    let mut max = 0.0f32;
    for (a, b) in actual.data.iter().zip(&reference.data) {
        for (&x, &y) in a.iter().zip(b) {
            let error = (x - y).abs();
            sum += f64::from(error);
            max = max.max(error);
        }
    }
    let compared_pixels = actual.data.len();
    let samples = compared_pixels.saturating_mul(4).max(1);
    Ok(FrameScore { compared_pixels, mean_absolute_error: sum / samples as f64, max_error: max })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_pixel_error_and_rejects_size_mismatch() {
        let mut actual = Image::filled(2, 1, [0.0, 0.0, 0.0, 1.0]);
        let reference = Image::filled(2, 1, [0.0, 0.0, 0.0, 1.0]);
        actual.data[1][0] = 0.5;
        let score = compare_frames(&actual, &reference).expect("same size");
        assert_eq!(score.compared_pixels, 2);
        assert!((score.mean_absolute_error - 0.0625).abs() < f64::EPSILON);
        assert_eq!(score.max_error, 0.5);
        assert!(compare_frames(&actual, &Image::new(1, 1)).is_err());
    }
}
