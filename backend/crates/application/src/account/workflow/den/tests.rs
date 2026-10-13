use adapter_mem::fixture::{alice, queue, supreme_arts};

use super::*;
use crate::den::tests::{self as testing, World, segment};

#[tokio::test]
async fn an_account_s_workflows_folder_lists_its_workflows() {
    let World { ports, .. } = testing::world().await;
    let alice = alice();
    let context = testing::context(&ports, &alice);

    let workflows = children(&context, &WorkflowPlace::Workflows(supreme_arts()))
        .await
        .expect("no store fails");

    let expected_place = Place::Workflow(WorkflowPlace::Workflow(supreme_arts(), queue()));
    let [queue_node] = workflows.as_slice() else {
        panic!("Supreme Arts has one Workflow, Queue");
    };
    assert_eq!(queue_node.place, expected_place);
    assert_eq!(queue_node.name.as_ref(), "Queue");
    assert_eq!(queue_node.level, Level::Private);
    assert_eq!(queue_node.lifts, Vec::new());
    assert!(queue_node.place.den_type().content_not_shown());
}

#[tokio::test]
async fn a_workflow_is_found_by_its_key_in_its_account_s_folder_only() {
    let World { ports, .. } = testing::world().await;
    let alice = alice();
    let context = testing::context(&ports, &alice);
    let folder = WorkflowPlace::Workflows(supreme_arts());

    let found = child(&context, &folder, &segment(&queue().to_string()))
        .await
        .expect("no store fails");
    let absent = child(&context, &folder, &segment("queue"))
        .await
        .expect("no store fails");
    let inside = child(
        &context,
        &WorkflowPlace::Workflow(supreme_arts(), queue()),
        &segment("anything"),
    )
    .await
    .expect("no store fails");

    assert!(found.is_some_and(|node| node.name.as_ref() == "Queue"));
    assert_eq!(absent, None);
    assert_eq!(inside, None);
}
