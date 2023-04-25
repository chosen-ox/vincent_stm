extern crate core;

use std::any::Any;
use std::sync::Arc;

mod space;
mod transaction;
mod tvar;

pub use space::Space;
pub use transaction::Transaction;
pub use tvar::Mtx;
pub use tvar::TVar;

pub type ArcAny = Arc<dyn Any + Send + Sync>;

pub fn atomically<F, T>(f: F) -> T
where
    F: Fn(&mut Transaction) -> Result<T, usize>,
{
    Transaction::atomically(f)
}
