pub mod outline;
pub mod patch;
pub mod projection;
pub mod section;

pub use outline::{Heading, outline};
pub use patch::{PatchError, delete_section, replace_section};
pub use projection::{SectionMeta, project};
pub use section::{SectionError, read_section};
