use std::any::Any;
use std::sync::Arc;

use crate::ArcAny;

#[derive(Clone)]
pub enum LogVar {
    Read(ArcAny),
    Write(ArcAny),
    ReadWrite(ArcAny, ArcAny),
}

impl LogVar {
    pub fn write(&mut self, val: ArcAny) {
        *self = match self.clone() {
            LogVar::Read(r) | LogVar::ReadWrite(r, _)=> LogVar::ReadWrite(r, val),
            LogVar::Write(_) => LogVar::Write(val),
        }
    }

    pub fn read(&mut self) -> ArcAny {
        match self {
            LogVar::Read(r) => r.clone(),
            LogVar::Write(w) => w.clone(),
            LogVar::ReadWrite(_, w) => w.clone(),
        }
    }
}
