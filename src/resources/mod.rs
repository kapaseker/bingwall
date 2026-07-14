mod colors;
mod context;
mod dimensions;
mod icons;
mod strings;

pub use context::{AppTheme, ResourceContext};
pub use dimensions::DimensionScale;
pub use icons::ImageResource;
pub use strings::Locale;

include!(concat!(env!("OUT_DIR"), "/resources_generated.rs"));
