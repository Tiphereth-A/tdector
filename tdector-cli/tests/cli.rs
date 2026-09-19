//! Process-level checks for the public, noninteractive CLI contract.

use std::fs::{self, File, FileTimes};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use tempfile::TempDir;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tdector"))
}

fn path(path: &Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn run(args: &[&str], input: Option<&[u8]>) -> Output {
    let mut command = binary();
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(input) = input {
        let mut child = command.stdin(Stdio::piped()).spawn().expect("start CLI");
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input)
            .expect("write stdin");
        child.wait_with_output().expect("wait for CLI")
    } else {
        command.stdin(Stdio::null()).output().expect("run CLI")
    }
}

fn assert_exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn envelope(output: &Output, exit: i32) -> Value {
    assert_exit(output, exit);
    assert!(
        output.stdout.ends_with(b"\n"),
        "JSON reports end with a newline"
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected exactly one JSON report: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["ok"], exit == 0);
    if exit == 0 {
        assert!(value.get("data").is_some());
        assert!(value.get("error").is_none());
    } else {
        assert!(value["error"]["code"].is_string());
        assert!(value["error"]["message"].is_string());
        assert!(value.get("data").is_none());
    }
    value
}

struct Project {
    directory: TempDir,
    file: PathBuf,
}

impl Project {
    fn new(text: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("project with spaces.json");
        let imported = run(
            &[
                "import",
                "-",
                "--name",
                "試験 project",
                "--output",
                path(&file),
                "--json",
            ],
            Some(text.as_bytes()),
        );
        envelope(&imported, 0);
        Self { directory, file }
    }

    fn write(&self, name: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let file = self.directory.path().join(name);
        fs::write(&file, contents).expect("write test input");
        file
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut full_args = vec!["--project", path(&self.file)];
        full_args.extend_from_slice(args);
        run(&full_args, None)
    }

    fn query(&self, args: &[&str]) -> Value {
        let mut full_args = args.to_vec();
        full_args.push("--json");
        envelope(&self.run(&full_args), 0)["data"].clone()
    }

    fn batch(&self, commands: Value) -> Output {
        let file = self.write(
            "commands.json",
            json!({"schema_version": 1, "commands": commands}).to_string(),
        );
        self.run(&["batch", path(&file), "--in-place", "--json"])
    }
}

#[test]
fn advertised_commands_have_nested_help_and_version_is_text() {
    for args in [
        vec![],
        vec!["info"],
        vec!["validate"],
        vec!["import"],
        vec!["segment", "list"],
        vec!["segment", "show"],
        vec!["segment", "translate"],
        vec!["vocab", "list"],
        vec!["vocab", "get"],
        vec!["vocab", "set"],
        vec!["vocab", "search"],
        vec!["comment", "get"],
        vec!["comment", "set"],
        vec!["lookup"],
        vec!["similar", "segments"],
        vec!["similar", "tokens"],
        vec!["rule", "list"],
        vec!["rule", "show"],
        vec!["rule", "add"],
        vec!["rule", "preview"],
        vec!["formation", "apply"],
        vec!["formation", "chain"],
        vec!["formation", "pop"],
        vec!["tokenize", "preview"],
        vec!["export", "json"],
        vec!["export", "typst"],
        vec!["batch"],
    ] {
        let mut help_args = args;
        help_args.push("--help");
        let output = run(&help_args, None);
        assert_exit(&output, 0);
        assert!(!output.stdout.is_empty(), "help for {help_args:?}");
    }
    let output = run(&["--json", "--version"], None);
    assert_exit(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
    assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
    let help = run(&["--help"], None);
    let help = String::from_utf8_lossy(&help.stdout).to_lowercase();
    assert!(help.contains("zero-based") || help.contains("0-based"));
}

#[test]
fn argument_errors_use_json_even_when_json_follows_the_subcommand() {
    for args in [
        vec!["info", "--json"],
        vec!["info", "--project", "--json"],
        vec!["segment", "show", "-1", "--json"],
        vec!["segment", "list", "--limit", "0", "--json"],
        vec!["segment", "list", "--offset", "-1", "--json"],
        vec!["segment", "list", "--all", "--limit", "1", "--json"],
        vec!["segment", "list", "--all", "--offset", "0", "--json"],
        vec!["similar", "tokens", "cat", "--limit", "21", "--json"],
        vec!["unknown-command", "--json"],
    ] {
        envelope(&run(&args, None), 2);
    }
}

#[test]
fn option_conflicts_are_rejected_before_reading_stdin() {
    for args in [
        vec![
            "-p",
            "-",
            "segment",
            "translate",
            "0",
            "--text-file",
            "-",
            "--dry-run",
            "--json",
        ],
        vec!["-p", "-", "batch", "-", "--dry-run", "--json"],
        vec![
            "-p",
            "-",
            "rule",
            "add",
            "--description",
            "Plural",
            "--type",
            "inflection",
            "--script-file",
            "-",
            "--dry-run",
            "--json",
        ],
        vec![
            "import",
            "-",
            "--tokenizer-script",
            "-",
            "--dry-run",
            "--json",
        ],
        vec![
            "-p",
            "-",
            "vocab",
            "set",
            "cat",
            "--text",
            "feline",
            "--in-place",
            "--json",
        ],
        vec![
            "-p",
            "-",
            "vocab",
            "set",
            "cat",
            "--text",
            "feline",
            "--dry-run",
            "--output",
            "copy.json",
            "--json",
        ],
    ] {
        let mut child = binary()
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start CLI");
        // Keep stdin open and empty: a command that tries to read it cannot exit.
        let held_stdin = child.stdin.take().expect("piped stdin");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().expect("poll CLI").is_some() {
                break;
            }
            if Instant::now() >= deadline {
                child.kill().expect("kill blocked CLI");
                let _ = child.wait();
                panic!("CLI read stdin before rejecting conflicting options: {args:?}");
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(held_stdin);
        envelope(&child.wait_with_output().expect("collect CLI output"), 2);
    }
}

#[test]
fn unicode_boms_and_text_file_newlines_survive_save_reload_and_stdin() {
    let project = Project::new("\u{feff}猫 café\n\n犬\n");
    let info = project.query(&["info"]);
    assert_eq!(info["project_name"], "試験 project");
    assert_eq!(info["segment_count"], 2);
    assert_eq!(info["token_count"], 3);
    let text = project.write(
        "translation with spaces.txt",
        "\u{feff}一匹の猫\nsecond line\n",
    );
    envelope(
        &project.run(&[
            "segment",
            "translate",
            "0",
            "--text-file",
            path(&text),
            "--in-place",
            "--json",
        ]),
        0,
    );
    assert_eq!(
        project.query(&["segment", "show", "0"])["translation"],
        "一匹の猫\nsecond line\n"
    );
    let bytes = fs::read(&project.file).expect("saved project");
    let mut bom_bytes = b"\xef\xbb\xbf".to_vec();
    bom_bytes.extend_from_slice(&bytes);
    let from_stdin = envelope(
        &run(
            &["segment", "show", "0", "--project", "-", "--json"],
            Some(&bom_bytes),
        ),
        0,
    );
    assert_eq!(from_stdin["data"]["translation"], "一匹の猫\nsecond line\n");
    assert_eq!(from_stdin["data"]["segment_index"], 0);
    assert_eq!(from_stdin["data"]["tokens"][0]["token_index"], 0);
    let exported = project.run(&["export", "json"]);
    assert_exit(&exported, 0);
    let artifact: Value = serde_json::from_slice(&exported.stdout).expect("raw saved project");
    assert_eq!(artifact["version"], 2);
    assert!(artifact.get("schema_version").is_none());
    envelope(
        &run(
            &["--project", "-", "validate", "--json"],
            Some(&exported.stdout),
        ),
        0,
    );
}

#[test]
fn leading_hyphen_input_paths_work_after_double_dash() {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(directory.path().join("-source.txt"), "cat").expect("source");
    let output = binary()
        .current_dir(directory.path())
        .args(["import", "--dry-run", "--json", "--", "-source.txt"])
        .output()
        .expect("run CLI");
    let report = envelope(&output, 0);
    assert_eq!(report["data"]["saved"], false);
    assert_eq!(report["data"]["dry_run"], true);
}

#[test]
fn fresh_import_names_and_empty_tokenizer_lines_follow_the_contract() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = directory.path().join("named source.txt");
    let tokenizer = directory.path().join("skip.rhai");
    fs::write(&source, "\ncat\nskip\n犬\n").expect("source");
    fs::write(
        &tokenizer,
        "fn tokenize(line) { if line == \"skip\" { [] } else { [line] } }",
    )
    .expect("tokenizer");
    let output = run(
        &[
            "import",
            path(&source),
            "--tokenizer-script",
            path(&tokenizer),
            "--output",
            "-",
        ],
        None,
    );
    assert_exit(&output, 0);
    let saved: Value = serde_json::from_slice(&output.stdout).expect("saved project");
    assert_eq!(saved["project_name"], "named source");
    assert_eq!(saved["sentences"].as_array().expect("segments").len(), 2);
    let output = run(&["import", "-", "--output", "-"], Some("猫".as_bytes()));
    assert_exit(&output, 0);
    let saved: Value = serde_json::from_slice(&output.stdout).expect("saved project");
    assert_eq!(saved["project_name"], "Untitled");
}

#[test]
fn pagination_sorting_and_empty_results_have_stable_list_envelopes() {
    let project = Project::new("zebra cat\nCat\na dog fish\n");
    let page = project.query(&[
        "segment",
        "list",
        "--sort",
        "token-count",
        "--offset",
        "1",
        "--limit",
        "1",
    ]);
    assert_eq!(page["total"], 3);
    assert_eq!(page["offset"], 1);
    assert_eq!(page["limit"], 1);
    assert_eq!(page["items"].as_array().expect("items").len(), 1);
    assert_eq!(page["items"][0]["segment_index"], 0);
    let all = project.query(&["vocab", "list", "--all"]);
    assert_eq!(all["offset"], 0);
    assert!(all["limit"].is_null());
    assert_eq!(all["total"], 6);
    let words: Vec<_> = all["items"]
        .as_array()
        .expect("vocabulary")
        .iter()
        .map(|entry| entry["word"].as_str().expect("word"))
        .collect();
    let mut sorted = words.clone();
    sorted.sort_unstable();
    assert_eq!(words, sorted);
    let empty = project.query(&["segment", "list", "--filter", "not-present"]);
    assert_eq!(empty["total"], 0);
    assert_eq!(empty["items"], json!([]));
}

#[test]
fn invalid_targets_io_and_invalid_project_have_distinct_exit_codes() {
    let project = Project::new("cat");
    let error = envelope(&project.run(&["segment", "show", "99", "--json"]), 3);
    assert_eq!(error["error"]["code"], "invalid_index");
    assert_eq!(error["error"]["details"]["kind"], "segment");
    assert_eq!(error["error"]["details"]["index"], 99);
    envelope(&project.run(&["vocab", "get", "not-present", "--json"]), 3);
    let missing = project.directory.path().join("does not exist.json");
    let error = envelope(
        &run(&["--project", path(&missing), "info", "--json"], None),
        5,
    );
    assert_eq!(error["error"]["code"], "io_error");
    let corrupt = project.write("broken.json", "{not JSON}");
    let error = envelope(
        &run(&["--project", path(&corrupt), "validate", "--json"], None),
        3,
    );
    assert_eq!(error["error"]["code"], "invalid_project");
    let text_error = project.run(&["segment", "show", "99"]);
    assert_exit(&text_error, 3);
    assert!(text_error.stdout.is_empty());
    assert!(!text_error.stderr.is_empty());
}

#[test]
fn every_mutation_requires_one_save_choice_and_text_choice() {
    let project = Project::new("cat");
    for args in [
        vec!["vocab", "set", "cat", "--text", "animal", "--json"],
        vec!["vocab", "set", "cat", "--in-place", "--json"],
        vec![
            "vocab",
            "set",
            "cat",
            "--text",
            "animal",
            "--clear",
            "--dry-run",
            "--json",
        ],
        vec![
            "vocab",
            "set",
            "cat",
            "--clear",
            "--dry-run",
            "--in-place",
            "--json",
        ],
        vec![
            "vocab",
            "set",
            "cat",
            "--clear",
            "--dry-run",
            "--overwrite",
            "--json",
        ],
        vec![
            "vocab",
            "set",
            "cat",
            "--clear",
            "--in-place",
            "--overwrite",
            "--json",
        ],
        vec![
            "vocab",
            "set",
            "cat",
            "--clear",
            "--output",
            "-",
            "--overwrite",
            "--json",
        ],
        vec!["vocab", "set", "cat", "--clear", "--output", "-", "--json"],
        vec!["export", "json", "--json"],
        vec!["export", "typst", "--json"],
        vec![
            "rule",
            "show",
            "--rule",
            "Plural",
            "--rule-index",
            "0",
            "--json",
        ],
    ] {
        envelope(&project.run(&args), 2);
    }
}

#[test]
fn existing_output_and_same_file_writes_preserve_the_destination() {
    let project = Project::new("cat");
    let destination = project.write("existing.json", b"preserve this destination");
    let output = project.run(&[
        "vocab",
        "set",
        "cat",
        "--text",
        "animal",
        "--output",
        path(&destination),
        "--json",
    ]);
    let error = envelope(&output, 6);
    assert_eq!(error["error"]["code"], "output_exists");
    assert_eq!(
        fs::read(&destination).expect("destination"),
        b"preserve this destination"
    );
    let original = fs::read(&project.file).expect("original");
    for args in [
        vec![
            "vocab",
            "set",
            "cat",
            "--text",
            "animal",
            "--output",
            path(&project.file),
            "--overwrite",
            "--json",
        ],
        vec![
            "export",
            "json",
            "--output",
            path(&project.file),
            "--overwrite",
            "--json",
        ],
    ] {
        envelope(&project.run(&args), 2);
        assert_eq!(fs::read(&project.file).expect("input"), original);
    }
    envelope(
        &project.run(&[
            "vocab",
            "set",
            "cat",
            "--text",
            "animal",
            "--output",
            path(&destination),
            "--overwrite",
            "--json",
        ]),
        0,
    );
    envelope(
        &run(
            &["--project", path(&destination), "validate", "--json"],
            None,
        ),
        0,
    );
}

#[test]
fn no_op_in_place_preserves_timestamp_and_explicit_output_still_copies() {
    let project = Project::new("cat");
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400);
    File::options()
        .write(true)
        .open(&project.file)
        .expect("open project")
        .set_times(FileTimes::new().set_modified(modified))
        .expect("set timestamp");
    let before_time = fs::metadata(&project.file)
        .expect("metadata")
        .modified()
        .expect("mtime");
    let before = fs::read(&project.file).expect("project");
    let receipt = project.query(&["segment", "translate", "0", "--clear", "--in-place"]);
    assert_eq!(receipt["changed"], false);
    assert_eq!(fs::read(&project.file).expect("project"), before);
    assert_eq!(
        fs::metadata(&project.file)
            .expect("metadata")
            .modified()
            .expect("mtime"),
        before_time
    );
    let copy = project.directory.path().join("copy.json");
    let receipt = project.query(&[
        "segment",
        "translate",
        "0",
        "--clear",
        "--output",
        path(&copy),
    ]);
    assert_eq!(receipt["changed"], false);
    assert_eq!(receipt["saved"], true);
    assert!(copy.exists());
    envelope(
        &run(&["--project", path(&copy), "validate", "--json"], None),
        0,
    );
}

#[test]
fn an_observed_external_edit_prevents_an_in_place_commit() {
    let project = Project::new("cat cats");
    envelope(&project.batch(json!([
        {"op": "create_rule", "description": "Plural", "type": "inflection", "script": "fn transform(word) { print(\"project loaded signal\"); word + \"s\" }"},
        {"op": "apply_formation", "word": "cats", "base": "cat", "rule": "Plural"}
    ])), 0);
    let original = fs::read(&project.file).expect("original project");
    let mut child = binary()
        .args([
            "--project",
            path(&project.file),
            "vocab",
            "set",
            "cat",
            "--text-file",
            "-",
            "--in-place",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start CLI");
    let mut input = child.stdin.take().expect("piped stdin");
    let stderr = child.stderr.take().expect("piped stderr");
    let (loaded_tx, loaded_rx) = mpsc::channel();
    let stderr_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut collected = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read stderr") == 0 {
                return collected;
            }
            if line.contains("project loaded signal") {
                let _ = loaded_tx.send(());
            }
            collected.push_str(&line);
        }
    });
    // The stored formation runs only after the input bytes have been read.
    // The edit then blocks on stdin, allowing a deterministic external write.
    if loaded_rx.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
        let output = child.wait_with_output().expect("collect blocked CLI");
        drop(input);
        let stderr = stderr_reader.join().expect("stderr reader");
        panic!(
            "CLI did not announce project load; stdout={}, stderr={stderr}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let mut external = original;
    external.extend_from_slice(b"\n \n");
    fs::write(&project.file, &external).expect("external edit");
    input.write_all(b"changed gloss").expect("finish stdin");
    drop(input);
    let output = child.wait_with_output().expect("wait for conflict");
    let error = envelope(&output, 6);
    assert_eq!(error["error"]["code"], "input_changed");
    assert_eq!(
        fs::read(&project.file).expect("external edit survives"),
        external
    );
    stderr_reader.join().expect("stderr reader");
}

#[cfg(windows)]
#[test]
fn failed_atomic_batch_replacement_preserves_destination_and_reports_commit_stage() {
    use std::os::windows::fs::OpenOptionsExt;

    let project = Project::new("cat");
    let original = fs::read(&project.file).expect("original project");
    let held_open = File::options()
        .read(true)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .open(&project.file)
        .expect("hold destination without delete sharing");
    let output = project.batch(json!([
        {"op": "set_gloss", "word": "cat", "meaning": "must not commit"}
    ]));
    let error = envelope(&output, 5);
    assert_eq!(error["error"]["code"], "io_error");
    assert_eq!(error["error"]["stage"], "commit");
    assert!(error["error"].get("command_index").is_none());
    assert_eq!(
        fs::read(&project.file).expect("preserved project"),
        original
    );
    drop(held_open);
    let mut files: Vec<_> = fs::read_dir(project.directory.path())
        .expect("directory")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["commands.json", "project with spaces.json"],
        "no staging file survives"
    );
}

#[test]
fn dry_run_and_raw_project_output_do_not_modify_input() {
    let project = Project::new("cat");
    let before = fs::read(&project.file).expect("project");
    let receipt = project.query(&["vocab", "set", "cat", "--text", "animal", "--dry-run"]);
    assert_eq!(receipt["changed"], true);
    assert_eq!(receipt["saved"], false);
    assert_eq!(receipt["dry_run"], true);
    assert_eq!(fs::read(&project.file).expect("project"), before);
    let output = project.run(&["vocab", "set", "cat", "--text", "animal", "--output", "-"]);
    assert_exit(&output, 0);
    let artifact: Value = serde_json::from_slice(&output.stdout).expect("raw project only");
    assert_eq!(artifact["version"], 2);
    assert!(artifact.get("ok").is_none());
    let reloaded = envelope(
        &run(
            &["-p", "-", "vocab", "get", "cat", "--json"],
            Some(&output.stdout),
        ),
        0,
    );
    assert_eq!(reloaded["data"]["meaning"], "animal");
    assert_eq!(fs::read(&project.file).expect("project"), before);
}

#[test]
fn script_diagnostics_stay_on_stderr_and_script_errors_are_typed() {
    let project = Project::new("cat");
    let script = project.write(
        "plural.rhai",
        "fn transform(word) { print(\"script diagnostic\"); word + \"s\" }",
    );
    let output = run(
        &[
            "rule",
            "preview",
            "--script-file",
            path(&script),
            "--word",
            "cat",
            "--json",
        ],
        None,
    );
    let result = envelope(&output, 0);
    assert_eq!(result["data"]["result"], "cats");
    assert!(String::from_utf8_lossy(&output.stderr).contains("script diagnostic"));
    let broken = project.write("broken.rhai", "fn transform(word) { let = ; }");
    let error = envelope(
        &run(
            &[
                "rule",
                "preview",
                "--script-file",
                path(&broken),
                "--word",
                "cat",
                "--json",
            ],
            None,
        ),
        4,
    );
    assert_eq!(error["error"]["code"], "script_error");
    let runtime = project.write(
        "runtime.rhai",
        "fn transform(word) { throw \"cannot transform\"; }",
    );
    let error = envelope(
        &run(
            &[
                "rule",
                "preview",
                "--script-file",
                path(&runtime),
                "--word",
                "cat",
                "--json",
            ],
            None,
        ),
        4,
    );
    assert_eq!(error["error"]["code"], "script_error");
}

#[test]
fn tokenizer_scripts_receive_literal_arguments_and_require_string_arrays() {
    let project = Project::new("cat");
    envelope(
        &run(&["tokenize", "preview", "--line", "a\nb", "--json"], None),
        3,
    );
    let literal = "quote\" and \\ path 雪";
    let script = project.write("tokenizer.rhai", "fn tokenize(line) { [line] }");
    let output = run(
        &[
            "tokenize",
            "preview",
            "--line",
            literal,
            "--tokenizer-script",
            path(&script),
            "--json",
        ],
        None,
    );
    assert_eq!(envelope(&output, 0)["data"]["tokens"], json!([literal]));
    let invalid = project.write("invalid-tokenizer.rhai", "fn tokenize(line) { [line, 7] }");
    let error = envelope(
        &run(
            &[
                "tokenize",
                "preview",
                "--line",
                "cat",
                "--tokenizer-script",
                path(&invalid),
                "--json",
            ],
            None,
        ),
        4,
    );
    assert_eq!(error["error"]["code"], "script_error");
    let character = envelope(
        &run(
            &[
                "tokenize",
                "preview",
                "--line",
                "猫犬",
                "--tokenizer",
                "character",
                "--json",
            ],
            None,
        ),
        0,
    );
    assert_eq!(character["data"]["tokens"], json!(["猫", "犬"]));
}

#[test]
fn batch_failure_does_not_commit_earlier_successful_commands() {
    let project = Project::new("cat");
    let before = fs::read(&project.file).expect("project");
    let output = project.batch(json!([
        {"op": "set_gloss", "word": "cat", "meaning": "would change"},
        {"op": "set_translation", "segment_index": 99, "translation": "invalid"}
    ]));
    let error = envelope(&output, 3);
    assert_eq!(error["error"]["code"], "invalid_index");
    assert_eq!(error["error"]["stage"], "command");
    assert_eq!(error["error"]["command_index"], 1);
    assert_eq!(fs::read(&project.file).expect("project"), before);
}

#[test]
fn batch_decode_errors_are_indexed_and_unknown_fields_are_rejected() {
    let project = Project::new("cat");
    let before = fs::read(&project.file).expect("project");
    for bad_command in [
        json!({"op": "not_an_operation"}),
        json!({"op": "set_gloss", "word": "cat", "meaning": "animal", "unknown": true}),
        json!({"op": "set_translation", "segment_index": -1, "translation": "invalid"}),
    ] {
        let output = project.batch(json!([
            {"op": "set_gloss", "word": "cat", "meaning": "would change"}, bad_command
        ]));
        let error = envelope(&output, 3);
        assert_eq!(error["error"]["stage"], "input");
        assert_eq!(error["error"]["command_index"], 1);
        assert_eq!(fs::read(&project.file).expect("project"), before);
    }
    for contents in [
        "{",
        "{\"schema_version\":99,\"commands\":[]}",
        "{\"schema_version\":1,\"commands\":[],\"unknown\":true}",
        "\u{feff}\u{feff}{\"schema_version\":1,\"commands\":[]}",
    ] {
        let file = project.write("bad-batch.json", contents);
        let error = envelope(
            &project.run(&["batch", path(&file), "--in-place", "--json"]),
            3,
        );
        assert_eq!(error["error"]["stage"], "input");
        assert!(error["error"].get("command_index").is_none());
        assert_eq!(fs::read(&project.file).expect("project"), before);
    }
}

#[test]
fn batch_success_resolves_new_rules_in_the_evolving_session() {
    let project = Project::new("cat cats\ncats");
    let output = project.batch(json!([
        {"op": "set_gloss", "word": "cat", "meaning": "animal"},
        {"op": "create_rule", "description": "Plural", "type": "inflection", "script": "fn transform(word) { word + \"s\" }"},
        {"op": "apply_formation", "word": "cats", "base": "cat", "rule": "Plural"},
        {"op": "set_comment", "segment_index": 0, "token_index": 1, "comment": "shared cats"},
        {"op": "set_translation", "segment_index": 0, "translation": "one and many"}
    ]));
    let result = envelope(&output, 0);
    assert_eq!(result["data"]["changed"], true);
    assert_eq!(result["data"]["saved"], true);
    assert_eq!(
        project.query(&["segment", "show", "0"])["translation"],
        "one and many"
    );
    assert_eq!(project.query(&["vocab", "get", "cat"])["meaning"], "animal");
    let comment = project.query(&["comment", "get", "--segment", "1", "--token", "0"]);
    assert_eq!(comment["editable_comment"], "shared cats");
}

#[test]
fn batch_and_edit_text_accept_stdin_when_project_comes_from_a_file() {
    let project = Project::new("cat");
    let batch = json!({"schema_version": 1, "commands": [
        {"op": "set_gloss", "word": "cat", "meaning": "動物"},
        {"op": "set_comment", "segment_index": 0, "comment": "共有"}
    ]});
    let mut bytes = b"\xef\xbb\xbf".to_vec();
    bytes.extend_from_slice(batch.to_string().as_bytes());
    let output = run(
        &[
            "--project",
            path(&project.file),
            "batch",
            "-",
            "--in-place",
            "--json",
        ],
        Some(&bytes),
    );
    envelope(&output, 0);
    assert_eq!(project.query(&["vocab", "get", "cat"])["meaning"], "動物");
    assert_eq!(
        project.query(&["comment", "get", "--segment", "0"])["editable_comment"],
        "共有"
    );
    let output = run(
        &[
            "--project",
            path(&project.file),
            "segment",
            "translate",
            "0",
            "--text-file",
            "-",
            "--in-place",
            "--json",
        ],
        Some("一匹\n".as_bytes()),
    );
    envelope(&output, 0);
    assert_eq!(
        project.query(&["segment", "show", "0"])["translation"],
        "一匹\n"
    );
}

#[test]
fn rule_indices_follow_save_order_and_duplicate_descriptions_require_disambiguation() {
    let project = Project::new("cat cats");
    let script = project.write("plural.rhai", "fn transform(word) { word + \"s\" }");
    for description in ["Zebra", "Alpha"] {
        let result = project.query(&[
            "rule",
            "add",
            "--description",
            description,
            "--type",
            "inflection",
            "--script-file",
            path(&script),
            "--in-place",
        ]);
        assert_eq!(result["description"], description);
        assert_eq!(result["type"], "inflection");
        assert!(
            result.get("rule_index").is_none(),
            "an appended index is not a persistent ID"
        );
    }
    let list = project.query(&["rule", "list", "--all"]);
    assert_eq!(list["items"][0]["description"], "Alpha");
    assert_eq!(list["items"][0]["rule_index"], 0);
    assert_eq!(list["items"][1]["description"], "Zebra");
    assert_eq!(
        project.query(&["rule", "show", "--rule-index", "0"])["description"],
        "Alpha"
    );
    let duplicate = project.write("duplicate.rhai", "fn transform(word) { word + \"es\" }");
    project.query(&[
        "rule",
        "add",
        "--description",
        "Alpha",
        "--type",
        "inflection",
        "--script-file",
        path(&duplicate),
        "--in-place",
    ]);
    let error = envelope(
        &project.run(&["rule", "show", "--rule", "Alpha", "--json"]),
        3,
    );
    assert_eq!(error["error"]["code"], "ambiguous_rule");
    assert_eq!(error["error"]["details"]["rule_indices"], json!([0, 1]));
    project.query(&["rule", "show", "--rule-index", "1"]);
    let missing = envelope(
        &project.run(&["rule", "show", "--rule", "alpha", "--json"]),
        3,
    );
    assert_eq!(missing["error"]["code"], "rule_not_found");
}

#[test]
fn token_similarity_rejects_limits_over_twenty_with_a_loaded_project() {
    let project = Project::new("cat cats bat dog");
    envelope(
        &project.run(&["similar", "tokens", "cat", "--limit", "21", "--json"]),
        2,
    );
    envelope(
        &project.run(&["similar", "tokens", "cat", "--limit", "0", "--json"]),
        2,
    );
    project.query(&["similar", "tokens", "cat", "--limit", "20"]);
}

#[test]
fn formation_and_comment_operations_share_targets_and_survive_reloading() {
    let project = Project::new("cat cats catss\ncats catss");
    envelope(&project.batch(json!([
        {"op": "create_rule", "description": "Plural", "type": "inflection", "script": "fn transform(word) { word + \"s\" }"},
        {"op": "set_comment", "segment_index": 0, "token_index": 0, "comment": "base comment"},
        {"op": "apply_formation", "word": "cats", "base": "cat", "rule": "Plural"},
        {"op": "apply_formation", "word": "catss", "base": "cats", "rule": "Plural"}
    ])), 0);
    let inherited = project.query(&["comment", "get", "--segment", "1", "--token", "0"]);
    assert_eq!(inherited["target"]["kind"], "formatted_word");
    assert_eq!(inherited["target"]["word"], "cats");
    assert_eq!(inherited["editable_comment"], "");
    assert_eq!(inherited["display_comment"], "base comment");
    let set = project.query(&[
        "comment",
        "set",
        "--segment",
        "0",
        "--token",
        "1",
        "--text",
        "derived comment",
        "--in-place",
    ]);
    assert!(set["scope"].as_str().expect("scope").contains("shared"));
    let shared = project.query(&["comment", "get", "--segment", "1", "--token", "0"]);
    assert_eq!(shared["editable_comment"], "derived comment");
    assert_eq!(shared["display_comment"], "derived comment");
    assert_eq!(
        project.query(&["vocab", "get", "cat"])["comment"],
        "base comment"
    );
    let chain = project.query(&["formation", "chain", "--segment", "1", "--token", "1"]);
    assert_eq!(chain["base_word"], "cat");
    assert_eq!(chain["word"], "catss");
    assert_eq!(chain["steps"].as_array().expect("steps").len(), 2);
    let popped = project.query(&[
        "formation",
        "pop",
        "--segment",
        "0",
        "--token",
        "2",
        "--in-place",
    ]);
    let scope = popped["scope"].as_str().expect("documented pop scope");
    assert!(scope.contains("all") && scope.contains("matching"));
    for (segment, token) in [("0", "2"), ("1", "1")] {
        let chain = project.query(&["formation", "chain", "--segment", segment, "--token", token]);
        assert_eq!(chain["word"], "cats");
        assert_eq!(chain["base_word"], "cat");
        assert_eq!(
            chain["steps"]
                .as_array()
                .expect("preceding step preserved")
                .len(),
            1
        );
    }
    assert_eq!(project.query(&["rule", "list"])["total"], 1);
    project.query(&[
        "comment",
        "set",
        "--segment",
        "1",
        "--token",
        "0",
        "--clear",
        "--in-place",
    ]);
    let cleared = project.query(&["comment", "get", "--segment", "0", "--token", "1"]);
    assert_eq!(cleared["editable_comment"], "");
    assert_eq!(cleared["display_comment"], "base comment");
}

#[test]
fn formation_apply_requires_existing_surface_and_exact_rule_result() {
    let project = Project::new("cat cats");
    envelope(&project.batch(json!([
        {"op": "create_rule", "description": "Plural", "type": "inflection", "script": "fn transform(word) { word + \"s\" }"},
        {"op": "set_gloss", "word": "dog", "meaning": "animal"}
    ])), 0);
    let before = fs::read(&project.file).expect("project");
    for (word, base) in [("dogs", "dog"), ("cats", "dog"), ("cats", "missing")] {
        envelope(
            &project.run(&[
                "formation",
                "apply",
                "--rule",
                "Plural",
                "--word",
                word,
                "--base",
                base,
                "--in-place",
                "--json",
            ]),
            3,
        );
        assert_eq!(fs::read(&project.file).expect("unchanged project"), before);
    }
}

#[test]
fn replacement_import_preserves_project_metadata_but_replaces_segments() {
    let project = Project::new("cat cats");
    envelope(&project.batch(json!([
        {"op": "set_gloss", "word": "cat", "meaning": "animal"},
        {"op": "set_translation", "segment_index": 0, "translation": "old translation"},
        {"op": "set_comment", "segment_index": 0, "comment": "old segment comment"},
        {"op": "set_comment", "segment_index": 0, "token_index": 0, "comment": "word comment"},
        {"op": "create_rule", "description": "Plural", "type": "inflection", "script": "fn transform(word) { word + \"s\" }"},
        {"op": "apply_formation", "word": "cats", "base": "cat", "rule": "Plural"}
    ])), 0);
    let source = project.write("replacement.txt", "cats cat\nnew");
    envelope(
        &project.run(&["import", path(&source), "--in-place", "--json"]),
        2,
    );
    envelope(
        &run(
            &[
                "import",
                path(&source),
                "--replace-text",
                "--dry-run",
                "--json",
            ],
            None,
        ),
        2,
    );
    project.query(&["import", path(&source), "--replace-text", "--in-place"]);
    let info = project.query(&["info"]);
    assert_eq!(info["project_name"], "試験 project");
    assert_eq!(info["segment_count"], 2);
    assert_eq!(info["rule_count"], 1);
    assert_eq!(project.query(&["vocab", "get", "cat"])["meaning"], "animal");
    assert_eq!(
        project.query(&["vocab", "get", "cat"])["comment"],
        "word comment"
    );
    let shown = project.query(&["segment", "show", "0"]);
    assert_eq!(shown["translation"], "");
    assert_eq!(shown["comment"], "");
    assert_eq!(shown["tokens"][0]["is_derived"], false);
    assert_eq!(shown["tokens"][0]["text"], "cats");
}

#[test]
fn segment_filters_ignore_glosses_comments_and_lookup_uses_surface_headwords() {
    let project = Project::new("cat cat\ndog cat\ndog");
    envelope(
        &project.batch(json!([
            {"op": "set_gloss", "word": "cat", "meaning": "gloss-only"},
            {"op": "set_comment", "segment_index": 0, "comment": "comment-only"},
            {"op": "set_translation", "segment_index": 2, "translation": "translation-only"}
        ])),
        0,
    );
    for excluded in ["gloss-only", "comment-only"] {
        assert_eq!(
            project.query(&["segment", "list", "--filter", excluded])["total"],
            0
        );
    }
    let translated = project.query(&["segment", "list", "--filter", "translation-only"]);
    assert_eq!(translated["total"], 1);
    assert_eq!(translated["items"][0]["segment_index"], 2);
    let usage = project.query(&["lookup", "cat", "--all"]);
    assert_eq!(
        usage["total"], 2,
        "repeated occurrences count once per segment"
    );
    let headwords = project.query(&["lookup", "cat", "--kind", "headword", "--all"]);
    assert_eq!(headwords["total"], 1);
    assert_eq!(headwords["items"][0]["segment_index"], 0);
    assert_eq!(
        project.query(&["lookup", "Cat"])["total"],
        0,
        "lookup is case sensitive"
    );
    project.query(&["vocab", "set", "cat", "--clear", "--in-place"]);
    assert_eq!(project.query(&["vocab", "get", "cat"])["meaning"], "");
}

#[test]
fn bundled_projects_validate_edit_reload_and_export_without_gui() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository");
    for relative in [
        "sample/ginger.json",
        "sample/epigraph.json",
        "tdector-file/tests/libs/project/sample/migrate_v1_to_v2.json",
    ] {
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = repository.join(relative);
        let edited = directory.path().join("edited.json");
        envelope(&run(&["-p", path(&source), "validate", "--json"], None), 0);
        envelope(
            &run(
                &[
                    "-p",
                    path(&source),
                    "segment",
                    "translate",
                    "0",
                    "--text",
                    "CLI round trip 雪",
                    "--output",
                    path(&edited),
                    "--json",
                ],
                None,
            ),
            0,
        );
        let shown = envelope(
            &run(
                &["-p", path(&edited), "segment", "show", "0", "--json"],
                None,
            ),
            0,
        );
        assert_eq!(shown["data"]["translation"], "CLI round trip 雪");
        let exported = run(&["-p", path(&edited), "export", "json"], None);
        assert_exit(&exported, 0);
        envelope(
            &run(&["-p", "-", "validate", "--json"], Some(&exported.stdout)),
            0,
        );
        let typst = run(&["-p", path(&edited), "export", "typst"], None);
        assert_exit(&typst, 0);
        assert!(!typst.stdout.is_empty());
    }
}
