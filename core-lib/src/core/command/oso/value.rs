use std::collections::hash_map::HashMap;

use super::Instance;

/// An enum of the possible value types that can be
/// sent to/from Polar.
///
/// All variants except `Instance` represent types that can
/// be used natively in Polar.
/// Any other types can be wrapped using `PolarValue::new_from_instance`.
/// If the instance has a registered `Class`, then this can be used
/// from the policy too.
#[derive(Clone, Debug)]
pub enum PolarValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Map(HashMap<String, PolarValue>),
    List(Vec<PolarValue>),
    Instance(Instance),
}

impl PartialEq for PolarValue {
    fn eq(&self, other: &PolarValue) -> bool {
        match (self, other) {
            (PolarValue::Boolean(b1), PolarValue::Boolean(b2)) => b1 == b2,
            (PolarValue::Float(f1), PolarValue::Float(f2)) => f1 == f2,
            (PolarValue::Integer(i1), PolarValue::Integer(i2)) => i1 == i2,
            (PolarValue::List(l1), PolarValue::List(l2)) => l1 == l2,
            (PolarValue::Map(m1), PolarValue::Map(m2)) => m1 == m2,
            (PolarValue::String(s1), PolarValue::String(s2)) => s1 == s2,
            _ => false,
        }
    }
}

impl PolarValue {
    /// Create a `PolarValue::Instance` from any type.
    pub fn new_from_instance<T>(instance: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self::Instance(Instance::new(instance))
    }
}

#[derive(Debug, Clone)]
pub enum ParamType {
    Integer,
    Float,
    String,
    Boolean,
    Instance
}