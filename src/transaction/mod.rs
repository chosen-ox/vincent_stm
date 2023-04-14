pub mod log_vars;

use std::any::Any;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::BTreeMap;
use std::sync::Arc;

use self::log_vars::LogVar;
use self::log_vars::LogVar::*;
use crate::{Mtx, TVar};

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
            let space = mtx.get_space();
            let id = space.get_id();
            if id == 0 {
                match *var {
                    Read(_, v) => {
                        is_write.push(0);
                        versions.push(v);
                    }
                    ReadWrite(_, _, v) => {
                        is_write.push(1);
                        versions.push(v);
                    }
                    Write(_) => {
                        is_write.push(2);
                        versions.push(0);
                    }
                }
                spaces.push(space);
                continue;
            }
            if id != current_id {
                current_id = id;
                match *var {
                    Read(_, v) => {
                        is_write.push(0);
                        versions.push(v);
                        current_version = v;
                    }
                    ReadWrite(_, _, v) => {
                        is_write.push(1);
                        versions.push(v);
                        current_version = v;
                    }
                    Write(_) => {
                        is_write.push(2);
                        versions.push(0);
                    }
                };
                spaces.push(space);
            } else {
                match *var {
                    Read(_, v) => {
                        if  *is_write.last().unwrap() == 2 {
                            current_version = v;
                            *versions.last_mut().unwrap() = v;
                            *is_write.last_mut().unwrap() = 0;
                        } else if v != current_version {
                            return false;
                        }
                    }
                    ReadWrite(_, _, v) => {
                        if *is_write.last().unwrap() == 2 {
                            current_version = v;
                            *versions.last_mut().unwrap() = v;
                        } else if v != current_version {
                            return false;
                        }
                        *is_write.last_mut().unwrap() = 1;
                    }
                    Write(_) => {
                        if *is_write.last().unwrap() != 2 {
                            *is_write.last_mut().unwrap() = 1;
                        }
                    }
                };
            }
        }

        let mut write_vec = Vec::with_capacity(spaces.len());
        let mut read_vec = Vec::with_capacity(spaces.len());

        // trick to pass the borrow checker
        {
            for (i, space) in spaces.iter().enumerate() {
                if is_write[i] == 2 {
                    let lock = space.version.write().unwrap();
                    write_vec.push(lock);
                } else if is_write[i] == 1 {
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
                    Write(val) | ReadWrite(_, val, _) => unsafe {
                        *mtx.value.get() = val.clone();
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

    pub fn read<T: Any + Send + Sync + Clone>(&mut self, var: &TVar<T>) -> Result<T, usize> {
        let mtx = var.get_mtx_ref();
        // let version = mtx.get_space().read_version();
        let val = match self.vars.entry(mtx.clone()) {
            Occupied(entry) => match entry.get().read() {
                Ok(val) => val,
                Err(v) => return Err(v),
            },
            Vacant(entry) => unsafe {
                let (val, version) = mtx.read_atomic();
                entry.insert(LogVar::Read(val.clone(), version));
                val
            }
        };
        Ok(Transaction::downcast(val))
    }

    pub fn write<T: Any + Send + Sync + Clone>(
        &mut self,
        var: &TVar<T>,
        val: T,
    ) -> Result<usize, usize> {
        let mtx = var.get_mtx_ref();
        // let version = mtx.space.read_version();
        let val = Arc::new(val);
        match self.vars.entry(mtx) {
            Occupied(mut entry) => {
                if let Err(v) = entry.get_mut().write(val) {
                    return Err(v);
                }
            }
            Vacant(entry) => {
                entry.insert(LogVar::Write(val));
            }
        }
        // Ok(version)
        Ok(0)
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
    use crate::TVar;
    use crate::{atomically, Space};
    use std::thread;

    #[test]
    fn test_multi_space() {
        let space1 = Space::new(1);
        let space2 = Space::new(2);
        let tvar0 = TVar::new(5);
        let tvar1 = TVar::new_with_space(5, space1.clone());
        let tvar2 = TVar::new_with_space(5, space2.clone());
        atomically(|transaction| {
            tvar0.write(transaction, 10)?;
            tvar1.write(transaction, 10)?;
            tvar2.write(transaction, 10)?;
            Ok(1)
        });
        let res0 = atomically(|transaction| tvar0.read(transaction));
        let res1 = atomically(|transaction| tvar1.read(transaction));
        let res2 = atomically(|transaction| tvar2.read(transaction));
        assert_eq!(res0, 10);
        assert_eq!(res1, 10);
        assert_eq!(res2, 10);
    }
    #[test]
    fn test_for_loop() {
        let tvar = TVar::new(5);
        let res = atomically(|transaction| {
            for _ in 0..100 {
                let val = tvar.read(transaction).unwrap();
                tvar.write(transaction, val + 1).unwrap();
            }
            tvar.read(transaction)
        });
        assert_eq!(res, 105);
    }

    #[test]
    fn test_multi_thread() {
        let mut threads = Vec::with_capacity(10);
        let tvar = TVar::new(5);

        for _ in 0..100 {
            let tvar = tvar.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..10000 {
                        if let Ok(val) = tvar.read(transaction) {
                            tvar.write(transaction, val + 1).unwrap();
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
        assert_eq!(res, 1000005);
    }

    #[test]
    fn test_multi_variables() {
        let space = Space::new(1);

        let mut tvars = Vec::with_capacity(100);
        let mut threads = Vec::with_capacity(10);
        for _ in 0..100 {
            tvars.push(TVar::new_with_space(0, space.clone()));
        }

        for _ in 0..100 {
            let tvars = tvars.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..10 {
                        for tvar in &tvars {
                            if let Ok(val) = tvar.read(transaction) {
                                tvar.write(transaction, val + 1).unwrap();
                            }
                        }
                    }
                    Ok(0)
                });
            }));
        }
        for i in 0..tvars.len() {
            let tvars = tvars.clone();
            for _ in 0..100 {
                let tvar = tvars[i].clone();
                threads.push(thread::spawn(move || {
                    atomically(|transaction| {
                        if let Ok(val) = tvar.read(transaction) {
                            tvar.write(transaction, val + 1).unwrap();
                        }
                        Ok(0)
                    });
                }));
            }
        }

        for thread in threads {
            thread.join().unwrap();
        }
        for tvar in &tvars {
            let res = atomically(|transaction| tvar.read(transaction));
            assert_eq!(res, 1100);
        }
    }
}
