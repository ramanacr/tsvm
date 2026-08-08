#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum InteropValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object(BTreeMap<String, InteropValue>),
    Array(Vec<InteropValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropError {
    pub message: String,
}

impl InteropError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub type HostFunction = fn(&[InteropValue]) -> Result<InteropValue, InteropError>;

#[derive(Clone, Default)]
pub struct HostEnvironment {
    functions: BTreeMap<String, HostFunction>,
}

impl HostEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_function(mut self, name: impl Into<String>, function: HostFunction) -> Self {
        self.functions.insert(name.into(), function);
        self
    }

    pub fn insert_function(&mut self, name: impl Into<String>, function: HostFunction) {
        self.functions.insert(name.into(), function);
    }

    pub fn call(
        &self,
        name: &str,
        args: &[InteropValue],
    ) -> Option<Result<InteropValue, InteropError>> {
        self.functions.get(name).map(|function| function(args))
    }

    pub fn contains_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }
}
