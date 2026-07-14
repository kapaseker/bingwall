mod colors;
mod dimensions;
mod icons;
mod strings;

pub use colors::ColorResource;
pub use dimensions::DimensionResource;
pub use icons::ImageResource;
#[cfg(test)]
pub(crate) use strings::lock_locale_tests;
pub use strings::{Locale, TextResource, current_locale, set_locale};

include!(concat!(env!("OUT_DIR"), "/resources_generated.rs"));
