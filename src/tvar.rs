use std::any::Any;
use super::space::Space;
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::spawn;

#[derive(Clone)]
pub struct Tvar<T> {
    value: Arc<Mutex<T>>,
    space: Option<Space>,
}

impl<T> Tvar<T>
where
    T: Any + Send + Sync + Clone
{
    pub fn new(value: T) -> Tvar<T> {
        Tvar {
            value: Arc::new(Mutex::new(value)),
            space: None,
        }
    }

    pub fn new_with_space(value: T, space: Space) -> Tvar<T> {
        Tvar {
            value: Arc::new(Mutex::new(value)),
            space: Some(space),
        }
    }

    pub fn set_space(&mut self, space: Space) {
        self.space = Some(space);
    }

    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.value.lock().unwrap().clone()
    }

    pub fn write(&self, value: T) {
        *self.value.lock().unwrap() = value;
    }
}

#[cfg(test)]
#[test]
fn test_tvar() {
    let mut tvar = Tvar::new(0);
    assert_eq!(tvar.read(), 0);
    let space = Space::new(2);
    space.add_var(&mut tvar);
    if let Some(ref space) = tvar.space {
        assert_eq!(space.cmp(&Space::new(1)), Ordering::Greater);
    } else {
        panic!("no value in tvar")
    }
    let tvar1 = tvar.clone();
    spawn(move || {
        tvar1.write(5);
    })
    .join()
    .unwrap();
    assert_eq!(tvar.read(), 5);
}
