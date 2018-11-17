//! Kernels

use float::Float;

/// Kernel function
pub trait Kernel<A>: Copy + Sync
where
    A: Float,
{
    /// Apply the kernel function to the given x-value.
    fn evaluate(&self, x: A) -> A;
}

/// Gaussian kernel
#[derive(Clone, Copy)]
pub struct Gaussian;

impl<A> Kernel<A> for Gaussian
where
    A: Float,
{
    fn evaluate(&self, x: A) -> A {
        use std::f32::consts::PI;

        // 1 / sqrt(e^(x^2) * 2 * pi) = e^(x^2 * -.5) * (1 / sqrt(2 * pi))
        (x.powi(2) * A::cast(-0.5)).exp() * (A::cast(2. * PI)).sqrt().recip()
    }
}

#[cfg(test)]
macro_rules! test {
    ($ty:ident) => {
        mod $ty {
            mod gaussian {
                use quickcheck::TestResult;

                use univariate::kde::kernel::{Gaussian, Kernel};

                quickcheck!{
                    fn symmetric(x: $ty) -> bool {
                        relative_eq!(Gaussian.evaluate(-x), Gaussian.evaluate(x))
                    }
                }

                // Any [a b] integral should be in the range [0 1]
                quickcheck!{
                    fn integral(a: $ty, b: $ty) -> TestResult {
                        const DX: $ty = 1e-3;

                        if a > b {
                            TestResult::discard()
                        } else {
                            let mut acc = 0.;
                            let mut x = a;
                            let mut y = Gaussian.evaluate(a);

                            while x < b {
                                acc += DX * y / 2.;

                                x += DX;
                                y = Gaussian.evaluate(x);

                                acc += DX * y / 2.;
                            }

                            TestResult::from_bool(
                                (acc > 0. || relative_eq!(acc, 0.)) &&
                                (acc < 1. || relative_eq!(acc, 1.)))
                        }
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    test!(f32);
    test!(f64);
}
