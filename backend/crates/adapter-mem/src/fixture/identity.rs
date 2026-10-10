//! The fixture actors: the users alice and bob, alice's Account "Supreme
//! Arts", and the Private characters Ember and Kael-sona (kept by alice) and
//! Abco (kept by bob). No record is written to the public-records fake.

use chrono::{DateTime, Utc};
use domain::datetime::DateTimeUtc;
use domain::elements::account::{Account, AccountId, AccountName};
use domain::elements::character::{
    Character, CharacterAttributes, CharacterDescription, CharacterId, CharacterName, Presence,
};
use domain::elements::did::Did;
use domain::elements::handle::Handle;
use domain::elements::profile::Profile;
use domain::elements::user::UserId;

use crate::MemBackend;

const ALICE_DID: &str = "did:plc:c5vzalicefixtureaaaaaaaa";
const BOB_DID: &str = "did:plc:m2xfbobfixtureaaaaaaaaaa";
const SUPREME_ARTS_DID: &str = "did:plc:t7nesupremeartsfixtureaa";
const ABCO_DID: &str = "did:plc:a6hwabcofixtureaaaaaaaaa";
const EMBER_DID: &str = "did:plc:e3qkemberfixtureaaaaaaaa";
const KAEL_SONA_DID: &str = "did:plc:k4sokaelsonafixtureaaaaa";

/// The fixture user alice: display name "Alice", handle `alice.test`.
pub fn alice() -> UserId {
    UserId::from(Did::from(ALICE_DID.to_string()))
}

/// The fixture user bob: handle `bob.test` and no display name, so his name
/// falls back to the handle.
pub fn bob() -> UserId {
    UserId::from(Did::from(BOB_DID.to_string()))
}

/// The fixture Account "Supreme Arts", owned by alice.
pub fn supreme_arts() -> AccountId {
    AccountId::from(Did::from(SUPREME_ARTS_DID.to_string()))
}

/// The fixture character Abco, Private, kept by bob.
pub fn abco() -> CharacterId {
    CharacterId::from(Did::from(ABCO_DID.to_string()))
}

/// The fixture character Ember, Private, kept by alice.
pub fn ember() -> CharacterId {
    CharacterId::from(Did::from(EMBER_DID.to_string()))
}

/// The fixture character Kael-sona, Private, kept by alice.
pub fn kael_sona() -> CharacterId {
    CharacterId::from(Did::from(KAEL_SONA_DID.to_string()))
}

/// Seed the fixture actors into a fresh `backend`: both users with their
/// cached profiles, Supreme Arts with alice as its Owner, and the three
/// characters. Seeding the same backend twice fails on the Account's handle.
/// It takes only the private store: no fixture writes a public record.
pub async fn seed_identity(backend: &MemBackend) -> anyhow::Result<()> {
    let seeded_at = seeded_at();

    backend.provision(alice().did()).await?;
    backend.provision(bob().did()).await?;

    let alice_profile =
        Profile::new(alice().did().clone(), "alice.test").with_display_name("Alice");
    let bob_profile = Profile::new(bob().did().clone(), "bob.test");
    backend.put_profile(&alice_profile).await?;
    backend.put_profile(&bob_profile).await?;

    let (supreme_arts_account, alice_ownership) = Account::open(
        alice(),
        supreme_arts().did().clone(),
        "supreme-arts.zurfur.app".parse::<Handle>()?,
        "Supreme Arts".parse::<AccountName>()?,
        seeded_at,
    );
    backend
        .create(&supreme_arts_account, &alice_ownership)
        .await?;

    let abco_character = private_character(bob(), abco(), "Abco", seeded_at)?;
    let ember_character = private_character(alice(), ember(), "Ember", seeded_at)?;
    let kael_sona_character = private_character(alice(), kael_sona(), "Kael-sona", seeded_at)?;
    backend.create_character(abco_character).await?;
    backend.create_character(ember_character).await?;
    backend.create_character(kael_sona_character).await?;
    Ok(())
}

/// The fixed instant every fixture entity is stamped with, so a world seeded
/// twice holds the same timestamps.
fn seeded_at() -> DateTimeUtc {
    DateTime::<Utc>::from_timestamp(1_791_590_400, 0).expect("a representable instant")
}

/// A Private, handle-less character named `name`, kept by `keeper`.
fn private_character(
    keeper: UserId,
    id: CharacterId,
    name: &str,
    seeded_at: DateTimeUtc,
) -> anyhow::Result<Character> {
    let attributes = CharacterAttributes {
        name: name.parse::<CharacterName>()?,
        description: "".parse::<CharacterDescription>()?,
        species: String::new(),
        dynamic_attributes: Vec::new(),
    };
    let character = Character::create(
        keeper,
        Presence::Private,
        id.did().clone(),
        attributes,
        seeded_at,
    );
    Ok(character)
}

#[cfg(test)]
mod tests;
