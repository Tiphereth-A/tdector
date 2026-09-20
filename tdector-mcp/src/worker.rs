//! A dedicated thread owns all project and evaluator state for its lifetime.

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::{Duration, Instant};

use rmcp::model::CallToolResult;
use tdector_app::{Session, api};
use tdector_eval::{ExecutionPolicy, check_execution};
use tdector_io::{BoundProject, CheckedSaveError};
use tokio::sync::oneshot;

use crate::{dto::*, tools::Operation};

#[derive(Clone, Default)]
pub struct Shutdown {
    stopped: Arc<AtomicBool>,
    active: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl Shutdown {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(active) = self.active.lock().expect("shutdown lock").as_ref() {
            active.store(true, Ordering::SeqCst);
        }
    }

    fn activate(&self, cancelled: Arc<AtomicBool>) {
        let mut active = self.active.lock().expect("shutdown lock");
        if self.stopped.load(Ordering::SeqCst) {
            cancelled.store(true, Ordering::SeqCst);
        }
        *active = Some(cancelled);
    }
}

enum Readiness {
    Loading,
    Ready(SessionMeta),
    Failed(ToolError),
}

struct Message {
    operation: Operation,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    reply: oneshot::Sender<CallToolResult>,
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::SyncSender<Message>,
    readiness: Arc<Mutex<Readiness>>,
    shutdown: Shutdown,
    limits: Limits,
}

impl Client {
    pub async fn call(&self, operation: Operation, cancelled: Arc<AtomicBool>) -> CallToolResult {
        let (reply, receive) = oneshot::channel();
        let message = Message {
            operation,
            cancelled,
            deadline: Instant::now() + Duration::from_millis(self.limits.operation_timeout_ms),
            reply,
        };
        if self.shutdown.stopped.load(Ordering::SeqCst) {
            return self.unavailable();
        }
        match self.sender.try_send(message) {
            Ok(()) => receive.await.unwrap_or_else(|_| self.unavailable()),
            Err(mpsc::TrySendError::Full(_)) => self.error(ToolError::new(
                ErrorCode::ServerBusy,
                "Project operation queue is full; retry after outstanding calls complete",
            )),
            Err(mpsc::TrySendError::Disconnected(_)) => self.unavailable(),
        }
    }

    pub fn error(&self, error: ToolError) -> CallToolResult {
        let state = self.readiness.lock().expect("readiness lock");
        let meta = match &*state {
            Readiness::Ready(meta) => Some(meta.clone()),
            _ => None,
        };
        failure(meta, error, self.limits.result_bytes)
    }

    fn unavailable(&self) -> CallToolResult {
        let state = self.readiness.lock().expect("readiness lock");
        match &*state {
            Readiness::Failed(error) => failure(None, error.clone(), self.limits.result_bytes),
            Readiness::Ready(meta) => failure(
                Some(meta.clone()),
                ToolError::new(ErrorCode::InternalError, "Project worker has stopped"),
                self.limits.result_bytes,
            ),
            Readiness::Loading => failure(
                None,
                ToolError::new(
                    ErrorCode::InternalError,
                    "Project worker stopped during initial load",
                ),
                self.limits.result_bytes,
            ),
        }
    }
}

pub struct Owner {
    pub client: Client,
    pub shutdown: Shutdown,
    pub finished: oneshot::Receiver<Result<(), String>>,
    thread: thread::JoinHandle<()>,
}

impl Owner {
    pub fn spawn(path: PathBuf, writable: bool, limits: Limits) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(limits.queued_operations);
        let (finish, finished) = oneshot::channel();
        let readiness = Arc::new(Mutex::new(Readiness::Loading));
        let shutdown = Shutdown::default();
        let client = Client {
            sender,
            readiness: readiness.clone(),
            shutdown: shutdown.clone(),
            limits: limits.clone(),
        };
        let thread_shutdown = shutdown.clone();
        let thread = thread::Builder::new()
            .name("tdector-project".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(
                        path,
                        writable,
                        limits,
                        receiver,
                        &readiness,
                        &thread_shutdown,
                    )
                }))
                .unwrap_or_else(|_| {
                    Err("Project owner thread panicked; restart the server".into())
                });
                thread_shutdown.stop();
                let _ = finish.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            shutdown,
            finished,
            thread,
        })
    }

    pub fn join(self) -> Result<(), String> {
        self.thread
            .join()
            .map_err(|_| "Project owner thread panicked".into())
    }
}

pub fn finished_result(
    result: Result<Result<(), String>, oneshot::error::RecvError>,
) -> Result<(), String> {
    result.map_err(|error| error.to_string())?
}

fn run(
    path: PathBuf,
    writable: bool,
    limits: Limits,
    receiver: mpsc::Receiver<Message>,
    readiness: &Mutex<Readiness>,
    shutdown: &Shutdown,
) -> Result<(), String> {
    // Both the Session and the initial evaluator are constructed on this thread.
    let initial = {
        let _policy = ExecutionPolicy::new(
            shutdown.stopped.clone(),
            Instant::now() + Duration::from_millis(limits.operation_timeout_ms),
        )
        .enter();
        (|| -> Result<State, ToolError> {
            check_execution()?;
            let binding = BoundProject::open(&path, limits.project_bytes)?;
            let mut session = Session::default();
            session.load_json(binding.text()?)?;
            check_execution()?;
            Ok(State {
                session,
                binding,
                id: new_id(),
                writable,
                limits: limits.clone(),
                poisoned: false,
            })
        })()
    };
    let mut state = match initial {
        Ok(state) => state,
        Err(error) => {
            *readiness.lock().expect("readiness lock") = Readiness::Failed(error.clone());
            shutdown.stop();
            while let Ok(message) = receiver.try_recv() {
                let _ = message
                    .reply
                    .send(failure(None, error.clone(), limits.result_bytes));
            }
            return Err(format!(
                "Initial project load failed ({:?}): {}",
                error.code, error.message
            ));
        }
    };
    *readiness.lock().expect("readiness lock") = Readiness::Ready(state.meta());
    while !shutdown.stopped.load(Ordering::SeqCst) {
        let message = match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        shutdown.activate(message.cancelled.clone());
        let _policy = ExecutionPolicy::new(message.cancelled.clone(), message.deadline).enter();
        let result = check_execution()
            .map_err(ToolError::from)
            .and_then(|()| state.execute(message.operation));
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                if error.code == ErrorCode::CommittedStateError {
                    state.poisoned = true;
                }
                failure(Some(state.meta()), error, limits.result_bytes)
            }
        };
        *readiness.lock().expect("readiness lock") = Readiness::Ready(state.meta());
        let _ = message.reply.send(result);
        *shutdown.active.lock().expect("shutdown lock") = None;
    }
    // Do not autosave. Any entered file replacement completed before reaching here.
    Ok(())
}

struct State {
    session: Session,
    binding: BoundProject,
    id: String,
    writable: bool,
    limits: Limits,
    poisoned: bool,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl State {
    fn meta(&self) -> SessionMeta {
        SessionMeta {
            session_id: self.id.clone(),
            revision: self.session.revision().to_string(),
            dirty: self.session.is_dirty(),
        }
    }

    fn validate(&self, id: &str, revision: Option<&str>) -> Result<(), ToolError> {
        if id != self.id {
            return Err(ToolError::new(
                ErrorCode::SessionExpired,
                "Session identity changed; call project_info and reacquire indices",
            ));
        }
        if let Some(revision) = revision {
            if revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ToolError::new(
                    ErrorCode::InvalidInput,
                    "expected_revision must be a decimal u64 string",
                ));
            }
            let revision: u64 = revision.parse().map_err(|_| {
                ToolError::new(
                    ErrorCode::InvalidInput,
                    "expected_revision is outside u64 range",
                )
            })?;
            if revision != self.session.revision() {
                return Err(ToolError::new(
                    ErrorCode::RevisionConflict,
                    "Project revision changed; inspect current state before retrying",
                ));
            }
        }
        Ok(())
    }

    fn write_policy(&self) -> Result<(), ToolError> {
        if !self.writable {
            return Err(ToolError::new(
                ErrorCode::ReadOnly,
                "Start with --write to enable annotation edits and saves",
            ));
        }
        self.healthy()
    }

    fn healthy(&self) -> Result<(), ToolError> {
        if self.poisoned {
            return Err(ToolError::committed(
                "A previous file commit could not be acknowledged. Further state changes are disabled; inspect the file and restart. Do not automatically retry.",
            ));
        }
        Ok(())
    }

    fn projected(&self, revision: u64, dirty: bool) -> SessionMeta {
        SessionMeta {
            session_id: if revision < self.session.revision() {
                new_id()
            } else {
                self.id.clone()
            },
            revision: revision.to_string(),
            dirty,
        }
    }

    fn execute(&mut self, operation: Operation) -> Result<CallToolResult, ToolError> {
        let limit = self.limits.result_bytes;
        match operation {
            Operation::Invalid(error) => Err(error),
            Operation::Info => {
                let api::QueryResponse::Info(project) =
                    api::query(&mut self.session, api::Query::Info)?
                else {
                    unreachable!("Info query response")
                };
                check_execution()?;
                let result = success(
                    self.meta(),
                    ProjectInfo {
                        project,
                        writable: self.writable && !self.poisoned,
                        limits: self.limits.clone(),
                        execution_limits: EvaluatorLimits::default(),
                        edit_lifecycle: EDIT_LIFECYCLE.into(),
                    },
                    limit,
                )?;
                check_execution()?;
                Ok(result)
            }
            Operation::Query {
                session_id,
                revision,
                query,
            } => {
                self.validate(&session_id, revision.as_deref())?;
                let data = api::query(&mut self.session, query)?;
                check_execution()?;
                let result = success(self.meta(), data, limit)?;
                check_execution()?;
                Ok(result)
            }
            Operation::Export(p) => {
                self.validate(&p.session_id, p.expected_revision.as_deref())?;
                let content = self.session.export_typst();
                check_execution()?;
                let result = success(
                    self.meta(),
                    ExportResult {
                        content,
                        mime_type: "text/x-typst; charset=utf-8".into(),
                    },
                    limit,
                )?;
                check_execution()?;
                Ok(result)
            }
            Operation::Edit(p) => {
                self.write_policy()?;
                self.validate(&p.session_id, Some(&p.expected_revision))?;
                // Validate again on the owner, independent of transport parsing.
                if p.batch.schema_version != 1
                    || p.batch.commands.is_empty()
                    || p.batch.commands.len() > self.limits.batch_commands
                {
                    return Err(ToolError::new(
                        ErrorCode::InvalidInput,
                        "Invalid annotation batch version or size",
                    ));
                }
                let commands: Vec<api::Mutation> =
                    p.batch.commands.into_iter().map(Into::into).collect();
                if !commands.iter().all(|c| {
                    matches!(
                        c,
                        api::Mutation::SetGloss { .. }
                            | api::Mutation::SetTranslation { .. }
                            | api::Mutation::SetComment { .. }
                    )
                }) {
                    return Err(ToolError::new(
                        ErrorCode::InvalidInput,
                        "Only annotation mutations are permitted",
                    ));
                }
                let prepared = api::prepare_batch(
                    &self.session,
                    api::BatchRequest {
                        schema_version: 1,
                        commands,
                    },
                )?;
                let receipt = prepared.receipt();
                let meta = if p.dry_run {
                    self.meta()
                } else {
                    self.projected(prepared.projected_revision(), prepared.projected_dirty())
                };
                let result = success(
                    meta.clone(),
                    EditResult {
                        changed: !p.dry_run && receipt.changed,
                        would_change: p.dry_run.then_some(receipt.changed),
                        saved: false,
                        dry_run: p.dry_run,
                        commands: receipt.commands.clone(),
                        edit_lifecycle: EDIT_LIFECYCLE.into(),
                    },
                    limit,
                )?;
                check_execution()?;
                if !p.dry_run {
                    api::commit_batch(&mut self.session, prepared)?;
                    self.id = meta.session_id;
                }
                Ok(result)
            }
            Operation::Save(p) => {
                self.write_policy()?;
                self.validate(&p.session_id, Some(&p.expected_revision))?;
                self.binding.check_unchanged()?;
                if !self.session.is_dirty() {
                    check_execution()?;
                    return success(self.meta(), SaveResult { saved: false }, limit);
                }
                let snapshot = self.session.save_snapshot()?;
                if snapshot.bytes.len() > self.limits.project_bytes {
                    return Err(ToolError::new(
                        ErrorCode::LimitExceeded,
                        "Serialized project exceeds configured project byte limit",
                    ));
                }
                let mut meta = self.meta();
                meta.dirty = false;
                let result = success(meta, SaveResult { saved: true }, limit)?;
                self.binding
                    .save_checked(&snapshot.bytes, check_execution)
                    .map_err(|e| match e {
                        CheckedSaveError::Io(e) => ToolError::from(e),
                        CheckedSaveError::Checkpoint(e) => ToolError::from(e),
                    })?;
                // Replacement has happened. Finish bookkeeping even after cancellation.
                if !self.session.acknowledge_saved(snapshot.token) {
                    return Err(ToolError::committed(
                        "File replacement succeeded, but save acknowledgment failed. Further edits are disabled; inspect the file and restart. Do not retry.",
                    ));
                }
                Ok(result)
            }
            Operation::Reload(p) => {
                self.healthy()?;
                self.validate(&p.session_id, Some(&p.expected_revision))?;
                if self.session.is_dirty() && !p.discard_changes {
                    return Err(ToolError::new(
                        ErrorCode::UnsavedChanges,
                        "Save edits first, or explicitly reload with discard_changes=true",
                    ));
                }
                let binding = self.binding.read_reload()?;
                check_execution()?;
                let prepared = self.session.prepare_load_json(binding.text()?)?;
                let meta = SessionMeta {
                    session_id: new_id(),
                    revision: prepared.projected_revision().to_string(),
                    dirty: false,
                };
                let result = success(meta.clone(), ReloadResult { reloaded: true }, limit)?;
                check_execution()?;
                self.session.commit_load(prepared)?;
                self.id = meta.session_id;
                self.binding = binding;
                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests;
