//! oso policy engine for authorization
//!
//! # Overview
//!
//! oso is a policy engine for authorization that's embedded in your application.
//! It provides a declarative policy language for expressing authorization logic.
//! You define this logic separately from the rest of your application code,
//! but it executes inside the application and can call directly into it.
//!
//! For more information, guides on using oso, writing policies and adding to your
//! application, go to the [oso documentation](https://docs.osohq.com).
//!
//! For specific information on using with Rust, see the [Rust documentation](https://docs.osohq.com/using/libraries/rust/index.html).
//!
//! ## Note
//!
//! The oso Rust library is still in early development relative to the other
//! oso libraries.
//!
//! For more examples, see the [oso documentation](https://docs.osohq.com).
//!

pub(crate) mod builtins;
mod class;
mod class_method;
mod errors;
mod from_polar;
mod method;
mod to_polar;
mod value;

pub use class::{Class, ClassBuilder, Instance};
pub use class_method::{AttributeGetter, Constructor, InstanceMethod};
pub use errors::{InvalidCallError, OsoError, Result, TypeError};
pub use from_polar::{FromPolar, FromPolarList};
pub use to_polar::{PolarIterator, ToPolar, ToPolarList, ToPolarResult};
pub use value::PolarValue;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

lazy_static::lazy_static! {
    /// Map of classes that have been globally registered
    ///
    /// These will be used as a fallback, and cached on the host when an unknown instance is seen
    pub static ref DEFAULT_CLASSES: Arc<RwLock<HashMap<std::any::TypeId, Class>>> = Default::default();
}

/// Classes that can be used as types in Polar policies.
///
/// Implementing this trait and `Clone` automatically makes the
/// type `FromPolar` and `ToPolar`, so it can be used with
/// `Oso::is_allowed` calls.
///
/// The default implementation creates a class definition with
/// no attributes or methods registered. Either use `get_polar_class_builder`
/// or the `#[derive(PolarClass)]` proc macro to register attributes and methods.
///
/// **Note** that the returned `Class` still must be registered on an `Oso`
/// instance using `Oso::register_class`.
pub trait PolarClass: Sized + 'static {
    /// Returns the `Class` ready for registration
    fn get_polar_class() -> Class {
        Self::get_polar_class_builder().build()
    }

    /// Returns the partially defined `Class` for this type.
    ///
    /// Can still have methods added to it with `add_method`, and attributes
    /// with `add_attribute_getter`.
    /// Use `Class::build` to finish defining the type.
    fn get_polar_class_builder() -> ClassBuilder<Self> {
        Class::builder()
    }
}

impl PolarClass for Class {}

fn metaclass() -> Class {
    Class::builder::<Class>().name("oso::host::Class").build()
}

/// Maintain mappings and caches for Rust classes & instances
#[derive(Clone)]
pub struct Host {
    /// Map from names to `Class`s
    classes: HashMap<String, Class>,

    class_name_to_fq_name: HashMap<String, String>,

    /// Map from type IDs, to class names
    /// This helps us go from a generic type `T` to the
    /// class name it is registered as
    class_names: HashMap<std::any::TypeId, String>,

    pub accept_expression: bool,
}

impl Host {
    pub fn new() -> Self {
        let mut host = Self {
            class_name_to_fq_name: HashMap::new(),
            class_names: HashMap::new(),
            classes: HashMap::new(),
            accept_expression: false,
        };
        let type_class = metaclass();
        host.cache_class(type_class)
            .expect("could not register the metaclass");
        host
    }

    pub fn get_class(&self, name: &str) -> Result<&Class> {
        match self.classes.get(name) {
            Some(class) => Ok(class),
            None => match self.class_name_to_fq_name.get(name) {
                None => {
                    return Err(OsoError::MissingClassError {
                        name: name.to_string(),
                    })
                }
                Some(fq_name) => match self.classes.get(fq_name) {
                    None => Err(OsoError::MissingClassError {
                        name: fq_name.to_string(),
                    }),
                    Some(class) => Ok(class),
                },
            },
        }
    }

    pub fn get_class_by_type_id(&self, id: std::any::TypeId) -> Result<&Class> {
        self.class_names
            .get(&id)
            .ok_or_else(|| OsoError::MissingClassError {
                name: format!("TypeId: {:?}", id),
            })
            .and_then(|name| self.get_class(name))
    }

    pub fn get_class_mut(&mut self, name: &str) -> Result<&mut Class> {
        self.classes
            .get_mut(name)
            .ok_or_else(|| OsoError::MissingClassError {
                name: name.to_string(),
            })
    }

    /// Add the class to the host classes
    ///
    /// Returns an instance of `Type` for this class.
    pub fn cache_class(&mut self, class: Class) -> Result<()> {
        // Insert into default classes here so that we don't repeat this the first
        // time we see an instance.
        DEFAULT_CLASSES
            .write()
            .unwrap()
            .entry(class.type_id)
            .or_insert_with(|| class.clone());

        if self.classes.contains_key(&class.fq_name) {
            Err(OsoError::DuplicateClassError {
                name: class.fq_name.clone(),
            })
        } else {
            self.class_names
                .insert(class.type_id, class.fq_name.clone());

            // only insert the short name if it does not exist already
            if !self.class_name_to_fq_name.contains_key(&class.name) {
                self.class_name_to_fq_name
                    .insert(class.name.clone(), class.fq_name.clone());
            }

            self.classes.insert(class.fq_name.clone(), class);
            Ok(())
        }
    }

    pub fn isa(&self, value: &PolarValue, class_tag: &str) -> Result<bool> {
        let res = match value {
            PolarValue::Instance(instance) => {
                let class = self.get_class(class_tag)?;
                instance.instance_of(class)
            }
            PolarValue::Boolean(_) => class_tag == "bool",
            PolarValue::Integer(_) => class_tag == "int",
            PolarValue::Float(_) => class_tag == "float",
            PolarValue::String(_) => class_tag == "string",
            _ => false,
        };
        Ok(res)
    }
}
