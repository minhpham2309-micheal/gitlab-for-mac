use std::ops::Range;

use gpui_kit::{rgb, FontWeight, HighlightStyle};

use crate::gitlab::log_parse::AnsiSpan;

pub fn merge_highlight_runs(
    spans: &[AnsiSpan],
    text_len: usize,
    hit: Option<(usize, usize)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let valid_hit = hit.filter(|&(s, e)| s < e && e <= text_len);
    let hit_style = HighlightStyle {
        background_color: Some(rgb(0xFFEF6C).into()),
        color: Some(rgb(0x18171D).into()),
        ..HighlightStyle::default()
    };

    let mut runs: Vec<(Range<usize>, HighlightStyle)> = Vec::with_capacity(spans.len() + 3);
    let mut hit_emitted = false;

    for span in spans {
        let start = span.range.start.min(text_len);
        let end = span.range.end.min(text_len);
        if start >= end {
            continue;
        }
        let c = span.color;
        let color_u32 = ((c.0 as u32) << 16) | ((c.1 as u32) << 8) | (c.2 as u32);
        let style = HighlightStyle {
            color: Some(rgb(color_u32).into()),
            font_weight: span.bold.then_some(FontWeight::BOLD),
            ..HighlightStyle::default()
        };

        if let Some((hs, he)) = valid_hit {
            if !hit_emitted && he <= start {
                runs.push((hs..he, hit_style));
                hit_emitted = true;
            }
            if he <= start || hs >= end {
                runs.push((start..end, style));
            } else {
                if start < hs {
                    runs.push((start..hs, style));
                }
                if !hit_emitted {
                    runs.push((hs..he, hit_style));
                    hit_emitted = true;
                }
                if he < end {
                    runs.push((he..end, style));
                }
            }
        } else {
            runs.push((start..end, style));
        }
    }

    if let Some((hs, he)) = valid_hit
        && !hit_emitted {
            runs.push((hs..he, hit_style));
        }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitlab::Rgb;

    #[test]
    fn merged_runs_are_strictly_sorted_and_non_overlapping() {
        let spans = vec![
            AnsiSpan { range: 0..10, color: Rgb(255, 255, 255), bold: false },
            AnsiSpan { range: 10..20, color: Rgb(100, 220, 100), bold: true },
        ];
        // Hit spans across both ANSI segments (bytes 6..14).
        let runs = merge_highlight_runs(&spans, 20, Some((6, 14)));
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].0, 0..6);
        assert_eq!(runs[1].0, 6..14);
        assert_eq!(runs[2].0, 14..20);
        for w in runs.windows(2) {
            assert!(w[0].0.end <= w[1].0.start);
        }
    }
}
