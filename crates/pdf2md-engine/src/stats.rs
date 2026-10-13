//! Document-wide typographic statistics.

use crate::lines::Line;

/// Body text size and line pitch, measured over the whole document so that
/// pages with little text are judged by the same yardstick as the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocStats {
    /// Most common font size, weighted by character count.
    pub body_size: f32,
    /// Baseline-to-baseline distance divided by font size, for lines inside
    /// one paragraph.
    pub pitch_ratio: f32,
}

impl Default for DocStats {
    fn default() -> Self {
        Self { body_size: 10.0, pitch_ratio: 1.2 }
    }
}

impl DocStats {
    /// Lines must be in top-down order per page for pitch measurement.
    pub fn from_lines<'a>(lines: impl Iterator<Item = &'a Line> + Clone) -> Self {
        let mut buckets: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
        for l in lines.clone() {
            let n: usize = l.spans.iter().map(|s| s.text.chars().filter(|c| !c.is_whitespace()).count()).sum();
            *buckets.entry((l.size * 2.0).round() as i32).or_default() += n;
        }
        let Some((&key, _)) = buckets.iter().max_by_key(|(k, c)| (**c, **k)) else {
            return Self::default();
        };
        let body_size = key as f32 / 2.0;

        // Pitch: consecutive body-size lines in the same column.
        let mut ratios: Vec<f32> = Vec::new();
        let mut prev: Option<&Line> = None;
        for l in lines {
            if let Some(p) = prev {
                let same_size = (p.size - body_size).abs() < 0.3 && (l.size - body_size).abs() < 0.3;
                if same_size && l.bbox.h_overlap(&p.bbox) > 0.0 && l.bbox.y0 >= p.bbox.y1 - 0.5 * body_size {
                    let r = (l.bbox.y1 - p.bbox.y1) / body_size;
                    if (0.95..=2.5).contains(&r) {
                        ratios.push(r);
                    }
                }
            }
            prev = Some(l);
        }
        let pitch_ratio = line_pitch(ratios).unwrap_or(1.2);
        Self { body_size, pitch_ratio }
    }

    pub fn pitch_for(&self, size: f32) -> f32 {
        size * self.pitch_ratio
    }
}

/// In-paragraph pitch: the smallest pitch value that recurs.
///
/// Consecutive-line pitches mix in-paragraph leading with paragraph gaps; in
/// documents made of short paragraphs the gaps are the majority, so any
/// quantile can land on a gap. Line leading is the *smallest* consistent
/// value, so cluster the samples and take the lowest cluster that is not a
/// lone outlier (unless it is the only evidence there is).
fn line_pitch(mut ratios: Vec<f32>) -> Option<f32> {
    if ratios.is_empty() {
        return None;
    }
    ratios.sort_by(f32::total_cmp);
    let min_support = (ratios.len() / 20).max(1);
    let mut i = 0;
    while i < ratios.len() {
        let mut j = i;
        while j + 1 < ratios.len() && ratios[j + 1] - ratios[i] <= 0.06 {
            j += 1;
        }
        if j - i + 1 >= min_support {
            return Some(ratios[i..=j].iter().sum::<f32>() / (j - i + 1) as f32);
        }
        i = j + 1;
    }
    Some(ratios[0])
}

#[cfg(test)]
mod tests {
    use super::line_pitch;

    #[test]
    fn short_paragraph_documents_use_the_smallest_pitch() {
        let r = line_pitch(vec![1.815, 1.27, 1.815, 1.815, 1.815]).unwrap();
        assert!((r - 1.27).abs() < 0.01, "{r}");
    }

    #[test]
    fn isolated_tight_outlier_ignored_in_large_samples() {
        let mut v = vec![1.0];
        v.extend(std::iter::repeat_n(1.2, 50));
        v.extend(std::iter::repeat_n(1.8, 20));
        assert!((line_pitch(v).unwrap() - 1.2).abs() < 0.01);
    }
}
