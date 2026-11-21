//! The dev key custodian: every custody key is derived with HKDF-SHA256 from one
//! 32-byte root, one key per purpose. Secrets are sealed in the v1 envelope
//! (XChaCha20-Poly1305) and Den records are tagged with HMAC-SHA256.
//!
//! HKDF's Extract can concentrate the root's entropy but never add to it: under a
//! public root, every derived key is public.

mod custodian;
mod derivation;
mod envelope;
mod mac;

#[cfg(test)]
mod golden;

pub use custodian::HkdfKeyCustodian;
