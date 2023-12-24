use std::fmt::Formatter;
use derive_more::{Constructor, Display};
use serde::Serializer;
use serde_derive::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Constructor)]
pub struct VecLengthAssertionError<T> {
    vec: Vec<T>,
    expected_length: usize,
}

#[derive(Debug, Display, Serialize, Deserialize, Error)]
pub struct CodeStringLengthError;

#[derive(Debug, Display, Error)]
pub struct TypeConversionError(Box<dyn std::error::Error + Send + Sync + 'static>);

#[derive(Debug, Display, Error)]
pub struct EnumUnspecifiedValue;


// IMPLEMENTATIONS


impl <T: ToString> std::fmt::Display for VecLengthAssertionError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let vec = self.vec.iter()
            .map(T::to_string)
            .collect::<Vec<String>>()
            .join(", ");
        f.write_fmt(format_args!("VecLengthAssertionError({}): {vec}", self.expected_length))
    }
}

impl serde::Serialize for TypeConversionError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.to_string().serialize(serializer)
    }
}

impl TypeConversionError {
    pub fn new(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}
