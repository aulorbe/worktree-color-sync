use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
        "#102a43", "#0f3d2e", "#3e1f47", "#124559", "#2f3f60", "#3f4c2f", "#4a1f2d", "#0f4c5c",
        "#251f47", "#1d4d3a", "#4f1d4d", "#1f4f6b", "#415a2a", "#31405c", "#1f5e52", "#2d3f73",
        "#6b2f1f", "#0b5a4a", "#3f2f6b", "#5a3f0b", "#2b4c7e", "#1f6f5c", "#4b2e83", "#2e6f9e",
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
}
