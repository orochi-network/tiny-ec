//! Pure Rust implementation of the secp256k1 curve and fast ECDSA
//! signatures. The secp256k1 curve is used extensively in Bitcoin and
//! Ethereum-alike cryptocurrencies.

#![deny(
    unused_import_braces,
    unused_imports,
    unused_comparisons,
    unused_must_use,
    unused_variables,
    non_shorthand_field_patterns,
    unreachable_code,
    unused_parens
)]
#![cfg_attr(not(feature = "std"), no_std)]

use curve::Field;
pub use tiny_ec_core::*;

use arrayref::{array_mut_ref, array_ref};
use core::convert::TryFrom;

use crate::curve::{Affine, ECMultContext, ECMultGenContext, Jacobian, Scalar};
#[cfg(feature = "std")]
#[cfg(all(feature = "static-context"))]
/// A static ECMult context.
// Correct `pre_g` values are fed into `ECMultContext::new_from_raw`, generated by build script.
pub static ECMULT_CONTEXT: ECMultContext =
    unsafe { ECMultContext::new_from_raw(include!(concat!(env!("OUT_DIR"), "/const.rs"))) };

#[cfg(all(feature = "static-context"))]
/// A static ECMultGen context.
// Correct `prec` values are fed into `ECMultGenContext::new_from_raw`, generated by build script.
pub static ECMULT_GEN_CONTEXT: ECMultGenContext =
    unsafe { ECMultGenContext::new_from_raw(include!(concat!(env!("OUT_DIR"), "/const_gen.rs"))) };

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Public key on a secp256k1 curve.
pub struct PublicKey(Affine);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Secret key (256-bit) on a secp256k1 curve.
pub struct SecretKey(Scalar);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// An ECDSA signature.
pub struct Signature {
    pub r: Scalar,
    pub s: Scalar,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Tag used for public key recovery from signatures.
pub struct RecoveryId(u8);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// Hashed message input to an ECDSA signature.
pub struct Message(pub Scalar);

/// Format for public key parsing.
pub enum PublicKeyFormat {
    /// Compressed public key, 33 bytes.
    Compressed,
    /// Full length public key, 65 bytes.
    Full,
    /// Raw public key, 64 bytes.
    Raw,
}

impl PublicKey {
    pub fn from_secret_key_with_context(
        seckey: &SecretKey,
        context: &ECMultGenContext,
    ) -> PublicKey {
        let mut pj = Jacobian::default();
        context.ecmult_gen(&mut pj, &seckey.0);
        let mut p = Affine::default();
        p.set_gej(&pj);
        PublicKey(p)
    }

    #[cfg(any(feature = "static-context"))]
    pub fn from_secret_key(seckey: &SecretKey) -> PublicKey {
        Self::from_secret_key_with_context(seckey, &ECMULT_GEN_CONTEXT)
    }

    pub fn parse_slice(p: &[u8], format: Option<PublicKeyFormat>) -> Result<PublicKey, Error> {
        let format = match (p.len(), format) {
            (util::FULL_PUBLIC_KEY_SIZE, None)
            | (util::FULL_PUBLIC_KEY_SIZE, Some(PublicKeyFormat::Full)) => PublicKeyFormat::Full,
            (util::COMPRESSED_PUBLIC_KEY_SIZE, None)
            | (util::COMPRESSED_PUBLIC_KEY_SIZE, Some(PublicKeyFormat::Compressed)) => {
                PublicKeyFormat::Compressed
            }
            (util::RAW_PUBLIC_KEY_SIZE, None)
            | (util::RAW_PUBLIC_KEY_SIZE, Some(PublicKeyFormat::Raw)) => PublicKeyFormat::Raw,
            _ => return Err(Error::InvalidInputLength),
        };

        match format {
            PublicKeyFormat::Full => {
                let mut a = [0; util::FULL_PUBLIC_KEY_SIZE];
                a.copy_from_slice(p);
                Self::parse(&a)
            }
            PublicKeyFormat::Raw => {
                use util::TAG_PUBKEY_FULL;

                let mut a = [0; util::FULL_PUBLIC_KEY_SIZE];
                a[0] = TAG_PUBKEY_FULL;
                a[1..].copy_from_slice(p);
                Self::parse(&a)
            }
            PublicKeyFormat::Compressed => {
                let mut a = [0; util::COMPRESSED_PUBLIC_KEY_SIZE];
                a.copy_from_slice(p);
                Self::parse_compressed(&a)
            }
        }
    }

    pub fn parse(p: &[u8; util::FULL_PUBLIC_KEY_SIZE]) -> Result<PublicKey, Error> {
        use util::{TAG_PUBKEY_FULL, TAG_PUBKEY_HYBRID_EVEN, TAG_PUBKEY_HYBRID_ODD};

        if !(p[0] == TAG_PUBKEY_FULL
            || p[0] == TAG_PUBKEY_HYBRID_EVEN
            || p[0] == TAG_PUBKEY_HYBRID_ODD)
        {
            return Err(Error::InvalidPublicKey);
        }
        let mut x = Field::default();
        let mut y = Field::default();
        if !x.set_b32(array_ref!(p, 1, 32)) {
            return Err(Error::InvalidPublicKey);
        }
        if !y.set_b32(array_ref!(p, 33, 32)) {
            return Err(Error::InvalidPublicKey);
        }
        let mut elem = Affine::default();
        elem.set_xy(&x, &y);
        if (p[0] == TAG_PUBKEY_HYBRID_EVEN || p[0] == TAG_PUBKEY_HYBRID_ODD)
            && (y.is_odd() != (p[0] == TAG_PUBKEY_HYBRID_ODD))
        {
            return Err(Error::InvalidPublicKey);
        }
        if elem.is_infinity() {
            return Err(Error::InvalidPublicKey);
        }
        if elem.is_valid_var() {
            Ok(PublicKey(elem))
        } else {
            Err(Error::InvalidPublicKey)
        }
    }

    pub fn parse_compressed(
        p: &[u8; util::COMPRESSED_PUBLIC_KEY_SIZE],
    ) -> Result<PublicKey, Error> {
        use util::{TAG_PUBKEY_EVEN, TAG_PUBKEY_ODD};

        if !(p[0] == TAG_PUBKEY_EVEN || p[0] == TAG_PUBKEY_ODD) {
            return Err(Error::InvalidPublicKey);
        }
        let mut x = Field::default();
        if !x.set_b32(array_ref!(p, 1, 32)) {
            return Err(Error::InvalidPublicKey);
        }
        let mut elem = Affine::default();
        elem.set_xo_var(&x, p[0] == TAG_PUBKEY_ODD);
        if elem.is_infinity() {
            return Err(Error::InvalidPublicKey);
        }
        if elem.is_valid_var() {
            Ok(PublicKey(elem))
        } else {
            Err(Error::InvalidPublicKey)
        }
    }

    pub fn serialize(&self) -> [u8; util::FULL_PUBLIC_KEY_SIZE] {
        use util::TAG_PUBKEY_FULL;

        debug_assert!(!self.0.is_infinity());

        let mut ret = [0u8; 65];
        let mut elem = self.0;

        elem.x.normalize_var();
        elem.y.normalize_var();
        elem.x.fill_b32(array_mut_ref!(ret, 1, 32));
        elem.y.fill_b32(array_mut_ref!(ret, 33, 32));
        ret[0] = TAG_PUBKEY_FULL;

        ret
    }

    pub fn serialize_compressed(&self) -> [u8; util::COMPRESSED_PUBLIC_KEY_SIZE] {
        use util::{TAG_PUBKEY_EVEN, TAG_PUBKEY_ODD};

        debug_assert!(!self.0.is_infinity());

        let mut ret = [0u8; 33];
        let mut elem = self.0;

        elem.x.normalize_var();
        elem.y.normalize_var();
        elem.x.fill_b32(array_mut_ref!(ret, 1, 32));
        ret[0] = if elem.y.is_odd() {
            TAG_PUBKEY_ODD
        } else {
            TAG_PUBKEY_EVEN
        };

        ret
    }
}

impl Into<Affine> for PublicKey {
    fn into(self) -> Affine {
        self.0
    }
}

impl TryFrom<Affine> for PublicKey {
    type Error = Error;

    fn try_from(value: Affine) -> Result<Self, Self::Error> {
        if value.is_infinity() || !value.is_valid_var() {
            Err(Error::InvalidAffine)
        } else {
            Ok(PublicKey(value))
        }
    }
}

impl SecretKey {
    pub fn parse(p: &[u8; util::SECRET_KEY_SIZE]) -> Result<SecretKey, Error> {
        let mut elem = Scalar::default();
        if !bool::from(elem.set_b32(p)) {
            Self::try_from(elem)
        } else {
            Err(Error::InvalidSecretKey)
        }
    }

    pub fn parse_slice(p: &[u8]) -> Result<SecretKey, Error> {
        if p.len() != util::SECRET_KEY_SIZE {
            return Err(Error::InvalidInputLength);
        }

        let mut a = [0; 32];
        a.copy_from_slice(p);
        Self::parse(&a)
    }

    pub fn serialize(&self) -> [u8; util::SECRET_KEY_SIZE] {
        self.0.b32()
    }

    pub fn tweak_add_assign(&mut self, tweak: &SecretKey) -> Result<(), Error> {
        let v = self.0 + tweak.0;
        if v.is_zero() {
            return Err(Error::TweakOutOfRange);
        }
        self.0 = v;
        Ok(())
    }

    pub fn tweak_mul_assign(&mut self, tweak: &SecretKey) -> Result<(), Error> {
        if tweak.0.is_zero() {
            return Err(Error::TweakOutOfRange);
        }

        self.0 *= &tweak.0;
        Ok(())
    }

    pub fn inv(&self) -> Self {
        SecretKey(self.0.inv())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl Default for SecretKey {
    fn default() -> SecretKey {
        let mut elem = Scalar::default();
        let overflowed = bool::from(elem.set_b32(&[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        ]));
        debug_assert!(!overflowed);
        debug_assert!(!elem.is_zero());
        SecretKey(elem)
    }
}

impl Into<Scalar> for SecretKey {
    fn into(self) -> Scalar {
        self.0
    }
}

impl TryFrom<Scalar> for SecretKey {
    type Error = Error;

    fn try_from(scalar: Scalar) -> Result<Self, Error> {
        if scalar.is_zero() {
            Err(Error::InvalidSecretKey)
        } else {
            Ok(Self(scalar))
        }
    }
}

impl core::fmt::LowerHex for SecretKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let scalar = self.0;

        write!(f, "{:x}", scalar)
    }
}

#[cfg(test)]
mod tests {
    use crate::SecretKey;
    use hex_literal::hex;

    #[test]
    fn secret_key_inverse_is_sane() {
        let sk = SecretKey::parse(&[1; 32]).unwrap();
        let inv = sk.inv();
        let invinv = inv.inv();
        assert_eq!(sk, invinv);
        // Check that the inverse of `[1; 32]` is same as rust-secp256k1
        assert_eq!(
            inv,
            SecretKey::parse(&hex!(
                "1536f1d756d1abf83aaf173bc5ee3fc487c93010f18624d80bd6d4038fadd59e"
            ))
            .unwrap()
        )
    }

    #[test]
    fn secret_key_clear_is_correct() {
        let mut sk = SecretKey::parse(&[1; 32]).unwrap();
        sk.clear();
        assert_eq!(sk.is_zero(), true);
    }
}
