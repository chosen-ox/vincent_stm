use std::any::Any;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use crate::space::Space;
use crate::ArcAny;

pub struct Mtx {
    pub value: Mutex<ArcAny>,
    space: Space,
}

impl Mtx {
    pub fn new(value: ArcAny, space: Space) -> Arc<Mtx> {
        let mtx = Mtx {
            value: Mutex::new(value),
            space,
        };
        Arc::new(mtx)
    }

    pub fn get_space(&self) -> Space {
        self.space.clone()
    }

    pub fn get_address(&self) -> usize {
        self as *const Mtx as usize
    }
}



impl Eq for Mtx {}

impl PartialEq for Mtx {
    fn eq(&self, other: &Self) -> bool {
        self.get_address() == other.get_address()
    }
}

impl Ord for Mtx {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_address().cmp(&other.get_address())
    }
}

impl PartialOrd for Mtx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
pub struct Tvar<T> {
    arc_mtx: Arc<Mtx>,
    _marker: PhantomData<T>,
}

impl<T> Tvar<T>
where
    T: Any + Send + Sync + Clone,
{
    pub fn new(value: T) -> Tvar<T> {
        let space = Space::new_single_var_space();
        Tvar {
            arc_mtx: Mtx::new(Arc::new(value), space),
            _marker: PhantomData,
        }
    }

    pub fn new_with_space(value: T, space: Space) -> Tvar<T> {
        Tvar {
            arc_mtx: Mtx::new(Arc::new(value), space),
            _marker: PhantomData,
        }
    }

    pub fn get_mtx_ref(&self) -> Arc<Mtx> {
        self.arc_mtx.clone()
    }

    pub fn atomic_read(&self) -> ArcAny {
        self.arc_mtx.value.lock().unwrap().clone()
    }

    pub fn atomic_write(&self, value: T) {
        *self.arc_mtx.value.lock().unwrap() = Arc::new(value);
    }
}

#[cfg(test)]
#[test]
fn test_tvar() {
    let space = Space::new(1);
    let tvar = Tvar::new_with_space(0, space.clone());
    assert_eq!(*tvar.atomic_read().downcast_ref::<i32>().unwrap(), 0);
    let s = tvar.get_mtx_ref().get_space();
    assert_eq!(s.cmp(&Space::new(2)), Ordering::Less);
    let tvar1 = tvar.clone();
    spawn(move || {
        tvar1.atomic_write(5);
    })
    .join()
    .unwrap();
    assert_eq!(*tvar.atomic_read().downcast_ref::<i32>().unwrap(), 5);
}
