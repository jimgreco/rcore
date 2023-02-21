#[cfg(test)]
mod tests {
    use std::rc::Weak;
    use oso::{Host, Instance, InvalidCallError, OsoError, PolarClass, PolarValue};

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

    #[test]
    fn make_instance_1_param() {
        let mut host = Host::new();
        host.cache_class(User::get_polar_class_builder()
                             .set_constructor(User::new)
                             .build(),
                         "User".to_owned());

        let id = host.make_instance(&"User", vec![PolarValue::String("jim".to_owned())]).unwrap();

        let instance: &Instance = host.get_instance(id).unwrap();
        let user: &User = instance.downcast::<User>(None).unwrap();
        assert_eq!(user.username, "jim");
    }

    #[test]
    fn make_instance_2_params() {
        let mut host = Host::new();
        host.cache_class(User2::get_polar_class_builder()
                             .set_constructor(User2::new)
                             .build(),
                         "User".to_owned());

        let id = host.make_instance(&"User", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();

        let instance: &Instance = host.get_instance(id).unwrap();
        let user: &User2 = instance.downcast::<User2>(None).unwrap();
        assert_eq!(user.username, "jim");
        assert_eq!(user.user_id, 42);
    }

    #[test]
    fn get_attribute() {
        let mut host = Host::new();
        host.cache_class(User2::get_polar_class_builder()
                             .set_constructor(User2::new)
                             .build(),
                         "User".to_owned());
        let id = host.make_instance(&"User", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(42)]).unwrap();
        let instance: &Instance = host.get_instance(id).unwrap();

        assert_eq!(PolarValue::Integer(42), instance.get_attr(&"user_id", &host).unwrap());
    }

    #[test]
    fn execute_instance_method() {
        let mut host = Host::new();
        host.cache_class(User2::get_polar_class_builder()
                             .set_constructor(User2::new)
                             .add_method("add_one", User2::add_one)
                             .build(),
                         "User".to_owned());
        let id = host.make_instance(&"User", vec![PolarValue::String("jim".to_owned()), PolarValue::Integer(10)]).unwrap();
        let instance: &Instance = host.get_instance(id).unwrap();

        assert_eq!(PolarValue::Integer(43), instance.call(&"add_one", vec![PolarValue::Integer(42)], &host).unwrap());
    }
}