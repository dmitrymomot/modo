/// Generate a new ULID as a lowercase string (26 chars).
pub fn generate_ulid() -> String {
    ulid::Ulid::new().to_string()
}

/// Generate a new NanoID (21 chars, default alphabet).
pub fn generate_nanoid() -> String {
    nanoid::nanoid!()
}
