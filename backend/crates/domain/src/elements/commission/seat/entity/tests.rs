use chrono::Utc;

use super::*;
use crate::elements::commission::element::SurfaceAddress;
use crate::elements::commission::{CommissionId, NewSeat};
use crate::elements::did::Did;

// A declared seat's envelope, with no occupant field anywhere.
#[test]
fn a_new_seat_is_born_vacant_with_its_requirements() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let address = SurfaceAddress::new(
        crate::elements::commission::TabId::from(uuid::Uuid::now_v7()),
        "content".parse().unwrap(),
    );
    let owner = UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())));
    let kind = "Creator".parse::<SeatKind>().unwrap();
    let prompt = "Two refs, please.".parse::<SeatPrompt>().unwrap();
    let link = "https://forms.example/apply".parse::<SeatLink>().unwrap();

    let seat = NewSeat::contributed_at(
        commission,
        address.clone(),
        kind.clone(),
        Some(prompt.clone()),
        Some(link.clone()),
        owner.clone(),
        Utc::now(),
    );

    assert_eq!(seat.commission_id, commission);
    assert_eq!(seat.address, address);
    assert_eq!(seat.kind, kind);
    assert_eq!(seat.prompt, Some(prompt));
    assert_eq!(seat.link, Some(link));
    assert_eq!(seat.created_by, owner);
    // The read shape's single occupant slot is the whole occupancy model.
    let read = Seat {
        id: seat.id,
        kind: seat.kind.clone(),
        prompt: seat.prompt.clone(),
        link: seat.link.clone(),
        occupant: None,
    };
    assert!(read.is_vacant(), "a seat is born vacant");
}
