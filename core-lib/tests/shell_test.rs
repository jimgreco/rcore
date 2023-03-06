#[allow(unused_imports)]
#[macro_use]
extern crate oso_derive;

#[doc(hidden)]
pub use oso_derive::*;

use rcore::command::{CommandContext, IoContext, Shell, UserContext};
use rcore::command::oso::Class;
use rcore::command::oso::PolarClass;

#[test]
fn test_add() {
    assert_eq!(3, rcore::add(1, 2));
}

/*
#[derive(PolarClass, Clone)]
struct Foo {
    bar: Bar
}

#[derive(PolarClass, Clone)]
struct Bar {
    #[polar(attribute)]
    name: String
}

#[test]
fn shell_integration() {
    impl Foo {
       fn new(bar: Bar) -> Self { Foo { bar } }
       fn add_one(&self, value: i32) -> i32 { value + 1 }
    }

    impl Bar {
        fn new(name: String) -> Self { Bar { name } }
    }

    // The use of annotations make much of this class building unnecessary
    let mut shell = Shell::default();
    Foo::get_polar_class_build();
    shell.cache_class(Class::builder().set_constructor(Bar::new, vec!["string"])
              .add_attribute_getter("name", |recv: &Bar| recv.name.clone()).build()).unwrap();
    shell.cache_class(Class::builder().set_constructor(Foo::new, vec!["Bar"])
              .add_method("add_one", Foo::add_one, vec!["int"], None).build()).unwrap();

    let mut input = std::io::Cursor::new("
        create /bar Bar \"Mr. Burns\"
        create /foo Foo @/bar
        /bar/name
        echo \" \"
        /foo/add_one 41".as_bytes());
    let mut output_vec: Vec<u8> = Vec::new();
    let mut output = std::io::Cursor::new(&mut output_vec);
    let mut io_context = IoContext::new("test", &mut input, &mut output);
    let mut user_context = UserContext::default();
    let com_context = CommandContext::default();
    let mut shell = Shell::default();
    shell.execute_commands(&mut user_context, &mut io_context, &com_context).unwrap();

    assert_eq!("Mr. Burns 42", &String::from_utf8(output_vec).unwrap());
}
*/