extern crate core;

mod space;
mod tvar;
mod transaction;
pub use space::Space;
pub use tvar::Tvar;
fn atomically<F, T>(f: F) -> T
where
    F: Fn() -> T,
{
    f()
}
