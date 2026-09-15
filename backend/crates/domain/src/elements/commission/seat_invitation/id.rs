/// The app-private key of a [`SeatInvitation`](super::SeatInvitation) (UUIDv7).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
pub struct SeatInvitationId(uuid::Uuid);
