//! repository — Query Postgres (server-only), dipisah per-domain.
//! Baris → struct sederhana; pemformatan tampilan dilakukan di layer service.

pub mod activity_log;
pub mod attendance;
pub mod books;
pub mod device;
pub mod gate;
pub mod hafalan;
pub mod kelas;
pub mod materials;
pub mod parents;
pub mod permits;
pub mod schedule;
pub mod settings;
pub mod users;

pub use activity_log::*;
pub use attendance::*;
pub use books::*;
pub use device::*;
pub use gate::*;
pub use hafalan::*;
pub use kelas::*;
pub use materials::*;
pub use parents::*;
pub use permits::*;
pub use schedule::*;
pub use settings::*;
pub use users::*;
