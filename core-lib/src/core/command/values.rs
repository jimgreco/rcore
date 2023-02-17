/// Conversions to/from value types as represented natively in Rust (i.e., i64) and in the command
/// system (i.e., CommandValue).
///
/// This is largely derived from the oso project: https://github.com/osohq/oso
///
use std::collections::{BinaryHeap, BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::fmt::{Display, Formatter};
use std::hash::Hash;

use impl_trait_for_tuples::*;

//
// Common
//

mod private {
    pub trait FromSealed {}
    pub trait ToSealed {}
}

pub type CommandValueResult<T> = Result<T, CommandValueError>;

#[derive(Debug)]
pub struct CommandValueTypeError {
    pub got: Option<String>,
    pub expected: String,
}

impl Display for CommandValueTypeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(ref got) = self.got {
            writeln!(f, "Type error: Expected {} got {}", self.expected, got)
        } else {
            writeln!(f, "Type error: Expected {}.", self.expected)
        }
    }
}

impl CommandValueTypeError {
    /// Create a type error with expected type `expected`.
    pub fn expected<T: Into<String>>(expected: T) -> Self {
        Self {
            got: None,
            expected: expected.into(),
        }
    }

    /// Set `got` on self.
    pub fn got<T: Into<String>>(mut self, got: T) -> Self {
        self.got.replace(got.into());
        self
    }

    /// Convert `self` into `OsoError`, indicating a user originating type error.
    /// For example, calling a method with a parameter of an incorrect type from within Polar.
    pub fn user(self) -> CommandValueError {
        CommandValueError::TypeError(self)
    }
}

#[derive(Debug)]
pub enum CommandValueError {
    ApplicationError {
        source: Box<dyn std::error::Error + 'static + Send + Sync>,
        type_name: Option<String>,
        attr: Option<String>,
    },
    FromCommandValue,
    TypeError(CommandValueTypeError)
}

//
// Values
//

/// An enum of the possible value types that can be
/// sent to/from the Core.
#[derive(Clone, Debug)]
pub enum CommandValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Map(HashMap<String, CommandValue>),
    List(Vec<CommandValue>),
    Variable(String),
    //Instance(Instance),
}

impl PartialEq for CommandValue {
    fn eq(&self, other: &CommandValue) -> bool {
        match (self, other) {
            (CommandValue::Boolean(b1), CommandValue::Boolean(b2)) => b1 == b2,
            (CommandValue::Float(f1), CommandValue::Float(f2)) => f1 == f2,
            (CommandValue::Integer(i1), CommandValue::Integer(i2)) => i1 == i2,
            (CommandValue::List(l1), CommandValue::List(l2)) => l1 == l2,
            (CommandValue::Map(m1), CommandValue::Map(m2)) => m1 == m2,
            (CommandValue::String(s1), CommandValue::String(s2)) => s1 == s2,
            _ => false,
        }
    }
}

//
// From Core
//

pub trait FromCommandValue: Clone {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self>;
}

impl FromCommandValue for CommandValue {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        Ok(val)
    }
}

macro_rules! command_value_to_int {
    ($i:ty) => {
        impl FromCommandValue for $i {
            fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
                if let CommandValue::Integer(i) = val {
                    <$i>::try_from(i).map_err(|_| CommandValueError::FromCommandValue)
                } else {
                    Err(CommandValueTypeError::expected("Integer").user())
                }
            }
        }
    };
}

command_value_to_int!(u8);
command_value_to_int!(i8);
command_value_to_int!(u16);
command_value_to_int!(i16);
command_value_to_int!(u32);
command_value_to_int!(i32);
command_value_to_int!(i64);

/*
impl<T> FromCore for T
    where
        T: 'static + Clone + CoreClass,
{
    fn from_command_value(val: CoreValue) -> CoreResult<Self> {
        if let CoreValue::Instance(instance) = val {
            Ok(instance.downcast::<T>(None).map_err(|e| e.user())?.clone())
        } else {
            Err(TypeError::expected("Instance").user())
        }
    }
}
 */

impl FromCommandValue for f64 {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::Float(f) = val {
            Ok(f)
        } else {
            Err(CommandValueTypeError::expected("Float").user())
        }
    }
}

impl FromCommandValue for String {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::String(s) = val {
            Ok(s)
        } else {
            Err(CommandValueTypeError::expected("String").user())
        }
    }
}

impl FromCommandValue for bool {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::Boolean(b) = val {
            Ok(b)
        } else {
            Err(CommandValueTypeError::expected("Boolean").user())
        }
    }
}

impl<T: FromCommandValue> FromCommandValue for HashMap<String, T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::Map(map) = val {
            let mut result = HashMap::new();
            for (k, v) in map {
                let val = T::from_command_value(v)?;
                result.insert(k, val);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("Map").user())
        }
    }
}

impl<T: FromCommandValue> FromCommandValue for BTreeMap<String, T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::Map(map) = val {
            let mut result = BTreeMap::new();
            for (k, v) in map {
                let val = T::from_command_value(v)?;
                result.insert(k, val);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("Map").user())
        }
    }
}

impl<T: FromCommandValue> FromCommandValue for Vec<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = vec![];
            for v in l {
                result.push(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

impl<T: FromCommandValue> FromCommandValue for LinkedList<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = LinkedList::new();
            for v in l {
                result.push_back(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

impl<T: FromCommandValue> FromCommandValue for VecDeque<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = VecDeque::new();
            for v in l {
                result.push_back(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

impl<T: Eq + Hash + FromCommandValue> FromCommandValue for HashSet<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = HashSet::new();
            for v in l {
                result.insert(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

impl<T: Eq + Ord + FromCommandValue> FromCommandValue for BTreeSet<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = BTreeSet::new();
            for v in l {
                result.insert(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

impl<T: Ord + FromCommandValue> FromCommandValue for BinaryHeap<T> {
    fn from_command_value(val: CommandValue) -> CommandValueResult<Self> {
        if let CommandValue::List(l) = val {
            let mut result = BinaryHeap::new();
            for v in l {
                result.push(T::from_command_value(v)?);
            }
            Ok(result)
        } else {
            Err(CommandValueTypeError::expected("List").user())
        }
    }
}

/*
impl<T: FromCore> FromCore for Option<T> {
    fn from_command_value(val: CoreValue) -> CoreResult<Self> {
        // if the value is a Option<CoreValue>, convert from CoreValue
        if let CoreValue::Instance(ref instance) = &val {
            if let Ok(opt) = instance.downcast::<Option<CoreValue>>(None) {
                return opt.clone().map(T::from_command_value).transpose();
            }
        }
        T::from_command_value(val).map(Some)
    }
}
 */

// well, you can't do this
// impl<U: FromCore> TryFrom<U> for CoreValue {
//     type Error = CoreError;

//     fn try_from(v: CoreValue) -> Result<Self, Self::Error> {
//         U::from_command_value(v)
//     }
// }

// so I have to do this
macro_rules! try_from_command_value {
    ($i:ty) => {
        impl TryFrom<CommandValue> for $i {
            type Error = CommandValueError;

            fn try_from(v: CommandValue) -> Result<Self, Self::Error> {
                Self::from_command_value(v)
            }
        }
    };
}

try_from_command_value!(u8);
try_from_command_value!(i8);
try_from_command_value!(u16);
try_from_command_value!(i16);
try_from_command_value!(u32);
try_from_command_value!(i32);
try_from_command_value!(i64);
try_from_command_value!(f64);
try_from_command_value!(String);
try_from_command_value!(bool);

impl<T: FromCommandValue> TryFrom<CommandValue> for HashMap<String, T> {
    type Error = CommandValueError;

    fn try_from(v: CommandValue) -> Result<Self, Self::Error> {
        Self::from_command_value(v)
    }
}

impl<T: FromCommandValue> TryFrom<CommandValue> for Vec<T> {
    type Error = CommandValueError;

    fn try_from(v: CommandValue) -> Result<Self, Self::Error> {
        Self::from_command_value(v)
    }
}

#[impl_for_tuples(16)]
#[tuple_types_custom_trait_bound(FromCommandValue)]
impl private::FromSealed for Tuple {}

pub trait FromCommandList: private::FromSealed {
    fn from_command_value_list(values: &[CommandValue]) -> CommandValueResult<Self>
        where
            Self: Sized;
}

/*
impl FromCore for Instance {
    fn from_command_value(value: CoreValue) -> CoreResult<Self> {
        // We need to handle converting all value variants to an
        // instance so that we can use the `Class` mechanism to
        // handle methods on them
        let instance = match value {
            CoreValue::Boolean(b) => Instance::new(b),
            CoreValue::Integer(i) => Instance::new(i),
            CoreValue::Float(f) => Instance::new(f),
            CoreValue::List(v) => Instance::new(v),
            CoreValue::String(s) => Instance::new(s),
            CoreValue::Map(d) => Instance::new(d),
            CoreValue::Instance(instance) => instance,
            v => {
                tracing::warn!(value = ?v, "invalid conversion attempted");
                return Err(CoreError::FromCore);
            }
        };
        Ok(instance)
    }
}
 */

#[impl_for_tuples(16)]
#[tuple_types_custom_trait_bound(FromCommandValue)]
impl FromCommandList for Tuple {
    fn from_command_value_list(values: &[CommandValue]) -> CommandValueResult<Self> {
        let mut iter = values.iter();
        let result = Ok((for_tuples!(
            #( Tuple::from_command_value(iter.next().ok_or(
                // TODO better error type
                CommandValueError::FromCommandValue
            )?.clone())? ),*
        )));

        if iter.len() > 0 {
            // TODO (dhatch): Debug this!!!
            tracing::warn!("Remaining items in iterator after conversion.");
            for item in iter {
                tracing::trace!("Remaining item {:?}", item);
            }

            return Err(CommandValueError::FromCommandValue);
        }

        result
    }
}

//
// ToCore
//

pub trait ToCommandValue {
    fn to_command_value(self) -> CommandValue;
}

/*
impl<C: crate::CoreClass + Send + Sync> ToCore for C {
    fn to_command_value(self) -> CoreValue {
        let registered = DEFAULT_CLASSES
            .read()
            .unwrap()
            .get(&std::any::TypeId::of::<C>())
            .is_some();

        if !registered {
            DEFAULT_CLASSES
                .write()
                .unwrap()
                .entry(std::any::TypeId::of::<C>())
                .or_insert_with(C::get_command_value_class);
        }

        CoreValue::new_from_instance(self)
    }
}
 */

pub trait ToCommandResult {
    fn to_command_value_result(self) -> CommandValueResult<CommandValue>;
}

impl<R: ToCommandValue> ToCommandResult for R {
    fn to_command_value_result(self) -> CommandValueResult<CommandValue> {
        Ok(self.to_command_value())
    }
}

impl<E: std::error::Error + Send + Sync + 'static, R: ToCommandValue> ToCommandResult for Result<R, E> {
    fn to_command_value_result(self) -> CommandValueResult<CommandValue> {
        self.map(|r| r.to_command_value())
            .map_err(|e| CommandValueError::ApplicationError {
                source: Box::new(e),
                attr: None,
                type_name: None,
            })
    }
}

pub trait ToCommandValueList: private::ToSealed {
    fn to_command_value_list(self) -> Vec<CommandValue>
        where
            Self: Sized;
}

impl ToCommandValueList for () {
    fn to_command_value_list(self) -> Vec<CommandValue> {
        Vec::new()
    }
}

#[impl_for_tuples(1, 16)]
#[tuple_types_custom_trait_bound(ToCommandValue)]
#[allow(clippy::vec_init_then_push)]
impl ToCommandValueList for Tuple {
    fn to_command_value_list(self) -> Vec<CommandValue> {
        let mut result = Vec::new();
        for_tuples!(
            #( result.push(self.Tuple.to_command_value()); )*
        );
        result
    }
}

impl ToCommandValue for bool {
    fn to_command_value(self) -> CommandValue {
        CommandValue::Boolean(self)
    }
}

macro_rules! int_to_command_value {
    ($i:ty) => {
        impl ToCommandValue for $i {
            fn to_command_value(self) -> CommandValue {
                CommandValue::Integer(self.into())
            }
        }
    };
}

int_to_command_value!(u8);
int_to_command_value!(i8);
int_to_command_value!(u16);
int_to_command_value!(i16);
int_to_command_value!(u32);
int_to_command_value!(i32);
int_to_command_value!(i64);

macro_rules! float_to_command_value {
    ($i:ty) => {
        impl ToCommandValue for $i {
            fn to_command_value(self) -> CommandValue {
                CommandValue::Float(self.into())
            }
        }
    };
}

float_to_command_value!(f32);
float_to_command_value!(f64);

impl ToCommandValue for String {
    fn to_command_value(self) -> CommandValue {
        CommandValue::String(self)
    }
}

impl<'a> ToCommandValue for &'a str {
    fn to_command_value(self) -> CommandValue {
        CommandValue::String(self.to_string())
    }
}

impl<T: ToCommandValue> ToCommandValue for Vec<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for VecDeque<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for LinkedList<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for HashSet<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for BTreeSet<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for BinaryHeap<T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.into_iter().map(|v| v.to_command_value()).collect())
    }
}

impl<'a, T: Clone + ToCommandValue> ToCommandValue for &'a [T] {
    fn to_command_value(self) -> CommandValue {
        CommandValue::List(self.iter().cloned().map(|v| v.to_command_value()).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for HashMap<String, T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::Map(self.into_iter().map(|(k, v)| (k, v.to_command_value())).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for HashMap<&str, T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::Map(
            self.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_command_value()))
                .collect(),
        )
    }
}

impl<T: ToCommandValue> ToCommandValue for BTreeMap<String, T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::Map(self.into_iter().map(|(k, v)| (k, v.to_command_value())).collect())
    }
}

impl<T: ToCommandValue> ToCommandValue for BTreeMap<&str, T> {
    fn to_command_value(self) -> CommandValue {
        CommandValue::Map(
            self.into_iter()
                .map(|(k, v)| (k.to_string(), v.to_command_value()))
                .collect(),
        )
    }
}

impl ToCommandValue for CommandValue {
    fn to_command_value(self) -> CommandValue {
        self
    }
}

/*
impl<T: ToCore> ToCore for Option<T> {
    fn to_command_value(self) -> CoreValue {
        CoreValue::new_from_instance(self.map(|t| t.to_command_value()))
    }
}
 */

pub struct CommandValueIterator(pub Box<dyn CommandValueResultIter>);

impl CommandValueIterator {
    pub fn new<I: CommandValueResultIter + 'static>(iter: I) -> Self {
        Self(Box::new(iter))
    }
}

impl Iterator for CommandValueIterator {
    type Item = CommandValueResult<CommandValue>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Clone for CommandValueIterator {
    fn clone(&self) -> Self {
        Self(self.0.box_clone())
    }
}

/*
impl crate::CoreClass for CoreIterator {
    fn get_command_value_class_builder() -> crate::ClassBuilder<Self> {
        crate::Class::builder::<Self>().with_iter()
    }
}
 */

pub trait CommandValueResultIter: Send + Sync {
    fn box_clone(&self) -> Box<dyn CommandValueResultIter>;
    fn next(&mut self) -> Option<CommandValueResult<CommandValue>>;
}

impl<I, V> CommandValueResultIter for I
    where
        I: Iterator<Item = V> + Clone + Send + Sync + 'static,
        V: ToCommandResult,
{
    fn box_clone(&self) -> Box<dyn CommandValueResultIter> {
        Box::new(self.clone())
    }

    fn next(&mut self) -> Option<CommandValueResult<CommandValue>> {
        Iterator::next(self).map(|v| v.to_command_value_result())
    }
}

#[impl_for_tuples(16)]
#[tuple_types_custom_trait_bound(ToCommandValue)]
impl private::ToSealed for Tuple {}

//
// Tests
//

#[cfg(test)]
mod tests {
    use std::i64;
    use crate::core::command::values::{CommandValue, FromCommandValue, ToCommandResult, ToCommandValue};

    #[test]
    fn bool_to_command_value_equality() {
        let val1 = true.to_command_value();
        let val2 = true.to_command_value();

        assert_eq!(val1, val2);
    }

    #[test]
    fn bool_to_command_value_not_equal() {
        let val1 = true.to_command_value();
        let val2 = false.to_command_value();

        assert_ne!(val1, val2);
    }

    #[test]
    fn i32_to_command_value_equality() {
        let val1 = 42.to_command_value();
        let val2 = 42.to_command_value();

        assert_eq!(val1, val2);
    }

    #[test]
    fn i32_to_command_value_not_equal() {
        let val1 = 42.to_command_value();
        let val2 = 32.to_command_value();

        assert_ne!(val1, val2);
    }

    #[test]
    fn f64_to_command_value_equality() {
        let val1 = 4.2.to_command_value();
        let val2 = 4.2.to_command_value();

        assert_eq!(val1, val2);
    }

    #[test]
    fn f64_to_command_value_not_equal() {
        let val1 = 4.2.to_command_value();
        let val2 = 3.2.to_command_value();

        assert_ne!(val1, val2);
    }

    #[test]
    fn String_to_command_value_equality() {
        let val1 = "foo".to_command_value();
        let val2 = "foo".to_command_value();

        assert_eq!(val1, val2);
    }

    #[test]
    fn String_to_command_value_not_equal() {
        let val1 = "foo".to_command_value();
        let val2 = "bar".to_command_value();

        assert_ne!(val1, val2);
    }

    #[test]
    fn bool_from_command_value_equality() {
        let val1 = bool::from_command_value(true.to_command_value()).unwrap();
        let val2 = bool::from_command_value(true.to_command_value()).unwrap();

        assert_eq!(val1, val2);
    }

    #[test]
    fn bool_from_command_value_not_equal() {
        let val1 = bool::from_command_value(true.to_command_value()).unwrap();
        let val2 = bool::from_command_value(false.to_command_value()).unwrap();

        assert_ne!(val1, val2);
    }

    #[test]
    fn i32_from_command_value_equality() {
        let val1 = i32::from_command_value(42.to_command_value()).unwrap();
        let val2 = i32::from_command_value(42.to_command_value()).unwrap();

        assert_eq!(val1, val2);
    }

    #[test]
    fn i32_from_command_value_not_equal() {
        let val1 = i32::from_command_value(42.to_command_value()).unwrap();
        let val2 = i32::from_command_value(32.to_command_value()).unwrap();

        assert_ne!(val1, val2);
    }

    #[test]
    fn f64_from_command_value_equality() {
        let val1 = f64::from_command_value(4.2.to_command_value()).unwrap();
        let val2 = f64::from_command_value(4.2.to_command_value()).unwrap();

        assert_eq!(val1, val2);
    }

    #[test]
    fn f64_from_command_value_not_equal() {
        let val1 = f64::from_command_value(4.2.to_command_value()).unwrap();
        let val2 = f64::from_command_value(3.2.to_command_value()).unwrap();

        assert_ne!(val1, val2);
    }

    #[test]
    fn String_from_command_value_equality() {
        let val1 = String::from_command_value("foo".to_command_value()).unwrap();
        let val2 = String::from_command_value("foo".to_command_value()).unwrap();

        assert_eq!(val1, val2);
    }

    #[test]
    fn String_from_command_value_not_equal() {
        let val1 = String::from_command_value("foo".to_command_value()).unwrap();
        let val2 = String::from_command_value("bar".to_command_value()).unwrap();

        assert_ne!(val1, val2);
    }
}
