use chrono::Utc;

use super::*;
use crate::elements::did::Did;

// The kind vocabulary is open: any label wraps, trimmed.
#[test]
fn seat_kind_is_an_open_trimmed_vocabulary() {
    assert_eq!(
        "  Creator ".parse::<SeatKind>().unwrap().as_str(),
        "Creator"
    );
    // Not a Role, not a closed list — arbitrary labels are fine.
    assert!("Background artist".parse::<SeatKind>().is_ok());
    assert!("客户".parse::<SeatKind>().is_ok());

    assert_eq!("   ".parse::<SeatKind>(), Err(SeatKindError::Empty));
    assert_eq!(
        SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS + 1)),
        Err(SeatKindError::TooLong)
    );
    assert!(SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS)).is_ok());
    assert_eq!(
        "a\nb".parse::<SeatKind>(),
        Err(SeatKindError::ControlCharacter)
    );
}

// The prompt is multi-line free text: newlines/tabs pass, other control
// characters and blank/oversized input refuse.
#[test]
fn seat_prompt_allows_lines_but_not_injection() {
    let prompt = " Provide:\n\t- two refs\n\t- your rate "
        .parse::<SeatPrompt>()
        .unwrap();
    assert_eq!(prompt.as_str(), "Provide:\n\t- two refs\n\t- your rate");

    assert_eq!("   ".parse::<SeatPrompt>(), Err(SeatPromptError::Empty));
    assert_eq!(
        SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS + 1)),
        Err(SeatPromptError::TooLong)
    );
    assert!(SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS)).is_ok());
    assert_eq!(
        "a\0b".parse::<SeatPrompt>(),
        Err(SeatPromptError::ControlCharacter)
    );
    assert_eq!(
        "a\u{1b}b".parse::<SeatPrompt>(),
        Err(SeatPromptError::ControlCharacter)
    );
}

// The link is an opaque pointer: no scheme allowlist, no control chars.
#[test]
fn seat_link_validates_shape_but_not_scheme() {
    assert_eq!(
        " https://forms.example/apply "
            .parse::<SeatLink>()
            .unwrap()
            .as_str(),
        "https://forms.example/apply"
    );
    // No scheme allowlist — a bare pointer is fine.
    assert!("form on my carrd".parse::<SeatLink>().is_ok());

    assert_eq!("   ".parse::<SeatLink>(), Err(SeatLinkError::Empty));
    assert_eq!(
        SeatLink::try_from("x".repeat(SeatLink::MAX_CHARS + 1)),
        Err(SeatLinkError::TooLong)
    );
    assert_eq!(
        "a\tb".parse::<SeatLink>(),
        Err(SeatLinkError::ControlCharacter)
    );
}

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
