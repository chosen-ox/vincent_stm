use crate::ArcAny;
use LogVar::*;

#[derive(Clone)]
pub enum LogVar {
    // val, version
    Read(ArcAny, usize),
    Write(ArcAny, usize),
    ReadWrite(ArcAny, ArcAny, usize),
}

impl LogVar {

    // `get_version` is not used currently.
    // pub fn get_version(&self) -> usize {
    //     match self {
    //         Read(_, v) | Write(_, v) | ReadWrite(_, _, v) => *v,
    //     }
    // }

    pub fn write(&mut self, val: ArcAny, version: usize) -> Result<usize, usize> {
        *self = match self.clone() {
            Read(r, v) | ReadWrite(r, _, v) => {
                if version != v {
                    return Err(v);
                }
                ReadWrite(r, val, v)
            }
            Write(_, v) => {
                if version != v {
                    return Err(v);
                }
                Write(val, v)
            }
        };

        Ok(0)
    }

    pub fn read(&mut self, version: usize) -> Result<ArcAny, usize> {
        match &*self {
            &Read(ref r, v) => {
                if version != v {
                    return Err(v);
                }
                Ok(r.clone())
            }
            &Write(ref w, v) | &ReadWrite(_, ref w, v) => {
                if version != v {
                    return Err(v);
                }
                Ok(w.clone())
            }
        }
    }
}
