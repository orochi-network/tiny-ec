use core::{cmp::Ordering, convert::TryInto, fmt, panic};
use serde::{de, Deserialize, Serialize};

use crate::{
    curve::{Affine, Jacobian, Scalar},
    field::Field,
};

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

impl From<&[u8; 64]> for Affine {
    fn from(value: &[u8; 64]) -> Self {
        let mut x = Field::default();
        let mut y = Field::default();
        if x.set_b32(value[0..32].try_into().unwrap())
            && y.set_b32(value[32..64].try_into().unwrap())
        {
            let mut r = Affine::default();
            r.set_xy(&x, &y);
            r.x.normalize();
            r.y.normalize();
            return r;
        }
        panic!("Failed to construct Affine from bytes")
    }
}

impl From<&[u8]> for Affine {
    fn from(value: &[u8]) -> Self {
        if value.len() != 64 {
            panic!("Bytes length must be 64 for Affine")
        }
        let mut tmp_bytes = [0u8; 64];
        tmp_bytes[0..64].copy_from_slice(value);
        Affine::from(&tmp_bytes)
    }
}

impl Into<[u8; 64]> for Affine {
    fn into(self) -> [u8; 64] {
        let mut ret = [0u8; 64];
        ret[0..32].copy_from_slice(&self.x.b32());
        ret[32..64].copy_from_slice(&self.y.b32());
        ret
    }
}

impl Affine {
    pub fn compose(x: &Field, z: &Field) -> Affine {
        let mut r = Affine::default();
        r.x = x.clone();
        r.y = z.clone();
        r.x.normalize();
        r.y.normalize();
        r
    }
}

struct AffineBytesVisitor;

#[cfg(feature = "std")]
impl<'de> de::Visitor<'de> for AffineBytesVisitor {
    type Value = Affine;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a byte slice that is 64 bytes in length")
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Affine::from(value))
    }
}

impl Serialize for Affine {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        [self.x.b32(), self.y.b32()].concat().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Affine {
    fn deserialize<D>(deserializer: D) -> Result<Affine, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(AffineBytesVisitor)
    }
}

impl PartialOrd for Scalar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Scalar {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut ret = Ordering::Equal;
        for i in (0..8).rev() {
            if self.0[i] < other.0[i] {
                ret = Ordering::Less;
                break;
            } else if self.0[i] > other.0[i] {
                ret = Ordering::Greater;
                break;
            }
        }
        ret
    }
}

impl From<&[u8]> for Scalar {
    fn from(bytes: &[u8]) -> Self {
        if bytes.len() != 32 {
            panic!("Bytes length must be 32")
        }
        let mut tmp_bytes = [0u8; 32];
        tmp_bytes[0..32].copy_from_slice(bytes);
        Scalar::from(&tmp_bytes)
    }
}

impl From<&[u8; 32]> for Scalar {
    fn from(bytes: &[u8; 32]) -> Self {
        let mut r = Scalar::default();
        r.set_b32(bytes).unwrap_u8();
        r
    }
}

impl Serialize for Scalar {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D>(deserializer: D) -> Result<Scalar, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Ok(Scalar::from(&bytes[..]))
    }
}
