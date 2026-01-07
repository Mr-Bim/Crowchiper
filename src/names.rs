//! Random name generator using element + bird combinations.

use std::sync::LazyLock;

static ELEMENTS: &[&str] = &[
    "Argon", "Barium", "Carbon", "Cobalt", "Copper", "Gallium", "Gold", "Helium", "Iodine", "Iron",
    "Krypton", "Lithium", "Neon", "Nickel", "Nitrogen", "Osmium", "Oxygen", "Platinum", "Radium",
    "Silver", "Sodium", "Sulfur", "Titanium", "Tungsten", "Uranium", "Xenon", "Zinc",
];

static BIRDS: &[&str] = &[
    "Albatross",
    "Cardinal",
    "Condor",
    "Crane",
    "Crow",
    "Dove",
    "Eagle",
    "Falcon",
    "Finch",
    "Flamingo",
    "Hawk",
    "Heron",
    "Hummingbird",
    "Ibis",
    "Jay",
    "Kestrel",
    "Kingfisher",
    "Kite",
    "Lark",
    "Magpie",
    "Martin",
    "Nightingale",
    "Oriole",
    "Osprey",
    "Owl",
    "Parrot",
    "Peacock",
    "Pelican",
    "Penguin",
    "Peregrine",
    "Phoenix",
    "Raven",
    "Robin",
    "Sparrow",
    "Starling",
    "Stork",
    "Swallow",
    "Swan",
    "Swift",
    "Thrush",
    "Toucan",
    "Warbler",
    "Wren",
];

static RNG: LazyLock<std::sync::Mutex<SimpleRng>> = LazyLock::new(|| {
    // Seed from current time
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    std::sync::Mutex::new(SimpleRng::new(seed))
});

/// Simple xorshift64 RNG - we don't need cryptographic randomness for names.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

/// Generate a random name in the format "ElementBird" (e.g., "CobaltFalcon").
pub fn generate_name() -> String {
    let mut rng = RNG.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let element = ELEMENTS[rng.next() as usize % ELEMENTS.len()];
    let bird = BIRDS[rng.next() as usize % BIRDS.len()];
    format!("{}{}", element, bird)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_name_format() {
        let name = generate_name();
        // Should be non-empty and contain only alphanumeric characters
        assert!(!name.is_empty());
        assert!(name.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_name_variety() {
        // Generate several names and ensure we get some variety
        let names: Vec<String> = (0..10).map(|_| generate_name()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        // With 10 names, we should have at least a few unique ones
        assert!(unique.len() > 1, "Should generate varied names");
    }
}
