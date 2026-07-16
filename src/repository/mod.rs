//! repository — Query Postgres (server-only), dipisah per-domain.
//! Baris → struct sederhana; pemformatan tampilan dilakukan di layer service.

pub mod attendance;
pub mod device;
pub mod parents;
pub mod permits;
pub mod schedule;
pub mod users;

pub use attendance::*;
pub use device::*;
pub use parents::*;
pub use permits::*;
pub use schedule::*;
pub use users::*;
