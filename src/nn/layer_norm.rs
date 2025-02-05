use crate::{gradients::Tape, optim::*, shapes::*, tensor::*, tensor_ops::*};

use super::{BuildModule, Module, ModuleMut, ResetParams, ToDevice};

/// Implements layer normalization as described in [Layer Normalization](https://arxiv.org/abs/1607.06450).
///
/// This calls [normalize()] on the last axis of the input to normalize to 0 mean and unit std dev, and then does an element-wise
/// affine transform using learnable parameters [Self::gamma] and [Self::beta].
///
/// [Self::epsilon] is passed to [normalize()] and added to the variance to ensure big enough numbers. It defaults to `1e-5`.
///
/// # Generics
/// - `M` The size of the affine transform tensors.
///
/// # Examples
/// ```rust
/// # use dfdx::prelude::*;
/// # let dev: Cpu = Default::default();
/// type Model = LayerNorm1D<5>;
/// let model = Model::build_on_device(&dev);
/// let _: Tensor<Rank1<5>, f32, _> = model.forward(dev.zeros::<Rank1<5>>());
/// ```
#[derive(Debug, Clone)]
pub struct LayerNorm1D<const M: usize, D: Device<f32> = Cpu> {
    pub gamma: Tensor<Rank1<M>, f32, D>,
    pub beta: Tensor<Rank1<M>, f32, D>,
    pub epsilon: f32,
}

impl<const M: usize, D: Device<f32>> BuildModule<D, f32> for LayerNorm1D<M, D> {
    /// Fills [Self::gamma] with 1s and [Self::beta] with 0s and sets [Self::epsilon] to `1e-5`.
    fn try_build(device: &D) -> Result<Self, D::Err> {
        Ok(Self {
            gamma: device.try_ones()?,
            beta: device.try_zeros()?,
            epsilon: 1e-5,
        })
    }
}

impl<const M: usize, D: Device<f32>> ResetParams<D, f32> for LayerNorm1D<M, D> {
    fn try_reset_params(&mut self) -> Result<(), D::Err> {
        self.gamma.try_fill_with_ones()?;
        self.beta.try_fill_with_zeros()?;
        Ok(())
    }
}

impl<const M: usize, D1: Device<f32>, D2: Device<f32>> ToDevice<D2> for LayerNorm1D<M, D1> {
    type Output = LayerNorm1D<M, D2>;

    fn to_device(&self, device: &D2) -> Self::Output {
        LayerNorm1D {
            gamma: self.gamma.to_device(device),
            beta: self.beta.to_device(device),
            epsilon: self.epsilon,
        }
    }
}

impl<const M: usize, D: Device<f32>> GradientUpdate<D, f32> for LayerNorm1D<M, D> {
    fn update<U>(&mut self, updater: &mut U, unused: &mut UnusedTensors) -> Result<(), <D>::Err>
    where
        U: ParamUpdater<D, f32>,
    {
        self.gamma.update(updater, unused)?;
        self.beta.update(updater, unused)?;
        Ok(())
    }
}

impl<const M: usize, D: Device<f32>, T: Tape<D>> Module<Tensor<Rank1<M>, f32, D, T>>
    for LayerNorm1D<M, D>
{
    type Output = Tensor<Rank1<M>, f32, D, T>;
    fn forward(&self, x: Tensor<Rank1<M>, f32, D, T>) -> Self::Output {
        x.normalize(self.epsilon) * self.gamma.clone() + self.beta.clone()
    }
}

impl<B: Dim, const M: usize, D: Device<f32>, T: Tape<D>> Module<Tensor<(B, Const<M>), f32, D, T>>
    for LayerNorm1D<M, D>
{
    type Output = Tensor<(B, Const<M>), f32, D, T>;
    fn forward(&self, x: Tensor<(B, Const<M>), f32, D, T>) -> Self::Output {
        let shape = *x.shape();
        x.normalize::<Axis<1>>(self.epsilon) * self.gamma.retaped::<T>().broadcast_like(&shape)
            + self.beta.retaped::<T>().broadcast_like(&shape)
    }
}

impl<B: Dim, S: Dim, const M: usize, D: Device<f32>, T: Tape<D>>
    Module<Tensor<(B, S, Const<M>), f32, D, T>> for LayerNorm1D<M, D>
{
    type Output = Tensor<(B, S, Const<M>), f32, D, T>;
    fn forward(&self, x: Tensor<(B, S, Const<M>), f32, D, T>) -> Self::Output {
        let shape = *x.shape();
        x.normalize::<Axis<2>>(self.epsilon) * self.gamma.retaped::<T>().broadcast_like(&shape)
            + self.beta.retaped::<T>().broadcast_like(&shape)
    }
}

impl<T, const M: usize, D: Device<f32>> ModuleMut<T> for LayerNorm1D<M, D>
where
    Self: Module<T>,
{
    type Output = <Self as Module<T>>::Output;
    fn forward_mut(&mut self, input: T) -> Self::Output {
        self.forward(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nn::tests::SimpleUpdater;
    use crate::tests::{assert_close, TestDevice};
    use crate::unique_id::HasUniqueId;

    #[test]
    fn test_layer_norm_reset() {
        let dev: TestDevice = Default::default();

        let mut m: LayerNorm1D<5, _> = BuildModule::build(&dev);
        assert_eq!(m.gamma.array(), [1.0; 5]);
        assert_eq!(m.beta.array(), [0.0; 5]);

        m.gamma = dev.sample_normal();
        m.beta = dev.sample_normal();

        assert_ne!(m.gamma.array(), [1.0; 5]);
        assert_ne!(m.beta.array(), [0.0; 5]);

        m.reset_params();

        assert_eq!(m.gamma.array(), [1.0; 5]);
        assert_eq!(m.beta.array(), [0.0; 5]);
    }

    #[test]
    fn test_layer_norm_1d_forward() {
        let dev: TestDevice = Default::default();
        let mut m: LayerNorm1D<5, _> = BuildModule::build(&dev);
        let x = dev.sample_normal::<Rank1<5>>();
        let r = m.forward_mut(x.trace());
        assert_close(
            &r.array(),
            &[0.873304, 0.9879816, -1.6083492, 0.44028836, -0.6932247],
        );
        let g = r.mean().backward();
        assert_close(
            &g.get(&m.gamma).array(),
            &[0.1746608, 0.19759633, -0.32166985, 0.088057674, -0.13864495],
        );
        assert_close(&g.get(&m.beta).array(), &[0.2; 5]);
    }

    #[test]
    fn test_layer_norm_2d_forward() {
        let dev: TestDevice = Default::default();
        let m: LayerNorm1D<5, _> = BuildModule::build(&dev);
        let x = dev.sample_normal::<Rank2<3, 5>>();
        let r = m.forward(x.trace());
        assert_close(
            &r.array(),
            &[
                [0.873304, 0.9879816, -1.6083492, 0.44028836, -0.6932247],
                [0.663322, -1.8449169, 0.05217871, 0.056903206, 1.0725129],
                [1.0343355, -1.5559655, -0.40086073, 1.1405537, -0.21806297],
            ],
        );
        let g = r.mean().backward();
        assert_close(
            &g.get(&m.gamma).array(),
            &[0.1713974, -0.16086, -0.1304687, 0.109183, 0.0107483],
        );
        assert_close(&g.get(&m.beta).array(), &[0.2; 5]);
    }

    #[test]
    fn test_layer_norm_missing_gradients() {
        let dev: TestDevice = Default::default();

        let mut model: LayerNorm1D<5, _> = BuildModule::build(&dev);
        let mut g: SimpleUpdater = Default::default();

        // no gradients present
        let mut unused = Default::default();
        model.update(&mut g, &mut unused).unwrap();
        assert_eq!(&unused.ids, &[*model.gamma.id(), *model.beta.id()]);

        g.0.try_alloc_for(&model.gamma).unwrap();

        // weight gradient is present
        let mut unused = Default::default();
        model.update(&mut g, &mut unused).unwrap();
        assert_eq!(&unused.ids, &[*model.beta.id()]);

        g.0.try_alloc_for(&model.gamma).unwrap();
        g.0.try_alloc_for(&model.beta).unwrap();

        // all gradients present
        let mut unused = Default::default();
        model.update(&mut g, &mut unused).unwrap();
        assert!(unused.is_empty());
    }
}
