use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, old_sketch, untitled, untitled_files};
use domain::{
    elements::{commission::FileName, level::Level, text::StoredText},
    ports::FileStore,
};
use test_support::recording::{
    CallLog, NEVER_FOR_A_MISSING_FILE, NEVER_FOR_A_STRANGER, assert_reads_none_of,
};
use tokio::io::AsyncRead;

use super::*;
use crate::{
    commission::Commissions,
    common_error::CommonError,
    test_world::{
        self, World, absent_commission, absent_file, recorded, seed_alice_s, seed_file,
        seed_file_named, stranger, stranger_targets,
    },
};

/// Downloads `file` of `commission` as `viewer`.
async fn download(
    world: &World,
    viewer: &UserId,
    commission: CommissionId,
    file: FileKey,
) -> CommissionResult<Output> {
    let query = Query {
        actor_id: viewer.clone(),
        commission_id: commission,
        file_id: file,
    };
    Commissions::new(&world.ports).files().download(query).await
}

/// The download's answer and the reads it made, alone.
async fn recorded_download(
    world: &World,
    log: &CallLog,
    viewer: &UserId,
    commission: CommissionId,
    file: FileKey,
) -> (CommissionResult<Output>, Vec<&'static str>) {
    let before = log.events().len();
    let answer = download(world, viewer, commission, file).await;
    let reads = log.events().split_off(before);
    (answer, reads)
}

#[tokio::test]
async fn a_stranger_s_absent_and_not_theirs_commission_are_one_refusal_with_the_same_reads() {
    let (world, log) = recorded().await;
    let stranger = stranger();

    for target in stranger_targets(&world).await {
        let (absent, absent_reads) =
            recorded_download(&world, &log, &stranger, absent_commission(), absent_file()).await;
        let (not_theirs, not_theirs_reads) =
            recorded_download(&world, &log, &stranger, target.id, target.file).await;

        let label = target.label;
        assert!(
            matches!(absent, Err(CommissionError::NotAMember)),
            "{label}: absent"
        );
        assert!(
            matches!(not_theirs, Err(CommissionError::NotAMember)),
            "{label}: not theirs"
        );
        assert_eq!(absent_reads, ["find_summary", "is_participant"], "{label}");
        assert_eq!(not_theirs_reads, absent_reads, "{label}");
        assert_reads_none_of(&absent_reads, &NEVER_FOR_A_STRANGER, label);
    }
}

#[tokio::test]
async fn a_participant_s_absent_key_and_another_commission_s_are_one_file_not_found() {
    let (world, log) = recorded().await;
    let elsewhere = seed_file(&world, old_sketch()).await;
    let bob = bob();

    let (absent, absent_reads) =
        recorded_download(&world, &log, &bob, untitled(), absent_file()).await;
    let (foreign, foreign_reads) =
        recorded_download(&world, &log, &bob, untitled(), elsewhere).await;

    assert!(matches!(absent, Err(CommissionError::FileNotFound)));
    assert!(matches!(foreign, Err(CommissionError::FileNotFound)));
    assert!(absent_reads.contains(&"files"), "{absent_reads:?}");
    assert_eq!(foreign_reads, absent_reads);
    assert_reads_none_of(&absent_reads, &NEVER_FOR_A_MISSING_FILE, "a missing file");
}

#[tokio::test]
async fn alice_downloads_her_file_from_untitled_and_from_the_archived_old_sketch() {
    let world = test_world::world().await;
    let [ref_abco, _] = untitled_files();
    let old_sketch_file = seed_file(&world, old_sketch()).await;

    let from_untitled = download(&world, &alice(), untitled(), ref_abco).await;
    let from_old_sketch = download(&world, &alice(), old_sketch(), old_sketch_file).await;

    let from_untitled = from_untitled.expect("alice downloads Untitled's file");
    let from_old_sketch = from_old_sketch.expect("alice downloads Old sketch's file");
    assert_eq!(
        from_untitled.result.metadata.filename.as_str(),
        "ref-abco.png"
    );
    assert_eq!(
        from_old_sketch.result.metadata.filename.as_str(),
        "seeded.png"
    );
}

#[tokio::test]
async fn bob_a_participant_downloads_untitled_s_file() {
    let world = test_world::world().await;
    let [_, sketch_01] = untitled_files();

    let downloaded = download(&world, &bob(), untitled(), sketch_01).await;

    let downloaded = downloaded.expect("bob downloads Untitled's file");
    assert_eq!(
        downloaded.result.metadata.filename.as_str(),
        "sketch-01.png"
    );
}

/// Downloads, as alice, a file of "Untitled" stored under `stored`, which no use case checks.
async fn download_stored_as(world: &World, stored: &str) -> Output {
    let name = FileName::try_from(stored).expect("the file name rule accepts it");
    let file = seed_file_named(world, untitled(), &name).await;
    download(world, &alice(), untitled(), file)
        .await
        .expect("alice downloads the file")
}

#[tokio::test]
async fn an_ordinary_stored_name_is_the_save_name_unchanged() {
    let world = test_world::world().await;

    let downloaded = download_stored_as(&world, "ref sheet, v2.png").await;

    assert_eq!(downloaded.save_name.as_ref(), "ref sheet, v2.png");
}

#[tokio::test]
async fn a_stored_name_the_den_s_rule_refuses_is_saved_as_unnamed() {
    let world = test_world::world().await;
    let legacy = "photo\u{202E}gpj.exe";

    let downloaded = download_stored_as(&world, legacy).await;

    assert_eq!(downloaded.save_name.as_ref(), "Unnamed");
    assert_eq!(downloaded.result.metadata.filename.as_str(), legacy);
}

#[tokio::test]
async fn a_stored_name_is_saved_as_the_den_names_it_with_what_the_rule_strips_left_out() {
    let world = test_world::world().await;

    let downloaded = download_stored_as(&world, "ref\u{200B}abco.png").await;

    assert_eq!(downloaded.save_name.as_ref(), "refabco.png");
}

#[tokio::test]
async fn a_stranger_s_absent_and_real_key_under_a_public_live_commission_are_one_file_not_found() {
    let (world, log) = recorded().await;
    let public = seed_alice_s(&world, "Public, live", Level::Public, false).await;
    let stranger = stranger();

    let (real, real_reads) =
        recorded_download(&world, &log, &stranger, public.id, public.file).await;
    let (absent, absent_reads) =
        recorded_download(&world, &log, &stranger, public.id, absent_file()).await;

    assert!(matches!(real, Err(CommissionError::FileNotFound)));
    assert!(matches!(absent, Err(CommissionError::FileNotFound)));
    assert_eq!(absent_reads, real_reads);
    assert_reads_none_of(&real_reads, &NEVER_FOR_A_STRANGER, "Public, live");
}

#[tokio::test]
async fn a_stranger_downloading_from_a_listed_live_commission_is_refused_at_the_commission() {
    let (world, log) = recorded().await;
    let listed = seed_alice_s(&world, "Listed, live", Level::Listed, false).await;

    let (answer, reads) =
        recorded_download(&world, &log, &stranger(), listed.id, listed.file).await;

    assert!(matches!(answer, Err(CommissionError::NotAMember)));
    assert_eq!(reads, ["find_summary", "is_participant"]);
}

/// A file store whose name read fails; the rest delegate.
struct FailingFilename(Arc<dyn FileStore>);

#[async_trait::async_trait]
impl FileStore for FailingFilename {
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64> {
        self.0.put(key, filename, content_type, content).await
    }

    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>> {
        self.0.get(key).await
    }

    async fn filename(&self, _key: FileKey) -> anyhow::Result<Option<StoredText>> {
        Err(anyhow::anyhow!("the file store is down"))
    }

    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        self.0.delete(key).await
    }
}

#[tokio::test]
async fn a_store_failure_inside_den_s_check_is_a_store_failure_not_a_miss() {
    let mut world = test_world::world().await;
    world.ports.files = Arc::new(FailingFilename(world.ports.files.clone()));
    let [_, sketch_01] = untitled_files();

    let answer = download(&world, &bob(), untitled(), sketch_01).await;

    assert!(matches!(
        answer,
        Err(CommissionError::CommonError(CommonError::Infrastructure(_)))
    ));
}
