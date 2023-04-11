use std::cmp::Ordering;
use std::sync::{Arc, RwLock};
#[cfg(test)]
use std::thread::spawn;

#[derive(Clone)]
pub struct Space {
    pub version: Arc<RwLock<usize>>,
    id: usize,
}

impl Space {
    pub fn new(id: usize) -> Space {
        if id == 0 {
            panic!("Space id can not be 0!");
        }
        Space {
            version: Arc::new(RwLock::new(0)),
            id,
        }
    }
    pub fn new_single_var_space() -> Space {
        Space {
            version: Arc::new(RwLock::new(0)),
            id: 0,
        }
    }

    pub fn get_id(&self) -> usize {
        self.id
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
    let space = Space::new_single_var_space();
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
