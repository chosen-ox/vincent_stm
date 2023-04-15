use crate::ArcAny;
use std::sync::Arc;
use LogVar::*;

#[derive(Clone)]
pub enum LogVar {
    // val, version
    Read(ArcAny, Arc<u8>),
    Write(ArcAny),
    ReadWrite(ArcAny, ArcAny, Arc<u8>),
}

impl LogVar {
    // `get_version` is not used currently.
    // pub fn get_version(&self) -> usize {
    //     match self {
    //         Read(_, v) | Write(_, v) | ReadWrite(_, _, v) => *v,
    //     }
    // }

    pub fn write(&mut self, val: ArcAny) {
        *self = match self.clone() {
            Read(r, v) | ReadWrite(r, _, v) => ReadWrite(r, val, v),
            Write(_) => Write(val),
        };
    }

    pub fn read(&self) -> ArcAny {
        match &*self {
            &Read(ref r, _) => r.clone(),
            &Write(ref w) | &ReadWrite(_, ref w, _) => w.clone(),
        }
    }
}
