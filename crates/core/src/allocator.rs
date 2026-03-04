use anyhow::{anyhow, Result};
use std::collections::HashSet;
use worktree_color_sync_color_engine::{
    default_palette, deterministic_fallback_color, deterministic_index_order, Color,
};

#[derive(Debug, Clone)]
pub struct ColorAllocator {
    palette: Vec<Color>,
    strict_palette: bool,
}

impl ColorAllocator {
    pub fn new(palette_hex: Vec<String>, strict_palette: bool) -> Self {
        let palette = if palette_hex.is_empty() {
            default_palette()
        } else {
            palette_hex
                .into_iter()
                .map(Color::new)
                .collect::<Vec<Color>>()
        };

        Self {
            palette,
            strict_palette,
        }
    }

    pub fn allocate(
        &self,
        seed: &str,
        existing: Option<&str>,
        active_in_use: &HashSet<String>,
    ) -> Result<String> {
        let existing_norm = existing.map(normalize_hex).transpose()?;

        if let Some(existing_color) = existing_norm.clone() {
            if !active_in_use.contains(&existing_color) {
                return Ok(existing_color);
            }
        }

        if !self.palette.is_empty() {
            let order = deterministic_index_order(seed, self.palette.len());
            for idx in order {
                let candidate = self.palette[idx].hex.clone();
                if !active_in_use.contains(&candidate) {
                    return Ok(candidate);
                }
            }
        }

        if self.strict_palette {
            return Err(anyhow!(
                "palette exhausted and strict_palette is enabled; unable to pick unique color"
            ));
        }

        let mut attempt = 0u32;
        while attempt < 1024 {
            let candidate = deterministic_fallback_color(seed, attempt).hex;
            if !active_in_use.contains(&candidate) {
                return Ok(candidate);
            }
            attempt += 1;
        }

        Err(anyhow!("unable to allocate unique fallback color"))
    }
}

fn normalize_hex(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let with_hash = if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{trimmed}")
    };

    if with_hash.len() != 7 || !with_hash.chars().skip(1).all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid color hex: {input}"));
    }

    Ok(with_hash.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_existing_color_when_available() {
        let allocator = ColorAllocator::new(vec![], false);
        let active = HashSet::new();
        let color = allocator
            .allocate("repo\0wt", Some("#123456"), &active)
            .unwrap();
        assert_eq!(color, "#123456");
    }

    #[test]
    fn avoids_active_collisions() {
        let allocator = ColorAllocator::new(vec!["#111111".into(), "#222222".into()], false);
        let mut active = HashSet::new();
        active.insert("#111111".to_string());

        let color = allocator.allocate("seed", None, &active).unwrap();
        assert_eq!(color, "#222222");
    }

    #[test]
    fn deterministic_fallback_when_palette_exhausted() {
        let allocator = ColorAllocator::new(vec!["#111111".into()], false);
        let mut active = HashSet::new();
        active.insert("#111111".to_string());

        let color = allocator.allocate("seed", None, &active).unwrap();
        assert_ne!(color, "#111111");
    }
}
