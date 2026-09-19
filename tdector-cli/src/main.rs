//! Noninteractive, single-session CLI adapter for tdector.

mod args;
mod error;
mod io;
mod report;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use serde::Serialize;
use serde_json::{Value, json};
use tdector_app::Session;
use tdector_app::api::{self, BatchStage, Mutation, Query};
use tdector_eval::TokenizationRule;

use args::*;
use error::{Failure, Result};

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().collect();
    let json_requested = recognizes_json(&arguments);
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            return match error.print() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Cannot write help: {error}");
                    ExitCode::from(5)
                }
            };
        }
        Err(error) => return report::failure(Failure::syntax(error.to_string()), json_requested),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&cli)))
        .unwrap_or_else(|_| Err(Failure::internal("Unexpected internal failure")));
    match result {
        Ok(Some(data)) => match report::success(data, cli.json) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{}", error.message);
                ExitCode::from(error.exit_code)
            }
        },
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => report::failure(error, cli.json),
    }
}

/// Recognize the global flag even when parsing fails, without mistaking a value
/// or a positional argument following `--` for an option.
fn recognizes_json(arguments: &[OsString]) -> bool {
    let takes_value = [
        "-p",
        "--project",
        "-o",
        "--output",
        "--name",
        "--tokenizer",
        "--tokenizer-script",
        "--text",
        "--text-file",
        "--filter",
        "--sort",
        "--offset",
        "--limit",
        "--segment",
        "--token",
        "--kind",
        "--rule",
        "--rule-index",
        "--description",
        "--type",
        "--script-file",
        "--word",
        "--base",
        "--line",
    ];
    let mut skip = false;
    for argument in arguments.iter().skip(1) {
        if skip {
            skip = false;
            // Clap does not accept leading-hyphen values unless attached with
            // '='. A missing value must not hide a later recognized flag.
            if !argument.to_string_lossy().starts_with('-') || argument == "-" {
                continue;
            }
        }
        if argument == "--" {
            break;
        }
        if argument == "--json" {
            return true;
        }
        skip = takes_value.iter().any(|option| argument == option);
    }
    false
}

fn run(cli: &Cli) -> Result<Option<Value>> {
    let is_batch = matches!(cli.command, Command::Batch { .. });
    cli.validate().map_err(|error| {
        if is_batch {
            error.at_stage(BatchStage::Input)
        } else {
            error
        }
    })?;
    let input_result = (|| {
        validate_paths(cli)?;
        let mut session = Session::default();
        let source = if let Some(path) = &cli.project {
            let bytes = io::read_input(path)?;
            session.load_json(io::decode_utf8(&bytes)?)?;
            Some(bytes)
        } else {
            None
        };
        Ok((session, source))
    })();
    let (mut session, source) = input_result.map_err(|error: Failure| {
        if is_batch {
            error.at_stage(BatchStage::Input)
        } else {
            error
        }
    })?;

    let data = match &cli.command {
        Command::Info => query(&mut session, Query::Info)?,
        Command::Validate => query(&mut session, Query::Validate)?,
        Command::Segment(command) => match command {
            SegmentCommand::List {
                filter,
                sort,
                descending,
                page,
            } => query(
                &mut session,
                Query::SegmentList {
                    filter: filter.clone(),
                    sort: (*sort).into(),
                    descending: *descending,
                    pagination: page.pagination(),
                },
            )?,
            SegmentCommand::Show { index } => query(
                &mut session,
                Query::SegmentShow {
                    segment_index: *index,
                },
            )?,
            SegmentCommand::Translate { index, text, .. } => mutate(
                &mut session,
                Mutation::SetTranslation {
                    segment_index: *index,
                    translation: read_text(text)?,
                },
            )?,
        },
        Command::Vocab(command) => match command {
            VocabCommand::List { page } => query(
                &mut session,
                Query::VocabList {
                    pagination: page.pagination(),
                },
            )?,
            VocabCommand::Get { word } => {
                query(&mut session, Query::VocabGet { word: word.clone() })?
            }
            VocabCommand::Search { text, limit } => query(
                &mut session,
                Query::VocabSearch {
                    text: text.clone(),
                    limit: *limit,
                },
            )?,
            VocabCommand::Set { word, text, .. } => mutate(
                &mut session,
                Mutation::SetGloss {
                    word: word.clone(),
                    meaning: read_text(text)?,
                },
            )?,
        },
        Command::Comment(command) => match command {
            CommentCommand::Get { target } => query(
                &mut session,
                Query::CommentGet {
                    segment_index: target.segment,
                    token_index: target.token,
                },
            )?,
            CommentCommand::Set { target, text, .. } => mutate(
                &mut session,
                Mutation::SetComment {
                    segment_index: target.segment,
                    token_index: target.token,
                    comment: read_text(text)?,
                },
            )?,
        },
        Command::Lookup { word, kind, page } => query(
            &mut session,
            Query::Lookup {
                word: word.clone(),
                kind: (*kind).into(),
                pagination: page.pagination(),
            },
        )?,
        Command::Similar(command) => match command {
            SimilarCommand::Segments { index, limit } => query(
                &mut session,
                Query::SimilarSegments {
                    segment_index: *index,
                    limit: *limit,
                },
            )?,
            SimilarCommand::Tokens { word, limit } => query(
                &mut session,
                Query::SimilarTokens {
                    word: word.clone(),
                    limit: *limit,
                },
            )?,
        },
        Command::Rule(command) => match command {
            RuleCommand::List { page } => query(
                &mut session,
                Query::RuleList {
                    pagination: page.pagination(),
                },
            )?,
            RuleCommand::Show { selector } => query(
                &mut session,
                Query::RuleShow {
                    selector: selector.selection(),
                },
            )?,
            RuleCommand::Add {
                description,
                kind,
                script_file,
                ..
            } => mutate(
                &mut session,
                Mutation::CreateRule {
                    description: description.clone(),
                    rule_type: (*kind).into(),
                    script: read_string(script_file)?,
                },
            )?,
            RuleCommand::Preview {
                selector,
                script_file,
                word,
            } => value(if let Some(file) = script_file {
                api::preview_script(&read_string(file)?, word)?
            } else {
                api::preview_rule(&session, selector.selection(), word)?
            })?,
        },
        Command::Formation(command) => match command {
            FormationCommand::Apply {
                selector,
                word,
                base,
                ..
            } => mutate(
                &mut session,
                Mutation::ApplyFormation {
                    word: word.clone(),
                    base: base.clone(),
                    rule: selector.rule.clone(),
                    rule_index: selector.rule_index,
                },
            )?,
            FormationCommand::Chain { target } => query(
                &mut session,
                Query::FormationChain {
                    segment_index: target.segment,
                    token_index: target.token,
                },
            )?,
            FormationCommand::Pop { target, .. } => mutate(
                &mut session,
                Mutation::PopFormation {
                    segment_index: target.segment,
                    token_index: target.token,
                },
            )?,
        },
        Command::Tokenize(TokenizeCommand::Preview { line, tokenizer }) => {
            value(api::preview_tokenization(&tokenization(tokenizer)?, line)?)?
        }
        Command::Import {
            input,
            name,
            tokenizer,
            ..
        } => {
            let name = name.clone().unwrap_or_else(|| {
                if cli.project.is_some() {
                    session.project().project_name.clone()
                } else if is_stdin(input) {
                    "Untitled".into()
                } else {
                    input
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Untitled".into())
                }
            });
            session.import_text(&read_string(input)?, &name, &tokenization(tokenizer)?)?;
            json!({"changed": true, "project_name": name, "segment_count": session.project().segments.len()})
        }
        Command::Batch { file, .. } => {
            let input = read_string(file).map_err(|e| e.at_stage(BatchStage::Input))?;
            let request = api::parse_batch(&input)?;
            value(api::execute_batch(&mut session, request)?)?
        }
        Command::Export {
            format,
            output,
            overwrite,
        } => {
            let bytes = match format {
                ExportFormat::Json => session.save_snapshot()?.bytes,
                ExportFormat::Typst => session.export_typst().into_bytes(),
            };
            let output = output.as_deref().unwrap_or_else(|| Path::new("-"));
            if is_stdin(output) {
                io::write_stdout(&bytes)?;
                return Ok(None);
            }
            io::atomic_write(output, &bytes, *overwrite, None)?;
            return Ok(Some(
                json!({"exported": true, "output": output, "format": match format { ExportFormat::Json => "json", ExportFormat::Typst => "typst" }}),
            ));
        }
    };
    if let Some(save) = cli.save_options() {
        save_project(cli, save, &mut session, source.as_deref(), data)
    } else {
        Ok(Some(data))
    }
}

fn value(value: impl Serialize) -> Result<Value> {
    Ok(serde_json::to_value(value)?)
}
fn query(session: &mut Session, request: Query) -> Result<Value> {
    value(api::query(session, request)?)
}
fn mutate(session: &mut Session, request: Mutation) -> Result<Value> {
    value(api::apply_mutation(session, request)?)
}
fn read_string(path: &Path) -> Result<String> {
    Ok(io::decode_utf8(&io::read_input(path)?)?.to_owned())
}
fn read_text(text: &TextOptions) -> Result<String> {
    if let Some(value) = &text.text {
        Ok(value.clone())
    } else if let Some(file) = &text.text_file {
        read_string(file)
    } else {
        Ok(String::new())
    }
}
fn tokenization(options: &TokenizerOptions) -> Result<TokenizationRule> {
    if let Some(path) = &options.tokenizer_script {
        Ok(TokenizationRule {
            description: "CLI tokenizer".into(),
            command: read_string(path)?,
            cached_ast: tdector_eval::default_cached_ast(),
        })
    } else {
        Ok(
            match options.tokenizer.unwrap_or(TokenizerArg::Whitespace) {
                TokenizerArg::Whitespace => TokenizationRule::default_whitespace(),
                TokenizerArg::Character => TokenizationRule::default_character(),
            },
        )
    }
}

fn validate_paths(cli: &Cli) -> Result<()> {
    let output = if let Some(save) = cli.save_options() {
        save.output.as_deref()
    } else if let Command::Export { output, .. } = &cli.command {
        output.as_deref()
    } else {
        None
    };
    if let Some(output) = output.filter(|path| !is_stdin(path)) {
        // A fresh import's text file is also an input and must never be overwritten.
        let input = cli.project.as_deref().or(match &cli.command {
            Command::Import { input, .. } => Some(input.as_path()),
            _ => None,
        });
        if let Some(input) = input.filter(|path| !is_stdin(path))
            && io::same_file(input, output)?
        {
            return Err(Failure::syntax(
                "Output refers to the input file; project edits must use --in-place and exports require a different destination",
            ));
        }
    }
    Ok(())
}

fn save_project(
    cli: &Cli,
    save: &SaveOptions,
    session: &mut Session,
    source: Option<&[u8]>,
    mut data: Value,
) -> Result<Option<Value>> {
    let batch = matches!(cli.command, Command::Batch { .. });
    let snapshot = session.save_snapshot().map_err(|e| {
        let error = Failure::from(e);
        if batch {
            error.at_stage(BatchStage::Serialize)
        } else {
            error
        }
    })?;
    let changed = data
        .get("changed")
        .and_then(Value::as_bool)
        .unwrap_or(session.is_dirty());
    let output: Option<PathBuf> = if save.in_place {
        cli.project.clone()
    } else {
        save.output.clone()
    };
    let result: Result<bool> = (|| {
        if save.dry_run {
            return Ok(false);
        }
        let path = output
            .as_deref()
            .ok_or_else(|| Failure::internal("Missing validated save destination"))?;
        if save.in_place && !changed {
            return Ok(false);
        }
        if is_stdin(path) {
            io::write_stdout(&snapshot.bytes)?;
        } else {
            let expected = if save.in_place {
                Some((
                    path,
                    source.ok_or_else(|| Failure::internal("Missing original project bytes"))?,
                ))
            } else {
                None
            };
            io::atomic_write(
                path,
                &snapshot.bytes,
                save.in_place || save.overwrite,
                expected,
            )?;
        }
        if !session.acknowledge_saved(snapshot.token) {
            return Err(Failure::internal("Save acknowledgement rejected"));
        }
        Ok(true)
    })();
    let saved = result.map_err(|e| {
        if batch {
            e.at_stage(BatchStage::Commit)
        } else {
            e
        }
    })?;
    if output.as_deref().is_some_and(is_stdin) {
        return Ok(None);
    }
    let receipt = data
        .as_object_mut()
        .ok_or_else(|| Failure::internal("Mutation receipt is not an object"))?;
    receipt.insert("changed".into(), json!(changed));
    receipt.insert("saved".into(), json!(saved));
    receipt.insert("output".into(), json!(output));
    if save.dry_run {
        receipt.insert("dry_run".into(), json!(true));
    }
    Ok(Some(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_structure_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn json_flag_recognition_respects_values_and_terminator() {
        for (args, expected) in [
            (vec!["tdector", "--json", "unknown"], true),
            (vec!["tdector", "vocab", "get", "--", "--json"], false),
            (vec!["tdector", "--project", "--json", "info"], true),
            (vec!["tdector", "--project=--json", "info"], false),
            (vec!["tdector", "--project", "--", "--json", "info"], false),
            (
                vec!["tdector", "vocab", "set", "cat", "--text=--json"],
                false,
            ),
        ] {
            assert_eq!(
                recognizes_json(&args.into_iter().map(OsString::from).collect::<Vec<_>>()),
                expected
            );
        }
    }
}
