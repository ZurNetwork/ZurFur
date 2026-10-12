//! The den Query over the in-memory runtime: every malformed path is the one not-found, and
//! the root reads not-found while no identity arm answers.

use application::den::{DenError, node::NodeQuery, root::RootQuery};
use domain::elements::{did::Did, user::UserId};

/// A runtime signed in as alice, and alice's id.
fn alice() -> (composition::Runtime, UserId) {
    let did = Did::from("did:plc:alice-den".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    (runtime, UserId::from(did))
}

/// Reads the node at `path` as alice.
async fn read(path: Vec<String>) -> Result<application::den::node::Output, DenError> {
    let (runtime, viewer) = alice();
    let query = NodeQuery {
        viewer,
        path,
        include_deleted: false,
        page: None,
    };
    runtime.app().den().node(query).await
}

#[tokio::test]
async fn an_empty_path_is_not_found() {
    let answer = read(Vec::new()).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

#[tokio::test]
async fn seventeen_pieces_are_not_found() {
    let path = vec!["accounts".to_owned(); 17];

    let answer = read(path).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

#[tokio::test]
async fn each_malformed_piece_is_not_found() {
    let too_long = "a".repeat(2049);
    let malformed = ["", ".", "..", "a/b", "a b", "\u{e9}", too_long.as_str()];

    for piece in malformed {
        let path = vec!["accounts".to_owned(), piece.to_owned()];

        let answer = read(path).await;

        assert!(matches!(answer, Err(DenError::NotFound)), "{piece:?}");
    }
}

#[tokio::test]
async fn a_well_formed_path_is_not_found_while_no_identity_arm_answers() {
    let path = vec!["accounts".to_owned()];

    let answer = read(path).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

/// Pins the interim: until the identity arms serve the user's directory, the viewer's own
/// root is the one not-found.
#[tokio::test]
async fn the_root_is_not_found_while_no_identity_arm_answers() {
    let (runtime, viewer) = alice();
    let query = RootQuery {
        viewer,
        include_deleted: false,
        page: None,
    };

    let answer = runtime.app().den().root(query).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}
