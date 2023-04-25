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
        let mut current_version = Arc::new(0);
        for (mtx, var) in &self.vars {
            let space = mtx.get_space();
            let id = space.id;
            if id == 0 {
                match var {
                    Read(_, v) => {
                        let version = space.read_version();
                        if !Arc::ptr_eq(v, &version) {
                            return false;
                        }
                        is_write.push(0);
                        versions.push(v.clone());
                    }
                    ReadWrite(_, _, v) => {
                        let version = space.read_version();
                        if !Arc::ptr_eq(v, &version) {
                            return false;
                        }
                        is_write.push(1);
                        versions.push(v.clone());
                    }
                    Write(_) => {
                        is_write.push(2);
                        versions.push(Arc::new(0));
                    }
                }
                spaces.push(space);
                continue;
            }
            if id != current_id {
                current_id = id;
                match var {
                    Read(_, v) => {
                        let version = space.read_version();
                        if !Arc::ptr_eq(v, &version) {
                            return false;
                        }
                        is_write.push(0);
                        versions.push(v.clone());
                        current_version = v.clone();
                    }
                    ReadWrite(_, _, v) => {
                        let version = space.read_version();
                        if !Arc::ptr_eq(v, &version) {
                            println!("retry len {}", self.vars.len());
                            return false;
                        }
                        is_write.push(1);
                        versions.push(v.clone());
                        current_version = v.clone();
                    }
                    Write(_) => {
                        is_write.push(2);
                        versions.push(Arc::new(0));
                        current_version = Arc::new(0);
                    }
                };
                spaces.push(space);
            } else {
                match var {
                    Read(_, v) => {
                        if *is_write.last().unwrap() == 2 {
                            current_version = v.clone();
                            *versions.last_mut().unwrap() = v.clone();
                            *is_write.last_mut().unwrap() = 1;
                        } else if !Arc::ptr_eq(v, &current_version) {
                            return false;
                        }
                    }
                    ReadWrite(_, _, v) => {
                        if *is_write.last().unwrap() == 2 {
                            current_version = v.clone();
                            *versions.last_mut().unwrap() = v.clone();
                        } else if !Arc::ptr_eq(v, &current_version) {
                            println!("retry len 1 {}", self.vars.len());
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
        for (i, space) in spaces.iter().enumerate() {
            if is_write[i] == 2 {
                // write only
                // println!("len {}", self.vars.len());
                let lock = space.version.write().unwrap();
                write_vec.push(lock);
            } else if is_write[i] == 1 {
                // read and write
                let lock = space.version.write().unwrap();
                if !Arc::ptr_eq(&lock, &versions[i]) {
                    // println!("retry1");
                    // println!("len {}", self.vars.len());
                    return false;
                }
                write_vec.push(lock);
            } else {
                // println!("read");
                // println!("len {} idx{}", self.vars.len(), i);
                // read only
                let lock = space.version.read().unwrap();
                if !Arc::ptr_eq(&lock, &versions[i]) {
                    return false;
                }
                read_vec.push(lock);
            }
        }

        drop(read_vec);
        println!("len {}", self.vars.len());
        for (mtx, var) in &self.vars {
            match var {
                Write(val) | ReadWrite(_, val, _) => unsafe {
                    *mtx.value.get() = val.clone();
                },
                _ => {}
            }
        }

        for msg in &self.msg_log {
            println!("{}", msg);
        }

        for mut lock in write_vec {
            *lock = Arc::new(0);
        }

        true
    }

    pub fn read<T: Any + Send + Sync + Clone>(&mut self, var: &TVar<T>) -> Result<T, usize> {
        let mtx = var.get_mtx_ref();
        // let version = mtx.get_space().read_version();
        let val = match self.vars.entry(mtx.clone()) {
            Occupied(entry) => entry.get().read(),
            Vacant(entry) => unsafe {
                let (val, version) = mtx.read_atomic();
                entry.insert(LogVar::Read(val.clone(), version));
                val
            },
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
                entry.get_mut().write(val);
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
            tvar0.debug_value(transaction, "tvar0");
            // tvar0.write(transaction, 10)?;
            // tvar0.debug_value(transaction, "tvar0");
            tvar1.write(transaction, 10)?;
            tvar1.debug_value(transaction, "tvar1");
            tvar2.write(transaction, 10)?;
            tvar2.debug_value(transaction, "tvar2");
            Ok(1)
        });
        let res0 = atomically(|transaction| tvar0.read(transaction));
        let res1 = atomically(|transaction| tvar1.read(transaction));
        let res2 = atomically(|transaction| tvar2.read(transaction));
        assert_eq!(res0, vec![1, 2, 3]);
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
    fn test_multi_thread() {
        for _ in 0..100 {
            let mut threads = Vec::with_capacity(10);
            let tvar = TVar::new(5);

            for _ in 0..100 {
                let tvar = tvar.clone();
                threads.push(thread::spawn(move || {
                    atomically(|transaction| {
                        for _ in 0..1000 {
                            if let Ok(val) = tvar.read(transaction) {
                                tvar.display_value(transaction, "tvar");
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
        for _ in 0..1000 {
            let space = Space::new(1);

            let mut tvars = Vec::with_capacity(100);
            let mut threads = Vec::with_capacity(10);
            for _ in 0..1 {
                tvars.push(TVar::new_with_space(0, space.clone()));
            }

            let tvars_cpy = tvars.clone();
            threads.push(thread::spawn(move || {
                for i in 0..10 {
                    atomically(|transaction| {
                        for _ in 0..100 {
                            if let Ok(val) = tvars_cpy[0].read(transaction) {
                                tvars_cpy[0].write(transaction, i)?;
                            }
                        }
                        Ok(0)
                    });
                }
            }));

            // for _ in 0..10 {
            // let tvars = tvars.clone();
            // sleep(std::time::Duration::from_millis(1000));
            atomically(|transaction| {
                for _ in 0..100 {
                    for tvar in &tvars {
                        if let Ok(val) = tvar.read(transaction) {
                            tvar.write(transaction, val + 1).unwrap();
                        }
                    }
                }
                // simulate some work
                Ok(0)
            });
            //360
            // sleep(std::time::Duration::from_millis(360));
            // }
            // let tvars = tvars.clone();
            // threads.push(thread::spawn(move || {
            //     for i in 0..500 {
            //         atomically(|transaction| {
            //             for _ in 0..10000 {
            //                 let tvar = tvars[50].read(transaction);
            //                 tvars[50].write(transaction, i)?;
            //             }
            //             Ok(0)
            //         });
            //         sleep(std::time::Duration::from_millis(50));
            //     }
            // }));

            threads.push(thread::spawn(|| {}));
            for thread in threads {
                thread.join().unwrap();
            }
        }
    }

    #[test]
    fn test_multi_variables() {

        // for _ in 0..10 {
            let space = Space::new(1);
        let mut tvars = Vec::with_capacity(100);
        let mut threads = Vec::with_capacity(10);
        for _ in 0..1000 {
            tvars.push(TVar::new_with_space(0, space.clone()));
            // tvars.push(TVar::new(0));
        }

        for _ in 0..10 {

            let tvars_cpy = tvars.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..100 {
                        for tvar in &tvars_cpy {
                            if let Ok(val) = tvar.read(transaction) {
                                tvar.write(transaction, val + 1).unwrap();
                            }
                        }
                    }
                    // simulate some work
                    Ok(0)
                });
            }));
            let tvars = tvars.clone();
            threads.push(thread::spawn(move || {
                atomically(|transaction| {
                    for _ in 0..100000 {
                        if let Ok(val) = tvars[0].read(transaction) {
                            tvars[999].write(transaction, val + 1).unwrap();
                        }
                    }
                    // simulate some work
                    Ok(0)
                });
            }));
            //360
            // sleep(std::time::Duration::from_millis(1000));
        }
        // let tvars = tvars.clone();
        // threads.push(thread::spawn(move || {
        //     for i in 0..500 {
        //         atomically(|transaction| {
        //             for _ in 0..10000 {
        //                 let tvar = tvars[50].read(transaction);
        //                 tvars[50].write(transaction, i)?;
        //             }
        //             Ok(0)
        //         });
        //         sleep(std::time::Duration::from_millis(50));
        //     }
        // }));

        for thread in threads {
            thread.join().unwrap();
        }
        // for (i, tvar) in tvars.iter().enumerate() {
        //     let res = atomically(|transaction| tvar.read(transaction));
        //     if i == 99 {
        //         println!("res: {}", res);
        //         continue;
        //     }
        //     assert_eq!(res, 1000);
        // }
        // }
    }
}
