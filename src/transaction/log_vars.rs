use crate::ArcAny;
use LogVar::*;

#[derive(Clone)]
pub enum LogVar {
    // val, version
    Read(ArcAny, usize),
    Write(ArcAny),
    ReadWrite(ArcAny, ArcAny, usize),
}

impl LogVar {
    // `get_version` is not used currently.
    // pub fn get_version(&self) -> usize {
    //     match self {
    //         Read(_, v) | Write(_, v) | ReadWrite(_, _, v) => *v,
    //     }
    // }

    pub fn write(&mut self, val: ArcAny) -> Result<usize, usize> {
        *self = match self.clone() {
            Read(r, v) | ReadWrite(r, _, v) => ReadWrite(r, val, v),
            Write(_) => Write(val),
        };

        Ok(0)
    }

    pub fn read(&self) -> Result<ArcAny, usize> {
        match &*self {
            &Read(ref r, _) => Ok(r.clone()),
            &Write(ref w) | &ReadWrite(_, ref w, _) => Ok(w.clone()),
        }
    }
}
