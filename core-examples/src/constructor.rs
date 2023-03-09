use rcore::command::Shell;
use rcore::command::oso::PolarClass;

#[derive(PolarClass, Debug)]
struct Foo {
    id: i32
}

impl Foo {
    fn new(id: i32) -> Foo { Foo { id } }
}

pub fn run() {
    let mut shell = Shell::default();
    shell.cache_class(Foo::get_polar_class_builder()
        .set_constructor(Foo::new, vec!["int"])
        .build());
}
