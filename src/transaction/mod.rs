pub mod log_vars;

use std::any::Any;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread::sleep;

use self::log_vars::LogVar;
use self::log_vars::LogVar::*;
use crate::{Mtx, Tvar};

#[cfg(test)]
use crate::atomically;

pub struct Transaction {
    vars: BTreeMap<Arc<Mtx>, LogVar>,
}

impl Transaction {
    pub fn new() -> Transaction {
        Transaction {
            vars: BTreeMap::new(),
        }
    }

    pub fn atomically<F, T>(f: F) -> T
    where
        F: Fn(&mut Transaction) -> Result<T, T>,
    {
        let mut transaction = Transaction::new();
        loop {
            match f(&mut transaction) {
                Ok(val) => {
                    if transaction.commit() {
                        return val;
                    }
                }
                Err(_) => {
                    sleep(std::time::Duration::from_millis(1000));
                }
            }
        }
    }

    pub fn commit(&self) -> bool {
        let mut spaces = Vec::with_capacity(self.vars.len());
        let mut versions = Vec::with_capacity(self.vars.len());
        let mut is_write = Vec::with_capacity(self.vars.len());
        let mut current_id = 0;
        let mut current_version = 0;
        for (mtx, var) in &self.vars {
            let id = mtx.get_space().get_id();
            if id == 0 {
                match var {
                    Read(_, v) => {
                        is_write.push(false);
                        versions.push(*v);
                    }
                    Write(_, v) | ReadWrite(_, _, v) => {
                        is_write.push(true);
                        versions.push(*v);
                    }
                }
                spaces.push(mtx.clone().get_space());
                continue;
            }
            if id != current_id {
                current_id = id;
                match var {
                    Read(_, v) => {
                        is_write.push(false);
                        versions.push(*v);
                        current_version = *v;
                    }
                    Write(_, v) | ReadWrite(_, _, v) => {
                        is_write.push(true);
                        versions.push(*v);
                        current_version = *v;
                    }
                };
                spaces.push(mtx.clone().get_space());
            } else {
                match var {
                    Read(_, v) => {
                        if *v != current_version {
                            return false;
                        }
                        versions.push(*v);
                    }
                    Write(_, v) | ReadWrite(_, _, v) => {
                        if *v != current_version {
                            return false;
                        }
                        *is_write.last_mut().unwrap() = true;
                        versions.push(*v);
                    }
                };
            }
        }

        let mut write_vec = Vec::with_capacity(self.vars.len());
        let mut read_vec = Vec::with_capacity(self.vars.len());

        {
            for (i, space) in spaces.iter().enumerate() {
                if is_write[i] {
                    let lock = space.version.write().unwrap();
                    if *lock != versions[i] {
                        return false;
                    }
                    write_vec.push(lock);
                } else {
                    let lock = space.version.read().unwrap();
                    if *lock != versions[i] {
                        return false;
                    }
                    read_vec.push(lock);
                }
            }

            for mut lock in write_vec {
                *lock += 1;
            }

            for (mtx, var) in &self.vars {
                match var {
                    Write(val, _) | ReadWrite(_, val, _) => {
                        *mtx.clone().value.lock().unwrap() = val.clone();
                    }
                    _ => {}
                }
            }
        }

        true
    }

    pub fn read<T: Any + Send + Sync + Clone>(&mut self, var: &Tvar<T>) -> Result<T, usize> {
        let mtx = var.get_mtx_ref();
        let version = mtx.get_space().read_version();
        let val = match self.vars.entry(mtx) {
            Occupied(mut entry) => match entry.get_mut().read(version) {
                Ok(val) => val,
                Err(v) => return Err(v),
            },
            Vacant(entry) => {
                let val = var.atomic_read();
                entry.insert(LogVar::Read(val.clone(), version));
                val
            }
        };
        Ok(Transaction::downcast(val))
    }

    pub fn write<T: Any + Send + Sync + Clone>(
        &mut self,
        var: &Tvar<T>,
        val: T,
    ) -> Result<usize, usize> {
        let mtx = var.get_mtx_ref();
        let version = mtx.get_space().read_version();
        let val = Arc::new(val);
        match self.vars.entry(mtx) {
            Occupied(mut entry) => {
                if let Err(v) = entry.get_mut().write(val, version) {
                    return Err(v);
                }
            }
            Vacant(entry) => {
                entry.insert(LogVar::Write(val, version));
            }
        }
        Ok(version)
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
    // let space = Space::new(1);
    // let tvar1 = Tvar::new_with_space(10, space);
    let tvar1 = Tvar::new(10);
    let res = atomically(|transaction| {
        transaction.write(&tvar, 10)?;
        assert_eq!(transaction.read(&tvar).unwrap(), 10);
        transaction.write(&tvar, 15)?;
        assert_eq!(transaction.read(&tvar).unwrap(), 15);
        transaction.write(&tvar1, 20)?;
        assert_eq!(transaction.read(&tvar1).unwrap(), 20);
        transaction.read(&tvar)
    });
    assert_eq!(res, 15);
    let res1 = atomically(|trans| trans.read(&tvar1));
    assert_eq!(res1, 20);
}
