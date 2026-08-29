#![forbid(unsafe_op_in_unsafe_fn)]

mod catalog;
mod description;
mod diagnostics;
mod validation;

pub(crate) use validation::validate_runtime_service_contracts;
