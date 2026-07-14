#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionResource {
    value: f32,
}

impl DimensionResource {
    /// Creates a compile-time fixed dimension descriptor.
    pub(crate) const fn new(value: f32) -> Self {
        Self { value }
    }

    /// Returns the fixed logical-pixel value declared by the resource.
    pub(crate) const fn resolve(self) -> f32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::generated_dimensions;

    #[test]
    /// Verifies generated dimensions retain their configured fixed values.
    fn generated_dimensions_are_unscaled() {
        assert_eq!(generated_dimensions::toggle_size.resolve(), 22.0);
        assert_eq!(generated_dimensions::text_label.resolve(), 16.0);
    }
}
