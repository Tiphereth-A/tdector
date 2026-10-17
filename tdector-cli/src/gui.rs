//! Optional shell and interactive clients for CLI-owned native GUI hosts.

#[cfg(windows)]
#[path = "gui_windows.rs"]
mod windows;
use crate::{
    args::{Cli, Command},
    error::{Failure, Result},
    report,
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};
use tdector_gui::control::protocol::{ControlError, DEFAULT_TIMEOUT_MS, Response, StartupConfig};
use tdector_gui::control::transport::{self, read_message, write_message};

#[derive(Debug, Args)]
pub struct GuiArgs {
    /// Capability returned by gui start; defaults to TDECTOR_GUI_SESSION
    #[arg(long, global = true)]
    pub session: Option<String>,
    /// Startup or command deadline in seconds (default: 30; maximum: 86400)
    #[arg(long, global = true, value_parser = timeout_seconds)]
    pub timeout: Option<f64>,
    #[command(subcommand)]
    pub command: GuiCommand,
}
#[derive(Debug, Subcommand)]
pub enum GuiCommand {
    /// Launch a new GUI host using this executable
    Start {
        #[arg(long)]
        font: Option<PathBuf>,
        #[arg(long, value_parser = positive_points)]
        width: Option<f32>,
        #[arg(long, value_parser = positive_points)]
        height: Option<f32>,
    },
    /// Interactive commands; single quotes preserve Windows path backslashes
    #[command(
        after_help = "Uses shlex quoting: spaces split words; single quotes preserve all characters; double quotes allow backslash escapes. No shell expansion or command execution. exit or EOF detaches; quit closes the GUI. --value-file - is unavailable in the prompt."
    )]
    Repl,
    Status,
    Windows,
    Open {
        #[arg(value_enum)]
        kind: OpenKind,
        #[arg(long)]
        word: Option<String>,
        #[arg(long)]
        segment: Option<usize>,
        #[arg(long)]
        token: Option<usize>,
        #[arg(long)]
        pinned: bool,
    },
    Inspect {
        handle: String,
    },
    Move {
        handle: String,
        #[arg(long, allow_hyphen_values = true, value_parser = finite_points)]
        x: f32,
        #[arg(long, allow_hyphen_values = true, value_parser = finite_points)]
        y: f32,
    },
    Resize {
        handle: String,
        #[arg(long, value_parser = positive_points)]
        width: f32,
        #[arg(long, value_parser = positive_points)]
        height: f32,
    },
    Focus {
        handle: String,
        #[arg(long)]
        control: Option<String>,
    },
    Scroll {
        handle: String,
        #[arg(long)]
        area: String,
        #[arg(long, value_parser = nonnegative_points)]
        x: f32,
        #[arg(long, value_parser = nonnegative_points)]
        y: f32,
    },
    Get {
        handle: String,
        control: String,
    },
    Set {
        handle: String,
        control: String,
        #[command(flatten)]
        value: ValueOptions,
    },
    Invoke {
        handle: String,
        action: String,
    },
    Close {
        handle: String,
    },
    Load {
        file: PathBuf,
    },
    Import {
        file: PathBuf,
    },
    Font {
        file: PathBuf,
    },
    Save {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    Export {
        #[arg(value_enum)]
        format: GuiExportFormat,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Capture the rendered main viewport to a PNG file
    Screenshot {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    /// Wait until earlier changes are observed in a rendered GUI frame
    Wait,
    Quit {
        #[arg(long)]
        discard: bool,
    },
}
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct ValueOptions {
    /// JSON scalar/object/array, or plain text when it is not valid JSON
    #[arg(long)]
    value: Option<String>,
    /// Read exact UTF-8 text, preserving newlines; '-' reads shell stdin
    #[arg(long, value_name = "FILE")]
    value_file: Option<PathBuf>,
    #[arg(long)]
    clear: bool,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OpenKind {
    Definition,
    References,
    SimilarSegments,
    SimilarTokens,
    WordMenu,
    SegmentMenu,
    WordFormation,
    FormationChain,
    RemoveFormationRule,
    NewFormationRule,
    WordComment,
    SegmentComment,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GuiExportFormat {
    Typst,
}
fn finite_points(text: &str) -> std::result::Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|x| x.is_finite())
        .ok_or_else(|| "Coordinate must be a finite number".into())
}
fn positive_points(text: &str) -> std::result::Result<f32, String> {
    finite_points(text).and_then(|x| {
        if x > 0.0 {
            Ok(x)
        } else {
            Err("Size must be positive".into())
        }
    })
}
fn nonnegative_points(text: &str) -> std::result::Result<f32, String> {
    finite_points(text).and_then(|x| {
        if x >= 0.0 {
            Ok(x)
        } else {
            Err("Scroll offset must not be negative".into())
        }
    })
}
fn timeout_seconds(text: &str) -> std::result::Result<f64, String> {
    text.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && *x >= 0.001 && *x <= 86_400.0)
        .ok_or_else(|| "Timeout must be between 0.001 and 86400 seconds".into())
}
fn duration(options: &GuiArgs) -> Duration {
    options
        .timeout
        .map(Duration::from_secs_f64)
        .unwrap_or(Duration::from_millis(DEFAULT_TIMEOUT_MS))
}
fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path == Path::new("-") {
        return Err(Failure::syntax(
            "GUI file operations require an explicit path; '-' is only accepted by set --value-file",
        ));
    }
    std::path::absolute(path).map_err(|error| Failure::new("io_error", error.to_string(), 5))
}
fn resolve_session(options: &GuiArgs) -> Result<String> {
    options
        .session
        .clone()
        .or_else(|| std::env::var("TDECTOR_GUI_SESSION").ok())
        .ok_or_else(|| {
            Failure::new(
                "missing_session",
                "Specify --session or set TDECTOR_GUI_SESSION from gui start",
                7,
            )
        })
}
pub fn run(cli: &Cli, options: &GuiArgs) -> Result<Option<Value>> {
    match &options.command {
        GuiCommand::Start {
            font,
            width,
            height,
        } => {
            if options.session.is_some() {
                return Err(Failure::syntax(
                    "gui start creates a new session; --session cannot be supplied",
                ));
            }
            let config = StartupConfig {
                project: cli.project.as_deref().map(absolute_path).transpose()?,
                font: font.as_deref().map(absolute_path).transpose()?,
                width: *width,
                height: *height,
            };
            start(config, duration(options)).map(Some)
        }
        GuiCommand::Repl => {
            if cli.project.is_some() {
                return Err(Failure::syntax(
                    "Use gui load to replace a running session's project",
                ));
            }
            repl(resolve_session(options)?, options.timeout, cli.json)?;
            Ok(None)
        }
        command => {
            if cli.project.is_some() {
                return Err(Failure::syntax(
                    "--project is only valid with gui start; use gui load for a running session",
                ));
            }
            let (name, args) = request(command, false)?;
            transport::send(
                &resolve_session(options)?,
                name.into(),
                args,
                duration(options),
            )
            .map(Some)
            .map_err(Failure::from)
        }
    }
}
fn request(command: &GuiCommand, in_repl: bool) -> Result<(&'static str, Value)> {
    Ok(match command {
        GuiCommand::Status => ("status", json!({})),
        GuiCommand::Windows => ("windows", json!({})),
        GuiCommand::Open {
            kind,
            word,
            segment,
            token,
            pinned,
        } => (
            "open",
            json!({"kind":kind.to_possible_value().expect("enum name").get_name(),"word":word,"segment":segment,"token":token,"pinned":pinned}),
        ),
        GuiCommand::Inspect { handle } => ("inspect", json!({"handle":handle})),
        GuiCommand::Move { handle, x, y } => ("move", json!({"handle":handle,"x":x,"y":y})),
        GuiCommand::Resize {
            handle,
            width,
            height,
        } => (
            "resize",
            json!({"handle":handle,"width":width,"height":height}),
        ),
        GuiCommand::Focus { handle, control } => {
            ("focus", json!({"handle":handle,"control":control}))
        }
        GuiCommand::Scroll { handle, area, x, y } => {
            ("scroll", json!({"handle":handle,"area":area,"x":x,"y":y}))
        }
        GuiCommand::Get { handle, control } => ("get", json!({"handle":handle,"control":control})),
        GuiCommand::Set {
            handle,
            control,
            value,
        } => {
            let value = if let Some(text) = &value.value {
                serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()))
            } else if let Some(path) = &value.value_file {
                if in_repl && path == Path::new("-") {
                    return Err(Failure::syntax(
                        "--value-file - cannot consume REPL command input",
                    ));
                }
                Value::String(crate::read_string(path)?)
            } else {
                Value::String(String::new())
            };
            (
                "set",
                json!({"handle":handle,"control":control,"value":value}),
            )
        }
        GuiCommand::Invoke { handle, action } => {
            ("invoke", json!({"handle":handle,"action":action}))
        }
        GuiCommand::Close { handle } => ("close", json!({"handle":handle})),
        GuiCommand::Load { file } => ("load", json!({"path":absolute_path(file)?})),
        GuiCommand::Import { file } => ("import", json!({"path":absolute_path(file)?})),
        GuiCommand::Font { file } => ("font", json!({"path":absolute_path(file)?})),
        GuiCommand::Save { output, overwrite } => (
            "save",
            json!({"output":output.as_deref().map(absolute_path).transpose()?,"overwrite":overwrite}),
        ),
        GuiCommand::Export {
            format: GuiExportFormat::Typst,
            output,
            overwrite,
        } => (
            "export",
            json!({"format":"typst","output":absolute_path(output)?,"overwrite":overwrite}),
        ),
        GuiCommand::Screenshot { output, overwrite } => (
            "screenshot",
            json!({"output":absolute_path(output)?,"overwrite":overwrite}),
        ),
        GuiCommand::Wait => ("wait", json!({})),
        GuiCommand::Quit { discard } => ("quit", json!({"discard":discard})),
        GuiCommand::Start { .. } | GuiCommand::Repl => {
            return Err(Failure::syntax(
                "start and repl cannot be nested inside the interactive prompt",
            ));
        }
    })
}
fn start(config: StartupConfig, timeout: Duration) -> Result<Value> {
    let deadline = Instant::now() + timeout;
    let executable = std::env::current_exe()
        .map_err(|error| Failure::new("startup_failed", error.to_string(), 7))?;
    let mut command = ProcessCommand::new(executable);
    // Do not retain a caller's redirected output pipes after the launcher exits.
    // Startup failures are returned on the dedicated readiness pipe.
    command
        .arg("__gui-host")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: the child creates its own GUI window.
    }
    #[cfg(windows)]
    let spawned = windows::spawn(&mut command);
    #[cfg(not(windows))]
    let spawned = command.spawn();
    let mut child =
        spawned.map_err(|error| Failure::new("startup_failed", error.to_string(), 7))?;
    let result = (|| -> Result<Value> {
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| Failure::internal("Missing host startup pipe"))?;
        write_message(&mut input, &config)?;
        drop(input);
        let output = child
            .stdout
            .take()
            .ok_or_else(|| Failure::internal("Missing host readiness pipe"))?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(read_message::<Response>(output));
        });
        let response = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| {
                Failure::new(
                    "startup_timeout",
                    "GUI did not render its first frame before the startup deadline",
                    7,
                )
            })??;
        if response.protocol_version != tdector_gui::control::protocol::PROTOCOL_VERSION {
            return Err(Failure::new(
                "protocol_mismatch",
                "GUI host startup protocol mismatch",
                7,
            ));
        }
        response.result.map_err(Failure::from)
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
/// Private parent/child pipe; configuration and credentials never appear in process arguments.
pub fn host() -> Result<Option<Value>> {
    let config = match read_message::<StartupConfig>(std::io::stdin().lock()) {
        Ok(config) => config,
        Err(error) => {
            write_message(&mut std::io::stdout().lock(), &Response::new(Err(error)))?;
            return Ok(None);
        }
    };
    // Readiness is the only stdout message. Diagnostics use stderr.
    let result = tdector_gui::native::run_controlled(config, |result| {
        let _ = write_message(&mut std::io::stdout().lock(), &Response::new(result));
    });
    if let Err(error) = result {
        eprintln!("GUI host: {}", error.message);
    }
    Ok(None)
}
fn repl(session: String, timeout: Option<f64>, json_mode: bool) -> Result<()> {
    // Validate the locator before claiming the terminal.
    transport::SessionLocator::parse(&session)?;
    let interactive = std::io::stdin().is_terminal();
    let input = std::io::stdin();
    let mut lines = input.lock().lines();
    loop {
        if interactive {
            eprint!("tdector gui> ");
            let _ = std::io::stderr().flush();
        }
        let Some(line) = lines.next() else {
            break;
        };
        let line = line.map_err(|error| Failure::new("io_error", error.to_string(), 5))?;
        let words = match shlex::split(&line) {
            Some(words) => words,
            None => {
                report::failure(
                    Failure::syntax("Unclosed quote or dangling backslash in interactive command"),
                    json_mode,
                );
                continue;
            }
        };
        if words.is_empty() {
            continue;
        }
        if words.as_slice() == ["exit"] {
            break;
        }
        let parsed = Cli::try_parse_from(
            ["tdector", "gui"]
                .into_iter()
                .map(str::to_owned)
                .chain(words),
        );
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error)
                if matches!(
                    error.kind(),
                    clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
                ) =>
            {
                let _ = error.print();
                continue;
            }
            Err(error) => {
                report::failure(Failure::syntax(error.to_string()), json_mode);
                continue;
            }
        };
        let Command::Gui(mut options) = parsed.command else {
            continue;
        };
        let output_json = json_mode || parsed.json;
        if options.timeout.is_none() {
            options.timeout = timeout;
        }
        let active_session = options.session.as_deref().unwrap_or(&session);
        let result = if parsed.project.is_some() {
            Err(Failure::syntax(
                "Use load FILE inside the interactive prompt",
            ))
        } else {
            request(&options.command, true).and_then(|(name, args)| {
                transport::send(active_session, name.into(), args, duration(&options))
                    .map_err(Failure::from)
            })
        };
        match result {
            Ok(data) => {
                let closed = data.get("closed").and_then(Value::as_bool) == Some(true)
                    && data.get("main").and_then(Value::as_bool) == Some(true);
                report::success(data, output_json)?;
                if closed {
                    break;
                }
            }
            Err(error) => {
                report::failure(error, output_json);
            }
        }
    }
    Ok(())
}
impl From<ControlError> for Failure {
    fn from(error: ControlError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            details: error.details,
            exit_code: error.exit_code,
            stage: None,
            command_index: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    #[test]
    fn gui_commands_share_parser_and_preserve_quoted_unicode() {
        Cli::command().debug_assert();
        let words =
            shlex::split("set w-id comment --value '猫\nsecond line'").expect("quoted text");
        let cli = Cli::try_parse_from(
            ["tdector", "gui"]
                .into_iter()
                .map(str::to_owned)
                .chain(words),
        )
        .expect("parse");
        let Command::Gui(options) = cli.command else {
            panic!("gui command");
        };
        let (_, args) = request(&options.command, true).expect("request");
        assert_eq!(args["value"], "猫\nsecond line");
        assert_eq!(
            shlex::split("font 'C:\\fonts\\sample.ttf'").expect("path")[1],
            "C:\\fonts\\sample.ttf"
        );
    }
    #[test]
    fn stdin_in_prompt_and_invalid_coordinates_are_rejected() {
        let cli = Cli::try_parse_from([
            "tdector",
            "gui",
            "set",
            "w-id",
            "comment",
            "--value-file",
            "-",
        ])
        .expect("parse");
        let Command::Gui(options) = cli.command else {
            panic!("gui command");
        };
        assert_eq!(
            request(&options.command, true)
                .expect_err("stdin reserved")
                .exit_code,
            2
        );
        assert!(
            Cli::try_parse_from(["tdector", "gui", "move", "w-id", "--x", "NaN", "--y", "0"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["tdector", "gui", "status", "--timeout", "0"]).is_err());
        let cli = Cli::try_parse_from([
            "tdector",
            "gui",
            "start",
            "--project",
            "sample/ginger.json",
            "--json",
        ])
        .expect("global options");
        assert!(cli.project.is_some() && cli.json);
    }
}
