#[allow(unused_imports)]
#[macro_use]
extern crate oso_derive;

#[doc(hidden)]
pub use oso_derive::*;

use rcore::command::{CommandContext, IoContext, Shell, UserContext};
use rcore::command::oso::Class;
use rcore::command::oso::PolarClass;
