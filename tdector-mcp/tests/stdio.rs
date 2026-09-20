//! Exercise the shipped executable through real, newline-delimited MCP messages.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(40);
const BASELINE: &str = "2025-11-25";
const CURRENT: &str = "2026-07-28";

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Receiver<Result<Value, String>>,
    stderr: Receiver<String>,
    diagnostics: Receiver<String>,
    next_id: u64,
    pending: HashMap<u64, Value>,
    schemas: HashMap<String, Value>,
    inline: bool,
    cancelled_ids: Vec<u64>,
}

impl Client {
    fn start(project: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tdector-mcp"))
            .arg("--project")
            .arg(project)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start MCP process");
        let stdin = child.stdin.take();
        let output = child.stdout.take().expect("stdout pipe");
        let errors = child.stderr.take().expect("stderr pipe");
        let (tx, stdout) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let parsed = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str(&line)
                        .map_err(|e| format!("Non-protocol stdout: {line:?}: {e}"))
                });
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        let (tx, stderr) = mpsc::channel();
        let (diagnostic_tx, diagnostics) = mpsc::channel();
        thread::spawn(move || {
            let mut text = String::new();
            for line in BufReader::new(errors).lines() {
                let Ok(line) = line else { break };
                text.push_str(&line);
                text.push('\n');
                let _ = diagnostic_tx.send(line);
            }
            let _ = tx.send(text);
        });
        Self {
            child,
            stdin,
            stdout,
            stderr,
            diagnostics,
            next_id: 1,
            pending: HashMap::new(),
            schemas: HashMap::new(),
            inline: false,
            cancelled_ids: Vec::new(),
        }
    }

    fn initialize(&mut self, version: &str) -> Value {
        if version == CURRENT {
            self.inline = true;
            let response = self.request("server/discover", json!({}));
            assert!(response.get("result").is_some(), "{response}");
            return response["result"].clone();
        }
        let response = self.request(
            "initialize",
            json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "tdector-integration-test", "version": "1"}
            }),
        );
        assert_eq!(response["result"]["protocolVersion"], version, "{response}");
        self.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        response["result"].clone()
    }

    fn write(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("open stdin");
        serde_json::to_writer(&mut *stdin, message).expect("write message");
        stdin.write_all(b"\n").expect("newline");
        stdin.flush().expect("flush");
    }

    fn send(&mut self, method: &str, mut params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        if self.inline {
            params["_meta"] = json!({
                "io.modelcontextprotocol/protocolVersion": CURRENT,
                "io.modelcontextprotocol/clientInfo": {"name": "tdector-integration-test", "version": "1"},
                "io.modelcontextprotocol/clientCapabilities": {}
            });
        }
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn receive(&mut self, id: u64) -> Value {
        if let Some(response) = self.pending.remove(&id) {
            return response;
        }
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        loop {
            let response = self
                .stdout
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| {
                    panic!(
                        "No response to {id}: {e}; child={:?}; stderr={:?}",
                        self.child.try_wait(),
                        self.stderr.try_recv()
                    )
                })
                .unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(response["jsonrpc"], "2.0", "{response}");
            if let Some(response_id) = response["id"].as_u64() {
                assert!(
                    !self.cancelled_ids.contains(&response_id),
                    "Cancelled request produced a response: {response}"
                );
                if response_id == id {
                    return response;
                }
                self.pending.insert(response_id, response);
            } else {
                assert!(
                    response["method"].is_string(),
                    "Unexpected response: {response}"
                );
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        self.receive(id)
    }

    fn discover(&mut self) -> Vec<Value> {
        let response = self.request("tools/list", json!({}));
        let tools = response["result"]["tools"]
            .as_array()
            .expect("tools list")
            .clone();
        for tool in &tools {
            assert_eq!(tool["outputSchema"]["type"], "object");
            let name = tool["name"].as_str().expect("tool name");
            jsonschema::validator_for(&tool["inputSchema"]).expect("valid input schema");
            jsonschema::validator_for(&tool["outputSchema"]).expect("valid output schema");
            self.schemas
                .insert(name.into(), tool["outputSchema"].clone());
        }
        tools
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        self.tool_envelope(tool, &response)
    }

    fn tool_envelope(&self, tool: &str, response: &Value) -> Value {
        assert!(
            response.get("error").is_none(),
            "Unexpected protocol error: {response}"
        );
        let result = &response["result"];
        let envelope = result["structuredContent"].clone();
        assert_eq!(envelope["schema_version"], 1, "{response}");
        assert_eq!(
            result["isError"].as_bool().unwrap_or(false),
            envelope["ok"] == false,
            "{response}"
        );
        let text = result["content"][0]["text"]
            .as_str()
            .expect("duplicate text result");
        assert_eq!(
            serde_json::from_str::<Value>(text).expect("text JSON"),
            envelope
        );
        if let Some(schema) = self.schemas.get(tool) {
            let validator = jsonschema::validator_for(schema).expect("output validator");
            let errors: Vec<_> = validator
                .iter_errors(&envelope)
                .map(|e| e.to_string())
                .collect();
            assert!(
                errors.is_empty(),
                "{tool} output fails schema: {errors:?}\n{envelope}"
            );
        }
        envelope
    }

    fn finish(&mut self) -> (ExitStatus, String) {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("child status") {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "MCP did not exit after stdin EOF"
            );
            thread::sleep(Duration::from_millis(10));
        };
        let stderr = self
            .stderr
            .recv_timeout(Duration::from_secs(2))
            .expect("stderr finished");
        loop {
            match self.stdout.recv_timeout(Duration::from_secs(2)) {
                Ok(output) => {
                    let output = output.unwrap_or_else(|error| panic!("{error}"));
                    if let Some(id) = output["id"].as_u64() {
                        assert!(
                            !self.cancelled_ids.contains(&id),
                            "Cancelled request produced a response: {output}"
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => panic!("stdout reader did not finish"),
            }
        }
        (status, stderr)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Project {
    _directory: TempDir,
    path: PathBuf,
}

impl Project {
    fn with_json(value: Value) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("project with spaces.json");
        fs::write(&path, value.to_string()).expect("write project");
        Self {
            _directory: directory,
            path,
        }
    }

    fn new() -> Self {
        Self::with_json(json!({
            "version": 2,
            "name": "Protocol test",
            "vocabulary": {"original": [{"word": "cat", "meaning": "animal"}, {"word": "dog", "meaning": "canine"}]},
            "sentences": [{"words": [0, 0], "meaning": "A cat."}, {"words": [1], "meaning": "A dog."}]
        }))
    }
}

fn identity(info: &Value) -> Value {
    json!({"session_id": info["session"]["session_id"], "expected_revision": info["session"]["revision"]})
}

fn edit_args(info: &Value, translation: &str, dry_run: bool) -> Value {
    let mut args = identity(info);
    args["batch"] = json!({"schema_version": 1, "commands": [{"op": "set_translation", "segment_index": 0, "translation": translation}]});
    args["dry_run"] = json!(dry_run);
    args
}

fn assert_error(response: &Value, code: &str) {
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(response["error"]["code"], code, "{response}");
}

#[test]
fn both_samples_are_readable_with_baseline_and_current_protocols() {
    for version in [BASELINE, CURRENT] {
        for sample in ["ginger", "epigraph"] {
            let path =
                Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../sample/{sample}.json"));
            let mut client = Client::start(&path, &[]);
            let advertised = client.initialize(version);
            assert!(advertised.to_string().contains("tools"));
            let tools = client.discover();
            assert!(tools.iter().all(|tool| !matches!(
                tool["name"].as_str(),
                Some("project_edit" | "project_save")
            )));
            let info = client.call("project_info", json!({}));
            assert_eq!(info["ok"], true, "{info}");
            assert_eq!(info["data"]["writable"], false);
            assert!(
                info["data"]["segment_count"]
                    .as_u64()
                    .expect("segment count")
                    > 0
            );
            assert_eq!(info["session"]["dirty"], false);
            let mut args = identity(&info);
            args["limit"] = json!(1);
            let segments = client.call("segments_list", args);
            assert_eq!(
                segments["data"]["items"].as_array().expect("items").len(),
                1
            );
            let mut args = identity(&info);
            args["segment_index"] = json!(0);
            let detail = client.call("segment_get", args);
            assert_eq!(detail["ok"], true, "{detail}");
            assert!(detail["data"]["tokens"].is_array());
            assert!(client.finish().0.success());
        }
    }
}

#[test]
fn schemas_and_runtime_reject_invalid_arguments_without_protocol_success() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &["--write"]);
    client.initialize(BASELINE);
    let tools = client.discover();
    let segments = tools
        .iter()
        .find(|t| t["name"] == "segments_list")
        .expect("segments tool");
    assert_eq!(
        segments["inputSchema"]["properties"]["limit"]["default"],
        50
    );
    assert_eq!(segments["inputSchema"]["properties"]["limit"]["minimum"], 1);
    assert_eq!(
        segments["inputSchema"]["properties"]["limit"]["maximum"],
        200
    );
    let info = client.call("project_info", json!({}));
    let segment_validator =
        jsonschema::validator_for(&segments["inputSchema"]).expect("segment input validator");
    assert!(segment_validator.is_valid(&identity(&info)));
    for field in [
        json!({"limit": 0}),
        json!({"limit": 201}),
        json!({"sort": "invalid"}),
        json!({"unknown": true}),
    ] {
        let mut invalid = identity(&info);
        invalid
            .as_object_mut()
            .expect("arguments")
            .extend(field.as_object().expect("field").clone());
        assert!(
            !segment_validator.is_valid(&invalid),
            "schema accepted {invalid}"
        );
    }
    let edit_tool = tools
        .iter()
        .find(|t| t["name"] == "project_edit")
        .expect("edit tool");
    let edit_validator =
        jsonschema::validator_for(&edit_tool["inputSchema"]).expect("edit input validator");
    assert!(edit_validator.is_valid(&edit_args(&info, "value", true)));
    for commands in [
        json!([]),
        json!([{"op": "unknown"}]),
        json!([{"op": "create_rule", "description": "not exposed", "type": "inflection", "script": "fn transform(word) { word }"}]),
        json!([{"op": "set_translation", "segment_index": -1, "translation": "bad index"}]),
        json!([{"op": "set_translation", "segment_index": 0, "translation": "value", "unknown": true}]),
    ] {
        let mut invalid = edit_args(&info, "value", false);
        invalid["batch"]["commands"] = commands;
        assert!(
            !edit_validator.is_valid(&invalid),
            "schema accepted {invalid}"
        );
    }
    let mut invalid = edit_args(&info, "value", false);
    invalid["batch"]["schema_version"] = json!(2);
    assert!(!edit_validator.is_valid(&invalid));
    let mut invalid = edit_args(&info, "value", false);
    invalid["expected_revision"] = json!("1e2");
    assert!(!edit_validator.is_valid(&invalid));
    assert_error(
        &client.call("project_info", json!({"unexpected": true})),
        "invalid_input",
    );
    assert_error(
        &client.call(
            "segment_get",
            json!({"session_id": info["session"]["session_id"], "segment_index": 99}),
        ),
        "invalid_index",
    );
    assert_error(
        &client.call(
            "segments_list",
            json!({"session_id": info["session"]["session_id"], "limit": 201}),
        ),
        "limit_exceeded",
    );
    assert_error(
        &client.call(
            "segments_list",
            json!({"session_id": info["session"]["session_id"], "limit": 0}),
        ),
        "limit_exceeded",
    );
    assert_error(
        &client.call(
            "vocabulary_get",
            json!({"session_id": info["session"]["session_id"], "word": "absent"}),
        ),
        "not_found",
    );
    let mut args = edit_args(&info, "new", false);
    args["batch"]["commands"] = json!([{"op": "create_rule", "description": "unsafe", "type": "inflection", "script": "fn transform(word) { word }"}]);
    let rejected = client.call("project_edit", args);
    assert_error(&rejected, "invalid_input");
    assert_eq!(rejected["error"]["stage"], "input");
    assert_eq!(rejected["error"]["command_index"], 0);
    let unknown = client.request(
        "tools/call",
        json!({"name": "absent_tool", "arguments": {}}),
    );
    assert!(unknown["error"].is_object(), "{unknown}");
    assert_eq!(
        client.call("project_info", json!({}))["session"],
        info["session"]
    );
    assert!(client.finish().0.success());
}

#[test]
fn preview_failure_noop_and_queued_revision_conflicts_are_atomic() {
    let project = Project::new();
    let original = fs::read(&project.path).expect("original");
    let mut client = Client::start(&project.path, &["--write"]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    let preview = client.call("project_edit", edit_args(&info, "Preview", true));
    assert_eq!(preview["data"]["changed"], false);
    assert_eq!(preview["data"]["would_change"], true);
    assert_eq!(preview["data"]["saved"], false);
    assert_eq!(preview["session"], info["session"]);
    let mut args = edit_args(&info, "Must roll back", false);
    args["batch"]["commands"]
        .as_array_mut()
        .expect("commands")
        .push(json!({"op":"set_translation", "segment_index": 99, "translation":"bad"}));
    let failed = client.call("project_edit", args);
    assert_error(&failed, "invalid_index");
    assert_eq!(failed["error"]["command_index"], 1);
    assert_eq!(failed["session"], info["session"]);
    let noop = client.call("project_edit", edit_args(&info, "A cat.", false));
    assert_eq!(noop["data"]["changed"], false);
    assert_eq!(noop["session"], info["session"]);
    let first = client.send(
        "tools/call",
        json!({"name": "project_edit", "arguments": edit_args(&info, "first", false)}),
    );
    let second = client.send(
        "tools/call",
        json!({"name": "project_edit", "arguments": edit_args(&info, "second", false)}),
    );
    let first_response = client.receive(first);
    let second_response = client.receive(second);
    let results = [
        client.tool_envelope("project_edit", &first_response),
        client.tool_envelope("project_edit", &second_response),
    ];
    assert_eq!(results.iter().filter(|r| r["ok"] == true).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| r["error"]["code"] == "revision_conflict")
            .count(),
        1
    );
    let changed = results
        .iter()
        .find(|r| r["ok"] == true)
        .expect("one change");
    assert_eq!(changed["session"]["dirty"], true);
    assert_eq!(changed["data"]["saved"], false);
    let mut stale_page = identity(&info);
    stale_page["offset"] = json!(1);
    assert_error(
        &client.call("segments_list", stale_page),
        "revision_conflict",
    );
    assert!(client.finish().0.success());
    assert_eq!(fs::read(&project.path).expect("unsaved project"), original);
}

#[test]
fn explicit_saves_refresh_baseline_and_reload_rotates_identity() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &["--write"]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    let modified = fs::metadata(&project.path)
        .expect("metadata")
        .modified()
        .expect("mtime");
    let clean = client.call("project_save", identity(&info));
    assert_eq!(clean["data"]["saved"], false);
    assert_eq!(
        fs::metadata(&project.path)
            .expect("metadata")
            .modified()
            .expect("mtime"),
        modified
    );
    let edited = client.call("project_edit", edit_args(&info, "First saved", false));
    let saved = client.call("project_save", identity(&edited));
    assert_eq!(saved["data"]["saved"], true, "{saved}");
    assert_eq!(saved["session"]["dirty"], false);
    assert_eq!(saved["session"]["revision"], edited["session"]["revision"]);
    let edited = client.call("project_edit", edit_args(&saved, "Second saved", false));
    let saved = client.call("project_save", identity(&edited));
    assert_eq!(saved["data"]["saved"], true, "{saved}");
    assert!(
        fs::read_to_string(&project.path)
            .expect("saved file")
            .contains("Second saved")
    );
    let dirty = client.call("project_edit", edit_args(&saved, "Discard me", false));
    assert_error(
        &client.call("project_reload", identity(&dirty)),
        "unsaved_changes",
    );
    let mut args = identity(&dirty);
    args["discard_changes"] = json!(true);
    let reloaded = client.call("project_reload", args);
    assert_eq!(reloaded["ok"], true, "{reloaded}");
    assert_ne!(
        reloaded["session"]["session_id"],
        saved["session"]["session_id"]
    );
    assert_eq!(reloaded["session"]["dirty"], false);
    assert_error(
        &client.call("segments_list", identity(&saved)),
        "session_expired",
    );
    let page = client.call("segments_list", identity(&reloaded));
    assert_eq!(page["data"]["items"][0]["translation"], "Second saved");
    assert!(client.finish().0.success());
}

#[test]
fn external_changes_and_failed_reload_preserve_local_edits() {
    let project = Project::new();
    let original = fs::read(&project.path).expect("original");
    let mut client = Client::start(&project.path, &["--write"]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    let edited = client.call("project_edit", edit_args(&info, "Keep local", false));
    fs::write(&project.path, b"invalid external JSON").expect("external write");
    let conflict = client.call("project_save", identity(&edited));
    assert_error(&conflict, "input_changed");
    assert_eq!(conflict["session"], edited["session"]);
    let mut args = identity(&edited);
    args["discard_changes"] = json!(true);
    let failed = client.call("project_reload", args.clone());
    assert_error(&failed, "invalid_input");
    assert_eq!(failed["session"], edited["session"]);
    let replacement = project.path.with_extension("replacement.json");
    fs::write(&replacement, original).expect("replacement");
    fs::remove_file(&project.path).expect("remove external file");
    fs::rename(replacement, &project.path).expect("replace external file");
    let reloaded = client.call("project_reload", args);
    assert_eq!(reloaded["ok"], true, "{reloaded}");
    assert_ne!(
        reloaded["session"]["session_id"],
        edited["session"]["session_id"]
    );
    let edited = client.call(
        "project_edit",
        edit_args(&reloaded, "After external replacement", false),
    );
    let saved = client.call("project_save", identity(&edited));
    assert_eq!(saved["data"]["saved"], true, "{saved}");
    fs::remove_file(&project.path).expect("external deletion");
    assert_error(
        &client.call("project_save", identity(&edited)),
        "input_changed",
    );
    assert!(client.finish().0.success());
}

#[test]
fn read_only_policy_is_enforced_even_for_unadvertised_writes() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &[]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    assert_error(
        &client.call("project_edit", edit_args(&info, "forbidden", false)),
        "read_only",
    );
    assert_error(&client.call("project_save", identity(&info)), "read_only");
    assert!(client.finish().0.success());
}

#[test]
fn query_and_export_tools_preserve_shared_api_semantics() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &[]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    let vocabulary = client.call("vocabulary_list", identity(&info));
    assert_eq!(vocabulary["data"]["total"], 2);
    let mut args = identity(&info);
    args["word"] = json!("cat");
    let word = client.call("vocabulary_get", args.clone());
    assert_eq!(word["data"]["meaning"], "animal");
    let usages = client.call("word_lookup", args.clone());
    assert_eq!(
        usages["data"]["total"], 1,
        "Repeated tokens return a segment only once"
    );
    args["kind"] = json!("headword");
    let headwords = client.call("word_lookup", args.clone());
    assert_eq!(headwords["data"]["items"][0]["segment_index"], 0);
    args["word"] = json!("at");
    assert_eq!(client.call("word_lookup", args)["data"]["total"], 0);
    let mut args = identity(&info);
    args["filter"] = json!("animal");
    assert_eq!(
        client.call("segments_list", args.clone())["data"]["total"],
        0,
        "Filter does not search glosses"
    );
    args["filter"] = json!("cat cat");
    assert_eq!(
        client.call("segments_list", args)["data"]["total"],
        0,
        "Filter does not span tokens"
    );
    let export = client.call("project_export_typst", identity(&info));
    assert_eq!(export["ok"], true, "{export}");
    assert!(
        export["data"]["mime_type"]
            .as_str()
            .expect("MIME type")
            .contains("typst")
    );
    assert!(
        !export["data"]["content"]
            .as_str()
            .expect("Typst source")
            .is_empty()
    );
    assert!(client.finish().0.success());
}

#[test]
fn result_budget_rejects_large_reads_and_edits_before_commit() {
    let source = Project::new();
    let mut value: Value =
        serde_json::from_slice(&fs::read(&source.path).expect("fixture")).expect("JSON");
    value["sentences"][0]["meaning"] = json!("x".repeat(6000));
    let project = Project::with_json(value);
    let mut client = Client::start(&project.path, &["--write", "--max-result-bytes", "4096"]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    assert_eq!(info["ok"], true, "{info}");
    let mut args = identity(&info);
    args["segment_index"] = json!(0);
    assert_error(&client.call("segment_get", args), "result_too_large");
    let mut args = identity(&info);
    args["batch"] = json!({"schema_version": 1, "commands": (0..100).map(|n| json!({"op": "set_comment", "segment_index": 0, "token_index": 0, "comment": format!("note {n}")})).collect::<Vec<_>>()});
    let rejected = client.call("project_edit", args);
    assert_error(&rejected, "result_too_large");
    assert_eq!(rejected["session"], info["session"]);
    assert_eq!(
        client.call("project_info", json!({}))["session"],
        info["session"]
    );
    assert!(client.finish().0.success());
}

#[test]
fn script_prints_stay_on_stderr_and_stdout_is_protocol_only() {
    let project = Project::with_json(json!({
        "version": 2,
        "formation": [{"description": "Plural", "type": "inflection", "command": "fn transform(word) { print(\"script diagnostic marker\"); word + \"s\" }"}],
        "vocabulary": {"original": [{"word": "cat", "meaning": "animal"}], "formatted": [{"word": [0, 0], "comment": ""}]},
        "sentences": [{"words": [-1], "meaning": "Cats."}]
    }));
    let mut client = Client::start(&project.path, &[]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    assert_eq!(info["ok"], true, "{info}");
    let (status, stderr) = client.finish();
    assert!(status.success(), "{stderr}");
    assert!(stderr.contains("script diagnostic marker"), "{stderr}");
}

#[test]
fn oversized_protocol_lines_terminate_without_unbounded_buffering() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &["--max-message-bytes", "1024"]);
    client.initialize(BASELINE);
    let stdin = client.stdin.as_mut().expect("stdin");
    // Deliberately omit the newline: rejection must happen before buffering an entire line or waiting for the sender to delimit it.
    stdin.write_all(&vec![b'x'; 2048]).expect("oversized input");
    stdin.flush().expect("flush");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if client.child.try_wait().expect("child status").is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "oversized unterminated line did not stop admission"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let (_, stderr) = client.finish();
    assert!(
        !stderr.is_empty(),
        "oversized transport input must produce a diagnostic"
    );
}

#[test]
fn invalid_oversized_and_script_failing_initial_loads_exit_without_an_empty_session() {
    let malformed = Project::new();
    fs::write(&malformed.path, b"not a project").expect("malformed input");
    let oversized = Project::new();
    let failing = Project::with_json(json!({
        "version": 2,
        "formation": [{"description": "Failure", "type": "inflection", "command": "fn transform(word) { throw \"intentional startup failure\"; }"}],
        "vocabulary": {"original": [{"word": "cat", "meaning": "animal"}], "formatted": [{"word": [0, 0], "comment": ""}]},
        "sentences": [{"words": [-1], "meaning": ""}]
    }));
    for (project, flags) in [
        (&malformed, vec![]),
        (&oversized, vec!["--max-project-bytes", "32"]),
        (&failing, vec![]),
    ] {
        let mut client = Client::start(&project.path, &flags);
        // Keep stdin open. Failure of initial loading must independently stop the process instead of exposing the default Session or awaiting EOF.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if client.child.try_wait().expect("child status").is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "initial load failure did not stop the server"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let (status, stderr) = client.finish();
        assert!(!status.success());
        assert!(stderr.contains("Initial project load failed"), "{stderr}");
    }
}

#[test]
fn unknown_protocol_methods_and_unsupported_inline_versions_use_protocol_errors() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &[]);
    client.initialize(BASELINE);
    let unknown = client.request("unknown/method", json!({}));
    assert!(unknown["error"].is_object(), "{unknown}");
    assert!(client.finish().0.success());

    let mut client = Client::start(&project.path, &[]);
    let unsupported = client.request(
        "server/discover",
        json!({"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2099-01-01",
            "io.modelcontextprotocol/clientInfo": {"name": "unsupported-test", "version": "1"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }}),
    );
    assert!(unsupported["error"].is_object(), "{unsupported}");
    client.finish();
}

#[test]
fn cancelling_an_executing_reload_preserves_session_edits_and_disk_baseline() {
    let project = Project::new();
    let baseline = fs::read(&project.path).expect("baseline bytes");
    let mut client = Client::start(&project.path, &["--write"]);
    client.initialize(BASELINE);
    client.discover();
    let info = client.call("project_info", json!({}));
    let edited = client.call("project_edit", edit_args(&info, "Retain this edit", false));
    let script = r#"fn transform(word) {
        let payload = "x";
        for n in 0..18 { payload += payload; }
        print("reload cancellation checkpoint");
        while true { payload = payload.to_upper(); }
        word
    }"#;
    let external = json!({
        "version": 2,
        "formation": [{"description": "Slow reload", "type": "inflection", "command": script}],
        "vocabulary": {"original": [{"word": "cat", "meaning": "animal"}], "formatted": [{"word": [0, 0], "comment": ""}]},
        "sentences": [{"words": [-1], "meaning": "external replacement"}]
    });
    fs::write(&project.path, external.to_string()).expect("external slow project");
    let mut arguments = identity(&edited);
    arguments["discard_changes"] = json!(true);
    let cancelled_id = client.send(
        "tools/call",
        json!({"name": "project_reload", "arguments": arguments}),
    );
    // A script diagnostic proves that reconstruction is running. Native string conversions keep the loop busy without reaching its evaluator budget in the interval needed to send cancellation; no timing-based sleep is used.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let diagnostic = client
            .diagnostics
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("reload script reached cancellation checkpoint");
        if diagnostic.contains("reload cancellation checkpoint") {
            break;
        }
    }
    client.cancelled_ids.push(cancelled_id);
    client.write(&json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": cancelled_id, "reason": "integration test cancellation"}}));
    let after = client.call("project_info", json!({}));
    assert_eq!(after["session"], edited["session"]);
    let segments = client.call("segments_list", identity(&after));
    assert_eq!(
        segments["data"]["items"][0]["translation"],
        "Retain this edit"
    );
    assert_error(
        &client.call("project_save", identity(&after)),
        "input_changed",
    );
    fs::write(&project.path, baseline).expect("restore original disk bytes");
    let saved = client.call("project_save", identity(&after));
    assert_eq!(saved["data"]["saved"], true, "{saved}");
    assert!(client.finish().0.success());
}

#[test]
fn malformed_messages_return_protocol_errors_and_leave_transport_usable() {
    let project = Project::new();
    let mut client = Client::start(&project.path, &[]);
    client.initialize(BASELINE);
    for (wire, code, id) in [
        ("{\n", -32700, Value::Null),
        ("[]\n", -32600, Value::Null),
        (
            "{\"jsonrpc\":\"2.0\",\"id\":91,\"method\":42}\n",
            -32600,
            json!(91),
        ),
    ] {
        let stdin = client.stdin.as_mut().expect("stdin");
        stdin.write_all(wire.as_bytes()).expect("malformed message");
        stdin.flush().expect("flush");
        let response = client
            .stdout
            .recv_timeout(RESPONSE_TIMEOUT)
            .expect("protocol error response")
            .expect("protocol JSON");
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["error"]["code"], code, "{response}");
        // The current SDK omits an unknown ID; earlier peers may see null.
        assert_eq!(response["id"], id, "{response}");
        assert!(response.get("result").is_none());
    }
    client.discover();
    let info = client.call("project_info", json!({}));
    assert_eq!(info["ok"], true, "{info}");
    assert!(client.finish().0.success());
}

/// Tests that deliberately stop reading stdout cannot use Client's background reader. This guard still reaps the child if an assertion or deadline fails.
struct RawProcess(Child);

impl Drop for RawProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn initialized_raw_process(project: &Path) -> (RawProcess, ChildStdin, BufReader<ChildStdout>) {
    let mut child = RawProcess(
        Command::new(env!("CARGO_BIN_EXE_tdector-mcp"))
            .arg("--project")
            .arg(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start raw MCP process"),
    );
    let mut stdin = child.0.stdin.take().expect("stdin pipe");
    let stdout = child.0.stdout.take().expect("stdout pipe");
    let (tx, receive) = mpsc::channel();
    thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        let read = stdout.read_line(&mut line);
        let _ = tx.send((stdout, line, read));
    });
    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": BASELINE, "capabilities": {},
            "clientInfo": {"name": "transport-shutdown-test", "version": "1"}}
    });
    writeln!(&mut stdin, "{initialize}").expect("initialize request");
    stdin.flush().expect("flush initialize");
    let (stdout, line, read) = receive
        .recv_timeout(RESPONSE_TIMEOUT)
        .expect("bounded initialization response");
    assert!(read.expect("read initialization") > 0);
    let response: Value = serde_json::from_str(&line).expect("protocol response");
    assert_eq!(
        response["result"]["protocolVersion"], BASELINE,
        "{response}"
    );
    writeln!(
        &mut stdin,
        "{}",
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
    )
    .expect("initialized notification");
    stdin.flush().expect("flush initialized");
    (child, stdin, stdout)
}

fn assert_raw_process_exits(child: &mut RawProcess, context: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.0.try_wait().expect("child status").is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "MCP did not exit within 10 seconds: {context}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn broken_stdout_stops_the_process_with_stdin_still_open() {
    let project = Project::new();
    let (mut child, mut stdin, stdout) = initialized_raw_process(&project.path);
    drop(stdout);
    writeln!(
        &mut stdin,
        "{}",
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "project_info", "arguments": {}}})
    )
    .expect("request whose response hits broken stdout");
    stdin.flush().expect("flush request");
    assert_raw_process_exits(&mut child, "stdout closed while stdin remained open");
    drop(stdin);
}

#[test]
fn stdout_backpressure_does_not_prevent_shutdown_on_stdin_eof() {
    let mut requests = String::new();
    for id in 2..18 {
        requests.push_str(
            &json!({"jsonrpc": "2.0", "id": id, "method": "tools/list", "params": {}}).to_string(),
        );
        requests.push('\n');
    }
    assert_shutdown_with_unread_stdout(requests);
}

#[test]
fn malformed_message_backpressure_does_not_prevent_shutdown() {
    assert_shutdown_with_unread_stdout("{\n".repeat(1000));
}

fn assert_shutdown_with_unread_stdout(requests: String) {
    let project = Project::new();
    let (mut child, mut stdin, stdout) = initialized_raw_process(&project.path);
    // A compact input burst requests far more JSON output than an OS pipe holds. The writer runs independently so the test's own pipe writes cannot prevent its exit deadline or panic cleanup from running.
    assert!(requests.len() < 2048);
    let (written_tx, written_rx) = mpsc::channel();
    let (close_tx, close_rx) = mpsc::channel();
    thread::spawn(move || {
        let written = stdin
            .write_all(requests.as_bytes())
            .and_then(|()| stdin.flush());
        let _ = written_tx.send(written);
        let _ = close_rx.recv_timeout(Duration::from_secs(10));
        drop(stdin);
    });
    written_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("small request burst fits in stdin pipe")
        .expect("write request burst");
    // Give responses time to fill the unread output pipe before EOF.
    thread::sleep(Duration::from_millis(200));
    close_tx.send(()).expect("close stdin");
    assert_raw_process_exits(&mut child, "stdin EOF while stdout was open and unread");
    drop(stdout);
}
