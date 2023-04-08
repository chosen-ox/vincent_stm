pub mod log_vars;

use std::any::Any;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

use crate::{ArcAny, Tvar, Mtx};
use self::log_vars::LogVar;

pub struct Transaction {
    vars: BTreeMap<Arc<Mtx>, LogVar>,
}

impl Transaction {
    pub fn new() -> Transaction {
        Transaction {
            vars: BTreeMap::new(),
        }
    }

    pub fn atomically<F, T>(f: F) -> Option<T>
    where
        F: Fn(& mut Transaction) -> Result<T, T>,
    {
        let mut transaction = Transaction::new();
        loop {
            match f( & mut transaction)  {
                Ok(val) => {
                    if transaction.commit() {
                        return Some(val);
                    }
                }
                Err(_) => {
                    transaction.rollback();
                    return None;
                }
            }

        }
    }

    fn read<T: Any + Send + Sync + Clone>(&mut self, var: &Tvar<T>) -> Result<T, T> {
        let mtx = var.get_mtx_ref();
        let val = match self.vars.entry(mtx) {
            Occupied(mut entry) => {
                entry.get_mut().read()
            }
            Vacant(entry) => {
                let val = var.atomic_read();
                entry.insert(LogVar::Read(val.clone()));
                val
            }
        };
        Ok(Transaction::downcast(val))
    }

    fn write<T: Any + Send + Sync + Clone>(&mut self, var: &Tvar<T>, val: T) {
        let mtx = var.get_mtx_ref();
        let val = Arc::new(val);
        match self.vars.entry(mtx) {
            Occupied(mut entry) => {
                entry.get_mut().write(val);
            }
            Vacant(entry) => {
                entry.insert(LogVar::Write(val));
            }
        }
    }

    fn downcast<T: Any + Clone>(var: Arc<dyn Any>) -> T {
        match var.downcast_ref::<T>() {
            Some(s) => s.clone(),
            None => unreachable!("TVar has wrong type"),
        }
    }
}

#[cfg(test)]
#[test]
fn test_transaction() {
    let tvar = Tvar::new(5);
    let mut transaction = Transaction::new();
    assert_eq!(transaction.read(&tvar).unwrap(), 5);
    transaction.write(&tvar, 10);
    assert_eq!(transaction.read(&tvar).unwrap(), 10);
}