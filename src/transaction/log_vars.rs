use std::any::Any;
use std::sync::Arc;

pub type ArcAny = Arc<dyn Any + Send + Sync>;

pub enum LogVar {
    Read(ArcAny),
    Write(ArcAny, ArcAny),
    ReadWrite(ArcAny, ArcAny),
}