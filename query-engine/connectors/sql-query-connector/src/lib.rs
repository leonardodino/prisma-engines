#![allow(clippy::wrong_self_convention)]

mod column_metadata;
mod cursor_condition;
mod database;
mod error;
mod filter_conversion;
mod ordering;
mod query_arguments_ext;
mod query_builder;
mod query_ext;
mod row;
mod sql_info;

use column_metadata::*;
use filter_conversion::*;
use query_ext::QueryExt;
use row::*;

pub use database::*;
pub use error::SqlError;

type Result<T> = std::result::Result<T, error::SqlError>;
