pub mod outline;
pub mod patch;
pub mod projection;
pub mod section;

pub use outline::{outline, Heading};
pub use patch::{delete_section, replace_section, PatchError};
pub use projection::{project, SectionMeta};
pub use section::{read_section, SectionError};
