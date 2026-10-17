//! Windows desktop regression for detached GUI startup and captured pipe EOF.
#![cfg(all(windows, feature = "gui-control"))]

use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

#[test]
#[ignore = "opens a native window; run explicitly on an interactive Windows desktop"]
fn detached_gui_releases_callers_stdout_and_stderr() {
    let binary = env!("CARGO_BIN_EXE_tdector");
    let mut launcher = Command::new(binary)
        .args([
            "--json",
            "gui",
            "start",
            "--timeout",
            "20",
            "--width",
            "800",
            "--height",
            "600",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn GUI launcher");
    let (ready_send, ready) = mpsc::sync_channel(1);
    let (stdout_send, stdout_done) = mpsc::sync_channel(1);
    let (stderr_send, stderr_done) = mpsc::sync_channel(1);
    let stdout = launcher.stdout.take().expect("launcher stdout");
    let mut stderr = launcher.stderr.take().expect("launcher stderr");
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        reader.read_line(&mut first).expect("read startup response");
        ready_send.send(first).expect("send startup response");
        let mut rest = String::new();
        let result = reader.read_to_string(&mut rest);
        let _ = stdout_send.send((result, rest));
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes);
        let _ = stderr_send.send((result, bytes));
    });
    let response = ready.recv_timeout(Duration::from_secs(25));
    if response.is_err() {
        let _ = launcher.kill();
        let _ = launcher.wait();
        panic!("GUI launcher did not return startup response within 25 seconds");
    }
    let response: Value = serde_json::from_str(&response.expect("checked response"))
        .expect("structured startup response");
    assert_eq!(response["ok"], true, "GUI startup failed");
    let session = response["data"]["session"]
        .as_str()
        .expect("startup session locator");
    // Always close the test-owned GUI before asserting EOF so a regression does
    // not leave a detached window behind or blocked capture-reader threads.
    let stdout_eof = stdout_done.recv_timeout(Duration::from_secs(5));
    let stderr_eof = stderr_done.recv_timeout(Duration::from_secs(5));
    let main = response["data"]["main_handle"]
        .as_str()
        .expect("main handle");
    let inspected = Command::new(binary)
        .args(["--json", "gui", "--session", session, "inspect", main])
        .stdin(Stdio::null())
        .output()
        .expect("inspect startup geometry");
    let quit = Command::new(binary)
        .args(["--json", "gui", "--session", session, "quit", "--discard"])
        .stdin(Stdio::null())
        .output()
        .expect("quit test-owned GUI");
    assert!(quit.status.success(), "test-owned GUI quit failed");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = launcher.try_wait().expect("poll launcher") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = launcher.kill();
            let _ = launcher.wait();
            panic!("launcher did not exit after readiness");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "launcher failed after readiness");
    assert!(inspected.status.success(), "startup inspection failed");
    let inspected: Value =
        serde_json::from_slice(&inspected.stdout).expect("startup inspection JSON");
    let geometry = &inspected["data"]["geometry"];
    assert_eq!(
        geometry["size"],
        serde_json::json!([800.0, 600.0]),
        "startup dimensions must be logical client-area points even with display scaling"
    );
    let (result, rest) = stdout_eof.expect("GUI child retained caller stdout pipe");
    result.expect("read stdout to EOF");
    assert!(rest.is_empty(), "launcher stdout has extra messages");
    stderr_eof
        .expect("GUI child retained caller stderr pipe")
        .0
        .expect("read stderr to EOF");
}
