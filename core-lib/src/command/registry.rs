use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::ptr::eq;
use std::str::FromStr;

extern crate rand;

use thiserror::Error;

use super::oso::{
    builtins, Class, Instance, OsoError, PolarValue, FromPolar, Host, ToPolar, Constructor,
    InstanceMethod,
};

#[derive(Debug)]
pub struct Path {
    id: usize,
    pub name: String,
    parent: Option<usize>,
    children: HashMap<String, usize>,
    pub full_path: String,
    instance: Option<Instance>,
    owner: Option<usize>,
    attr: Option<&'static str>,
    method: Option<&'static str>,
}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub struct Registry {
    host: Host,
    paths: HashMap<usize, Path>,
    root_id: usize,
}

impl Registry {
    pub fn new() -> Registry {
        let mut host = Host::new();
        for class in builtins::classes() {
            host.cache_class(class).expect("builtins failed");
        }

        let mut reg = Registry {
            host,
            paths: HashMap::new(),
            root_id: rand::random(),
        };
        reg.paths.insert(reg.root_id, Path {
            children: HashMap::new(),
            id: reg.root_id,
            instance: None,
            name: "".to_owned(),
            parent: None,
            full_path: "/".to_owned(),
            owner: None,
            method: None,
            attr: None,
        });
        reg
    }

    pub fn cache_class(&mut self, class: Class) -> Result<(), RegistryError> {
        let class_name = class.fq_name.to_owned();

        // check to ensure that attributes and instance methods don't conflict
        let mut name_conflict = HashSet::new();
        for attr in class.attributes.keys() {
            if !name_conflict.insert(attr) {
                return Err(RegistryError::ClassChildNameConflict {
                    class: class_name,
                    child: attr.to_string(),
                });
            }
        }
        for method in class.instance_methods.keys() {
            if !name_conflict.insert(method) {
                return Err(RegistryError::ClassChildNameConflict {
                    class: class_name,
                    child: method.to_string(),
                });
            }
        }

        self.host.cache_class(class).map_err(|_| RegistryError::DuplicateClass(class_name))
    }

    //
    // Utility methods
    //

    fn to_path_segments(pwd: &str, cd: &str) -> Result<Vec<String>, RegistryError> {
        // this method is inefficient, but the command system isn't designed for the critical path
        let mut segments: Vec<String> = Vec::new();

        let mut first = true;
        for segment in pwd.split("/") {
            if first && !segment.is_empty() || segment == "." || segment == ".." {
                return Err(RegistryError::IllegalPathNavigation {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    reason: "invalid path segment name",
                });
            } else if !segment.is_empty() {
                segments.push(segment.to_owned())
            }
            first = false;
        }

        let mut first = true;
        for segment in cd.split("/") {
            if first && segment.is_empty() {
                segments.clear();
            } else if segment == ".." {
                if segments.pop().is_none() {
                    return Err(RegistryError::IllegalPathNavigation {
                        pwd: pwd.to_owned(),
                        cd: cd.to_owned(),
                        reason: "navigation beyond root",
                    });
                }
            } else if !segment.is_empty() && segment != "." {
                segments.push(segment.to_owned());
            }
            first = false;
        }

        Ok(segments)
    }

    fn to_path_str(pwd: &str, cd: &str) -> Result<String, RegistryError> {
        let segments = Registry::to_path_segments(pwd, cd)?;
        if segments.is_empty() {
            Ok("/".to_owned())
        } else {
            let mut str = "".to_owned();
            for segment in segments {
                str.push_str("/");
                str.push_str(&segment);
            }
            Ok(str)
        }
    }

    fn to_type_strings(&self, value: &PolarValue) -> (&str, &str) {
        match value {
            PolarValue::Instance(instance) => {
                let clz = instance.class(&self.host).unwrap();
                (&clz.name, &clz.fq_name)
            }
            PolarValue::Boolean(_) => ("bool", "boolean"),
            PolarValue::Integer(_) => ("int", "integer"),
            PolarValue::Float(_) => ("float", "float"),
            PolarValue::String(_) => ("string", "string"),
            PolarValue::Map(_) => ("map", "dict"),
            PolarValue::List(_) => ("list", "vec")
        }
    }

    fn parse<T: FromStr>(param: &str,
                         class_name: &str,
                         method_name: &str,
                         param_index: usize,
                         param_type: &'static str) -> Result<T, RegistryError> {
        param.parse().map_err(|_| RegistryError::InvalidMethodParameter {
            class: class_name.to_owned(),
            method: method_name.to_owned(),
            param_index,
            param_type,
            reason: "could not parse from string",
        })
    }

    fn cast<T: 'static + FromPolar>(pwd: &str,
                                    cd: &str,
                                    cast_type: &'static str,
                                    result: PolarValue) -> Result<T, RegistryError> {
        T::from_polar(result).map_err(|e| {
            match e {
                OsoError::TypeError(e) => RegistryError::InvalidCast {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    cast_type,
                    expected: e.expected,
                    got: e.got.unwrap_or("".to_owned()),
                },
                _ => RegistryError::InvalidCast {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    cast_type,
                    expected: "".to_owned(),
                    got: "".to_owned(),
                }
            }
        })
    }

    //
    // Create paths
    //

    /// Creates a new directory for the specified working directory and change directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::command::Registry;
    /// let mut registry = Registry::new();
    /// registry.mkdir("/foo/bar", "soo");
    /// assert_eq!("/foo/bar/soo", registry.path("/foo/bar/soo").unwrap().full_path);
    /// ```
    pub fn mkdir(&mut self, pwd: &str, cd: &str) -> Result<(), RegistryError> {
        self.create_path(pwd, cd, true, None, None, None, None)
    }

    fn create_path(&mut self,
                   pwd: &str,
                   cd: &str,
                   fail_on_duplicate: bool,
                   instance: Option<Instance>,
                   owner: Option<usize>,
                   method: Option<&'static str>,
                   attr: Option<&'static str>) -> Result<(), RegistryError> {
        let mut pwd_node = self.paths.get(&self.root_id).unwrap();
        let mut created = false;

        let segments = Registry::to_path_segments(pwd, cd)?;
        for segment in segments {
            let mut found = false;

            for (child_name, child_id) in &pwd_node.children {
                // something is seriously wrong if unwrap fails
                if child_name == &segment {
                    pwd_node = self.paths.get(child_id).unwrap();
                    found = true;
                    break;
                }
            }

            if !found {
                let id = pwd_node.id;
                let full_path = pwd_node.full_path.to_owned();
                let pwd_id = self.create_child(id, full_path, &segment)?;
                pwd_node = self.paths.get(&pwd_id).unwrap();
                created = true;
            }
        }

        if created || (!fail_on_duplicate && pwd_node.instance.is_none()) {
            if instance.is_some() {
                let class = instance.as_ref().unwrap().class(&self.host).unwrap();
                let instance_methods = class.instance_methods.clone();
                let attributes = class.attributes.clone();

                let id = pwd_node.id;
                let path = self.paths.get_mut(&id).unwrap();
                path.owner = owner;
                path.method = method;
                path.attr = attr;
                path.instance = instance;

                let full_path = path.full_path.clone();
                let id = path.id;

                for (name, method) in instance_methods {
                    let command_path = method.path().unwrap_or(name);
                    self.create_path(
                        &full_path,
                        command_path,
                        true,
                        None,
                        Some(id),
                        Some(name),
                        None)?;
                }
                for (attr_name, _) in attributes {
                    // TODO: customize attribute path
                    let attr_path = attr_name;
                    self.create_path(
                        &full_path,
                        attr_path,
                        true,
                        None,
                        Some(id),
                        None,
                        Some(attr_name))?;
                }
            } else {
                let id = pwd_node.id;
                let path = self.paths.get_mut(&id).unwrap();
                path.owner = owner;
                path.method = method;
                path.attr = attr;
            }

            Ok(())
        } else {
            Err(RegistryError::DuplicatePath(Registry::to_path_str(pwd, cd).unwrap()))
        }
    }

    fn create_child(&mut self, node_id: usize, pwd: String, name: &str)
                    -> Result<usize, RegistryError> {
        let parent = self.paths.get_mut(&node_id).unwrap();
        if name.is_empty() || name == "." || name == ".." {
            Err(RegistryError::InvalidPathChildName {
                pwd,
                child: name.to_owned(),
            })
        } else {
            let path_copy = parent.full_path.clone();
            let child_id = rand::random();
            let child = Path {
                children: HashMap::new(),
                id: child_id,
                instance: None,
                name: name.to_owned(),
                parent: Some(parent.id),
                full_path: match parent.parent {
                    None => path_copy + name,
                    Some(_) => path_copy + "/" + name
                },
                owner: None,
                method: None,
                attr: None,
            };
            parent.children.insert(name.to_owned(), child_id);
            self.paths.insert(child_id, child);
            Ok(child_id)
        }
    }

    //
    // Navigate Paths
    //

    pub fn cd(&self, pwd: &str, cd: &str) -> Result<&Path, RegistryError> {
        let mut pwd_node = self.paths.get(&self.root_id).unwrap();

        let segments = Registry::to_path_segments(pwd, cd)?;
        for segment in segments {
            let mut found = false;

            for (child_name, child_id) in &pwd_node.children {
                if child_name == &segment {
                    pwd_node = self.paths.get(child_id).unwrap();
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(RegistryError::IllegalPathNavigation {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    reason: "unknown path",
                });
            }
        }

        Ok(pwd_node)
    }

    pub fn path(&self, pwd: &str) -> Result<&Path, RegistryError> {
        self.cd(pwd, ".")
    }

    //
    // Utility methods for dealing with constructor and method params
    //

    fn parse_params(&self,
                    class_name: &str,
                    method_name: &str,
                    args: &Vec<&str>,
                    param_types: &Vec<&'static str>) -> Result<Vec<PolarValue>, RegistryError> {
        if args.len() != param_types.len() {
            return Err(RegistryError::InvalidNumberOfMethodParameters {
                class: class_name.to_owned(),
                method: method_name.to_owned(),
                expected: param_types.len(),
                received: args.len(),
            });
        }

        let mut params: Vec<PolarValue> = Vec::new();
        for i in 0..param_types.len() {
            let arg = args[i];
            let pt = param_types[i];

            if pt == "bool" {
                params.push(Registry::parse::<bool>(
                    arg, class_name, method_name, i, pt)?.to_polar());
            } else if pt == "int" {
                params.push(Registry::parse::<i32>(
                    arg, class_name, method_name, i, pt)?.to_polar());
            } else if pt == "float" {
                params.push(Registry::parse::<f64>(
                    arg, class_name, method_name, i, pt)?.to_polar());
            } else if pt == "string" {
                params.push(PolarValue::String(arg.to_owned()));
            } else {
                let instance = self.instance(arg, ".")?;
                let class = instance.class(&self.host).unwrap();
                if pt == &class.name || pt == &class.fq_name {
                    params.push(PolarValue::Instance(instance.to_owned()))
                } else {
                    return Err(RegistryError::InvalidCast {
                        pwd: arg.to_string(),
                        cd: ".".to_string(),
                        cast_type: "",
                        expected: pt.to_string(),
                        got: class.fq_name.to_string(),
                    });
                }
            }
        }

        Ok(params)
    }

    fn validate_params(&self,
                       params: &Vec<PolarValue>,
                       class_name: &str,
                       method_name: &str,
                       param_types: &Vec<&'static str>) -> Result<(), RegistryError> {
        if params.len() != param_types.len() {
            return Err(RegistryError::InvalidNumberOfMethodParameters {
                class: class_name.to_owned(),
                method: method_name.to_owned(),
                expected: param_types.len(),
                received: params.len(),
            });
        }
        for i in 0..param_types.len() {
            let (expected1, expected2) = self.to_type_strings(&params[i]);
            let pt = param_types[i];
            if pt != expected1 && pt != expected2 {
                return Err(RegistryError::InvalidMethodParameter {
                    class: class_name.to_owned(),
                    method: method_name.to_owned(),
                    param_index: i,
                    param_type: pt,
                    reason: "param is of the wrong type",
                });
            }
        }
        Ok(())
    }

    //
    // Class components
    //

    fn class(&self, class_name: &str) -> Result<&Class, RegistryError> {
        self.host.get_class(class_name).map_err(
            |_| RegistryError::UnknownClass(class_name.to_owned()))
    }

    fn constructor(&self, class_name: &str) -> Result<&Constructor, RegistryError> {
        match &self.class(class_name)?.constructor {
            Some(c) => Ok(c),
            None => Err(RegistryError::NoConstructor(class_name.to_owned()))
        }
    }

    //
    // Create and get instances
    //

    pub fn create_instance(&mut self,
                           pwd: &str,
                           cd: &str,
                           class_name: &str,
                           params: Vec<PolarValue>) -> Result<(), RegistryError> {
        let param_types = self.constructor(class_name)?.get_param_types();
        self.validate_params(&params, class_name, "<constructor>", param_types)?;
        self._create_instance(pwd, cd, class_name, params)
    }

    pub fn parsed_create_instance(&mut self,
                                  pwd: &str,
                                  cd: &str,
                                  class_name: &str,
                                  args: &Vec<&str>) -> Result<(), RegistryError> {
        let param_types = self.constructor(class_name)?.get_param_types();
        let params = self.parse_params(class_name, "<constructor>", args, param_types)?;
        self._create_instance(pwd, cd, class_name, params)
    }

    fn _create_instance(&mut self,
                        pwd: &str,
                        cd: &str,
                        class_name: &str,
                        params: Vec<PolarValue>) -> Result<(), RegistryError> {
        let constructor = self.constructor(class_name)?;
        let instance = constructor.invoke(params)
            .map_err(|e| RegistryError::InvocationFailure {
                pwd: pwd.to_owned(),
                cd: cd.to_owned(),
                class: class_name.to_owned(),
                method: "<constructor>".to_owned(),
                invocation_type: "constructor",
                reason: "constructor invocation failure",
                error: e,
            })?;
        self.create_path(pwd, cd, false, Some(instance), None, None, None)
    }

    pub fn instance_value<T: 'static>(&self, pwd: &str, cd: &str) -> Result<&T, RegistryError> {
        let instance = self.instance(pwd, cd)?;
        match instance.downcast::<T>(Some(&self.host)) {
            Ok(value) => Ok(value),
            Err(e) => Err(RegistryError::InvalidCast {
                pwd: pwd.to_owned(),
                cd: cd.to_owned(),
                cast_type: "instance cast",
                expected: e.expected,
                got: e.got.unwrap_or("".to_owned()),
            })
        }
    }

    pub fn instance(&self, pwd: &str, cd: &str) -> Result<&Instance, RegistryError> {
        return match &self.cd(pwd, cd)?.instance {
            None => Err(RegistryError::MissingAtPath {
                path: pwd.to_owned(),
                expected: "instance",
            }),
            Some(instance) => Ok(instance)
        };
    }

    // Get Attributes

    pub fn attr_value<T: 'static + FromPolar>(&self, pwd: &str, cd: &str)
                                              -> Result<T, RegistryError> {
        let result = self.attr(pwd, cd)?;
        Self::cast::<T>(pwd, cd, "attribute", result)
    }

    pub fn attr(&self, pwd: &str, cd: &str) -> Result<PolarValue, RegistryError> {
        let attr_path = self.cd(pwd, cd)?;
        // check that we are an attribute node
        let attr_name = match &attr_path.attr {
            Some(name) => name,
            None => return Err(RegistryError::MissingAtPath {
                path: attr_path.full_path.to_owned(),
                expected: "attribute",
            })
        };
        // lookup the instance node
        let instance_path = self.paths.get(&attr_path.owner.unwrap()).unwrap();

        let instance = instance_path.instance.as_ref().unwrap();
        instance.get_attr(attr_name, &self.host).map_err(|e| {
            let class_name = &instance.class(&self.host).unwrap().fq_name;

            RegistryError::InvocationFailure {
                pwd: pwd.to_owned(),
                cd: cd.to_owned(),
                class: class_name.to_string(),
                method: attr_name.to_string(),
                invocation_type: "attribute",
                reason: "attribute invocation failure",
                error: e,
            }
        })
    }

    //
    // Invoke methods
    //

    pub fn parsed_invoke_method_value<T: 'static + FromPolar>(
        &mut self, pwd: &str, cd: &str, params: Vec<&str>) -> Result<T, RegistryError> {
        let result = self.parsed_invoke_method(pwd, cd, params)?;
        Self::cast::<T>(pwd, cd, "method", result)
    }

    pub fn parsed_invoke_method(&mut self, pwd: &str, cd: &str, params: Vec<&str>)
                                -> Result<PolarValue, RegistryError> {
        // check that we are an method node
        let method_path = self.cd(pwd, cd)?;
        let method_name = match method_path.method {
            Some(name) => name,
            None => return Err(RegistryError::MissingAtPath {
                path: method_path.full_path.to_owned(),
                expected: "method",
            })
        };

        // lookup the instance for the method and then the class
        let instance_path = self.paths.get(&method_path.owner.unwrap()).unwrap();
        let instance = instance_path.instance.as_ref().unwrap();
        let class = instance.class(&self.host).unwrap();
        let instance_method = class.instance_methods.get(method_name).unwrap();

        // parse the params into PolarValues and invoke method
        let params = self.parse_params(
            &class.name, method_name, &params, instance_method.param_types())?;
        self._invoke_method(pwd, cd, &class.fq_name, method_name, instance, instance_method, params)
    }

    pub fn invoke_method_value<T: 'static + FromPolar>(&mut self,
                                                       pwd: &str,
                                                       cd: &str,
                                                       params: Vec<PolarValue>)
                                                       -> Result<T, RegistryError> {
        let result = self.invoke_method(pwd, cd, params)?;
        Self::cast::<T>(pwd, cd, "method", result)
    }

    pub fn invoke_method(&mut self, pwd: &str, cd: &str, params: Vec<PolarValue>)
                         -> Result<PolarValue, RegistryError> {
        // check that we are an method node
        let method_path = self.cd(pwd, cd)?;
        let method_name = match method_path.method {
            Some(name) => name,
            None => return Err(RegistryError::MissingAtPath {
                path: method_path.full_path.to_owned(),
                expected: "method",
            })
        };

        // lookup the instance for the method and then the class
        let instance_path = self.paths.get(&method_path.owner.unwrap()).unwrap();
        let instance = instance_path.instance.as_ref().unwrap();
        let class = instance.class(&self.host).unwrap();
        let instance_method = class.instance_methods.get(method_name).unwrap();

        // validate params of the instance method
        self.validate_params(&params, &class.fq_name, method_name, instance_method.param_types())?;
        self._invoke_method(pwd, cd, &class.fq_name, method_name, instance, instance_method, params)
    }

    fn _invoke_method(&self,
                      pwd: &str,
                      cd: &str,
                      class_name: &str,
                      method_name: &str,
                      instance: &Instance,
                      method: &InstanceMethod,
                      params: Vec<PolarValue>) -> Result<PolarValue, RegistryError> {
        method.invoke(instance, params, &self.host).map_err(|e| {
            RegistryError::InvocationFailure {
                pwd: pwd.to_owned(),
                cd: cd.to_owned(),
                class: class_name.to_owned(),
                method: method_name.to_owned(),
                invocation_type: "method",
                reason: "method invocation failure",
                error: e,
            }
        })
    }
}

/// Errors thrown when navigating the command tree.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("path does not contain an {expected} to retrieve: {path}")]
    MissingAtPath {
        path: String,
        expected: &'static str,
    },
    #[error("cannot create child node at {pwd}/{child}")]
    InvalidPathChildName {
        pwd: String,
        child: String,
    },
    #[error("cannot navigate to path: pwd={pwd}, cd={cd}, reason={reason}")]
    IllegalPathNavigation {
        pwd: String,
        cd: String,
        reason: &'static str,
    },
    #[error("duplicate path: {0}")]
    DuplicatePath(String),
    #[error("methods or attributes of class share the same name: class={class}, child={child}")]
    ClassChildNameConflict {
        class: String,
        child: String,
    },
    #[error("class has already been registered: {0}")]
    DuplicateClass(String),
    #[error("cannot make instance from class that is not registered: {0}")]
    UnknownClass(String),
    #[error("class does not have a constructor: {0}")]
    NoConstructor(String),
    #[error("invalid number of method parameters provided: {class}::{method} expects {expected} but received {received}")]
    InvalidNumberOfMethodParameters {
        class: String,
        method: String,
        expected: usize,
        received: usize,
    },
    #[error("invalid method parameter type: {class}::{method} parameter {param_index} has type {param_type}: {reason}")]
    InvalidMethodParameter {
        class: String,
        method: String,
        param_index: usize,
        param_type: &'static str,
        reason: &'static str,
    },
    #[error("invalid {cast_type} cast: pwd={pwd}, cd={cd}, expected={expected}, got={got}")]
    InvalidCast {
        pwd: String,
        cd: String,
        cast_type: &'static str,
        expected: String,
        got: String,
    },
    #[error("an unhandled error from oso: reason={reason}, error={error}")]
    InvocationFailure {
        pwd: String,
        cd: String,
        class: String,
        method: String,
        invocation_type: &'static str,
        reason: &'static str,
        error: OsoError,
    },
}

impl PartialEq for RegistryError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RegistryError::MissingAtPath { path, expected },
                RegistryError::MissingAtPath { path: path2, expected: expected2 }) =>
                path == path2 && expected == expected2,
            (RegistryError::InvalidPathChildName { pwd, child },
                RegistryError::InvalidPathChildName { pwd: pwd2, child: child2 }) =>
                pwd == pwd2 && child == child2,
            (RegistryError::IllegalPathNavigation { pwd, cd, .. },
                RegistryError::IllegalPathNavigation { pwd: pwd2, cd: cd2, .. }) =>
                pwd == pwd2 && cd == cd2,
            (RegistryError::DuplicatePath(path),
                RegistryError::DuplicatePath(path2)) =>
                path == path2,
            (RegistryError::DuplicateClass(class),
                RegistryError::DuplicateClass(class2)) =>
                class == class2,
            (RegistryError::ClassChildNameConflict { class, child },
                RegistryError::ClassChildNameConflict { class: class2, child: child2 }) =>
                class == class2 && child == child2,
            (RegistryError::UnknownClass(class),
                RegistryError::UnknownClass(class2)) =>
                class == class2,
            (RegistryError::NoConstructor(class),
                RegistryError::NoConstructor(class2)) =>
                class == class2,
            (RegistryError::InvalidNumberOfMethodParameters { class, method, expected, received },
                RegistryError::InvalidNumberOfMethodParameters { class: class2, method: method2, expected: expected2, received: received2 }) =>
                class == class2 && method == method2 && expected == expected2 && received == received2,
            (RegistryError::InvalidMethodParameter { class, method, param_index, param_type, .. },
                RegistryError::InvalidMethodParameter { class: class2, method: method2, param_index: param_index2, param_type: param_type2, .. }) =>
                class == class2 && method == method2 && param_index == param_index2 && param_type == param_type2,
            (RegistryError::InvalidCast { pwd, cd, cast_type, expected, got },
                RegistryError::InvalidCast { pwd: pwd2, cd: cd2, cast_type: cast_type2, expected: expected2, got: got2 }) =>
                pwd == pwd2 && cd == cd2 && cast_type == cast_type2 && expected == expected2 && got == got2,
            (RegistryError::InvocationFailure { pwd, cd, class, method, invocation_type, .. },
                RegistryError::InvocationFailure { pwd: pwd2, cd: cd2, class: class2, method: method2, invocation_type: invocation_type2, .. }) =>
                pwd == pwd2 && cd == cd2 && class == class2 && method == method2 && invocation_type == invocation_type2,
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !eq(self, other)
    }
}

#[cfg(test)]
mod path_tests {
    use std::collections::HashMap;
    use crate::command::registry::RegistryError;
    use super::Registry;

    #[test]
    fn mkdir_absolute_directory() {
        let mut registry = Registry::new();
        registry.mkdir("/bar/me", "/foo").unwrap();

        registry.mkdir("/foo", "/bar/soo").unwrap();

        let node = registry.path("/bar/soo").unwrap();
        assert_eq!("/bar/soo", node.full_path);
    }

    #[test]
    fn mkdir_creates_child_directory() {
        let mut registry = Registry::new();

        registry.mkdir("/", "foo").unwrap();

        let node = registry.path("/foo").unwrap();
        assert_eq!("foo", node.name);
        assert_eq!("/foo", node.full_path);
        assert_eq!(0, node.children.len());
        assert_eq!(true, node.instance.is_none());
    }

    #[test]
    fn mkdir_creates_grandchild_directories() {
        let mut registry = Registry::new();

        registry.mkdir("/", "foo/bar/soo").unwrap();

        // then
        let root = registry.path("/").unwrap();
        let child = registry.path("/foo").unwrap();
        let grandchild = registry.path("/foo/bar").unwrap();
        let great_grandchild = registry.path("/foo/bar/soo").unwrap();

        assert_eq!("soo", great_grandchild.name);
        assert_eq!("/foo/bar/soo", great_grandchild.full_path);
        assert_eq!(Some(grandchild.id), great_grandchild.parent);
        assert!(great_grandchild.children.is_empty());
        assert!(great_grandchild.instance.is_none());

        assert_eq!("bar", grandchild.name);
        assert_eq!("/foo/bar", grandchild.full_path);
        assert_eq!(Some(child.id), grandchild.parent);
        let mut children = HashMap::new();
        children.insert("soo".to_owned(), great_grandchild.id);
        assert_eq!(children, grandchild.children);

        assert_eq!("foo", child.name);
        assert_eq!("/foo", child.full_path);
        assert_eq!(Some(root.id), child.parent);
        children.clear();
        children.insert("bar".to_owned(), grandchild.id);
        assert_eq!(children, child.children);

        assert_eq!("", root.name);
        assert_eq!("/", root.full_path);
        assert_eq!(None, root.parent);
        children.clear();
        children.insert("foo".to_owned(), child.id);
        assert_eq!(children, root.children);
    }

    #[test]
    fn mkdir_from_child() {
        let mut registry = Registry::new();
        registry.mkdir("/", "/foo/bar").unwrap();

        registry.mkdir("/foo/bar", "../soo").unwrap();

        let node = registry.path("/foo/soo").unwrap();
        assert_eq!("/foo/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_absolute_path_from_child_directory() {
        let mut registry = Registry::new();
        registry.mkdir("/", "/foo/bar").unwrap();

        registry.mkdir("/foo/bar", "/soo").unwrap();

        let node = registry.path("/soo").unwrap();
        assert_eq!("/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_current_directory() {
        let mut registry = Registry::new();
        registry.mkdir("/", "/foo/bar").unwrap();

        registry.mkdir("/foo/bar", "./soo").unwrap();

        let node = registry.path("/foo/bar/soo").unwrap();
        assert_eq!("/foo/bar/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_empty_directories() {
        let mut registry = Registry::new();
        registry.mkdir("/", "/foo/bar").unwrap();

        registry.mkdir("/foo/bar", "soo///doo").unwrap();

        let node = registry.path("/foo/bar/soo/doo").unwrap();
        assert_eq!("/foo/bar/soo/doo", node.full_path);
    }

    #[test]
    fn cd_to_parent() {
        let mut registry = Registry::new();
        registry.mkdir("/", "foo/bar/soo").unwrap();

        let grandchild = registry.cd("/foo/bar/soo", "..").unwrap();

        assert_eq!("/foo/bar", grandchild.full_path);
    }

    #[test]
    fn cd_to_grandparent() {
        let mut registry = Registry::new();
        registry.mkdir("/", "foo/bar/soo").unwrap();

        let grandchild = registry.cd("/foo/bar/soo", "../../").unwrap();

        assert_eq!("/foo", grandchild.full_path);
    }

    #[test]
    fn cd_to_great_grandparent() {
        let mut registry = Registry::new();
        registry.mkdir("/", "foo/bar/soo").unwrap();

        let grandchild = registry.cd("/foo/bar/soo", "../../../").unwrap();

        assert_eq!("/", grandchild.full_path);
    }

    #[test]
    fn cd_beyond_root_is_error() {
        let mut registry = Registry::new();
        registry.mkdir("/", "foo/bar/soo").unwrap();

        let result = registry.cd("/foo/bar/soo", "../../../..").err().unwrap();

        assert_eq!(RegistryError::IllegalPathNavigation {
            pwd: "/foo/bar/soo".to_owned(),
            cd: "../../../..".to_owned(),
            reason: "",
        }, result);
    }

    #[test]
    fn cd_to_unknown_directory_is_error() {
        let mut registry = Registry::new();
        registry.mkdir("/", "foo/bar/soo").unwrap();

        let result = registry.cd("/foo/bar/soo", "doo").err().unwrap();

        assert_eq!(RegistryError::IllegalPathNavigation {
            pwd: "/foo/bar/soo".to_owned(),
            cd: "doo".to_owned(),
            reason: "",
        }, result);
    }
}

#[cfg(test)]
mod registry_tests {
    use crate::command::oso::{PolarClass, PolarValue};
    use crate::command::registry::{RegistryError, Registry};

    #[derive(Clone, PolarClass, Default)]
    struct User {
        #[polar(attribute)]
        pub username: String,
    }

    impl User {
        fn new(username: String) -> User {
            User { username }
        }
    }

    #[derive(Clone, PolarClass, Default)]
    struct User2 {
        pub username: String,
        #[polar(attribute)]
        pub user_id: i32,
    }

    impl User2 {
        fn new(username: String, user_id: i32) -> User2 {
            User2 { username, user_id }
        }

        fn add_one(&self, num: i32) -> i32 {
            num + 1
        }
    }

    fn create_registry() -> Registry {
        let mut registry = Registry::new();
        registry.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec!["string"])
            .build()).unwrap();
        registry.cache_class(User2::get_polar_class_builder()
            .set_constructor(User2::new, vec!["string", "int"])
            .add_method("add_one", User2::add_one, vec!["int"], None)
            .build()).unwrap();
        registry
    }

    #[test]
    fn make_instance_1_param() {
        let mut registry = create_registry();

        registry.create_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let user = registry.instance_value::<User>("/", "foo").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params() {
        let mut registry = create_registry();

        registry.create_instance("/foo", ".", "User2", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();

        let user = registry.instance_value::<User2>("/foo", ".").unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    #[test]
    fn make_instance_1_param_strings() {
        let mut registry = create_registry();

        registry.parsed_create_instance("/foo", ".", "User", &vec!["jim"]).unwrap();

        let user = registry.instance_value::<User>("/foo", ".").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params_strings() {
        let mut registry = create_registry();

        registry.parsed_create_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let user = registry.instance_value::<User2>("/", "foo").unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    #[test]
    fn make_instance_at_already_created_directory() {
        let mut registry = create_registry();
        registry.mkdir("/foo", ".").unwrap();

        registry.create_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let user = registry.instance_value::<User>("/foo", ".").unwrap();
        assert_eq!(user.username, "jim");
    }


    #[test]
    fn make_instance_at_same_directory_is_error() {
        let mut registry = create_registry();
        registry.create_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let result = registry.create_instance("/foo", ".", "User", vec![PolarValue::String("greco".to_owned())]).err().unwrap();

        assert_eq!(RegistryError::DuplicatePath("/foo".to_owned()), result);
        let user = registry.instance_value::<User>("/foo", ".").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn get_attribute() {
        let mut registry = create_registry();
        registry.parsed_create_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result: i32 = registry.attr_value("/foo", "user_id").unwrap();

        assert_eq!(42, result);
    }

    #[test]
    fn get_attribute_wrong_path_is_error() {
        let mut registry = create_registry();
        registry.parsed_create_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result = registry.attr_value::<i32>("/foo", ".").err().unwrap();

        assert_eq!(RegistryError::MissingAtPath {
            path: "/foo".to_owned(),
            expected: "attribute",
        }, result);
    }

    #[test]
    fn get_attribute_wrong_type_is_error() {
        let mut registry = create_registry();
        registry.parsed_create_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result = registry.attr_value::<f64>("/foo/user_id", ".").err().unwrap();

        assert_eq!(RegistryError::InvalidCast {
            pwd: "/foo/user_id".to_owned(),
            cd: ".".to_owned(),
            cast_type: "attribute",
            expected: "float".to_owned(),
            got: "".to_string(),
        }, result);
    }

    #[test]
    fn call_instance_method() {
        let mut registry = create_registry();
        registry.parsed_create_instance(
            "/foo", ".", "User2", &vec!["jim", "10"]).unwrap();

        let result = registry.invoke_method(
            "/foo", "add_one", vec![PolarValue::Integer(42)]).unwrap();

        assert_eq!(PolarValue::Integer(43), result);
    }

    #[test]
    fn call_instance_method_with_wrong_param_type_is_error() {
        let mut registry = create_registry();
        registry.parsed_create_instance(
            "/foo", ".", "User2", &vec!["jim", "10"]).unwrap();

        let result = registry.invoke_method(
            "/foo", "add_one", vec![PolarValue::Float(42.0)]).err().unwrap();

        assert_eq!(RegistryError::InvalidMethodParameter {
            class: "core::command::registry::registry_tests::User2".to_string(),
            method: "add_one".to_string(),
            param_index: 0,
            param_type: "int",
            reason: "",
        }, result);
    }

    #[test]
    fn call_instance_method_value() {
        let mut registry = create_registry();
        registry.parsed_create_instance(
            "/foo", ".", "User2", &vec!["jim", "10"]).unwrap();

        let result: i32 = registry.invoke_method_value(
            "/foo", "add_one", vec![PolarValue::Integer(42)]).unwrap();

        assert_eq!(43, result);
    }

    #[test]
    fn parse_and_call_instance_method() {
        let mut registry = create_registry();
        registry.parsed_create_instance(
            "/foo", ".", "User2", &vec!["jim", "10"]).unwrap();

        let result = registry.parsed_invoke_method(
            "/foo/add_one", ".", vec!["42"]).unwrap();

        assert_eq!(PolarValue::Integer(43), result);
    }

    #[test]
    fn parse_and_call_instance_method_value() {
        let mut registry = create_registry();
        registry.parsed_create_instance(
            "/foo", ".", "User2", &vec!["jim", "10"]).unwrap();

        let result: i32 = registry.parsed_invoke_method_value(
            "/foo/add_one", ".", vec!["42"]).unwrap();

        assert_eq!(43, result);
    }

    #[derive(PolarClass, Clone, Default, PartialEq, Debug)]
    struct Bar {}

    #[derive(PolarClass, Clone, Default)]
    struct Foo {}

    impl Foo {
        fn new() -> Self {
            Foo {}
        }

        fn add(&self, a: i16, b: i32) -> i16 {
            i16::try_from(i32::from(a) + b).unwrap()
        }

        fn bar(&self) -> Bar {
            Bar {}
        }

        fn doit(&self) {
            println!("hi");
        }
    }

    fn create_registry2() -> Registry {
        let mut registry = Registry::new();
        let bar_class = Bar::get_polar_class_builder()
            .set_constructor(Bar::default, vec![])
            .build();
        // let x: fn(&Foo, i16, i32) -> i16 = Foo::add;
        let foo_class = Foo::get_polar_class_builder()
            .set_constructor(Foo::new, vec![])
            .add_method("add", Foo::add,
                        vec!["int", "int"], Some("add_two"))
            .add_method("bar", Foo::bar, vec![], None)
            .add_method("doit", |f: &Foo| -> bool {
                f.doit();
                true
            }, vec![], None)
            .build();
        registry.cache_class(bar_class).unwrap();
        registry.cache_class(foo_class).unwrap();
        registry.create_instance("/foo/bar", ".", "Foo", vec![]).unwrap();
        registry
    }

    #[test]
    fn call_instance_method_two_params() {
        let mut registry = create_registry2();

        let result: i32 = registry.parsed_invoke_method_value(
            "/foo/bar", "add_two", vec!["1", "2"]).unwrap();

        assert_eq!(3, result);
    }

    #[test]
    fn call_instance_method_with_instance_returned_object() {
        let mut registry = create_registry2();

        let result: Bar = registry.parsed_invoke_method_value(
            "/foo/bar", "bar", vec![]).unwrap();

        assert_eq!(Bar {}, result);
    }

    #[test]
    fn call_instance_method_with_no_return_value() {
        let mut registry = create_registry2();

        let result: bool = registry.parsed_invoke_method_value(
            "/foo/bar", "doit", vec![]).unwrap();

        assert!(result);
    }
}