use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::iter::Map;
use std::rc::Weak;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

extern crate rand;
use thiserror::Error;

use crate::core::command::oso::{FromPolar, Host, ToPolar, TypeError};
use super::oso::{
    builtins, Class, Instance, InvalidCallError, OsoError, ParamType, PolarValue, PolarClass
};

/// Errors thrown when navigating the command tree.
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("path does not contain an {expected} to retrieve: {path}")]
    MissingAtPath {
        path: String,
        expected: &'static str
    },
    #[error("cannot create child node at {pwd}/{child}: {reason}")]
    InvalidPathChildName {
        pwd: String,
        child: String,
        reason: &'static str
    },
    #[error("cannot navigate to path: pwd={pwd}, cd={cd}, reason={reason}")]
    IllegalPathNavigation {
        pwd: String,
        cd: String,
        reason: &'static str
    },
    #[error("duplicate path: {path}")]
    DuplicatePath {
        path: String
    },
    #[error("methods or attributes of class share the same name: class={class}, child={child}")]
    ClassChildNameConflict {
        class: String,
        child: String
    },
    #[error("class has already been registered: {class}")]
    DuplicateClass {
        class: String
    },
    #[error("cannot make instance from class that is not registered: {class}")]
    UnknownClass {
        class: String
    },
    #[error("class does not have a constructor: {class}")]
    NoClassConstructor {
        class: String
    },
    #[error("invalid number of method parameters provided: {class}::{method} expects {expected} but received {received}")]
    InvalidNumberOfMethodParameters {
        class: String,
        method: String,
        expected: usize,
        received: usize
    },
    #[error("invalid method parameter type: {class}::{method} parameter {param_index} has type {param_type}: {reason}")]
    InvalidMethodParameter {
        class: String,
        method: String,
        param_index: usize,
        param_type: ParamType,
        reason: &'static str
    },
    #[error("invalid {cast_type} cast: path={path}, expected={expected}, got={got}")]
    InvalidCast {
        path: String,
        cast_type: &'static str,
        expected: String,
        got: String
    },
    #[error("an unhandled error from oso: reason={reason}, error={error}")]
    InternalError {
        reason: &'static str,
        error: OsoError
    }
}

pub struct CommandRegistry {
    host: Host,
    paths: HashMap<usize, CommandPath>,
    root_id: usize
}

pub struct CommandPath {
    id: usize,
    pub name: String,
    parent: Option<usize>,
    children: HashMap<String, usize>,
    pub full_path: String,
    instance: Option<Instance>,
    owner: Option<usize>,
    attr: Option<String>,
    method: Option<String>
}

impl CommandRegistry {
    pub fn new() -> CommandRegistry {
        let mut host = Host::new();
        for class in builtins::classes() {
            host.cache_class(class).expect("builtins failed");
        }

        let mut reg = CommandRegistry {
            host,
            paths: HashMap::new(),
            root_id: rand::random()
        };
        reg.paths.insert(reg.root_id, CommandPath {
            children: HashMap::new(),
            id: reg.root_id,
            instance: None,
            name: "".to_owned(),
            parent: None,
            full_path: "/".to_owned(),
            owner: None,
            method: None,
            attr: None
        });
        reg
    }

    fn to_path_segments(pwd: &str, cd: &str) -> Result<Vec<String>, CommandError> {
        // this method is inefficient, but the command system isn't designed for the critical path
        let mut segments: Vec<String> = Vec::new();

        let mut first = true;
        for segment in pwd.split("/") {
            if first && !segment.is_empty() || segment == "." || segment == ".." {
                return Err(CommandError::IllegalPathNavigation {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    reason: "invalid path segment name"
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
                    return Err(CommandError::IllegalPathNavigation {
                        pwd: pwd.to_owned(),
                        cd: cd.to_owned(),
                        reason: "navigation beyond root"
                    });
                }
            } else if !segment.is_empty() && segment != "." {
                segments.push(segment.to_owned());
            }
            first = false;
        }

        Ok(segments)
    }

    fn to_path_str(pwd: &str, cd: &str) -> Result<String, CommandError> {
        let segments = CommandRegistry::to_path_segments(pwd, cd)?;
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

    /// Creates a new directory for the specified working directory and change directory.
    ///
    /// # Examples
    ///
    /// ```
    /// use rcore::core::command::CommandRegistry;
    /// let mut registry = CommandRegistry::new();
    /// registry.mkdir("/foo/bar", "soo");
    /// assert_eq!("/foo/bar/soo", registry.path("/foo/bar/soo").unwrap().full_path);
    /// ```
    pub fn mkdir(&mut self, pwd: &str, cd: &str) -> Result<(), CommandError> {
        self.create_path(pwd, cd, true, None, None, None, None)
    }

    fn create_path(
            &mut self,
            pwd: &str,
            cd: &str,
            fail_on_duplicate: bool,
            instance: Option<Instance>,
            owner: Option<usize>,
            method: Option<String>,
            attr: Option<String>) -> Result<(), CommandError> {
        let mut pwd_node = self.paths.get(&self.root_id).unwrap();
        let mut created = false;

        let segments = CommandRegistry::to_path_segments(pwd, cd)?;
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
                    let command_path = match method.path() {
                        None => name,
                        Some(command_path) => command_path
                    };
                    self.create_path(
                        &full_path,
                        command_path,
                        true,
                        None,
                        Some(id),
                        Some(name.to_string()),
                        None)?;
                }
                for (attr_name, attr) in attributes {
                    // TODO: customize attribute path
                    let attr_path = attr_name;
                    self.create_path(
                        &full_path,
                        attr_path,
                        true,
                        None,
                        Some(id),
                        None,
                        Some(attr_name.to_string()))?;
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
            Err(CommandError::DuplicatePath {
                path: CommandRegistry::to_path_str(pwd, cd).unwrap()
            })
        }
    }

    fn create_child(&mut self, node_id: usize, pwd: String, name: &str)
            -> Result<usize, CommandError> {
        let parent = self.paths.get_mut(&node_id).unwrap();
        if name.is_empty() || name == "." || name == ".." {
            Err(CommandError::InvalidPathChildName {
                pwd,
                child: name.to_owned(),
                reason: "illegal child name"
            })
        } else {
            let path_copy = parent.full_path.clone();
            let child_id = rand::random();
            let child = CommandPath {
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
                attr: None
            };
            parent.children.insert(name.to_owned(), child_id);
            self.paths.insert(child_id, child);
            Ok(child_id)
        }
    }

    pub fn cd(&self, pwd: &str, cd: &str) -> Result<&CommandPath, CommandError> {
        let mut pwd_node = self.paths.get(&self.root_id).unwrap();

        let segments = CommandRegistry::to_path_segments(pwd, cd)?;
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
                return Err(CommandError::IllegalPathNavigation {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned(),
                    reason: "unknown path",
                });
            }
        }

        Ok(pwd_node)
    }

    pub fn path(&self, pwd: &str) -> Result<&CommandPath, CommandError> {
        self.cd(pwd, ".")
    }

    fn class(&self, class_name: &str) -> Result<&Class, CommandError> {
        self.host.get_class(class_name).map_err(|e| match e {
            OsoError::MissingClassError { name } => CommandError::UnknownClass {
                class: name
            },
            _ => CommandError::InternalError {
                reason: "could not retrieve class",
                error: e,
            }
        })
    }

    pub fn cache_class(&mut self, class: Class) -> Result<(), CommandError> {
        let class_name = class.fq_name.to_owned();

        // check to ensure that attributes and instance methods don't conflict
        let mut name_conflict = HashSet::new();
        for attr in class.attributes.keys() {
            if !name_conflict.insert(attr) {
                return Err(CommandError::ClassChildNameConflict {
                    class: class_name,
                    child: attr.to_string(),
                })
            }
        }
        for method in class.instance_methods.keys() {
            if !name_conflict.insert(method) {
                return Err(CommandError::ClassChildNameConflict {
                    class: class_name,
                    child: method.to_string(),
                })
            }
        }

        self.host.cache_class(class).map_err(|e| {
            match e {
                OsoError::DuplicateClassError { name } => CommandError::DuplicateClass {
                    class: name
                },
                _ => CommandError::InternalError {
                    reason: "failed to cache class",
                    error: e
                }
            }
        })
    }

    pub fn parse_and_make_instance(
            &mut self,
            pwd: &str,
            cd: &str,
            class_name: &str,
            args: &Vec<&str>) -> Result<(), CommandError> {
        // get the right constructor
        let class = self.class(class_name)?;
        let constructor = match &class.constructor {
            Some(c) => c,
            None => return Err(CommandError::NoClassConstructor { class: class_name.to_owned() })
        };
        let param_types = constructor.param_types();
        if args.len() != param_types.len() {
            return Err(CommandError::InvalidNumberOfMethodParameters {
                class: class_name.to_owned(),
                method: "constructor".to_owned(),
                expected: param_types.len(),
                received: args.len()
            });
        }

        // parse parameters
        let mut params: Vec<PolarValue> = Vec::new();
        for i in 0..param_types.len() {
            let arg = args[i];
            let pt = &param_types[i];

            let polar_value = match pt {
                ParamType::Boolean => Ok(CommandRegistry::parse::<bool>(
                    arg, class_name, i, pt, "constructor")?.to_polar()),
                ParamType::Integer => Ok(CommandRegistry::parse::<i32>(
                    arg, class_name, i, pt, "constructor")?.to_polar()),
                ParamType::Float => Ok(CommandRegistry::parse::<f64>(
                    arg, class_name, i, pt, "constructor")?.to_polar()),
                ParamType::String => Ok(PolarValue::String(arg.to_owned())),
                ParamType::Instance => {
                    let instance = self.get_instance(arg)?;
                    Ok(PolarValue::new_from_instance(instance.to_owned()))
                }
            }?;
            params.push(polar_value);
        }

        let instance = constructor.invoke(params)
            .map_err(|e| CommandError::InternalError {
                reason: "failed to make instance",
                error: e,
            })?;
        self.create_path(pwd, cd, false, Some(instance), None, None, None)
    }

    fn parse<T: FromStr>(
            param: &str,
            class_name: &str,
            param_index: usize,
            param_type: &ParamType,
            method: &str) -> Result<T, CommandError> {
        param.parse().map_err(|e| CommandError::InvalidMethodParameter {
            class: class_name.to_owned(),
            method: method.to_owned(),
            param_index,
            param_type: param_type.clone(),
            reason: "could not parse from string"
        })
    }

    pub fn make_instance(
            &mut self,
            pwd: &str,
            cd: &str,
            class_name: &str,
            params: Vec<PolarValue>) -> Result<(), CommandError>{
        // get the right constructor
        let class = self.class(class_name)?;
        let constructor = match &class.constructor {
            Some(c) => c,
            None => return Err(CommandError::NoClassConstructor{ class: class_name.to_owned() })
        };
        let param_types = constructor.param_types();
        if params.len() != param_types.len() {
            return Err(CommandError::InvalidNumberOfMethodParameters {
                class: class_name.to_owned(),
                method: "constructor".to_string(),
                expected: param_types.len(),
                received: params.len()
            });
        }

        let instance = constructor.invoke(params)
            .map_err(|e| CommandError::InternalError {
                reason: "failed to make instance",
                error: e,
            })?;
        self.create_path(pwd, cd, false, Some(instance), None, None, None)
    }

    pub fn get_instance(&self, pwd: &str) -> Result<&Instance, CommandError> {
        return match &self.path(pwd)?.instance {
            None => Err(CommandError::MissingAtPath {
                path: pwd.to_owned(),
                expected: "instance"
            }),
            Some(instance) => Ok(instance)
        }
    }

    pub fn get_instance_value<T: 'static>(&self, pwd: &str) -> Result<&T, CommandError> {
        let instance = self.get_instance(pwd)?;
        match instance.downcast::<T>(Some(&self.host)) {
            Ok(value) => Ok(value),
            Err(e) => Err(CommandError::InvalidCast {
                path: pwd.to_owned(),
                cast_type: "instance cast",
                expected: e.expected,
                got: e.got.unwrap_or("".to_owned()),
            })
        }
    }

    pub fn get_attr(&self, path: &str) -> Result<PolarValue, CommandError> {
        // path to the attribute
        let attr_path = self.path(path).unwrap();
        // check that we are an attribute node
        let attr_name = match &attr_path.attr {
            Some(name) => name,
            None => return Err(CommandError::MissingAtPath {
                path: attr_path.full_path.to_owned(),
                expected: "attribute"
            })
        };
        // lookup the instance node
        let instance_path = self.paths.get(&attr_path.owner.unwrap()).unwrap();
        // get the attribute
        let instance = instance_path.instance.as_ref().unwrap();
        match instance.get_attr(attr_name, &self.host) {
            Ok(value) => Ok(value),
            Err(_) => Err(CommandError::MissingAtPath {
                path: attr_path.full_path.to_owned(),
                expected: "attribute",
            })
        }
    }

    pub fn get_attr_value<T: 'static + FromPolar>(&self, path: &str) -> Result<T, CommandError> {
        let result = self.get_attr(path)?;
        match T::from_polar(result) {
            Ok(v) => Ok(v),
            Err(e) => match e {
                OsoError::TypeError(e) => Err(CommandError::InvalidCast {
                    path: path.to_owned(),
                    cast_type: "attribute",
                    expected: e.expected,
                    got: e.got.unwrap_or("".to_owned())
                }),
                e => Err(CommandError::InternalError {
                    reason: "attribute cast error",
                    error: e,
                })
            }
        }
    }
}

impl PartialEq for CommandPath {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod path_tests {
    use std::collections::HashMap;
    use crate::core::command::registry::CommandError;
    use super::CommandRegistry;
    use crate::core::command::oso::PolarClass;

    #[test]
    fn mkdir_absolute_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/bar/me", "/foo");

        registry.mkdir("/foo", "/bar/soo");

        let node = registry.path("/bar/soo").unwrap();
        assert_eq!("/bar/soo", node.full_path);
    }

    #[test]
    fn mkdir_creates_child_directory() {
        let mut registry = CommandRegistry::new();

        registry.mkdir("/", "foo");

        let node = registry.path("/foo").unwrap();
        assert_eq!("foo", node.name);
        assert_eq!("/foo", node.full_path);
        assert_eq!(0, node.children.len());
        assert_eq!(true, node.instance.is_none());
    }

    #[test]
    fn mkdir_creates_grandchild_directories() {
        let mut registry = CommandRegistry::new();

        registry.mkdir("/", "foo/bar/soo");

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
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        registry.mkdir("/foo/bar", "../soo").unwrap();

        let node = registry.path("/foo/soo").unwrap();
        assert_eq!("/foo/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_absolute_path_from_child_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        registry.mkdir("/foo/bar", "/soo");

        let node = registry.path("/soo").unwrap();
        assert_eq!("/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_current_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        registry.mkdir("/foo/bar", "./soo");

        let node = registry.path("/foo/bar/soo").unwrap();
        assert_eq!("/foo/bar/soo", node.full_path);
    }

    #[test]
    fn mkdir_with_empty_directories() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        registry.mkdir("/foo/bar", "soo///doo");

        let node = registry.path("/foo/bar/soo/doo").unwrap();
        assert_eq!("/foo/bar/soo/doo", node.full_path);
    }

    #[test]
    fn cd_to_parent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "..").unwrap();

        assert_eq!("/foo/bar", grandchild.full_path);
    }

    #[test]
    fn cd_to_grandparent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "../../").unwrap();

        assert_eq!("/foo", grandchild.full_path);
    }

    #[test]
    fn cd_to_great_grandparent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "../../../").unwrap();

        assert_eq!("/", grandchild.full_path);
    }

    #[test]
    fn cd_beyond_root_is_error() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let result = registry.cd("/foo/bar/soo", "../../../..").err().unwrap();

        match result {
            CommandError::IllegalPathNavigation { pwd, cd, .. } => {
                assert_eq!("/foo/bar/soo".to_owned(), pwd);
                assert_eq!("../../../..".to_owned(), cd);
            },
            _ => assert!(false)
        }
    }

    #[test]
    fn cd_to_unknown_directory_is_error() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let result = registry.cd("/foo/bar/soo", "doo").err().unwrap();

        match result {
            CommandError::IllegalPathNavigation { pwd, cd, .. } => {
                assert_eq!("/foo/bar/soo".to_owned(), pwd);
                assert_eq!("doo".to_owned(), cd);
            },
            _ => assert!(false)
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use crate::core::command::oso::{FromPolar, Host, Instance, InvalidCallError, OsoError, ParamType, PolarValue, ToPolar};
    use crate::core::command::oso::PolarClass;
    use crate::core::command::registry::{CommandError, CommandRegistry};

    #[derive(Clone, PolarClass, Default)]
    struct User {
        #[polar(attribute)]
        pub username: String,
    }

    impl User {
        fn superuser() -> Vec<String> {
            return vec!["alice".to_string(), "charlie".to_string()]
        }

        fn new(username: String) -> User {
            User { username }
        }
    }

    #[derive(Clone, PolarClass, Default)]
    struct User2 {
        pub username: String,
        #[polar(attribute)]
        pub user_id: i32
    }

    impl User2 {
        fn new(username: String, user_id: i32) -> User2 {
            User2 { username, user_id }
        }

        fn add_one(&self, num: i32) -> i32 {
            num + 1
        }
    }

    fn create_registry() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.cache_class(User::get_polar_class_builder()
                             .set_constructor(User::new, vec![ParamType::String])
                             .build()).unwrap();
        registry.cache_class(User2::get_polar_class_builder()
                             .set_constructor(User2::new, vec![ParamType::String, ParamType::Integer])
                             .add_method("add_one", User2::add_one)
                             .build()).unwrap();
        registry
    }

    #[test]
    fn make_instance_1_param() {
        let mut registry = create_registry();

        registry.make_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let user = registry.get_instance_value::<User>("/foo").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params() {
        let mut registry = create_registry();

        registry.make_instance("/foo", ".", "User2", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();

        let user = registry.get_instance_value::<User2>("/foo").unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    #[test]
    fn make_instance_1_param_strings() {
        let mut registry = create_registry();

        registry.parse_and_make_instance("/foo", ".", "User", &vec!["jim"]).unwrap();

        let user = registry.get_instance_value::<User>("/foo").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params_strings() {
        let mut registry = create_registry();

        registry.parse_and_make_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let user = registry.get_instance_value::<User2>("/foo").unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    #[test]
    fn make_instance_at_already_created_directory() {
        let mut registry = create_registry();
        registry.mkdir("/foo", ".");

        registry.make_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let user = registry.get_instance_value::<User>("/foo").unwrap();
        assert_eq!(user.username, "jim");
    }


    #[test]
    fn make_instance_at_same_directory_is_error() {
        let mut registry = create_registry();
        registry.make_instance("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let result = registry.make_instance("/foo", ".", "User", vec![PolarValue::String("greco".to_owned())]).err().unwrap();

        match result {
            CommandError::DuplicatePath { path } => {
                assert_eq!("/foo", path);
            },
            _ => assert!(false)
        }
        let user = registry.get_instance_value::<User>("/foo").unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn get_attribute() {
        let mut registry = create_registry();
        registry.parse_and_make_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result = registry.get_attr_value("/foo/user_id").unwrap();

        assert_eq!(42, result);
    }

    #[test]
    fn get_attribute_wrong_path_is_error() {
        let mut registry = create_registry();
        registry.parse_and_make_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result = registry.get_attr_value::<i32>("/foo").err().unwrap();

        match result {
            CommandError::MissingAtPath{ path, .. } => assert_eq!(path, "/foo".to_string()),
            _ => assert!(false)
        }
    }

    #[test]
    fn get_attribute_wrong_type_is_error() {
        let mut registry = create_registry();
        registry.parse_and_make_instance("/foo", ".", "User2", &vec!["jim", "42"]).unwrap();

        let result = registry.get_attr_value::<f64>("/foo/user_id").err().unwrap();

        match result {
            CommandError::InvalidCast { path, cast_type, expected, got } => {
                assert_eq!("/foo/user_id", path);
                assert_eq!("Float", expected);
            },
            _ => assert!(false)
        }
    }

    /*
    #[test]
    fn call_instance_method() {
        let mut host = create_registry();
        host.make_instance_from_values("foo", ".", "User2", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(10)]).unwrap();
        let instance = host.get_instance("foo").unwrap();

        let result = instance.call(&"add_one", vec![PolarValue::Integer(42)], &host).unwrap();

        assert_eq!(PolarValue::Integer(43), result);
    }
     */

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
    }

    fn create_registry2() -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        let bar_class = Bar::get_polar_class_builder()
            .set_constructor(Bar::default, vec![])
            .build();
        let foo_class = Foo::get_polar_class_builder()
            .set_constructor(Foo::new, vec![])
            .add_typed_method("add", Foo::add,
                              vec![ParamType::Integer, ParamType::Integer], None)
            .add_typed_method("bar", Foo::bar, vec![], None)
            .build();
        registry.cache_class(bar_class);
        registry.cache_class(foo_class);
        registry.make_instance("foo/bar", ".", "Foo", vec![]).unwrap();
        registry
    }

    /*
    #[test]
    fn call_instance_method_two_params_and_return() {
        let host = create_registry2();
        let foo_instance = host.get_instance("foo/bar").unwrap();

        let result = foo_instance.call("add", vec![1.to_polar(), 2.to_polar()], &host).unwrap();

        assert_eq!(PolarValue::Integer(3), result);
    }

    #[test]
    fn call_instance_method_with_instance_returned_object() {
        let host = create_registry2();
        let foo_instance = host.get_instance("foo/bar").unwrap();

        let result = foo_instance.call("bar", vec![], &host).unwrap();

        match result {
            PolarValue::Instance(i) => {
                let bar = i.downcast::<Bar>(Some(&host)).unwrap();
                assert_eq!(&Bar {}, bar);
            },
            _ => panic!("bad")
        }
    }
    */
}