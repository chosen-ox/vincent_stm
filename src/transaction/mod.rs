pub mod log_vars;

use std::any::Any;
use std::collections::btree_map::Entry::{Occupied, Vacant};
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::sync::Arc;

use self::log_vars::LogVar;
use self::log_vars::LogVar::*;
use crate::{Mtx, TVar};

pub struct Transaction {
    vars: BTreeMap<Arc<Mtx>, LogVar>,
    msg_log: Vec<String>,
}

impl Transaction {
    pub fn new() -> Transaction {
        Transaction {
            vars: BTreeMap::new(),
            msg_log: Vec::new(),
        }
    }

    pub fn atomically<F, T>(f: F) -> T
    where
        F: Fn(&mut Transaction) -> Result<T, usize>,
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
                        if *is_write.last().unwrap() == 2 {
                            current_version = v;
                            *versions.last_mut().unwrap() = v;
                            *is_write.last_mut().unwrap() = 1;
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
                        if *is_write.last().unwrap() == 0 {
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
                    Write(val) | ReadWrite(_, val, _) => {
                        *mtx.value.lock().unwrap() = val.clone();
                    }
                    _ => {}
                }
            }

            for msg in &self.msg_log {
                println!("{}", msg);
            }

            for mut lock in write_vec {
                *lock += 1;
            }
        }

        true
    }

    pub fn read<T: Any + Send + Sync + Clone>(&mut self, var: &TVar<T>) -> Result<T, usize> {
        let mtx = var.get_mtx_ref();
        let val = match self.vars.entry(mtx.clone()) {
            Occupied(entry) => match entry.get().read() {
                Ok(val) => val,
                Err(v) => return Err(v),
            },
            Vacant(entry) => {
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
        Ok(0)
    }

    pub fn display_value<T: Any + Send + Sync + Clone + Display>(
        &mut self,
        var: &TVar<T>,
        msg: &str,
    ) {
        let msg = format!("{} {}", msg, self.read(var).unwrap());
        self.msg_log.push(msg);
    }

    pub fn debug_value<T: Any + Send + Sync + Clone + Debug>(&mut self, var: &TVar<T>, msg: &str) {
        let msg = format!("{:?} {:?}", msg, self.read(var).unwrap());
        self.msg_log.push(msg);
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
    #[allow(unused_imports)]
    use std::thread::sleep;

    #[test]
    fn test_display() {
        let tvar0 = TVar::new(1);
        let tvar1 = TVar::new(2);
        let tvar2 = TVar::new(3);

        atomically(|transaction| {
            tvar0.display_value(transaction, "tvar0 before modify");
            tvar0.write(transaction, 10)?;
            tvar0.display_value(transaction, "tvar0 after modify");

            tvar1.display_value(transaction, "tvar1 before modify");
            tvar1.write(transaction, 10)?;
            tvar1.display_value(transaction, "tvar1 after modify");

            tvar2.display_value(transaction, "tvar2 before modify");
            tvar2.write(transaction, 10)?;
            tvar2.display_value(transaction, "tvar2 after modify");

            tvar1.read(transaction)
        });
    }

    #[test]
    fn test_multi_space() {
        let space1 = Space::new(1);
        let space2 = Space::new(2);

        let tvar0 = TVar::new(vec![1, 2, 3]);
        let tvar1 = TVar::new_with_space(5, space1.clone());
        let tvar2 = TVar::new_with_space(5, space2.clone());

        atomically(|transaction| {
            tvar0.write(transaction, vec![1, 2, 3, 4])?;
            tvar0.debug_value(transaction, "tvar0");
            tvar1.write(transaction, 10)?;
            tvar2.write(transaction, 10)?;
            Ok(1)
        });

        let res0 = atomically(|transaction| tvar0.read(transaction));
        let res1 = atomically(|transaction| tvar1.read(transaction));
        let res2 = atomically(|transaction| tvar2.read(transaction));

        assert_eq!(res0, vec![1, 2, 3, 4]);
        assert_eq!(res1, 10);
        assert_eq!(res2, 10);
    }
    #[test]
    fn test_single_thread() {
        for _ in 0..100 {
            let tvar = TVar::new(5);
            let res = atomically(|transaction| {
                for _ in 0..100000 {
                    let val = tvar.read(transaction).unwrap();
                    tvar.write(transaction, val + 1).unwrap();
                }
                tvar.read(transaction)
            });
            assert_eq!(res, 100005);
        }
    }

    #[test]
    fn test_multi_threads() {
        for _ in 0..100 {
            let mut threads = Vec::with_capacity(10);
            let tvar = TVar::new(5);

            for _ in 0..100 {
                let tvar = tvar.clone();
                threads.push(thread::spawn(move || {
                    atomically(|transaction| {
                        for _ in 0..1000 {
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
            assert_eq!(res, 100005);
        }
    }
    #[test]
    fn test_single_variable() {
        let space = Space::new(1);

        let mut tvars = Vec::with_capacity(100);
        let mut threads = Vec::with_capacity(10);

        tvars.push(TVar::new_with_space(0, space.clone()));

        let tvars_cpy = tvars.clone();
        threads.push(thread::spawn(move || {
            for _ in 0..10 {
                atomically(|transaction| {
                    for _ in 0..100 {
                        if let Ok(val) = tvars_cpy[0].read(transaction) {
                            tvars_cpy[0].write(transaction, val + 1)?;
                        }
                    }
                    Ok(0)
                });
            }
        }));

        atomically(|transaction| {
            for _ in 0..100 {
                for tvar in &tvars {
                    if let Ok(val) = tvar.read(transaction) {
                        tvar.write(transaction, val + 1).unwrap();
                    }
                }
            }
            Ok(0)
        });

        for thread in threads {
            thread.join().unwrap();
        }

        let res = atomically(|transaction| tvars[0].read(transaction));
        assert_eq!(res, 1100)

    }

    #[test]
    fn test_multi_variables() {
        let space = Space::new(1);

        let mut tvars = Vec::with_capacity(100);
        let mut threads = Vec::with_capacity(10);

        for _ in 0..1000 {
            tvars.push(TVar::new_with_space(0, space.clone()));
        }

        for _ in 0..10 {
            let tvars_cpy = tvars.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..100 {
                        for tvar in &tvars_cpy {
                            if let Ok(val) = tvar.read(transaction) {
                                tvar.write(transaction, val + 1)?;
                            }
                        }
                    }
                    Ok(0)
                });
            }));

            let tvars = tvars.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..100000 {
                        if let Ok(val) = tvars[0].read(transaction) {
                            tvars[999].write(transaction, val + 1)?;
                        }
                    }
                    Ok(0)
                });
            }));
        }


        for thread in threads {
            thread.join().unwrap();
        }
        for (i, tvar) in tvars.iter().enumerate() {
            let res = atomically(|transaction| tvar.read(transaction));
            if i == 999 {
                assert_eq!(res, 1001);
                continue;
            }
            assert_eq!(res, 1000);
        }
    }
}
