//! Deterministic ownership/queue/installation-boundary tests.

use super::*;
use std::fs;
use tdector_eval::TokenizationRule;

fn limits() -> Limits {
    Limits {
        project_bytes: 32 * 1024 * 1024,
        message_bytes: 1024 * 1024,
        result_bytes: 256 * 1024,
        queued_operations: 32,
        batch_commands: 100,
        page_size: 200,
        default_page_size: 50,
        operation_timeout_ms: 30_000,
        output_write_timeout_ms: crate::transport::OUTPUT_TIMEOUT.as_millis() as u64,
        shutdown_timeout_ms: crate::transport::SHUTDOWN_TIMEOUT.as_millis() as u64,
    }
}

fn project_bytes(text: &str) -> Vec<u8> {
    let mut session = Session::default();
    session
        .import_text(text, "Fixture", &TokenizationRule::default_whitespace())
        .expect("import fixture");
    session.save_snapshot().expect("serialize fixture").bytes
}

fn fixture() -> (tempfile::TempDir, State) {
    let directory = tempfile::tempdir().expect("create test directory");
    let path = directory.path().join("project.json");
    fs::write(&path, project_bytes("cat dog")).expect("write fixture");
    let limits = limits();
    let binding = BoundProject::open(&path, limits.project_bytes).expect("bind fixture");
    let mut session = Session::default();
    session
        .load_json(binding.text().expect("decode fixture"))
        .expect("load fixture");
    (
        directory,
        State {
            session,
            binding,
            id: new_id(),
            writable: true,
            limits,
            poisoned: false,
        },
    )
}

fn edit(id: &str, revision: &str, translation: &str) -> Operation {
    Operation::Edit(EditArgs {
        session_id: id.into(),
        expected_revision: revision.into(),
        dry_run: false,
        batch: AnnotationBatch {
            schema_version: 1,
            commands: vec![Annotation::SetTranslation {
                segment_index: 0,
                translation: translation.into(),
            }],
        },
    })
}

fn reload(state: &State, discard_changes: bool) -> Operation {
    Operation::Reload(ReloadArgs {
        session_id: state.id.clone(),
        expected_revision: state.session.revision().to_string(),
        discard_changes,
    })
}

fn assert_state_unchanged(state: &State, meta: &SessionMeta, snapshot: &[u8], baseline: &[u8]) {
    assert_eq!(state.id, meta.session_id);
    assert_eq!(state.session.revision().to_string(), meta.revision);
    assert_eq!(state.session.is_dirty(), meta.dirty);
    assert_eq!(
        state
            .session
            .save_snapshot()
            .expect("snapshot unchanged state")
            .bytes,
        snapshot
    );
    assert_eq!(state.binding.bytes(), baseline);
}

#[test]
fn result_budget_failure_never_installs_a_prepared_edit() {
    let (_directory, mut state) = fixture();
    let meta = state.meta();
    let snapshot = state.session.save_snapshot().expect("snapshot before edit");
    let baseline = state.binding.bytes().to_vec();
    // Smaller than the complete duplicated edit receipt; preparation itself succeeds.
    state.limits.result_bytes = 100;
    let error = state
        .execute(edit(&meta.session_id, &meta.revision, "candidate"))
        .expect_err("receipt exceeds budget");
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_state_unchanged(&state, &meta, &snapshot.bytes, &baseline);
    assert!(
        state.session.acknowledge_saved(snapshot.token),
        "failed preparation retains save-token authority"
    );
    state
        .binding
        .check_unchanged()
        .expect("disk remains unchanged");
}

#[test]
fn expired_edit_and_cancelled_reload_preserve_the_live_state_and_baseline() {
    let (_directory, mut state) = fixture();
    let meta = state.meta();
    let snapshot = state
        .session
        .save_snapshot()
        .expect("snapshot before edit")
        .bytes;
    let baseline = state.binding.bytes().to_vec();
    {
        let _policy = ExecutionPolicy::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() - Duration::from_millis(1),
        )
        .enter();
        let error = state
            .execute(edit(&meta.session_id, &meta.revision, "expired"))
            .expect_err("expired edit is rejected");
        assert_eq!(error.code, ErrorCode::DeadlineExceeded);
    }
    assert_state_unchanged(&state, &meta, &snapshot, &baseline);
    fs::write(state.binding.path(), project_bytes("replacement data"))
        .expect("write reload candidate");
    {
        let _policy = ExecutionPolicy::new(
            Arc::new(AtomicBool::new(true)),
            Instant::now() + Duration::from_secs(10),
        )
        .enter();
        let operation = reload(&state, false);
        let error = state
            .execute(operation)
            .expect_err("cancelled reload is rejected");
        assert_eq!(error.code, ErrorCode::DeadlineExceeded);
    }
    assert_state_unchanged(&state, &meta, &snapshot, &baseline);
    assert!(
        state.binding.check_unchanged().is_err(),
        "cancelled reload did not accept the external baseline"
    );
}

#[test]
fn failed_and_oversized_reload_candidates_keep_dirty_state_and_old_identity() {
    let (_directory, mut state) = fixture();
    let before = state.meta();
    state
        .execute(edit(
            &before.session_id,
            &before.revision,
            "unsaved annotation",
        ))
        .expect("edit fixture");
    let meta = state.meta();
    assert!(meta.dirty);
    let snapshot = state
        .session
        .save_snapshot()
        .expect("snapshot dirty state")
        .bytes;
    let baseline = state.binding.bytes().to_vec();

    fs::write(state.binding.path(), b"{broken").expect("write malformed reload candidate");
    let operation = reload(&state, true);
    let error = state
        .execute(operation)
        .expect_err("malformed reload fails");
    assert_eq!(error.code, ErrorCode::InvalidInput);
    assert_state_unchanged(&state, &meta, &snapshot, &baseline);

    fs::write(state.binding.path(), project_bytes("valid new project"))
        .expect("write valid reload candidate");
    state.limits.result_bytes = 100;
    let operation = reload(&state, true);
    let error = state
        .execute(operation)
        .expect_err("reload receipt exceeds budget");
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_state_unchanged(&state, &meta, &snapshot, &baseline);
    state.limits.result_bytes = limits().result_bytes;
    let operation = reload(&state, true);
    state
        .execute(operation)
        .expect("later explicit reload succeeds");
    assert_ne!(state.id, meta.session_id);
    assert!(!state.session.is_dirty());
    state
        .binding
        .check_unchanged()
        .expect("only successful reload adopts baseline");
}

#[test]
fn save_result_budget_failure_keeps_edits_and_destination_intact() {
    let (_directory, mut state) = fixture();
    let before = state.meta();
    state
        .execute(edit(&before.session_id, &before.revision, "unsaved"))
        .expect("edit fixture");
    let meta = state.meta();
    let snapshot = state
        .session
        .save_snapshot()
        .expect("snapshot dirty state")
        .bytes;
    let baseline = state.binding.bytes().to_vec();
    state.limits.result_bytes = 100;
    let error = state
        .execute(Operation::Save(WriteArgs {
            session_id: meta.session_id.clone(),
            expected_revision: meta.revision.clone(),
        }))
        .expect_err("save receipt exceeds budget");
    assert_eq!(error.code, ErrorCode::ResultTooLarge);
    assert_state_unchanged(&state, &meta, &snapshot, &baseline);
    state
        .binding
        .check_unchanged()
        .expect("save did not touch destination");
}

#[tokio::test(flavor = "current_thread")]
async fn full_queue_rejects_immediately_without_admitting_another_operation() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, _receive) = oneshot::channel();
    sender
        .try_send(Message {
            operation: Operation::Info,
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            reply,
        })
        .unwrap_or_else(|_| panic!("fill queue"));
    let client = Client {
        sender,
        readiness: Arc::new(Mutex::new(Readiness::Loading)),
        shutdown: Shutdown::default(),
        limits: limits(),
    };
    let result = client
        .call(Operation::Info, Arc::new(AtomicBool::new(false)))
        .await;
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("error envelope")["error"]["code"],
        "server_busy"
    );
    assert!(matches!(
        receiver
            .try_recv()
            .expect("original queued operation")
            .operation,
        Operation::Info
    ));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
}

async fn start_owner() -> (tempfile::TempDir, Owner, SessionMeta) {
    let (directory, state) = fixture();
    let path = state.binding.path().to_owned();
    drop(state);
    let owner = Owner::spawn(path, true, limits()).expect("spawn owner");
    let info = owner
        .client
        .call(Operation::Info, Arc::new(AtomicBool::new(false)))
        .await;
    let value = info.structured_content.expect("project info envelope");
    assert_eq!(value["ok"], true);
    let meta = SessionMeta {
        session_id: value["session"]["session_id"]
            .as_str()
            .expect("session ID")
            .into(),
        revision: value["session"]["revision"]
            .as_str()
            .expect("revision")
            .into(),
        dirty: value["session"]["dirty"].as_bool().expect("dirty flag"),
    };
    (directory, owner, meta)
}

fn enqueue(
    owner: &Owner,
    operation: Operation,
    cancelled: bool,
    expired: bool,
) -> oneshot::Receiver<CallToolResult> {
    let (reply, receive) = oneshot::channel();
    owner
        .client
        .sender
        .try_send(Message {
            operation,
            cancelled: Arc::new(AtomicBool::new(cancelled)),
            deadline: if expired {
                Instant::now() - Duration::from_millis(1)
            } else {
                Instant::now() + Duration::from_secs(10)
            },
            reply,
        })
        .unwrap_or_else(|_| panic!("enqueue bounded request"));
    receive
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_and_expired_queued_edits_never_execute() {
    let (directory, owner, meta) = start_owner().await;
    let baseline = fs::read(directory.path().join("project.json")).expect("capture disk baseline");
    let cancelled = enqueue(
        &owner,
        edit(&meta.session_id, &meta.revision, "cancelled"),
        true,
        false,
    );
    let expired = enqueue(
        &owner,
        edit(&meta.session_id, &meta.revision, "expired"),
        false,
        true,
    );
    for receiver in [cancelled, expired] {
        let result = receiver.await.expect("queued failure");
        assert_eq!(result.is_error, Some(true));
        let content = result.structured_content.expect("failure envelope");
        assert_eq!(content["error"]["code"], "deadline_exceeded");
        assert_eq!(content["session"]["revision"], meta.revision);
        assert_eq!(content["session"]["dirty"], false);
    }
    let info = owner
        .client
        .call(Operation::Info, Arc::new(AtomicBool::new(false)))
        .await;
    let value = info
        .structured_content
        .expect("info after skipped requests");
    assert_eq!(value["session"]["revision"], meta.revision);
    assert_eq!(value["session"]["dirty"], false);
    owner.shutdown.stop();
    owner.join().expect("join owner");
    assert_eq!(
        fs::read(directory.path().join("project.json")).expect("read destination"),
        baseline
    );
}

#[tokio::test(flavor = "current_thread")]
async fn two_queued_edits_check_the_revision_when_each_is_dequeued() {
    let (_directory, owner, meta) = start_owner().await;
    let first = enqueue(
        &owner,
        edit(&meta.session_id, &meta.revision, "first"),
        false,
        false,
    );
    let second = enqueue(
        &owner,
        edit(&meta.session_id, &meta.revision, "second"),
        false,
        false,
    );
    assert_eq!(
        first
            .await
            .expect("first response")
            .structured_content
            .expect("first envelope")["ok"],
        true
    );
    let second = second
        .await
        .expect("second response")
        .structured_content
        .expect("second envelope");
    assert_eq!(second["error"]["code"], "revision_conflict");
    assert_eq!(second["session"]["dirty"], true);
    owner.shutdown.stop();
    owner.join().expect("join owner without autosave");
}

#[tokio::test(flavor = "current_thread")]
async fn dropped_save_reply_still_finishes_commit_bookkeeping() {
    let (directory, owner, meta) = start_owner().await;
    let edited = owner
        .client
        .call(
            edit(
                &meta.session_id,
                &meta.revision,
                "saved despite dropped reply",
            ),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    let edited = edited.structured_content.expect("edit envelope");
    let revision = edited["session"]["revision"]
        .as_str()
        .expect("edit revision")
        .to_owned();
    let save = enqueue(
        &owner,
        Operation::Save(WriteArgs {
            session_id: meta.session_id,
            expected_revision: revision.clone(),
        }),
        false,
        false,
    );
    drop(save);
    // The next serialized read observes all bookkeeping even though nobody reads the save result. A dropped receiver is not a cancellation signal.
    let info = owner
        .client
        .call(Operation::Info, Arc::new(AtomicBool::new(false)))
        .await;
    let info = info.structured_content.expect("info after save");
    assert_eq!(info["session"]["revision"], revision);
    assert_eq!(info["session"]["dirty"], false);
    owner.shutdown.stop();
    owner.join().expect("join after save");
    let bytes = fs::read(directory.path().join("project.json")).expect("saved project");
    let mut loaded = Session::default();
    loaded
        .load_json(std::str::from_utf8(&bytes).expect("saved UTF-8"))
        .expect("load saved project");
    let detail = api::query(&mut loaded, api::Query::SegmentShow { segment_index: 0 })
        .expect("saved translation query");
    let value = serde_json::to_value(detail).expect("serialize detail");
    assert_eq!(value["translation"], "saved despite dropped reply");
}
