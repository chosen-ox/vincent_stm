use std::sync::{Arc, RwLock};

pub struct Space {
    pub version: RwLock<Arc<u8>>,
    pub id: usize,
}

impl Space {
    pub fn new(id: usize) -> Arc<Space> {
        if id == 0 {
            panic!("Space id can not be 0!");
        }
        let space = Space {
            version: RwLock::new(Arc::new(0)),
            id,
        };
        Arc::new(space)
    }
    pub fn new_single_var_space() -> Space {
        Space {
            version: RwLock::new(Arc::new(0)),
            id: 0,
        }
    }

    pub fn get_id(&self) -> usize {
        self.id
    }

    pub fn read_version(&self) -> Arc<u8> {
        self.version.read().unwrap().clone()
    }

    pub fn get_address(&self) -> usize {
        self as *const Space as usize
    }
}
