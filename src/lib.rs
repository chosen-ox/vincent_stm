extern crate core;

use std::any::Any;
use std::sync::Arc;

mod space;
mod transaction;
mod tvar;
pub use space::Space;
pub use tvar::Tvar;
pub use tvar::Mtx;
use crate::transaction::Transaction;

pub type ArcAny = Arc<dyn Any + Send + Sync>;

fn atomically<F, T>(f: F) -> T
where
    F: Fn(& mut Transaction) -> Result<T, T>,
{
    Transaction::atomically(f)
}
