use rand::distributions::Distribution;
use rand_distr::{Standard, StandardNormal};

use crate::{
    shapes::{ConstShape, Dtype, HasShape, HasUnitType, Shape, Unit},
    unique_id::unique_id,
};

use super::Tensor;

/// Represents something that has an error associated type
pub trait HasErr: Sized {
    type Err: std::fmt::Debug + std::fmt::Display;
}

/// Something that can store nd arrays for a given [Shape] and [Dtype]
pub trait DeviceStorage: 'static + Default + Clone + HasErr {
    /// Generic storage type
    type Storage<S: Shape, E: Unit>: 'static
        + std::fmt::Debug
        + Clone
        + Send
        + Sync
        + HasShape<Shape = S>;

    /// Generates a random u64 number
    fn random_u64(&self) -> u64;

    /// Allocates a gradient for the given nd array
    fn try_alloc_grad<S: Shape, E: Dtype>(
        &self,
        storage: &Self::Storage<S, E>,
    ) -> Result<Self::Storage<S, E>, Self::Err>;

    /// Upgrades the device storage into a tensor
    fn upgrade<S: Shape, E: Unit>(&self, storage: Self::Storage<S, E>) -> Tensor<S, E, Self> {
        Tensor {
            id: unique_id(),
            storage,
            device: self.clone(),
            tape: Default::default(),
        }
    }
}

/// Internal trait - Represents something that can allocate its own gradient.
pub trait AllocGrad: HasErr {
    type Gradient: 'static;
    fn try_alloc_grad(&self) -> Result<Self::Gradient, Self::Err>;
}

impl<S: Shape, E: Dtype, D: DeviceStorage, T> AllocGrad for Tensor<S, E, D, T> {
    type Gradient = D::Storage<S, E>;
    fn try_alloc_grad(&self) -> Result<Self::Gradient, D::Err> {
        self.device.try_alloc_grad(&self.storage)
    }
}

/// Enables copying data into and out of tensors
pub trait CopySlice<E: Unit>: DeviceStorage {
    fn copy_from<S: Shape, T>(dst: &mut Tensor<S, E, Self, T>, src: &[E]);
    fn copy_into<S: Shape, T>(src: &Tensor<S, E, Self, T>, dst: &mut [E]);
}

impl<S: Shape, E: Unit, D: CopySlice<E>, T> Tensor<S, E, D, T> {
    /// Copy data from a slice - **panics** if there are not enough elements in the slice.
    ///
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let data = [1.0, 2.0, 3.0, 4.0];
    /// let mut t: Tensor<Rank2<2, 2>, f32, _> = dev.zeros();
    /// t.copy_from(&data);
    /// assert_eq!(t.array(), [[1.0, 2.0], [3.0, 4.0]]);
    /// ```
    pub fn copy_from(&mut self, src: &[E]) {
        D::copy_from(self, src);
    }

    /// Copy data into a slice - **panics** if there are not enough elements in the tensor.
    ///
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let t: Tensor<Rank2<2, 2>, f32, _> = dev.tensor([[1.0, 2.0], [3.0, 4.0]]);
    /// let mut data = [0.0; 4];
    /// t.copy_into(&mut data);
    /// assert_eq!(data, [1.0, 2.0, 3.0, 4.0]);
    /// ```
    pub fn copy_into(&self, dst: &mut [E]) {
        D::copy_into(self, dst);
    }
}

/// Construct tensors filled with zeros.
pub trait ZerosTensor<E: Unit>: DeviceStorage {
    /// Creates a tensor filled with zeros.
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<Rank2<2, 3>, f32, _> = dev.zeros();
    /// ```
    fn zeros<S: ConstShape>(&self) -> Tensor<S, E, Self> {
        self.try_zeros_like::<S>(&Default::default()).unwrap()
    }

    /// Fallible version of [ZerosTensor::zeros]
    fn try_zeros<S: ConstShape>(&self) -> Result<Tensor<S, E, Self>, Self::Err> {
        self.try_zeros_like::<S>(&Default::default())
    }

    /// Build the tensor with a shape given by something else.
    ///
    /// Given a shape directly:
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<(usize, Const<3>), f32, _> = dev.zeros_like(&(5, Const));
    /// ```
    ///
    /// Given another tensor:
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<Rank2<2, 3>, f32, _> = dev.zeros();
    /// let b: Tensor<Rank2<2, 3>, f32, _> = dev.zeros_like(&a);
    /// ```
    fn zeros_like<S: HasShape>(&self, src: &S) -> Tensor<S::Shape, E, Self> {
        self.try_zeros_like(src).unwrap()
    }

    /// Fallible version of [ZerosTensor::zeros_like]
    fn try_zeros_like<S: HasShape>(&self, src: &S) -> Result<Tensor<S::Shape, E, Self>, Self::Err>;
}

pub trait ZeroFillStorage<E: Unit>: DeviceStorage {
    fn try_fill_with_zeros<S: Shape>(
        &self,
        storage: &mut Self::Storage<S, E>,
    ) -> Result<(), Self::Err>;
}

/// Construct tensors filled with ones.
pub trait OnesTensor<E: Unit>: DeviceStorage {
    /// Creates a tensor filled with ones.
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<Rank2<2, 3>, f32, _> = dev.ones();
    /// ```
    fn ones<S: ConstShape>(&self) -> Tensor<S, E, Self> {
        self.try_ones_like::<S>(&Default::default()).unwrap()
    }

    /// Fallible version of [OnesTensor::ones]
    fn try_ones<S: ConstShape>(&self) -> Result<Tensor<S, E, Self>, Self::Err> {
        self.try_ones_like::<S>(&Default::default())
    }

    /// Build the tensor with a shape given by something else.
    ///
    /// Given a shape directly:
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<(usize, Const<3>), f32, _> = dev.ones_like(&(5, Const));
    /// ```
    ///
    /// Given another tensor:
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let a: Tensor<Rank2<2, 3>, f32, _> = dev.ones();
    /// let b: Tensor<_, f32, _> = dev.ones_like(&a);
    /// ```
    fn ones_like<S: HasShape>(&self, src: &S) -> Tensor<S::Shape, E, Self> {
        self.try_ones_like(src).unwrap()
    }

    /// Fallible version of [OnesTensor::ones_like]
    fn try_ones_like<S: HasShape>(&self, src: &S) -> Result<Tensor<S::Shape, E, Self>, Self::Err>;
}

pub trait OneFillStorage<E: Unit>: DeviceStorage {
    fn try_fill_with_ones<S: Shape>(
        &self,
        storage: &mut Self::Storage<S, E>,
    ) -> Result<(), Self::Err>;
}

/// Constructs tensors filled with random values from a given distribution.
pub trait SampleTensor<E: Unit>: DeviceStorage {
    fn sample_uniform<S: ConstShape>(&self) -> Tensor<S, E, Self>
    where
        Standard: Distribution<E>,
    {
        self.sample::<S, _>(Standard)
    }

    fn sample_normal<S: ConstShape>(&self) -> Tensor<S, E, Self>
    where
        StandardNormal: Distribution<E>,
    {
        self.sample::<S, _>(StandardNormal)
    }

    fn sample<S: ConstShape, D: Distribution<E>>(&self, distr: D) -> Tensor<S, E, Self> {
        self.try_sample_like::<S, D>(&Default::default(), distr)
            .unwrap()
    }
    fn try_sample<S: ConstShape, D: Distribution<E>>(
        &self,
        distr: D,
    ) -> Result<Tensor<S, E, Self>, Self::Err> {
        self.try_sample_like::<S, D>(&Default::default(), distr)
    }
    fn sample_like<S: HasShape, D: Distribution<E>>(
        &self,
        src: &S,
        distr: D,
    ) -> Tensor<S::Shape, E, Self> {
        self.try_sample_like(src, distr).unwrap()
    }
    fn try_sample_like<S: HasShape, D: Distribution<E>>(
        &self,
        src: &S,
        distr: D,
    ) -> Result<Tensor<S::Shape, E, Self>, Self::Err>;

    fn try_fill_with_distr<S: Shape, D: Distribution<E>>(
        &self,
        storage: &mut Self::Storage<S, E>,
        distr: D,
    ) -> Result<(), Self::Err>;
}

/// Construct tensors from rust arrays
pub trait TensorFromArray<Src, S: Shape, E: Unit>: DeviceStorage {
    /// Create a tensor from a rust array
    /// ```rust
    /// # use dfdx::prelude::*;
    /// # let dev: Cpu = Default::default();
    /// let _: Tensor<Rank2<2, 3>, f32, _> = dev.tensor([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    /// ```
    fn tensor(&self, src: Src) -> Tensor<S, E, Self> {
        self.try_tensor(src).unwrap()
    }
    /// Fallible version of [TensorFromArray::tensor]
    fn try_tensor(&self, src: Src) -> Result<Tensor<S, E, Self>, Self::Err>;
}

/// Convert tensors to rust arrays
pub trait AsArray {
    type Array: std::fmt::Debug + PartialEq;
    fn array(&self) -> Self::Array;
}
impl<S: Shape, E: Unit, D: DeviceStorage, T> AsArray for Tensor<S, E, D, T>
where
    D::Storage<S, E>: AsArray,
{
    type Array = <D::Storage<S, E> as AsArray>::Array;
    fn array(&self) -> Self::Array {
        self.storage.array()
    }
}

/// Convert tensors to [std::vec::Vec]
pub trait AsVec: HasUnitType {
    fn as_vec(&self) -> std::vec::Vec<Self::Unit>;
}

impl<S: Shape, E: Unit, D: DeviceStorage, T> AsVec for Tensor<S, E, D, T>
where
    D::Storage<S, E>: HasUnitType<Unit = E> + AsVec,
{
    fn as_vec(&self) -> std::vec::Vec<E> {
        self.storage.as_vec()
    }
}
