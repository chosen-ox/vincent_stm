pub mod log_vars;

use std::any::Any;
use std::sync::Arc;
use std::collections::BTreeMap;

use self::log_vars::LogVar;

pub struct Transaction {
    vars: BTreeMap<Arc<dyn Any + Send + Sync>, LogVar>
}