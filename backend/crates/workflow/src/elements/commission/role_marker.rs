/// The role of a write any Participant of the commission may make. A type-level marker,
/// never a value.
pub enum AsParticipant {}

/// The role of a write only the commission's Owner may make: archive, delete, transfer, and
/// granting or revoking Admin. A type-level marker, never a value.
pub enum AsOwner {}
