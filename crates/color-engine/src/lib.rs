use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEFAULT_PALETTE_SIZE: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub hex: String,
}

impl Color {
    pub fn new(hex: impl Into<String>) -> Self {
        let mut hex = hex.into();
        if !hex.starts_with('#') {
            hex.insert(0, '#');
        }
        Self {
            hex: hex.to_lowercase(),
        }
    }
}

pub fn default_palette() -> Vec<Color> {
    [
        "#c0c040", "#00ffff", "#ff00ff", "#6060ff", "#00ff40", "#ff0040", "#80ffc0", "#ff80c0",
        "#a000a0", "#00a0a0", "#0020c0", "#e06000", "#60e000", "#ffff00", "#ffff80", "#00a000",
        "#a00000", "#c06060", "#60c060", "#808000", "#20ffa0", "#8000ff", "#0080ff", "#ff20a0",
    ]
    .iter()
    .map(|hex| Color::new(*hex))
    .collect()
}

pub fn deterministic_index_order(seed: &str, len: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..len).collect();
    if len <= 1 {
        return indices;
    }

    for i in (1..len).rev() {
        let rand = hash_u64(seed, i as u64);
        let j = (rand as usize) % (i + 1);
        indices.swap(i, j);
    }

    indices
}

pub fn deterministic_fallback_color(seed: &str, attempt: u32) -> Color {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(b"::fallback::");
    hasher.update(attempt.to_be_bytes());
    let bytes = hasher.finalize();

    let r = 32 + (bytes[0] % 160);
    let g = 32 + (bytes[1] % 160);
    let b = 32 + (bytes[2] % 160);

    Color::new(format!("#{r:02x}{g:02x}{b:02x}"))
}

fn hash_u64(seed: &str, counter: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(b":");
    hasher.update(counter.to_be_bytes());
    let digest = hasher.finalize();

    u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn deterministic_order_is_stable() {
        let a = deterministic_index_order("repo\0worktree", 24);
        let b = deterministic_index_order("repo\0worktree", 24);
        assert_eq!(a, b);
    }

    #[test]
    fn fallback_color_has_hex_shape() {
        let color = deterministic_fallback_color("seed", 0);
        assert!(color.hex.starts_with('#'));
        assert_eq!(color.hex.len(), 7);
    }

    #[test]
    fn default_palette_has_expected_size_and_unique_values() {
        let palette = default_palette();
        assert_eq!(palette.len(), DEFAULT_PALETTE_SIZE);

        let unique: HashSet<String> = palette.into_iter().map(|c| c.hex).collect();
        assert_eq!(unique.len(), DEFAULT_PALETTE_SIZE);
    }

    #[test]
    fn default_palette_colors_are_far_apart() {
        let palette = default_palette();
        let mut min_distance = f32::MAX;

        for (i, a) in palette.iter().enumerate() {
            for b in palette.iter().skip(i + 1) {
                let distance = rgb_distance(&a.hex, &b.hex);
                min_distance = min_distance.min(distance);
            }
        }

        // Guardrail: keep the built-in palette highly distinct.
        assert!(
            min_distance >= 100.0,
            "palette too similar; min RGB distance was {min_distance}"
        );
    }

    fn rgb_distance(a: &str, b: &str) -> f32 {
        let (ar, ag, ab) = parse_rgb(a);
        let (br, bg, bb) = parse_rgb(b);

        let dr = ar as f32 - br as f32;
        let dg = ag as f32 - bg as f32;
        let db = ab as f32 - bb as f32;
        (dr * dr + dg * dg + db * db).sqrt()
    }

    fn parse_rgb(hex: &str) -> (u8, u8, u8) {
        let r = u8::from_str_radix(&hex[1..3], 16).expect("valid red channel");
        let g = u8::from_str_radix(&hex[3..5], 16).expect("valid green channel");
        let b = u8::from_str_radix(&hex[5..7], 16).expect("valid blue channel");
        (r, g, b)
    }
}
