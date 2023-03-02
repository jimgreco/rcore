pub mod command;

#[allow(unused_imports)]
#[macro_use]
extern crate oso_derive;

#[doc(hidden)]
pub use oso_derive::*;

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}