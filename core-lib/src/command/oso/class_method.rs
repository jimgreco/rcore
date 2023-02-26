//! Wrapper structs for the generic `Function` and `Method` traits
use std::sync::Arc;
use super::{PolarIterator, ToPolar, ToPolarResult, ParamType, Class, Instance, PolarValue,
            FromPolarList, Host};
use super::method::{Function, Method};

fn join<A, B>(left: crate::command::oso::Result<A>, right: crate::command::oso::Result<B>) -> super::Result<(A, B)> {
    left.and_then(|l| right.map(|r| (l, r)))
}

type TypeErasedFunction<R> = Arc<dyn Fn(Vec<PolarValue>) -> crate::command::oso::Result<R> + Send + Sync>;
type TypeErasedMethod<R> =
    Arc<dyn Fn(&Instance, Vec<PolarValue>, &Host) -> super::Result<R> + Send + Sync>;

#[derive(Clone)]
pub struct Constructor(TypeErasedFunction<Instance>, Vec<ParamType>);

impl Constructor {
    pub fn new<Args, F>(f: F, param_types: Vec<ParamType>) -> Self
    where
        Args: FromPolarList,
        F: Function<Args>,
        F::Result: Send + Sync + 'static,
    {
        Constructor(Arc::new(move |args: Vec<PolarValue>| {
            Args::from_polar_list(&args).map(|args| Instance::new(f.invoke(args)))
        }),
        param_types)
    }

    pub fn invoke(&self, args: Vec<PolarValue>) -> crate::command::oso::Result<Instance> {
        self.0(args)
    }

    pub fn param_types(&self) -> &Vec<ParamType> {
        &self.1
    }
}

type AttributeGetterMethod =
    Arc<dyn Fn(&Instance, &Host) -> crate::command::oso::Result<PolarValue> + Send + Sync>;

#[derive(Clone)]
pub struct AttributeGetter(AttributeGetterMethod);

impl AttributeGetter {
    pub fn new<T, F, R>(f: F) -> Self
    where
        T: 'static,
        F: Fn(&T) -> R + Send + Sync + 'static,
        R: ToPolarResult,
    {
        Self(Arc::new(move |receiver, host: &Host| {
            let receiver = receiver
                .downcast(Some(host))
                .map_err(|e| e.invariant().into());
            receiver.map(&f).and_then(|v| v.to_polar_result())
        }))
    }

    pub fn invoke(&self, receiver: &Instance, host: &Host) -> crate::command::oso::Result<PolarValue> {
        self.0(receiver, host)
    }
}

#[derive(Clone)]
pub struct InstanceMethod(TypeErasedMethod<PolarValue>, Vec<ParamType>, Option<String>);

impl InstanceMethod {
    pub fn new<T, F, Args>(f: F, param_types: Vec<ParamType>, path: Option<String>) -> Self
    where
        Args: FromPolarList,
        F: Method<T, Args>,
        F::Result: ToPolarResult,
        T: 'static,
    {
        Self(
            Arc::new(
                move |receiver: &Instance, args: Vec<PolarValue>, host: &Host| {
                    let receiver = receiver
                        .downcast(Some(host))
                        .map_err(|e| e.invariant().into());

                    let args = Args::from_polar_list(&args);

                    join(receiver, args)
                        .and_then(|(receiver, args)| f.invoke(receiver, args).to_polar_result())
                },
            ),
            param_types,
            path
        )
    }

    pub fn new_iterator<T, F, Args, I>(f: F) -> Self
    where
        Args: FromPolarList,
        F: Method<T, Args>,
        F::Result: IntoIterator<Item = I>,
        <<F as Method<T, Args>>::Result as IntoIterator>::IntoIter:
            Iterator<Item = I> + Clone + Send + Sync + 'static,
        I: ToPolarResult,
        T: 'static,
    {
        Self(
            Arc::new(
                move |receiver: &Instance, args: Vec<PolarValue>, host: &Host| {
                    let receiver = receiver
                        .downcast(Some(host))
                        .map_err(|e| e.invariant().into());

                    let args = Args::from_polar_list(&args);

                    join(receiver, args)
                        .map(|(receiver, args)| {
                            PolarIterator::new(f.invoke(receiver, args).into_iter())
                        })
                        .map(|results| results.to_polar())
                },
            ),
            vec![],
            None
        )
    }

    pub fn invoke(
        &self,
        receiver: &Instance,
        args: Vec<PolarValue>,
        host: &Host,
    ) -> crate::command::oso::Result<PolarValue> {
        self.0(receiver, args, host)
    }

    pub fn from_class_method(name: String) -> Self {
        Self (
            Arc::new(
                move |receiver: &Instance, args: Vec<PolarValue>, host: &Host| {
                    receiver
                        .downcast::<Class>(Some(host))
                        .map_err(|e| e.invariant().into())
                        .and_then(|class| {
                            tracing::trace!(class = %class.name, method=%name, "class_method");
                            class.call(&name, args)
                        })
                },
            ),
            vec![],
            None
        )
    }

    pub fn param_types(&self) -> &Vec<ParamType> {
        &self.1
    }

    pub fn path(&self) -> &Option<String> {
        &self.2
    }
}

#[derive(Clone)]
pub struct ClassMethod(TypeErasedFunction<PolarValue>);

impl ClassMethod {
    pub fn new<F, Args>(f: F) -> Self
    where
        Args: FromPolarList,
        F: Function<Args>,
        F::Result: ToPolarResult,
    {
        Self(Arc::new(move |args: Vec<PolarValue>| {
            Args::from_polar_list(&args).and_then(|args| f.invoke(args).to_polar_result())
        }))
    }

    pub fn invoke(&self, args: Vec<PolarValue>) -> crate::command::oso::Result<PolarValue> {
        self.0(args)
    }
}
