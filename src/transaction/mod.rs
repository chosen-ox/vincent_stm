pub mod log_vars;

use std::any::Any;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::BTreeMap;
use std::sync::Arc;

use self::log_vars::LogVar;
use self::log_vars::LogVar::*;
use crate::{Mtx, Tvar};


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
                    // sleep(std::time::Duration::from_millis(1000));
                }
            }
            transaction.clear();
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

        // trick to pass the borrow checker
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


            for (mtx, var) in &self.vars {
                match var {
                    Write(val, _) | ReadWrite(_, val, _) => {
                        *mtx.clone().value.lock().unwrap() = val.clone();
                    }
                    _ => {}
                }
            }

            for mut lock in write_vec {
                *lock += 1;
            }

        }

        true
    }

    pub fn read<T: Any + Send + Sync + Clone>(&mut self, var: &Tvar<T>) -> Result<T, usize> {
        let mtx = var.get_mtx_ref();
        let version = mtx.get_space().read_version();
        let val = match self.vars.entry(mtx.clone()) {
            Occupied(mut entry) => match entry.get_mut().read(version) {
                Ok(val) => val,
                Err(v) => return Err(v),
            },
            Vacant(entry) => {
                let space = mtx.clone().get_space();
                let _read_lcok = space.version.read().unwrap();
                let val = mtx.clone().value.lock().unwrap().clone();
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

    pub fn clear(&mut self) {
        self.vars.clear();
    }
}
#[cfg(test)]
mod test_transaction {
    use crate::atomically;
    use crate::Tvar;
    use std::thread;
    #[test]
    fn test_for_loop() {
        let tvar = Tvar::new(5);
        let res = atomically(|transaction| {
            for _ in 0..100 {
                let val = tvar.read(transaction).unwrap();
                tvar.write(val + 1, transaction)?;
            }
            tvar.read(transaction)
        });
        assert_eq!(res, 105);
    }

    #[test]
    fn test_multi_thread() {
        let mut threads = Vec::with_capacity(10);
        let tvar = Tvar::new(5);

        for _ in 0..10 {
            let tvar = tvar.clone();
            threads.push(thread::spawn(move || {
                    atomically(|transaction| {
                        for _ in 0..1000 {
                            if let Ok(val) = tvar.read(transaction) {
                                tvar.write(val + 1, transaction)?;
                            }
                        }
                        tvar.read(transaction)

                    });
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let res = atomically(|transaction| tvar.read(transaction));
        assert_eq!(res, 10005);
    }
}