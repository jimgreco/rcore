use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::iter::Map;
use std::rc::Weak;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use crate::core::command::oso::{Host, ToPolar};
use super::oso::{builtins, Class, Instance, InvalidCallError, OsoError, ParamType, PolarValue, PolarClass};

extern crate rand;

/// Errors thrown when navigating the command tree.
#[derive(Debug)]
pub enum CommandError {
    InvalidPathSegment(String),
    UnknownDirectory(usize),
    PathNavigationError {
        /// the working directories name
        pwd: String,
        /// the change directory command
        cd: String
    },
    DuplicatePathError {
        /// the working directories name
        pwd: String,
        /// the change directory command
        cd: String
    },
    DuplicateClass(String),
    UnknownClass(String),
    NoClassConstructor(String),
    InvalidNumberOfMethodParameters {
        class: String,
        method: String,
        expected: usize,
        received: usize
    },
    InvalidNumberOfConstructorParameters {
        class: String,
        expected: usize,
        received: usize
    },
    InvalidMethodParameter {
        class: String,
        method: String,
        param_index: usize,
        param_type: ParamType
    },
    InvalidConstructorParameter {
        class: String,
        param_index: usize,
        param_type: ParamType
    },
    InvalidParameterConversion {
        class: String,
        method: String,
        param_index: usize,
        param_type: ParamType,
        value: String
    },
    InternalConstructorError {
        class: String,
        error: OsoError
    }
}

pub struct CommandRegistry {
    host: Host,
    paths: HashMap<usize, CommandPath>,
    root_id: usize
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
            children: vec![],
            id: reg.root_id,
            instance: None,
            name: "".to_owned(),
            parent: None,
            path: "/".to_owned()
        });
        reg
    }

    pub fn to_path_segments(pwd: &str, cd: &str) -> Result<Vec<String>, CommandError> {
        let mut segments: Vec<String> = Vec::new();

        let mut first = true;
        let split = pwd.split("/");
        for segment in split {
            if first && !segment.is_empty() || segment == "." || segment == ".." {
                return Err(CommandError::InvalidPathSegment(pwd.to_owned()));
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
                    return Err(CommandError::PathNavigationError {
                        pwd: pwd.to_owned(),
                        cd: cd.to_owned()
                    });
                }
            } else if !segment.is_empty() && segment != "." {
                segments.push(segment.to_owned());
            }
            first = false;
        }

        Ok(segments)
    }

    pub fn mkdir(&mut self, pwd: &str, cd: &str) -> Result<&CommandPath, CommandError> {
        self.create_path(pwd, cd, true, None)
    }

    fn create_path(
            &mut self,
            pwd: &str,
            cd: &str,
            fail_on_duplicate: bool,
            instance: Option<Instance>) -> Result<&CommandPath, CommandError> {
        let mut pwd_node = self.get_path_by_id(self.root_id)?;
        let mut created = false;

        let segments = CommandRegistry::to_path_segments(pwd, cd)?;
        for segment in segments {
            let mut found = false;

            for child_id in &pwd_node.children {
                let child_node = self.get_path_by_id(*child_id)?;
                if child_node.name == segment {
                    pwd_node = child_node;
                    found = true;
                    break;
                }
            }

            if !found {
                let pwd_id = self.create_child(pwd_node.id, &segment)?;
                pwd_node = self.get_path_by_id(pwd_id)?;
                created = true;
            }
        }

        if created || (!fail_on_duplicate && pwd_node.instance.is_none()) {
            let path = self.get_path_by_id_mut(pwd_node.id).unwrap();
            path.instance = instance;
            Ok(path)
        } else {
            Err(CommandError::DuplicatePathError {
                pwd: pwd.to_string(),
                cd: cd.to_owned()
            })
        }
    }

    fn create_child(&mut self, pwd: usize, name: &str) -> Result<usize, CommandError> {
        match self.paths.get_mut(&pwd) {
            Some(parent) => {
                if name.is_empty() || name == "." || name == ".." {
                    Err(CommandError::InvalidPathSegment(name.to_owned()))
                } else {
                    let path_copy = parent.path.clone();
                    let child_id = rand::random();
                    let child = CommandPath {
                        children: vec![],
                        id: child_id,
                        instance: None,
                        name: name.to_owned(),
                        parent: Some(parent.id),
                        path: match parent.parent {
                            None => path_copy + name,
                            Some(_) => path_copy + "/" + name
                        }
                    };
                    parent.children.push(child_id);
                    self.paths.insert(child_id, child);
                    Ok(child_id)
                }
            }
            None => Err(CommandError::UnknownDirectory(pwd))
        }
    }

    pub fn cd(&self, pwd: &str, cd: &str) -> Result<&CommandPath, CommandError> {
        let mut pwd_node = self.get_path_by_id(self.root_id)?;

        let segments = CommandRegistry::to_path_segments(pwd, cd)?;
        for segment in segments {
            let mut found = false;

            for child_id in &pwd_node.children {
                let child_node = self.get_path_by_id(*child_id)?;
                if child_node.name == segment {
                    pwd_node = child_node;
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(CommandError::PathNavigationError {
                    pwd: pwd.to_owned(),
                    cd: cd.to_owned()
                });
            }
        }

        Ok(pwd_node)
    }

    pub fn get_path(&self, pwd: &str) -> Result<&CommandPath, CommandError> {
        self.cd(pwd, ".")
    }

    fn get_path_by_id(&self, id: usize) -> Result<&CommandPath, CommandError> {
        match self.paths.get(&id) {
            Some(v) => Ok(v),
            None => Err(CommandError::UnknownDirectory(id))
        }
    }

    fn get_path_by_id_mut(&mut self, id: usize) -> Result<&mut CommandPath, CommandError> {
        match self.paths.get_mut(&id) {
            Some(v) => Ok(v),
            None => Err(CommandError::UnknownDirectory(id))
        }
    }

    fn get_class(&self, class_name: &str) -> Result<&Class, CommandError> {
        self.host.get_class(class_name).map_err(|e| CommandError::UnknownClass(class_name.to_owned()))
    }

    pub fn cache_class(&mut self, class: Class) -> Result<(), CommandError> {
        let class_name = class.fq_name.to_owned();
        self.host.cache_class(class).map_err(|e| {
            CommandError::DuplicateClass(class_name)
        })
    }

    pub fn make_instance(
            &mut self,
            pwd: &str,
            cd: &str,
            class_name: &str,
            args: &Vec<&str>) -> Result<&CommandPath, CommandError> {
        // get the right constructor
        let class = self.get_class(class_name)?;
        let constructor = match &class.constructor {
            Some(c) => c,
            None => return Err(CommandError::NoClassConstructor(class_name.to_owned()))
        };
        let param_types = constructor.param_types();
        if args.len() != param_types.len() {
            return Err(CommandError::InvalidNumberOfConstructorParameters {
                class: class_name.to_owned(),
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
                ParamType::Boolean =>
                    CommandRegistry::parse::<bool>(arg, class_name, i, pt)?.to_polar(),
                ParamType::Integer =>
                    CommandRegistry::parse::<i32>(arg, class_name, i, pt)?.to_polar(),
                ParamType::Float =>
                    CommandRegistry::parse::<f64>(arg, class_name, i, pt)?.to_polar(),
                ParamType::String => PolarValue::String(arg.to_owned()),
                /*ParamType::Instance => {
                    match self.get_instance(arg) {
                        Ok(instance) => PolarValue::new_from_instance(instance.to_owned()),
                        Err(e) => return Err(CommandError::InvalidConstructorParameter {
                            class: class_name.to_owned(),
                            param_index: i,
                            param_type: pt.clone(),
                        })
                    }
                },*/
                _ => return Err(CommandError::InvalidConstructorParameter {
                    class: class_name.to_owned(),
                    param_index: i,
                    param_type: pt.clone(),
                })
            };
            params.push(polar_value);
        }

        let instance = constructor.invoke(params)
            .map_err(|e| CommandError::InternalConstructorError {
                class: class_name.to_owned(),
                error: e,
            })?;
        self.create_path(pwd, cd, false, Some(instance))
    }

    pub fn make_instance_from_values(
            &mut self,
            pwd: &str,
            cd: &str,
            class_name: &str,
            params: Vec<PolarValue>) -> Result<&CommandPath, CommandError>{
        // get the right constructor
        let class = self.get_class(class_name)?;
        let constructor = match &class.constructor {
            Some(c) => c,
            None => return Err(CommandError::NoClassConstructor(class_name.to_owned()))
        };
        let param_types = constructor.param_types();
        if params.len() != param_types.len() {
            return Err(CommandError::InvalidNumberOfConstructorParameters {
                class: class_name.to_owned(),
                expected: param_types.len(),
                received: params.len()
            });
        }

        let instance = constructor.invoke(params)
            .map_err(|e| CommandError::InternalConstructorError {
                class: class_name.to_owned(),
                error: e,
            })?;
        self.create_path(pwd, cd, false, Some(instance))
    }

    pub fn get_instance(&self, pwd: &str) -> Option<&Instance> {
        match self.get_path(pwd) {
            Ok(path) => path.instance.as_ref(),
            Err(_) => None
        }
    }

    fn parse<T: FromStr>(
            param: &str,
            class_name: &str,
            param_index: usize,
            param_type: &ParamType) -> Result<T, CommandError> {
        param.parse().map_err(|e| CommandError::InvalidConstructorParameter {
            class: class_name.to_owned(),
            param_index,
            param_type: param_type.clone(),
        })
    }
}

pub struct CommandPath {
    children: Vec<usize>,
    id: usize,
    instance: Option<Instance>,
    name: String,
    parent: Option<usize>,
    path: String
}

impl PartialEq for CommandPath {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl CommandPath {
    pub fn associate_object(&mut self, instance: Instance) {
        self.instance = Some(instance);
    }
}

#[cfg(test)]
mod path_tests {
    use crate::core::command::registry::CommandError;
    use super::CommandRegistry;
    use crate::core::command::oso::PolarClass;

    #[test]
    fn mkdir_absolute_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/bar/me", "/foo").unwrap();

        let node = registry.mkdir("/foo", "/bar/soo").unwrap();

        assert_eq!("/bar/soo", node.path);
    }

    #[test]
    fn mkdir_creates_child_directory() {
        let mut registry = CommandRegistry::new();

        let node = registry.mkdir("/", "foo").unwrap();

        assert_eq!("foo", node.name);
        assert_eq!("/foo", node.path);
        assert_eq!(0, node.children.len());
        assert_eq!(true, node.instance.is_none());
    }

    #[test]
    fn mkdir_creates_grandchild_directories() {
        let mut registry = CommandRegistry::new();

        registry.mkdir("/", "foo/bar/soo").unwrap();

        // then
        let root = registry.get_path("/").unwrap();
        let child = registry.get_path("/foo").unwrap();
        let grandchild = registry.get_path("/foo/bar").unwrap();
        let great_grandchild = registry.get_path("/foo/bar/soo").unwrap();

        assert_eq!("soo", great_grandchild.name);
        assert_eq!("/foo/bar/soo", great_grandchild.path);
        assert_eq!(Some(grandchild.id), great_grandchild.parent);
        assert_eq!(0, great_grandchild.children.len());
        assert_eq!(true, great_grandchild.instance.is_none());

        assert_eq!("bar", grandchild.name);
        assert_eq!("/foo/bar", grandchild.path);
        assert_eq!(Some(child.id), grandchild.parent);
        assert_eq!(vec![great_grandchild.id], grandchild.children);

        assert_eq!("foo", child.name);
        assert_eq!("/foo", child.path);
        assert_eq!(Some(root.id), child.parent);
        assert_eq!(vec![grandchild.id], child.children);

        assert_eq!("", root.name);
        assert_eq!("/", root.path);
        assert_eq!(None, root.parent);
        assert_eq!(vec![child.id], root.children);
    }

    #[test]
    fn mkdir_from_child() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        let node = registry.mkdir("/foo/bar", "../soo").unwrap();

        assert_eq!("/foo/soo", node.path);
    }

    #[test]
    fn mkdir_with_absolute_path_from_child_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        let node = registry.mkdir("/foo/bar", "/soo").unwrap();

        assert_eq!("/soo", node.path);
    }

    #[test]
    fn mkdir_with_current_directory() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        let node = registry.mkdir("/foo/bar", "./soo").unwrap();

        assert_eq!("/foo/bar/soo", node.path);
    }

    #[test]
    fn mkdir_with_empty_directories() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "/foo/bar");

        let node = registry.mkdir("/foo/bar", "soo///doo").unwrap();

        assert_eq!("/foo/bar/soo/doo", node.path);
    }

    #[test]
    fn cd_to_parent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "..").unwrap();

        assert_eq!("/foo/bar", grandchild.path);
    }

    #[test]
    fn cd_to_grandparent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "../../").unwrap();

        assert_eq!("/foo", grandchild.path);
    }

    #[test]
    fn cd_to_great_grandparent() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let grandchild = registry.cd("/foo/bar/soo", "../../../").unwrap();

        assert_eq!("/", grandchild.path);
    }

    #[test]
    fn cd_beyond_root_is_error() {
        let mut registry = CommandRegistry::new();
        registry.mkdir("/", "foo/bar/soo");

        let result = registry.cd("/foo/bar/soo", "../../../..").err().unwrap();

        match result {
            CommandError::PathNavigationError { pwd, cd } => {
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
            CommandError::PathNavigationError { pwd, cd } => {
                assert_eq!("/foo/bar/soo".to_owned(), pwd);
                assert_eq!("doo".to_owned(), cd);
            },
            _ => assert!(false)
        }
    }
}

#[cfg(test)]
mod registration_tests {
    use crate::core::command::registry::CommandRegistry;
    use crate::core::command::oso::PolarClass;

    #[derive(PolarClass, Clone, Default)]
    struct User {
        pub username: String,
        #[polar(attribute)]
        pub user_id: i32
    }

    impl User {
        fn new(username: String, user_id: i32) -> User {
            User { username, user_id }
        }

        fn add_one(&self, num: i32) -> i32 {
            num + 1
        }
    }

    #[test]
    fn class_registration() {
        let mut registry = CommandRegistry::new();

        registry.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec![])
            .build());
    }
}

#[cfg(test)]
mod oso_integration_tests {
    use crate::core::command::oso::{FromPolar, Host, Instance, InvalidCallError, OsoError, ParamType, PolarValue, ToPolar};
    use crate::core::command::oso::PolarClass;
    use crate::core::command::registry::CommandRegistry;

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

        registry.make_instance_from_values("/foo", ".", "User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let instance = registry.get_instance("/foo").unwrap();
        let user = instance.downcast::<User>(None).unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params() {
        let mut registry = create_registry();

        registry.make_instance_from_values("/foo", ".", "User2", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();

        let instance = registry.get_instance("/foo").unwrap();
        let user = instance.downcast::<User2>(None).unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    /*
    #[test]
    fn get_attribute() {
        let mut registry = create_registry();
        registry.make_instance_from_values("foo",  ".", "User2", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();
        let instance = registry.get_instance("foo").unwrap();

        let result = instance.get_attr(&"user_id", &registry).unwrap();

        assert_eq!(PolarValue::Integer(42), result);
    }

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
                              vec![ParamType::Integer, ParamType::Integer])
            .add_typed_method("bar", Foo::bar, vec![])
            .build();
        registry.cache_class(bar_class);
        registry.cache_class(foo_class);
        registry.make_instance_from_values("foo/bar", ".", "Foo", vec![]).unwrap();
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