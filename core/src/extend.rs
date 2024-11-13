use crate::curve::{Affine, Jacobian};

impl From<&Jacobian> for Affine {
    fn from(value: &Jacobian) -> Self {
        let mut ra = Affine::from_gej(value);
        ra.x.normalize();
        ra.y.normalize();
        ra
    }
}

impl From<Jacobian> for Affine {
    fn from(value: Jacobian) -> Self {
        Affine::from(&value)
    }
}
