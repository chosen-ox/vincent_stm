use std::any::Any;
use super::tvar::Tvar;
use std::cmp::Ordering;
use std::sync::{Arc, RwLock};
use std::thread::spawn;

#[derive(Clone)]
pub struct Space {
    version: Arc<RwLock<usize>>,
    id: usize,
}

impl Space {
    pub fn new(id: usize) -> Space {
        Space {
            version: Arc::new(RwLock::new(0)),
            id,
        }
    }

    pub fn read_version(&self) -> usize {
        *self.version.read().unwrap()
    }

    pub fn write_version(&self, version_id: usize) -> bool {
        let mut lock = self.version.write().unwrap();
        if *lock == version_id {
            *lock += 1;
            return true;
        }
        false
    }

    pub fn add_var<T: Any + Send + Sync + Clone>(&self, var: &mut Tvar<T>) {
        var.set_space(self.clone());
    }
}

impl Eq for Space {}

impl PartialEq for Space {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Space {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Space {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

#[cfg(test)]
#[test]
fn test_space() {
    let space = Space::new(0);
    assert_eq!(space.read_version(), 0);
    assert_eq!(space.write_version(0), true);
    let space1 = space.clone();
    spawn(move || {
        assert_eq!(space1.write_version(1), true);
        assert_eq!(space1.write_version(2), true);
    })
    .join()
    .unwrap();
    println!("space version: {}", space.read_version());
}
