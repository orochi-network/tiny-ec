#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InvalidSignature,
    InvalidPublicKey,
    InvalidSecretKey,
    InvalidRecoveryId,
    InvalidMessage,
    InvalidInputLength,
    TweakOutOfRange,
    InvalidAffine,
}
