use std::any::Any;
use std::cell::UnsafeCell;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(test)]
use std::thread::spawn;

use crate::space::Space;
use crate::ArcAny;
use crate::Transaction;

pub struct Mtx {
    pub value: UnsafeCell<ArcAny>,
    pub space: Arc<Space>,
}
unsafe impl Sync for Mtx {}

impl Mtx {
    pub fn new(value: ArcAny, space: Arc<Space>) -> Arc<Mtx> {
        let mtx = Mtx {
            value: UnsafeCell::new(value),
            space,
        };
        Arc::new(mtx)
    }

    pub fn read_version(&self) -> Arc<u8> {
        self.space.version.read().unwrap().clone()
    }

    pub unsafe fn read_atomic(&self) -> (ArcAny, Arc<u8>) {
        let read_lock = self.space.version.read().unwrap();
        let value = (*self.value.get()).clone();
        let version = Arc::clone(&read_lock);
        drop(read_lock);
        (value, version)
    }

    pub fn get_space(&self) -> Arc<Space> {
        self.space.clone()
    }

    pub fn get_address(&self) -> usize {
        self as *const Mtx as usize
    }
}

impl Eq for Mtx {}

impl PartialEq for Mtx {
    fn eq(&self, other: &Self) -> bool {
        return self.get_address() == other.get_address();
    }
}

impl Ord for Mtx {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.space, &other.space) {
            return self.get_address().cmp(&other.get_address());
        }
        self.space.get_address().cmp(&other.space.get_address())
    }
}

impl PartialOrd for Mtx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
pub struct TVar<T> {
    arc_mtx: Arc<Mtx>,
    _marker: PhantomData<T>,
}

impl<T> TVar<T>
where
    T: Any + Send + Sync + Clone,
{
    pub fn new(value: T) -> TVar<T> {
        let space = Space::new_single_var_space();
        TVar {
            arc_mtx: Mtx::new(Arc::new(value), Arc::new(space)),
            _marker: PhantomData,
        }
    }

    pub fn new_with_space(value: T, space: Arc<Space>) -> TVar<T> {
        TVar {
            arc_mtx: Mtx::new(Arc::new(value), space),
            _marker: PhantomData,
        }
    }

    pub fn get_mtx_ref(&self) -> Arc<Mtx> {
        self.arc_mtx.clone()
    }

    pub fn read(&self, transaction: &mut Transaction) -> Result<T, usize> {
        transaction.read(&self)
    }

    pub fn write(&self, transaction: &mut Transaction, value: T) -> Result<usize, usize> {
        transaction.write(&self, value)
    }

    pub unsafe fn atomic_read(&self) -> ArcAny {
        (*self.arc_mtx.value.get()).clone()
    }

    pub unsafe fn atomic_write(&self, value: T) {
        *self.arc_mtx.value.get() = Arc::new(value);
    }
}

#[cfg(test)]
#[test]
fn test_tvar() {
    let space = Space::new(1);
    let tvar = TVar::new_with_space(0, space.clone());
    unsafe {
        assert_eq!(*tvar.atomic_read().downcast_ref::<i32>().unwrap(), 0);
    }
    let tvar1 = TVar::new_with_space(0, space.clone());
    spawn(move || unsafe {
        tvar1.atomic_write(5);
        assert_eq!(*tvar1.atomic_read().downcast_ref::<i32>().unwrap(), 5);
    })
    .join()
    .unwrap();
    let mut spaces = Vec::new();
    spaces.push(tvar.arc_mtx.get_space().clone());
    let mut locks = Vec::new();
    locks.push(spaces[0].version.write().unwrap());
}
