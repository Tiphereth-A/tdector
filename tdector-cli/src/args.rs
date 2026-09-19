//! Command syntax and validation that must complete before any input is read.

use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use tdector_app::api::{FormationKind, LookupKind, Pagination, RuleSelector, SegmentSort};

use crate::error::{Failure, Result};

const INDICES: &str = "Segment, token, and rule indices are zero-based (the GUI displays segments starting at one). Rule indices refer to the loaded snapshot and may change after saving. No command opens a dialog.";

#[derive(Debug, Parser)]
#[command(name = "tdector", version, about, after_help = INDICES, propagate_version = true)]
pub struct Cli {
    /// Load this project; '-' reads UTF-8 JSON from stdin
    #[arg(short, long, global = true, value_name = "FILE")]
    pub project: Option<PathBuf>,
    /// Emit a versioned JSON report (incompatible with artifacts on stdout)
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show project counts and supported saved format
    Info,
    /// Parse, migrate, validate references, and reconstruct derived words without saving
    Validate,
    /// Create a project or explicitly replace its text, retaining vocabulary and rules
    Import {
        input: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        replace_text: bool,
        #[command(flatten)]
        tokenizer: TokenizerOptions,
        #[command(flatten)]
        save: SaveOptions,
    },
    #[command(subcommand, after_help = INDICES)]
    Segment(SegmentCommand),
    #[command(subcommand)]
    Vocab(VocabCommand),
    #[command(subcommand, about = "Read or edit segment comments or shared word comments", after_help = INDICES)]
    Comment(CommentCommand),
    /// Exact surface lookup; headword means the first token of a segment
    Lookup {
        word: String,
        #[arg(long, value_enum, default_value = "usage")]
        kind: LookupArg,
        #[command(flatten)]
        page: PageOptions,
    },
    #[command(subcommand)]
    Similar(SimilarCommand),
    #[command(subcommand, after_help = INDICES)]
    Rule(RuleCommand),
    #[command(subcommand, after_help = INDICES)]
    Formation(FormationCommand),
    #[command(subcommand)]
    Tokenize(TokenizeCommand),
    /// Export saved JSON or Typst; output defaults to stdout
    Export {
        #[arg(value_enum)]
        format: ExportFormat,
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    /// Apply a version-1 JSON command list and save only after every command succeeds
    Batch {
        file: PathBuf,
        #[command(flatten)]
        save: SaveOptions,
    },
}

#[derive(Debug, Subcommand)]
pub enum SegmentCommand {
    List {
        #[arg(long, default_value = "")]
        filter: String,
        #[arg(long, value_enum, default_value = "index")]
        sort: SortArg,
        #[arg(long)]
        descending: bool,
        #[command(flatten)]
        page: PageOptions,
    },
    /// Show a segment and annotated tokens, with zero-based indices
    Show { index: usize },
    Translate {
        index: usize,
        #[command(flatten)]
        text: TextOptions,
        #[command(flatten)]
        save: SaveOptions,
    },
}

#[derive(Debug, Subcommand)]
pub enum VocabCommand {
    List {
        #[command(flatten)]
        page: PageOptions,
    },
    /// Read an exact base vocabulary entry and its own comment
    Get { word: String },
    Set {
        word: String,
        #[command(flatten)]
        text: TextOptions,
        #[command(flatten)]
        save: SaveOptions,
    },
    Search {
        text: String,
        #[arg(long, default_value_t = 20, value_parser = positive)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    /// Distinguish the editable comment from inherited annotation text
    Get {
        #[command(flatten)]
        target: CommentTarget,
    },
    /// Token coordinates edit a shared base/derived word comment, affecting all uses
    Set {
        #[command(flatten)]
        target: CommentTarget,
        #[command(flatten)]
        text: TextOptions,
        #[command(flatten)]
        save: SaveOptions,
    },
}

#[derive(Debug, Subcommand)]
pub enum SimilarCommand {
    Segments {
        index: usize,
        #[arg(long, default_value_t = 20, value_parser = positive)]
        limit: usize,
    },
    Tokens {
        word: String,
        #[arg(long, default_value_t = 20, value_parser = token_limit)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
pub enum RuleCommand {
    List {
        #[command(flatten)]
        page: PageOptions,
    },
    Show {
        #[command(flatten)]
        selector: Selector,
    },
    /// Compile and register transform(word); the receipt has no persistent rule ID
    Add {
        #[arg(long)]
        description: String,
        #[arg(long = "type", value_enum)]
        kind: FormationArg,
        #[arg(long, value_name = "FILE")]
        script_file: PathBuf,
        #[command(flatten)]
        save: SaveOptions,
    },
    /// Evaluate a stored rule or a standalone script without applying it
    Preview {
        #[command(flatten)]
        selector: Selector,
        #[arg(long, value_name = "FILE", conflicts_with_all = ["rule", "rule_index"])]
        script_file: Option<PathBuf>,
        #[arg(long)]
        word: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum FormationCommand {
    /// Associate all existing occurrences of WORD with a validated derivation
    Apply {
        #[command(flatten)]
        selector: Selector,
        #[arg(long)]
        word: String,
        #[arg(long)]
        base: String,
        #[command(flatten)]
        save: SaveOptions,
    },
    Chain {
        #[command(flatten)]
        target: TokenTarget,
    },
    /// Pop the final step on ALL occurrences sharing this token's surface, base, and chain
    Pop {
        #[command(flatten)]
        target: TokenTarget,
        #[command(flatten)]
        save: SaveOptions,
    },
}

#[derive(Debug, Subcommand)]
pub enum TokenizeCommand {
    /// Tokenize exactly one line; this does not preview the full document import pipeline
    Preview {
        #[arg(long)]
        line: String,
        #[command(flatten)]
        tokenizer: TokenizerOptions,
    },
}

#[derive(Debug, Args)]
#[group(multiple = true)]
pub struct SaveOptions {
    /// Write saved-project JSON to a separate file, or '-' for raw stdout
    #[arg(short, long, value_name = "FILE", conflicts_with_all = ["in_place", "dry_run"])]
    pub output: Option<PathBuf>,
    /// Atomically replace --project; no-op edits do not rewrite it
    #[arg(long, conflicts_with = "dry_run")]
    pub in_place: bool,
    /// Execute and serialize in memory without saving
    #[arg(long)]
    pub dry_run: bool,
    /// Allow replacing an existing separate output file
    #[arg(long, requires = "output", conflicts_with_all = ["in_place", "dry_run"])]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct TextOptions {
    #[arg(long)]
    pub text: Option<String>,
    /// Read exact UTF-8 contents, preserving trailing newlines; '-' reads stdin
    #[arg(long, value_name = "FILE")]
    pub text_file: Option<PathBuf>,
    /// Set empty text; a gloss retains its vocabulary entry
    #[arg(long)]
    pub clear: bool,
}

#[derive(Debug, Args)]
#[group(multiple = true)]
pub struct PageOptions {
    #[arg(long, conflicts_with = "all")]
    pub offset: Option<usize>,
    #[arg(long, conflicts_with = "all", value_parser = positive)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub all: bool,
}

impl PageOptions {
    pub fn pagination(&self) -> Pagination {
        Pagination {
            offset: self.offset.unwrap_or(0),
            limit: if self.all {
                None
            } else {
                Some(self.limit.unwrap_or(50))
            },
        }
    }
}

#[derive(Debug, Args)]
#[group(multiple = true)]
pub struct Selector {
    /// Exact, case-sensitive description; duplicates require --rule-index
    #[arg(long, conflicts_with = "rule_index")]
    pub rule: Option<String>,
    /// Zero-based index in the currently loaded snapshot
    #[arg(long)]
    pub rule_index: Option<usize>,
}

impl Selector {
    pub fn selection(&self) -> RuleSelector {
        RuleSelector {
            rule: self.rule.clone(),
            rule_index: self.rule_index,
        }
    }
    fn require(&self) -> Result<()> {
        if self.rule.is_none() && self.rule_index.is_none() {
            return Err(Failure::syntax(
                "Specify exactly one of --rule or --rule-index",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct CommentTarget {
    #[arg(long)]
    pub segment: usize,
    #[arg(long)]
    pub token: Option<usize>,
}

#[derive(Debug, Args)]
pub struct TokenTarget {
    #[arg(long)]
    pub segment: usize,
    #[arg(long)]
    pub token: usize,
}

#[derive(Debug, Args)]
#[group(multiple = true)]
pub struct TokenizerOptions {
    /// Built-in strategy (default: whitespace)
    #[arg(long, value_enum, conflicts_with = "tokenizer_script")]
    pub tokenizer: Option<TokenizerArg>,
    /// UTF-8 Rhai source defining tokenize(line); '-' reads stdin
    #[arg(long, value_name = "FILE")]
    pub tokenizer_script: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TokenizerArg {
    Whitespace,
    Character,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    Json,
    Typst,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortArg {
    Index,
    Text,
    TokenCount,
}
impl From<SortArg> for SegmentSort {
    fn from(value: SortArg) -> Self {
        match value {
            SortArg::Index => Self::Index,
            SortArg::Text => Self::Text,
            SortArg::TokenCount => Self::TokenCount,
        }
    }
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LookupArg {
    Usage,
    Headword,
}
impl From<LookupArg> for LookupKind {
    fn from(value: LookupArg) -> Self {
        match value {
            LookupArg::Usage => Self::Usage,
            LookupArg::Headword => Self::Headword,
        }
    }
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormationArg {
    Derivation,
    Inflection,
    Nonmorphological,
}
impl From<FormationArg> for FormationKind {
    fn from(value: FormationArg) -> Self {
        match value {
            FormationArg::Derivation => Self::Derivation,
            FormationArg::Inflection => Self::Inflection,
            FormationArg::Nonmorphological => Self::Nonmorphological,
        }
    }
}

fn positive(value: &str) -> std::result::Result<usize, String> {
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| "Limit must be a positive integer".into())
}
fn token_limit(value: &str) -> std::result::Result<usize, String> {
    positive(value).and_then(|n| {
        if n <= 20 {
            Ok(n)
        } else {
            Err("Token similarity limit must be from 1 through 20".into())
        }
    })
}

pub fn is_stdin(path: &Path) -> bool {
    path == Path::new("-")
}

impl Cli {
    pub fn save_options(&self) -> Option<&SaveOptions> {
        match &self.command {
            Command::Import { save, .. }
            | Command::Batch { save, .. }
            | Command::Segment(SegmentCommand::Translate { save, .. })
            | Command::Vocab(VocabCommand::Set { save, .. })
            | Command::Comment(CommentCommand::Set { save, .. })
            | Command::Rule(RuleCommand::Add { save, .. })
            | Command::Formation(
                FormationCommand::Apply { save, .. } | FormationCommand::Pop { save, .. },
            ) => Some(save),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let needs_project = match &self.command {
            Command::Import { replace_text, .. } => {
                if self.project.is_some() != *replace_text {
                    return Err(Failure::syntax(
                        "Import with --project requires --replace-text, and --replace-text requires --project",
                    ));
                }
                false
            }
            Command::Tokenize(_) => false,
            Command::Rule(RuleCommand::Preview {
                script_file: Some(_),
                ..
            }) => false,
            _ => true,
        };
        if needs_project && self.project.is_none() {
            return Err(Failure::syntax("This command requires --project FILE|-"));
        }
        match &self.command {
            Command::Rule(RuleCommand::Show { selector })
            | Command::Rule(RuleCommand::Preview {
                selector,
                script_file: None,
                ..
            })
            | Command::Formation(FormationCommand::Apply { selector, .. }) => selector.require()?,
            _ => {}
        }
        if let Some(save) = self.save_options() {
            if usize::from(save.output.is_some())
                + usize::from(save.in_place)
                + usize::from(save.dry_run)
                != 1
            {
                return Err(Failure::syntax(
                    "Every edit requires exactly one of --output, --in-place, or --dry-run",
                ));
            }
            if save.in_place && self.project.as_deref().is_none_or(is_stdin) {
                return Err(Failure::syntax("--in-place requires a real --project path"));
            }
            if save.output.as_deref().is_some_and(is_stdin) && (self.json || save.overwrite) {
                return Err(Failure::syntax(
                    "--output - conflicts with --json and --overwrite",
                ));
            }
        }
        if let Command::Export {
            output, overwrite, ..
        } = &self.command
            && output.as_deref().is_none_or(is_stdin)
            && (self.json || *overwrite)
        {
            return Err(Failure::syntax(
                "Export to stdout conflicts with --json and --overwrite",
            ));
        }
        let mut stdin_count = usize::from(self.project.as_deref().is_some_and(is_stdin));
        let mut count = |path: Option<&Path>| {
            stdin_count += usize::from(path.is_some_and(is_stdin));
        };
        match &self.command {
            Command::Import {
                input, tokenizer, ..
            } => {
                count(Some(input));
                count(tokenizer.tokenizer_script.as_deref());
            }
            Command::Batch { file, .. } => count(Some(file)),
            Command::Rule(RuleCommand::Add { script_file, .. }) => count(Some(script_file)),
            Command::Rule(RuleCommand::Preview { script_file, .. }) => {
                count(script_file.as_deref())
            }
            Command::Tokenize(TokenizeCommand::Preview { tokenizer, .. }) => {
                count(tokenizer.tokenizer_script.as_deref())
            }
            Command::Segment(SegmentCommand::Translate { text, .. })
            | Command::Vocab(VocabCommand::Set { text, .. })
            | Command::Comment(CommentCommand::Set { text, .. }) => {
                count(text.text_file.as_deref())
            }
            _ => {}
        }
        if stdin_count > 1 {
            return Err(Failure::syntax("Only one input source may consume stdin"));
        }
        Ok(())
    }
}
