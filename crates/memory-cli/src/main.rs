use std::{
    collections::{HashMap, HashSet},
    env, fs,
    fs::{File, OpenOptions},
    io::{self, BufRead, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    str::FromStr,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use memory_core::{
    check_ignored_path, collect_importable_files, evaluate, import_path, parse_file, Embedder,
    EvalCase, FastEmbedOnnxEmbedder, HashEmbedder, ImportFormat, ImportOptions, MapOutputFormat,
    MapRequest, MapType, MemoryEdit, MemoryEngine, MemoryKind, MemoryLayer, MemoryPermission,
    MemorySource, MemoryStatus, NewMemory, OllamaEmbedder, OpenAiCompatibleEmbedder,
    PersonaProfile, PolicyMode, RecallQuery, SharedEmbedder, DEFAULT_MEMORYIGNORE,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[derive(Debug, Parser)]
#[command(name = "memory")]
#[command(about = "SQLite for AI memory. One file. Local. Fast. Private.")]
struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    db: Option<PathBuf>,

    #[arg(long, global = true, value_enum, default_value_t = EmbedderChoice::Hash)]
    embedder: EmbedderChoice,

    #[arg(long, global = true)]
    endpoint: Option<String>,

    #[arg(long, global = true)]
    model: Option<String>,

    #[arg(long, global = true, default_value_t = 384)]
    dimensions: usize,

    #[arg(long, global = true, default_value = "MEMORY_CPP_OPENAI_API_KEY")]
    api_key_env: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, ValueEnum)]
enum EmbedderChoice {
    Hash,
    Ollama,
    Openai,
    Fastembed,
    Onnx,
}

impl EmbedderChoice {
    fn provider_name(&self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::Ollama => "ollama",
            Self::Openai => "openai",
            Self::Fastembed | Self::Onnx => "fastembed",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum SearchProfile {
    Dev,
    Error,
    Decision,
    Code,
    Docs,
    Test,
    Terminal,
    Git,
    Ci,
}

#[derive(Debug, Clone, ValueEnum)]
enum DevContextTarget {
    Cursor,
    Codex,
    Claude,
    Vscode,
    Continue,
    Aider,
    Copilot,
    Ollama,
    Openai,
    Generic,
    SmallModel,
    LargeModel,
}

#[derive(Debug, Clone, ValueEnum)]
enum DevOnboardOutput {
    Markdown,
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliImportFormat {
    Auto,
    Text,
    Markdown,
    Json,
    Jsonl,
}

impl From<CliImportFormat> for ImportFormat {
    fn from(value: CliImportFormat) -> Self {
        match value {
            CliImportFormat::Auto => Self::Auto,
            CliImportFormat::Text => Self::Text,
            CliImportFormat::Markdown => Self::Markdown,
            CliImportFormat::Json => Self::Json,
            CliImportFormat::Jsonl => Self::Jsonl,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum ExportFormat {
    Jsonl,
    Markdown,
    Graphml,
    Sqlite,
}

#[derive(Debug, Clone, ValueEnum)]
enum AttachTarget {
    Cursor,
    Claude,
    Codex,
    Continue,
    Ollama,
    Vscode,
    All,
}

#[derive(Debug, Clone, ValueEnum)]
enum CompileTarget {
    Cursor,
    Claude,
    Codex,
    Ollama,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliMapType {
    Evolution,
    Timeline,
    Decisions,
    Architecture,
    Bugs,
    Dependencies,
}

impl From<CliMapType> for MapType {
    fn from(value: CliMapType) -> Self {
        match value {
            CliMapType::Evolution => Self::Evolution,
            CliMapType::Timeline => Self::Timeline,
            CliMapType::Decisions => Self::Decisions,
            CliMapType::Architecture => Self::Architecture,
            CliMapType::Bugs => Self::Bugs,
            CliMapType::Dependencies => Self::Dependencies,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CliMapOutput {
    Json,
    Markdown,
    Mermaid,
    Html,
}

impl From<CliMapOutput> for MapOutputFormat {
    fn from(value: CliMapOutput) -> Self {
        match value {
            CliMapOutput::Json => Self::Json,
            CliMapOutput::Markdown => Self::Markdown,
            CliMapOutput::Mermaid => Self::Mermaid,
            CliMapOutput::Html => Self::Html,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CliPolicyMode {
    Forever,
    Decay,
    Ephemeral,
    SessionOnly,
    ManualReview,
    NeverStore,
    Reject,
}

impl From<CliPolicyMode> for PolicyMode {
    fn from(value: CliPolicyMode) -> Self {
        match value {
            CliPolicyMode::Forever => Self::Forever,
            CliPolicyMode::Decay => Self::Decay,
            CliPolicyMode::Ephemeral => Self::Ephemeral,
            CliPolicyMode::SessionOnly => Self::SessionOnly,
            CliPolicyMode::ManualReview => Self::ManualReview,
            CliPolicyMode::NeverStore => Self::NeverStore,
            CliPolicyMode::Reject => Self::Reject,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        encrypted: bool,

        #[arg(long)]
        workspace: Option<String>,
    },
    #[command(alias = "add")]
    Remember {
        #[arg(required = true, num_args = 1..)]
        content: Vec<String>,

        #[arg(long, default_value = "note", value_parser = parse_kind)]
        kind: MemoryKind,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        #[arg(long)]
        metadata: Option<String>,

        #[arg(long)]
        importance: Option<f32>,

        #[arg(long)]
        confidence: Option<f32>,

        #[arg(long)]
        source: Option<String>,

        #[arg(long)]
        source_type: Option<String>,

        #[arg(long)]
        source_file: Option<String>,

        #[arg(long)]
        source_line: Option<u64>,

        #[arg(long)]
        source_commit: Option<String>,

        #[arg(long)]
        source_conversation: Option<String>,

        #[arg(long)]
        created_by: Option<String>,

        #[arg(long)]
        permission: Option<String>,

        #[arg(long)]
        layer: Option<String>,

        #[arg(long)]
        json: bool,
    },
    #[command(alias = "search")]
    Recall {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long = "kind", value_parser = parse_kind)]
        kinds: Vec<MemoryKind>,

        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        #[arg(long, value_enum)]
        profile: Option<SearchProfile>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long)]
        content: bool,

        #[arg(long)]
        include_inactive: bool,

        #[arg(long)]
        no_global: bool,

        #[arg(long)]
        json: bool,
    },
    Explain {
        #[arg(required = false, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long)]
        last: bool,

        #[arg(long)]
        json: bool,
    },
    Forget {
        id: String,

        #[arg(long, default_value = "forgotten by user")]
        reason: String,

        #[arg(long)]
        json: bool,
    },
    Patch {
        id: String,

        #[arg(required = true, num_args = 1..)]
        content: Vec<String>,

        #[arg(long, default_value = "note", value_parser = parse_kind)]
        kind: MemoryKind,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        #[arg(long)]
        confidence: Option<f32>,

        #[arg(long)]
        json: bool,
    },
    Context {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long, default_value_t = 1_200)]
        tokens: usize,
    },
    Compile {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, value_enum, default_value_t = CompileTarget::Codex)]
        target: CompileTarget,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long, default_value_t = 1_200)]
        tokens: usize,
    },
    Import {
        path: PathBuf,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value = "note", value_parser = parse_kind)]
        kind: MemoryKind,

        #[arg(long, value_enum, default_value_t = CliImportFormat::Auto)]
        format: CliImportFormat,

        #[arg(long, default_value_t = 1_800)]
        chunk_chars: usize,

        #[arg(long)]
        no_recursive: bool,

        #[arg(long)]
        preview_redactions: bool,

        #[arg(long)]
        json: bool,
    },
    Watch {
        path: PathBuf,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value = "note", value_parser = parse_kind)]
        kind: MemoryKind,

        #[arg(long, default_value_t = 10)]
        interval_secs: u64,

        #[arg(long, default_value_t = 1_800)]
        chunk_chars: usize,

        #[arg(long)]
        once: bool,
    },
    Sleep {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Timeline {
        #[arg(required = false, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 20)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Replay {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 12)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Graph {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        entity: Option<String>,

        #[arg(long, default_value_t = 50)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Eval {
        file: PathBuf,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Export {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, value_enum, default_value_t = ExportFormat::Jsonl)]
        format: ExportFormat,

        output: PathBuf,
    },
    Persona {
        #[command(subcommand)]
        command: PersonaCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    Diff {
        left: String,
        right: String,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Inbox {
        #[command(subcommand)]
        command: InboxCommand,
    },
    Attach {
        target: AttachTarget,

        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value_t = 7331)]
        port: u16,

        #[arg(long, default_value = "http://localhost:11434")]
        upstream: String,

        #[arg(long)]
        start_proxy: bool,

        #[arg(long)]
        workspace: Option<String>,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value_t = 7331)]
        port: u16,
    },
    Dashboard {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value_t = 7331)]
        port: u16,
    },
    Proxy {
        #[arg(long, default_value = "127.0.0.1:7332")]
        listen: String,

        #[arg(long, default_value = "http://localhost:11434")]
        upstream: String,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long, default_value_t = 1_200)]
        tokens: usize,

        #[arg(long)]
        learn: bool,

        #[arg(long)]
        approval_required: bool,

        #[arg(long, default_value_t = 0.58)]
        min_confidence: f32,

        #[arg(long)]
        dry_run: bool,
    },
    Mcp {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        allow_writes: bool,

        #[arg(long)]
        no_redaction: bool,

        #[arg(long)]
        audit_log: Option<PathBuf>,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 20)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Compact {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    Stats {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        conflicts: bool,

        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    Create {
        name: String,

        #[arg(long, default_value = "")]
        description: String,

        #[arg(long, default_value = "project")]
        category: String,

        #[arg(long)]
        activate: bool,
    },
    Switch {
        name: String,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Current {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    Set {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        memory_type: Option<String>,

        #[arg(long, value_enum)]
        mode: CliPolicyMode,

        #[arg(long)]
        retain_days: Option<u32>,

        #[arg(long)]
        metadata: Option<String>,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SnapshotCommand {
    Save {
        name: String,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Restore {
        name: String,

        #[arg(long)]
        workspace: Option<String>,
    },
    List {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PersonaCommand {
    Export {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value = "default-persona")]
        name: String,

        output: PathBuf,
    },
    Import {
        file: PathBuf,

        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum InboxCommand {
    List {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        status: Option<String>,

        #[arg(long)]
        simple: bool,

        #[arg(long)]
        important: bool,

        #[arg(long)]
        risky: bool,

        #[arg(long)]
        json: bool,
    },
    Stats {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Review {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Explain {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Edit {
        id: String,
        content: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, value_parser = parse_kind)]
        kind: Option<MemoryKind>,
        #[arg(long)]
        confidence: Option<f32>,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(long)]
        source_file: Option<String>,
        #[arg(long)]
        source_commit: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Approve {
        id: String,
    },
    Reject {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    RejectAll {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    Snooze {
        id: String,
    },
    Merge {
        a: String,
        b: String,
    },
    Similar {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Source {
        id: String,
    },
    Preview {
        id: String,
    },
    Rules {
        #[command(subcommand)]
        command: Option<InboxRulesCommand>,
    },
    Export {
        output: PathBuf,
        #[arg(long)]
        workspace: Option<String>,
    },
    ClearRejected {
        #[arg(long)]
        yes: bool,
    },
    ApproveAll {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 0.9)]
        confidence_above: f32,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum InboxRulesCommand {
    Add {
        pattern: String,
        #[arg(long, default_value = "review")]
        action: String,
        #[arg(long)]
        confidence_above: Option<f32>,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum DevCommand {
    Watch {
        path: PathBuf,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 10)]
        interval_secs: u64,

        #[arg(long, default_value_t = 1_800)]
        chunk_chars: usize,

        #[arg(long)]
        once: bool,
    },
    Morning {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    Resume {
        query: Option<String>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 8)]
        limit: usize,

        #[arg(long, default_value_t = 1_200)]
        tokens: usize,

        #[arg(long)]
        json: bool,
    },
    ExplainRepo {
        path: Option<PathBuf>,

        #[arg(long)]
        workspace: Option<String>,

        #[arg(long)]
        json: bool,
    },
    Next {
        #[arg(long)]
        workspace: Option<String>,

        #[arg(long, default_value_t = 5)]
        limit: usize,

        #[arg(long)]
        json: bool,
    },
    RecallError {
        error: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    TestFailures {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    RecallTest {
        test: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Context {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long = "for", value_enum, default_value_t = DevContextTarget::Generic)]
        target: DevContextTarget,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, alias = "budget", default_value_t = 1_600)]
        tokens: usize,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },
    Onboard {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, value_enum, default_value_t = DevOnboardOutput::Markdown)]
        output: DevOnboardOutput,
        #[arg(long)]
        save: Option<PathBuf>,
    },
    ReadmeSuggest {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Changelog {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Health {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    PrSummary {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Review {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Evening {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },
    Today {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },
    Yesterday {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },
    Week {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
    },
    Focus {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Tasks {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Blockers {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Risks {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Cleanup {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    DocsGap {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    StaleDecisions {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    StaleTodos {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ChangedFiles {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    HotFiles {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    CommonErrors {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    CommonCommands {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Roadmap {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ReleaseNotes {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
    },
    SetupGuide {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Architecture {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ExplainCommand {
        cmd: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualEditCli {
    id: String,
    content: Option<String>,
    #[arg(long, value_parser = parse_kind)]
    kind: Option<MemoryKind>,
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    #[arg(long)]
    metadata: Option<String>,
    #[arg(long)]
    importance: Option<f32>,
    #[arg(long)]
    confidence: Option<f32>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    source_type: Option<String>,
    #[arg(long)]
    source_file: Option<String>,
    #[arg(long)]
    source_line: Option<u64>,
    #[arg(long)]
    source_commit: Option<String>,
    #[arg(long)]
    source_conversation: Option<String>,
    #[arg(long)]
    created_by: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualRestoreCli {
    id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualInitCli {
    #[arg(long)]
    encrypted: bool,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    interactive: bool,
}

#[derive(Debug, Parser)]
struct ManualImportCli {
    path: PathBuf,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, default_value = "note", value_parser = parse_kind)]
    kind: MemoryKind,
    #[arg(long, value_enum, default_value_t = CliImportFormat::Auto)]
    format: CliImportFormat,
    #[arg(long, default_value_t = 1_800)]
    chunk_chars: usize,
    #[arg(long)]
    no_recursive: bool,
    #[arg(long)]
    preview_redactions: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualRememberCli {
    #[arg(required = true, num_args = 1..)]
    content: Vec<String>,
    #[arg(long, default_value = "note", value_parser = parse_kind)]
    kind: MemoryKind,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    #[arg(long)]
    metadata: Option<String>,
    #[arg(long)]
    importance: Option<f32>,
    #[arg(long)]
    confidence: Option<f32>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    source_type: Option<String>,
    #[arg(long)]
    source_file: Option<String>,
    #[arg(long)]
    source_line: Option<u64>,
    #[arg(long)]
    source_commit: Option<String>,
    #[arg(long)]
    source_conversation: Option<String>,
    #[arg(long)]
    created_by: Option<String>,
    #[arg(long)]
    permission: Option<String>,
    #[arg(long)]
    layer: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualRecallCli {
    #[arg(required = true, num_args = 1..)]
    query: Vec<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long = "kind", value_parser = parse_kind)]
    kinds: Vec<MemoryKind>,
    #[arg(long, value_delimiter = ',')]
    tags: Vec<String>,
    #[arg(long, value_enum)]
    profile: Option<SearchProfile>,
    #[arg(long)]
    explain: bool,
    #[arg(long, default_value_t = 8)]
    limit: usize,
    #[arg(long)]
    content: bool,
    #[arg(long)]
    include_inactive: bool,
    #[arg(long)]
    no_global: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualInboxCli {
    #[command(subcommand)]
    command: Option<InboxCommand>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualEmbeddingsCli {
    #[command(subcommand)]
    command: EmbeddingsCommand,
}

#[derive(Debug, Subcommand)]
enum EmbeddingsCommand {
    Status {
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Set {
        provider: EmbedderChoice,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        dimensions: Option<usize>,
    },
    Migrate {
        #[arg(long = "to")]
        provider: EmbedderChoice,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        json: bool,
    },
    Refresh {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Benchmark {
        #[arg(long)]
        json: bool,
    },
    Compare {
        left: Option<EmbedderChoice>,
        right: Option<EmbedderChoice>,
        #[arg(long)]
        json: bool,
    },
    Explain,
}

#[derive(Debug, Parser)]
struct ManualTerminalCli {
    #[command(subcommand)]
    command: TerminalCommand,
}

#[derive(Debug, Subcommand)]
enum TerminalCommand {
    Status {
        #[arg(long)]
        json: bool,
    },
    Enable {
        #[arg(long)]
        shell: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Record {
        #[arg(long)]
        command: String,
        #[arg(long, default_value_t = 0)]
        exit_code: i32,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long)]
        duration_ms: Option<u64>,
    },
    Commands {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    LastError {
        #[arg(long)]
        json: bool,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Suggest {
        query: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Pause {
        #[arg(long)]
        json: bool,
    },
    Resume {
        #[arg(long)]
        json: bool,
    },
    Purge {
        #[arg(long)]
        yes: bool,
    },
    Export {
        output: PathBuf,
    },
    InstallShell {
        shell: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Privacy {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualCiCli {
    #[command(subcommand)]
    command: CiCommand,
}

#[derive(Debug, Subcommand)]
enum CiCommand {
    Ingest {
        path: PathBuf,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ExplainFailure {
        query: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Last {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Similar {
        query: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Flaky {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    KnownFailures {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    FixHistory {
        query: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Health {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Export {
        output: PathBuf,
        #[arg(long)]
        workspace: Option<String>,
    },
    Report {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        workspace: Option<String>,
    },
    PrComment {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        workspace: Option<String>,
    },
}

#[derive(Debug, Parser)]
struct ManualDevCli {
    #[command(subcommand)]
    command: DevCommand,
}

#[derive(Debug, Parser)]
struct ManualMapCli {
    path: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long = "type", value_enum, default_value_t = CliMapType::Evolution)]
    map_type: CliMapType,
    #[arg(long)]
    evolution: bool,
    #[arg(long)]
    timeline: bool,
    #[arg(long)]
    decisions: bool,
    #[arg(long)]
    architecture: bool,
    #[arg(long)]
    bugs: bool,
    #[arg(long)]
    dependencies: bool,
    #[arg(long, value_enum, default_value_t = CliMapOutput::Markdown)]
    output: CliMapOutput,
    #[arg(long, alias = "since")]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    chronological: bool,
    #[arg(long)]
    why: bool,
    #[arg(long)]
    impact: Option<String>,
    #[arg(long)]
    compare_left: Option<String>,
    #[arg(long)]
    compare_right: Option<String>,
    #[arg(long)]
    save: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ManualMapCompareCli {
    left: String,
    right: String,
    #[arg(long)]
    path: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long = "type", value_enum, default_value_t = CliMapType::Evolution)]
    map_type: CliMapType,
    #[arg(long, value_enum, default_value_t = CliMapOutput::Markdown)]
    output: CliMapOutput,
    #[arg(long, alias = "since")]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    chronological: bool,
    #[arg(long)]
    why: bool,
    #[arg(long)]
    impact: Option<String>,
    #[arg(long)]
    save: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ManualMapFocusCli {
    target: String,
    path: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, value_enum, default_value_t = CliMapOutput::Markdown)]
    output: CliMapOutput,
    #[arg(long, alias = "since")]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    chronological: bool,
    #[arg(long)]
    save: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ManualStartCli {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 7331)]
    port: u16,
    #[arg(long)]
    proxy: bool,
    #[arg(long, default_value = "127.0.0.1:7332")]
    proxy_listen: String,
    #[arg(long, default_value = "http://localhost:11434")]
    upstream: String,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, default_value_t = 8)]
    limit: usize,
    #[arg(long, default_value_t = 1200)]
    tokens: usize,
}

#[derive(Debug, Parser)]
struct ManualAttachCli {
    target: AttachTarget,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 7331)]
    port: u16,
    #[arg(long, default_value = "http://localhost:11434")]
    upstream: String,
    #[arg(long)]
    start_proxy: bool,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long = "print-config")]
    print_config: bool,
}

#[derive(Debug, Parser)]
struct ManualDetachCli {
    target: AttachTarget,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Parser)]
struct ManualWatchCli {
    #[command(subcommand)]
    action: Option<WatchAction>,
    #[arg(long, global = true)]
    workspace: Option<String>,
    #[arg(long, default_value_t = 15, global = true)]
    interval: u64,
    #[arg(long, global = true)]
    foreground: bool,
    #[arg(long, global = true)]
    once: bool,
    #[arg(long, global = true)]
    dry_run: bool,
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum WatchAction {
    Start,
    Stop,
    Status,
    Once,
    Pause,
    Resume,
    Doctor,
}

#[derive(Debug, Parser)]
struct ManualContextCli {
    #[command(subcommand)]
    action: Option<ContextAction>,
    #[arg(long = "for", value_enum, default_value_t = DevContextTarget::Generic, global = true)]
    target: DevContextTarget,
    #[arg(long, global = true)]
    workspace: Option<String>,
    #[arg(long, default_value_t = 10, global = true)]
    limit: usize,
    #[arg(long, default_value_t = 1600, global = true)]
    budget: usize,
    #[arg(long, global = true)]
    output: Option<PathBuf>,
    #[arg(long, default_value = "markdown", global = true)]
    format: String,
    #[arg(long, global = true)]
    verbose: bool,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    copy: bool,
}

#[derive(Debug, Subcommand)]
enum ContextAction {
    Build,
    Open,
    Write,
    Status,
    Refresh,
    Diff,
    Explain,
}

#[derive(Debug, Parser)]
struct ManualProxyCli {
    #[arg(long, default_value = "127.0.0.1:7332")]
    listen: String,
    #[arg(long, default_value = "http://localhost:11434")]
    upstream: String,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, default_value_t = 8)]
    limit: usize,
    #[arg(long, default_value_t = 1_200)]
    tokens: usize,
    #[arg(long)]
    learn: bool,
    #[arg(long)]
    approval_required: bool,
    #[arg(long, default_value_t = 0.58)]
    min_confidence: f32,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Parser)]
struct ManualMcpCli {
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    allow_writes: bool,
    #[arg(long)]
    no_redaction: bool,
    #[arg(long)]
    audit_log: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ManualDemoCli {
    #[arg(default_value = "seed", value_parser = ["seed", "reset"])]
    action: String,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    path: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualDoctorCli {
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualAuditLogCli {
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ManualExtractCli {
    path: Option<PathBuf>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, value_parser = parse_kind)]
    kind: Option<MemoryKind>,
    #[arg(long)]
    from_git: bool,
    #[arg(long)]
    since: Option<String>,
    #[arg(long, default_value_t = 32)]
    limit: usize,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualGitCli {
    #[command(subcommand)]
    command: GitCommand,
}

#[derive(Debug, Subcommand)]
enum GitCommand {
    Ingest {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 32)]
        limit: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Summary {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Decisions {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Bugs {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Map {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, value_enum, default_value_t = CliMapOutput::Markdown)]
        output: CliMapOutput,
        #[arg(long)]
        save: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Watch {
        #[command(subcommand)]
        action: Option<GitWatchAction>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 15)]
        interval_secs: u64,
        #[arg(long)]
        daemon: bool,
        #[arg(long)]
        once: bool,
        #[arg(long, default_value_t = 32)]
        limit: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Today {
        #[arg(long)]
        json: bool,
    },
    Yesterday {
        #[arg(long)]
        json: bool,
    },
    Week {
        #[arg(long)]
        json: bool,
    },
    Branch {
        branch: Option<String>,
        #[arg(long)]
        json: bool,
    },
    DiffMemory {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ReleaseNotes {
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
    },
    WhyFileChanged {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    HotFiles {
        #[arg(long)]
        json: bool,
    },
    DependencyChanges {
        #[arg(long)]
        json: bool,
    },
    TestChanges {
        #[arg(long)]
        json: bool,
    },
    DocsChanges {
        #[arg(long)]
        json: bool,
    },
    RiskyChanges {
        #[arg(long)]
        json: bool,
    },
    ForgottenChanges {
        #[arg(long)]
        json: bool,
    },
    SummarizeCommit {
        sha: String,
        #[arg(long)]
        json: bool,
    },
    SummarizeBranch {
        branch: String,
        #[arg(long)]
        json: bool,
    },
    CompareBranches {
        left: String,
        right: String,
        #[arg(long)]
        json: bool,
    },
    MapBranch {
        branch: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        save: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum GitWatchAction {
    Status {
        #[arg(long)]
        json: bool,
    },
    Pause,
    Resume,
    ResetBaseline {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualSetupCli {
    #[arg(long)]
    interactive: bool,
    #[arg(long)]
    minimal: bool,
    #[arg(long)]
    developer: bool,
    #[arg(long)]
    ai_coding: bool,
    #[arg(long)]
    private: bool,
    #[arg(long)]
    offline: bool,
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    reset: bool,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualDayCli {
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualStatusCli {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    runtime: bool,
}

#[derive(Debug, Parser)]
struct ManualExplainCli {
    #[arg(required = false, num_args = 1..)]
    query: Vec<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, default_value_t = 8)]
    limit: usize,
    #[arg(long)]
    last: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualExamplesCli {
    area: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualFixCli {
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct ManualTutorialCli {
    #[command(subcommand)]
    command: Option<TutorialCommand>,
}

#[derive(Debug, Subcommand)]
enum TutorialCommand {
    Start {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualPrivacyCli {
    #[command(subcommand)]
    command: Option<PrivacyCommand>,
}

#[derive(Debug, Subcommand)]
enum PrivacyCommand {
    Status {
        #[arg(long)]
        json: bool,
    },
    Explain,
    Purge {
        #[arg(long)]
        yes: bool,
    },
    Reset {
        #[arg(long)]
        yes: bool,
    },
    Export {
        output: PathBuf,
    },
    Receipts {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualShowMapCli {
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long, default_value = ".memory.cpp/demo/evolution.html")]
    save: PathBuf,
}

#[derive(Debug, Parser)]
struct ManualOpenCli {
    target: Option<String>,
    #[arg(long = "print")]
    print_target: Option<String>,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 7331)]
    port: u16,
}

#[derive(Debug, Parser)]
struct ManualRedactCli {
    #[command(subcommand)]
    command: RedactCommand,
}

#[derive(Debug, Subcommand)]
enum RedactCommand {
    Preview {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Test {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Parser)]
struct ManualConfigCli {
    #[command(subcommand)]
    command: Option<ConfigCommand>,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Show {
        #[arg(long)]
        json: bool,
    },
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
    },
    Edit,
    Doctor {
        #[arg(long)]
        json: bool,
    },
    Reset {
        #[arg(long)]
        yes: bool,
    },
    Export {
        output: PathBuf,
    },
    Import {
        input: PathBuf,
    },
    Path,
    Profiles,
}

#[derive(Debug, Parser)]
struct ManualIgnoreCli {
    #[command(subcommand)]
    command: IgnoreCommand,
}

#[derive(Debug, Subcommand)]
enum IgnoreCommand {
    Init {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    Check {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Explain,
    Add {
        pattern: String,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    Remove {
        pattern: String,
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
struct McpPersistedConfig {
    read_only: bool,
    redact_sensitive: bool,
    workspace: Option<String>,
    audit_log: Option<String>,
}

impl Default for McpPersistedConfig {
    fn default() -> Self {
        Self {
            read_only: true,
            redact_sensitive: true,
            workspace: None,
            audit_log: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct EmbeddingPersistedConfig {
    provider: Option<String>,
    endpoint: Option<String>,
    model: Option<String>,
    dimensions: Option<usize>,
    migrated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct AppConfig {
    default_workspace: Option<String>,
    profile: Option<String>,
    encrypted_requested: bool,
    mcp: McpPersistedConfig,
    embedding: EmbeddingPersistedConfig,
}

#[derive(Debug, Clone)]
struct McpRuntimeConfig {
    workspace: Option<String>,
    allow_writes: bool,
    redact_sensitive: bool,
    audit_log: PathBuf,
}

#[derive(Debug, Serialize)]
struct AuditLogEntry<'a> {
    recorded_at: DateTime<Utc>,
    channel: &'a str,
    action: &'a str,
    workspace: Option<&'a str>,
    allowed: bool,
    detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredAuditLogEntry {
    recorded_at: DateTime<Utc>,
    channel: String,
    action: String,
    workspace: Option<String>,
    allowed: bool,
    detail: String,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: String,
    status: String,
    detail: String,
    suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    store: String,
    workspace: Option<String>,
    checks: Vec<DoctorCheck>,
}

#[derive(Debug, Clone)]
struct EngineOptions {
    db: Option<PathBuf>,
    embedder: EmbedderChoice,
    endpoint: Option<String>,
    model: Option<String>,
    dimensions: usize,
    api_key_env: String,
}

#[derive(Debug, Clone)]
struct ProxyLearningConfig {
    enabled: bool,
    approval_required: bool,
    min_confidence: f32,
    dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ExtractedCandidate {
    content: String,
    kind: MemoryKind,
    confidence: f32,
    reason: String,
    tags: Vec<String>,
    source_file: Option<String>,
    source_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RedactionPreviewHit {
    path: String,
    reason: String,
    preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GitCommitRecord {
    sha: String,
    short_sha: String,
    committed_at: String,
    subject: String,
    body: String,
    files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoHit {
    path: String,
    line: usize,
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TerminalEntry {
    recorded_at: DateTime<Utc>,
    command: String,
    exit_code: i32,
    cwd: String,
    #[serde(default)]
    git_branch: Option<String>,
    duration_ms: Option<u64>,
}

impl From<&Cli> for EngineOptions {
    fn from(value: &Cli) -> Self {
        Self {
            db: value.db.clone(),
            embedder: value.embedder.clone(),
            endpoint: value.endpoint.clone(),
            model: value.model.clone(),
            dimensions: value.dimensions,
            api_key_env: value.api_key_env.clone(),
        }
    }
}

fn main() -> Result<()> {
    let raw_args = env::args().collect::<Vec<_>>();
    if raw_args
        .get(1)
        .is_some_and(|arg| matches!(arg.as_str(), "-h" | "--help"))
        && raw_args.len() == 2
    {
        print_extended_help()?;
        return Ok(());
    }
    if try_handle_manual_command(&raw_args)? {
        return Ok(());
    }

    let cli = Cli::parse();
    let engine = build_engine(&cli)?;

    match &cli.command {
        Command::Init {
            encrypted,
            workspace,
        } => init_command(&engine, *encrypted, workspace.clone())?,
        Command::Remember {
            content,
            kind,
            workspace,
            tags,
            metadata,
            importance,
            confidence,
            source,
            source_type,
            source_file,
            source_line,
            source_commit,
            source_conversation,
            created_by,
            permission,
            layer,
            json,
        } => remember_command(
            &engine,
            content,
            *kind,
            workspace.as_ref(),
            tags,
            metadata,
            *importance,
            *confidence,
            source.as_deref(),
            source_type.as_deref(),
            source_file.as_deref(),
            *source_line,
            source_commit.as_deref(),
            source_conversation.as_deref(),
            created_by.as_deref(),
            permission.as_deref(),
            layer.as_deref(),
            *json,
        )?,
        Command::Recall {
            query,
            workspace,
            kinds,
            tags,
            profile,
            limit,
            content,
            include_inactive,
            no_global,
            json,
        } => recall_command(
            &engine,
            RecallCommandOptions {
                query,
                workspace: workspace.as_ref(),
                kinds,
                tags,
                profile: profile.as_ref(),
                explain: false,
                limit: *limit,
                include_content: *content,
                include_inactive: *include_inactive,
                include_global: !*no_global,
                json_output: *json,
            },
        )?,
        Command::Explain {
            query,
            workspace,
            limit,
            last,
            json,
        } => explain_command(&engine, query, workspace.as_ref(), *limit, *last, *json)?,
        Command::Forget { id, reason, json } => forget_command(&engine, id, reason, *json)?,
        Command::Patch {
            id,
            content,
            kind,
            workspace,
            tags,
            confidence,
            json,
        } => patch_command(
            &engine,
            PatchCommandOptions {
                id,
                content,
                kind: *kind,
                workspace: workspace.as_ref(),
                tags,
                confidence: *confidence,
                json_output: *json,
            },
        )?,
        Command::Context {
            query,
            workspace,
            limit,
            tokens,
        } => context_command(&engine, query, workspace.as_ref(), *limit, *tokens)?,
        Command::Compile {
            query,
            workspace,
            target,
            limit,
            tokens,
        } => compile_command(&engine, query, workspace.as_ref(), target, *limit, *tokens)?,
        Command::Import {
            path,
            workspace,
            kind,
            format,
            chunk_chars,
            no_recursive,
            preview_redactions,
            json,
        } => import_command(
            &engine,
            ImportCommandOptions {
                path,
                workspace: workspace.as_ref(),
                kind: *kind,
                format: format.clone().into(),
                chunk_chars: *chunk_chars,
                recursive: !*no_recursive,
                preview_redactions: *preview_redactions,
                json_output: *json,
            },
        )?,
        Command::Watch {
            path,
            workspace,
            kind,
            interval_secs,
            chunk_chars,
            once,
        } => watch_command(
            &engine,
            path,
            workspace.as_ref(),
            *kind,
            *interval_secs,
            *chunk_chars,
            *once,
        )?,
        Command::Sleep { workspace, json } => sleep_command(&engine, workspace.as_ref(), *json)?,
        Command::Timeline {
            query,
            workspace,
            limit,
            json,
        } => timeline_command(&engine, query, workspace.as_ref(), *limit, *json)?,
        Command::Replay {
            query,
            workspace,
            limit,
            json,
        } => replay_command(&engine, query, workspace.as_ref(), *limit, *json)?,
        Command::Graph {
            workspace,
            entity,
            limit,
            json,
        } => graph_command(&engine, workspace.as_ref(), entity, *limit, *json)?,
        Command::Eval { file, limit, json } => eval_command(&engine, file, *limit, *json)?,
        Command::Export {
            workspace,
            format,
            output,
        } => export_command(&engine, workspace.as_ref(), format, output)?,
        Command::Persona { command } => persona_command(&engine, command)?,
        Command::Workspace { command } => workspace_command(&engine, command)?,
        Command::Policy { command } => policy_command(&engine, command)?,
        Command::Snapshot { command } => snapshot_command(&engine, command)?,
        Command::Diff {
            left,
            right,
            workspace,
            json,
        } => diff_command(&engine, left, right, workspace.as_ref(), *json)?,
        Command::Inbox { command } => inbox_command(&engine, command)?,
        Command::Attach {
            target,
            host,
            port,
            upstream,
            start_proxy,
            workspace,
        } => attach_command(
            &engine,
            target,
            host,
            *port,
            upstream,
            *start_proxy,
            workspace.as_ref(),
            false,
            true,
            false,
        )?,
        Command::Serve { host, port } => serve_command(engine, host, *port, false)?,
        Command::Dashboard { host, port } => serve_command(engine, host, *port, true)?,
        Command::Proxy {
            listen,
            upstream,
            workspace,
            limit,
            tokens,
            learn,
            approval_required,
            min_confidence,
            dry_run,
        } => proxy_command(
            engine,
            listen,
            upstream,
            workspace.as_ref(),
            *limit,
            *tokens,
            ProxyLearningConfig {
                enabled: *learn,
                approval_required: *approval_required,
                min_confidence: *min_confidence,
                dry_run: *dry_run,
            },
        )?,
        Command::Mcp {
            workspace,
            allow_writes,
            no_redaction,
            audit_log,
        } => {
            let mcp = resolve_mcp_runtime_config(
                &engine,
                workspace.as_ref(),
                *allow_writes,
                *no_redaction,
                audit_log.as_ref(),
            )?;
            mcp_command(&engine, &mcp)?
        }
        Command::List {
            workspace,
            limit,
            json,
        } => list_command(&engine, workspace.as_ref(), *limit, *json)?,
        Command::Compact { workspace, limit } => {
            compact_command(&engine, workspace.as_ref(), *limit)?
        }
        Command::Stats {
            workspace,
            conflicts,
            json,
        } => stats_command(&engine, workspace.as_ref(), *conflicts, *json)?,
    }

    Ok(())
}

fn print_extended_help() -> Result<()> {
    println!("memory");
    println!(
        "memory.cpp helps your repo remember what happened, why it changed, and what to do next."
    );
    println!();
    println!("Usage:");
    println!("  memory [--db PATH] [--embedder hash|fastembed|ollama|openai] <command>");
    println!();
    println!("Beginner-friendly commands:");
    println!("  welcome                         Friendly first-run overview");
    println!("  setup --interactive             Guided local setup for this repo");
    println!("  what                            Explain what memory.cpp is doing");
    println!("  where                           Show where local data is stored");
    println!("  today | yesterday               Show a simple repo recap");
    println!("  next                            Suggest the next practical action");
    println!("  show-map                        Generate/open the project evolution map");
    println!("  show-context                    Build an AI assistant context pack");
    println!("  show-inbox                      Review pending memory candidates");
    println!("  privacy status                  Show local-first safety status");
    println!();
    println!("Core commands:");
    println!("  init [--workspace <name>]       Initialize a local memory store");
    println!("  remember|add <text>             Store a memory");
    println!("  recall|search <query>           Search memory; supports --profile dev|error|decision|code|docs|test");
    println!("  explain <query>                 Explain recall/ranking");
    println!("  edit <id> [content]             Edit memory content or metadata");
    println!("  restore <id>                    Restore the latest active version");
    println!("  workspace <cmd>                 Create, switch, list, or show workspaces");
    println!("  stats                           Show store statistics");
    println!();
    println!("Developer workflow:");
    println!("  dev morning                     Daily recap: work, changes, breakage, TODOs, next command");
    println!("  dev resume [query]              Reconstruct interrupted work with AI context");
    println!("  dev explain-repo                Instant repo briefing");
    println!("  dev next                        Practical next actions grounded in repo state");
    println!("  dev recall-error <error>        Recall previous fixes for an error");
    println!("  dev test-failures               Show remembered flaky/failing tests");
    println!("  dev recall-test <name>          Recall fixes for a specific test");
    println!("  dev context --for cursor|codex|claude");
    println!("  dev onboard --output markdown   Generate onboarding notes");
    println!("  dev readme-suggest              Suggest README updates without editing");
    println!("  dev changelog --since <ref|30d> Generate changelog bullets");
    println!("  dev health                      Repo health summary");
    println!("  dev pr-summary                  Lightweight PR summary");
    println!("  dev review                      Recall review/style memory");
    println!();
    println!("Automation and inbox:");
    println!("  git ingest|summary|decisions|bugs|map|watch");
    println!("  inbox [list]                    Review pending candidates");
    println!("  inbox stats|explain|edit|approve|reject|approve-all");
    println!("  terminal enable|record|commands|last-error|search");
    println!("  ci ingest <log>|explain-failure");
    println!();
    println!("Maps and integrations:");
    println!("  map [PATH] --type evolution --output html --save evolution.html");
    println!("  map why <topic>                 Explain why a feature or decision exists");
    println!(
        "  map impact <topic>              Show affected files, commands, tests, docs, and risks"
    );
    println!("  attach cursor|claude|codex|ollama");
    println!("  proxy --learn --approval-required");
    println!("  mcp                             Read-only, redacted MCP server by default");
    println!("  embeddings status|list|set|migrate");
    println!("  doctor                          Diagnose local setup and exact fixes");
    println!("  start | stop | status           Lightweight runtime management");
    println!();
    println!("Parser note:");
    println!(
        "  Launch commands use a small manual pre-parser to avoid a known Clap stack-overflow edge"
    );
    println!("  case from the oversized nested command tree. The static help page keeps the CLI launchable");
    println!("  while the command tree is split into smaller modules.");
    Ok(())
}

fn try_handle_manual_command(raw_args: &[String]) -> Result<bool> {
    let Some((options, command, rest)) = split_manual_args(raw_args)? else {
        return Ok(false);
    };

    match command.as_str() {
        "init" => {
            let args = ManualInitCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            if args.interactive {
                setup_command(
                    &engine,
                    &ManualSetupCli {
                        interactive: true,
                        minimal: false,
                        developer: true,
                        ai_coding: false,
                        private: false,
                        offline: false,
                        yes: false,
                        reset: false,
                        workspace: args.workspace,
                        json: false,
                    },
                )?;
            } else {
                init_command(&engine, args.encrypted, args.workspace)?;
            }
        }
        "edit" => {
            let args = ManualEditCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            edit_command(
                &engine,
                &args.id,
                &args.content,
                args.kind,
                &args.tags,
                &args.metadata,
                args.importance,
                args.confidence,
                args.source.as_deref(),
                args.source_type.as_deref(),
                args.source_file.as_deref(),
                args.source_line,
                args.source_commit.as_deref(),
                args.source_conversation.as_deref(),
                args.created_by.as_deref(),
                args.status.as_deref(),
                args.json,
            )?;
        }
        "restore" => {
            let args = ManualRestoreCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            restore_command(&engine, &args.id, args.json)?;
        }
        "import" => {
            let args = ManualImportCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            import_command(
                &engine,
                ImportCommandOptions {
                    path: &args.path,
                    workspace: args.workspace.as_ref(),
                    kind: args.kind,
                    format: args.format.into(),
                    chunk_chars: args.chunk_chars,
                    recursive: !args.no_recursive,
                    preview_redactions: args.preview_redactions,
                    json_output: args.json,
                },
            )?;
        }
        "remember" | "add" => {
            let args = ManualRememberCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            remember_command(
                &engine,
                &args.content,
                args.kind,
                args.workspace.as_ref(),
                &args.tags,
                &args.metadata,
                args.importance,
                args.confidence,
                args.source.as_deref(),
                args.source_type.as_deref(),
                args.source_file.as_deref(),
                args.source_line,
                args.source_commit.as_deref(),
                args.source_conversation.as_deref(),
                args.created_by.as_deref(),
                args.permission.as_deref(),
                args.layer.as_deref(),
                args.json,
            )?;
        }
        "recall" | "search" => {
            let args = ManualRecallCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            recall_command(
                &engine,
                RecallCommandOptions {
                    query: &args.query,
                    workspace: args.workspace.as_ref(),
                    kinds: &args.kinds,
                    tags: &args.tags,
                    profile: args.profile.as_ref(),
                    explain: args.explain,
                    limit: args.limit,
                    include_content: args.content,
                    include_inactive: args.include_inactive,
                    include_global: !args.no_global,
                    json_output: args.json,
                },
            )?;
        }
        "inbox" => {
            let args = ManualInboxCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            let command = args.command.unwrap_or(InboxCommand::List {
                workspace: args.workspace,
                status: args.status,
                simple: false,
                important: false,
                risky: false,
                json: args.json,
            });
            inbox_command(&engine, &command)?;
        }
        "dev" => {
            let args = ManualDevCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            dev_command(&engine, &args.command)?;
        }
        "embeddings" => {
            let args = ManualEmbeddingsCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            embeddings_command(&engine, &options, &args.command)?;
        }
        "terminal" => {
            let args = ManualTerminalCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            terminal_command(&engine, &args.command)?;
        }
        "ci" => {
            let args = ManualCiCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            ci_command(&engine, &args.command)?;
        }
        "explain" => {
            let args = ManualExplainCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            explain_or_topic_command(&engine, &args)?;
        }
        "examples" => {
            let args = ManualExamplesCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            examples_command(args.area.as_deref(), args.json)?;
        }
        "welcome" => {
            let engine = build_engine_from_options(&options)?;
            welcome_command(&engine)?;
        }
        "setup" => {
            let args = ManualSetupCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            setup_command(&engine, &args)?;
        }
        "tutorial" => {
            let args = ManualTutorialCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            tutorial_command(&engine, &args.command)?;
        }
        "what" => {
            let engine = build_engine_from_options(&options)?;
            what_command(&engine)?;
        }
        "where" => {
            let engine = build_engine_from_options(&options)?;
            where_command(&engine)?;
        }
        "today" => {
            let args = ManualDayCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            day_recap_command(&engine, args.workspace.as_ref(), 0, args.verbose, args.json)?;
        }
        "yesterday" => {
            let args = ManualDayCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            day_recap_command(&engine, args.workspace.as_ref(), 1, args.verbose, args.json)?;
        }
        "week" => {
            let args = ManualDayCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            dev_week_command(&engine, args.workspace.as_ref(), args.verbose, args.json)?;
        }
        "next" => {
            let args = ManualDayCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            dev_next_command(&engine, args.workspace.as_ref(), 5, args.json)?;
        }
        "open" => {
            let args = ManualOpenCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            open_command(&args)?;
        }
        "clean" => {
            let engine = build_engine_from_options(&options)?;
            clean_command(&engine)?;
        }
        "reset-demo" => {
            let engine = build_engine_from_options(&options)?;
            demo_reset_command(&engine, None, None, false)?;
        }
        "help-me" => {
            let engine = build_engine_from_options(&options)?;
            help_me_command(&engine)?;
        }
        "explain-this" | "explain-command" => {
            explain_this_command(&rest.join(" "))?;
        }
        "show-map" => {
            let args = ManualShowMapCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            show_map_command(&engine, args.workspace.as_ref(), &args.save)?;
        }
        "show-brain" => {
            let engine = build_engine_from_options(&options)?;
            stats_command(&engine, None, true, false)?;
        }
        "show-timeline" => {
            let engine = build_engine_from_options(&options)?;
            timeline_command(&engine, &[], None, 20, false)?;
        }
        "show-context" => {
            let engine = build_engine_from_options(&options)?;
            dev_context_command(
                &engine,
                None,
                &DevContextTarget::Generic,
                10,
                1600,
                false,
                false,
            )?;
        }
        "show-inbox" => {
            let engine = build_engine_from_options(&options)?;
            inbox_command(
                &engine,
                &InboxCommand::List {
                    workspace: None,
                    status: Some("pending".to_string()),
                    simple: false,
                    important: false,
                    risky: false,
                    json: false,
                },
            )?;
        }
        "privacy" => {
            let args = ManualPrivacyCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            privacy_command(&engine, &args.command)?;
        }
        "fix" => {
            let args = ManualFixCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            fix_command(&engine, &options, args.apply, args.json)?;
        }
        "redact" => {
            let args = ManualRedactCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            redact_command(&args.command)?;
        }
        "config" => {
            let args = ManualConfigCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            config_command(&engine, &args.command)?;
        }
        "attach" => {
            let engine = build_engine_from_options(&options)?;
            if rest.first().is_some_and(|value| value == "status") {
                attach_status_command(&engine, false)?;
            } else if rest.first().is_some_and(|value| value == "doctor") {
                attach_doctor_command(&engine)?;
            } else if rest.first().is_some_and(|value| value == "list") {
                attach_list_command()?;
            } else {
                let args = ManualAttachCli::parse_from(
                    std::iter::once(command.clone()).chain(rest.iter().cloned()),
                );
                attach_command(
                    &engine,
                    &args.target,
                    &args.host,
                    args.port,
                    &args.upstream,
                    args.start_proxy,
                    args.workspace.as_ref(),
                    args.dry_run,
                    args.yes,
                    args.print_config,
                )?;
            }
        }
        "detach" => {
            let args = ManualDetachCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            detach_command(&args.target, args.dry_run, args.yes)?;
        }
        "watch" => {
            let args = ManualWatchCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            public_watch_command(&engine, &args)?;
        }
        "context" => {
            let args = ManualContextCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            public_context_command(&engine, &args)?;
        }
        "map" => {
            let engine = build_engine_from_options(&options)?;
            if rest.first().is_some_and(|value| value == "latest") {
                map_latest_command(&engine, false)?;
            } else if rest.first().is_some_and(|value| value == "open") {
                map_latest_command(&engine, true)?;
            } else if rest.first().is_some_and(|value| value == "status") {
                map_status_command(&engine)?;
            } else if rest.first().is_some_and(|value| value == "refresh") {
                map_refresh_command(&engine)?;
            } else if rest.first().is_some_and(|value| value == "export-readme") {
                map_export_markdown_command(
                    &engine,
                    "README map section",
                    ".memory.cpp/maps/readme-map.md",
                )?;
            } else if rest
                .first()
                .is_some_and(|value| value == "export-onboarding")
            {
                map_export_markdown_command(
                    &engine,
                    "Onboarding map",
                    ".memory.cpp/maps/onboarding-map.md",
                )?;
            } else if rest.first().is_some_and(|value| value == "export-context") {
                map_export_markdown_command(
                    &engine,
                    "AI context map",
                    ".memory.cpp/maps/context-map.md",
                )?;
            } else if rest.first().is_some_and(|value| value == "changed") {
                map_changed_command(&engine, &rest[1..])?;
            } else if rest.first().is_some_and(|value| value == "compare") {
                let args = ManualMapCompareCli::parse_from(
                    std::iter::once(command.clone()).chain(rest.iter().skip(1).cloned()),
                );
                map_command(
                    &engine,
                    args.path.as_deref(),
                    args.project.as_ref(),
                    args.workspace.as_ref(),
                    resolve_map_type(args.map_type, false, false, false, false, false, false),
                    args.output,
                    args.from.as_deref(),
                    args.to.as_deref(),
                    args.chronological,
                    args.why,
                    args.impact.as_deref(),
                    Some(args.left.as_str()),
                    Some(args.right.as_str()),
                    args.save.as_deref(),
                )?;
            } else if rest.first().is_some_and(|value| value == "why") {
                let args = ManualMapFocusCli::parse_from(
                    std::iter::once(command.clone()).chain(rest.iter().skip(1).cloned()),
                );
                map_command(
                    &engine,
                    args.path.as_deref(),
                    args.project.as_ref(),
                    args.workspace.as_ref(),
                    CliMapType::Decisions,
                    args.output,
                    args.from.as_deref(),
                    args.to.as_deref(),
                    args.chronological,
                    true,
                    Some(args.target.as_str()),
                    None,
                    None,
                    args.save.as_deref(),
                )?;
            } else if rest.first().is_some_and(|value| value == "impact") {
                let args = ManualMapFocusCli::parse_from(
                    std::iter::once(command.clone()).chain(rest.iter().skip(1).cloned()),
                );
                map_command(
                    &engine,
                    args.path.as_deref(),
                    args.project.as_ref(),
                    args.workspace.as_ref(),
                    CliMapType::Architecture,
                    args.output,
                    args.from.as_deref(),
                    args.to.as_deref(),
                    args.chronological,
                    true,
                    Some(args.target.as_str()),
                    None,
                    None,
                    args.save.as_deref(),
                )?;
            } else {
                let args = ManualMapCli::parse_from(
                    std::iter::once(command.clone()).chain(rest.iter().cloned()),
                );
                map_command(
                    &engine,
                    args.path.as_deref(),
                    args.project.as_ref(),
                    args.workspace.as_ref(),
                    resolve_map_type(
                        args.map_type,
                        args.evolution,
                        args.timeline,
                        args.decisions,
                        args.architecture,
                        args.bugs,
                        args.dependencies,
                    ),
                    args.output,
                    args.from.as_deref(),
                    args.to.as_deref(),
                    args.chronological,
                    args.why,
                    args.impact.as_deref(),
                    args.compare_left.as_deref(),
                    args.compare_right.as_deref(),
                    args.save.as_deref(),
                )?;
            }
        }
        "start" => {
            let args = ManualStartCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            start_command(
                &options,
                &args.host,
                args.port,
                args.proxy,
                &args.proxy_listen,
                &args.upstream,
                args.workspace.as_ref(),
                args.limit,
                args.tokens,
            )?;
        }
        "stop" => stop_command(&options)?,
        "status" => {
            let args = ManualStatusCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            if args.runtime {
                status_command(&options)?;
            } else {
                let engine = build_engine_from_options(&options)?;
                product_status_command(&engine, &options, args.json, args.verbose)?;
            }
        }
        "proxy" => {
            let args = ManualProxyCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            proxy_command(
                engine,
                &args.listen,
                &args.upstream,
                args.workspace.as_ref(),
                args.limit,
                args.tokens,
                ProxyLearningConfig {
                    enabled: args.learn,
                    approval_required: args.approval_required,
                    min_confidence: args.min_confidence,
                    dry_run: args.dry_run,
                },
            )?;
        }
        "demo" => {
            let args = ManualDemoCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            match args.action.as_str() {
                "seed" => demo_seed_command(
                    &engine,
                    args.workspace.as_ref(),
                    args.path.as_ref(),
                    args.json,
                )?,
                "reset" => demo_reset_command(
                    &engine,
                    args.workspace.as_ref(),
                    args.path.as_ref(),
                    args.json,
                )?,
                _ => unreachable!("demo action is validated by clap"),
            }
        }
        "doctor" => {
            let args = ManualDoctorCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            doctor_command(&engine, &options, args.workspace.as_ref(), args.json)?;
        }
        "audit-log" => {
            let args = ManualAuditLogCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            audit_log_command(&engine, args.limit, args.path.as_deref(), args.json)?;
        }
        "mcp" => {
            let args = ManualMcpCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            let mcp = resolve_mcp_runtime_config(
                &engine,
                args.workspace.as_ref(),
                args.allow_writes,
                args.no_redaction,
                args.audit_log.as_ref(),
            )?;
            mcp_command(&engine, &mcp)?;
        }
        "extract" => {
            let args = ManualExtractCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            extract_command(
                &engine,
                args.path.as_deref(),
                args.workspace.as_ref(),
                args.kind,
                args.from_git,
                args.since.as_deref(),
                args.limit,
                args.dry_run,
                args.json,
            )?;
        }
        "git" => {
            let args = ManualGitCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            git_command(&engine, &args.command)?;
        }
        "ignore" => {
            let args = ManualIgnoreCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            ignore_command(&args.command)?;
        }
        _ => return Ok(false),
    }

    Ok(true)
}

fn split_manual_args(raw_args: &[String]) -> Result<Option<(EngineOptions, String, Vec<String>)>> {
    let mut options = EngineOptions {
        db: None,
        embedder: EmbedderChoice::Hash,
        endpoint: None,
        model: None,
        dimensions: 384,
        api_key_env: "MEMORY_CPP_OPENAI_API_KEY".to_string(),
    };
    let manual_commands = [
        "init",
        "edit",
        "restore",
        "import",
        "remember",
        "add",
        "recall",
        "search",
        "explain",
        "examples",
        "inbox",
        "dev",
        "embeddings",
        "terminal",
        "ci",
        "welcome",
        "setup",
        "tutorial",
        "what",
        "where",
        "today",
        "yesterday",
        "week",
        "next",
        "open",
        "clean",
        "reset-demo",
        "help-me",
        "explain-this",
        "explain-command",
        "show-map",
        "show-brain",
        "show-timeline",
        "show-context",
        "show-inbox",
        "privacy",
        "fix",
        "redact",
        "config",
        "attach",
        "detach",
        "watch",
        "context",
        "map",
        "start",
        "stop",
        "status",
        "proxy",
        "demo",
        "doctor",
        "audit-log",
        "mcp",
        "extract",
        "git",
        "ignore",
    ];
    let mut index = 1usize;

    while index < raw_args.len() {
        let arg = &raw_args[index];
        if manual_commands.contains(&arg.as_str()) {
            let rest = raw_args[index + 1..].to_vec();
            return Ok(Some((options, arg.clone(), rest)));
        }

        match arg.as_str() {
            "--db" => {
                index += 1;
                let value = raw_args
                    .get(index)
                    .ok_or_else(|| anyhow!("--db requires a path"))?;
                options.db = Some(PathBuf::from(value));
            }
            "--embedder" => {
                index += 1;
                let value = raw_args
                    .get(index)
                    .ok_or_else(|| anyhow!("--embedder requires a value"))?;
                options.embedder = match value.trim().to_ascii_lowercase().as_str() {
                    "hash" => EmbedderChoice::Hash,
                    "ollama" => EmbedderChoice::Ollama,
                    "openai" => EmbedderChoice::Openai,
                    "fastembed" => EmbedderChoice::Fastembed,
                    "onnx" | "fastembed-onnx" => EmbedderChoice::Onnx,
                    other => return Err(anyhow!("unknown embedder: {other}")),
                };
            }
            "--endpoint" => {
                index += 1;
                options.endpoint = Some(
                    raw_args
                        .get(index)
                        .ok_or_else(|| anyhow!("--endpoint requires a value"))?
                        .clone(),
                );
            }
            "--model" => {
                index += 1;
                options.model = Some(
                    raw_args
                        .get(index)
                        .ok_or_else(|| anyhow!("--model requires a value"))?
                        .clone(),
                );
            }
            "--dimensions" => {
                index += 1;
                let value = raw_args
                    .get(index)
                    .ok_or_else(|| anyhow!("--dimensions requires a value"))?;
                options.dimensions = value.parse::<usize>()?;
            }
            "--api-key-env" => {
                index += 1;
                options.api_key_env = raw_args
                    .get(index)
                    .ok_or_else(|| anyhow!("--api-key-env requires a value"))?
                    .clone();
            }
            value if value.starts_with('-') => return Ok(None),
            _ => return Ok(None),
        }

        index += 1;
    }

    Ok(None)
}

fn init_command(engine: &MemoryEngine, encrypted: bool, workspace: Option<String>) -> Result<()> {
    let mut config = load_app_config(engine.store_path())?;
    if let Some(workspace) = workspace.clone() {
        engine.create_workspace(&workspace, "default workspace", "project", true)?;
        config.default_workspace = Some(workspace);
    }
    config.encrypted_requested = encrypted;
    save_app_config(engine.store_path(), &config)?;

    println!(
        "initialized memory store at {}",
        engine.store_path().display()
    );
    println!("embedder: {}", engine.embedder_name());
    if encrypted {
        println!("note: encrypted local storage is marked as requested in config; live DB encryption is not yet enabled.");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn remember_command(
    engine: &MemoryEngine,
    content: &[String],
    kind: MemoryKind,
    workspace: Option<&String>,
    tags: &[String],
    metadata: &Option<String>,
    importance: Option<f32>,
    confidence: Option<f32>,
    source: Option<&str>,
    source_type: Option<&str>,
    source_file: Option<&str>,
    source_line: Option<u64>,
    source_commit: Option<&str>,
    source_conversation: Option<&str>,
    created_by: Option<&str>,
    permission: Option<&str>,
    layer: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let mut input = NewMemory::new(content.join(" "))
        .scope(scope)
        .try_kind(kind.as_str())?
        .metadata(parse_metadata(metadata.as_deref())?);

    for tag in tags {
        input = input.tag(tag.clone());
    }
    if let Some(importance) = importance {
        input = input.importance(importance);
    }
    if let Some(confidence) = confidence {
        input = input.confidence(confidence);
    }
    if let Some(permission) = permission {
        input = input.permission(parse_permission(permission)?);
    }
    if let Some(layer) = layer {
        input = input.layer(parse_layer(layer)?);
    }
    if let Some(source_meta) = build_memory_source(
        source,
        source_type,
        source_file,
        source_line,
        source_commit,
        source_conversation,
        created_by,
        confidence,
        None,
    ) {
        input = input.source(source_meta);
    }

    let memory = engine.remember(input)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!("remembered {}", memory.id);
        println!("{}", memory.summary);
    }
    Ok(())
}

struct RecallCommandOptions<'a> {
    query: &'a [String],
    workspace: Option<&'a String>,
    kinds: &'a [MemoryKind],
    tags: &'a [String],
    profile: Option<&'a SearchProfile>,
    explain: bool,
    limit: usize,
    include_content: bool,
    include_inactive: bool,
    include_global: bool,
    json_output: bool,
}

fn apply_search_profile(
    words: &[String],
    kinds: &[MemoryKind],
    tags: &[String],
    profile: Option<&SearchProfile>,
) -> (Vec<String>, Vec<MemoryKind>, Vec<String>) {
    let mut query_words = words.to_vec();
    let mut profile_kinds = kinds.to_vec();
    let profile_tags = tags.to_vec();

    match profile {
        Some(SearchProfile::Dev) => {
            query_words.extend(["todo", "next", "workflow", "file"].map(str::to_string));
        }
        Some(SearchProfile::Error) => {
            if profile_kinds.is_empty() {
                profile_kinds.push(MemoryKind::Bug);
            }
            query_words.extend(["error", "failure", "fix", "workaround"].map(str::to_string));
        }
        Some(SearchProfile::Decision) => {
            if profile_kinds.is_empty() {
                profile_kinds.push(MemoryKind::Decision);
            }
            query_words.extend(["why", "because", "chosen", "alternative"].map(str::to_string));
        }
        Some(SearchProfile::Code) => {
            query_words.extend(["symbol", "file", "module", "implementation"].map(str::to_string));
        }
        Some(SearchProfile::Docs) => {
            query_words.extend(["README", "docs", "architecture", "run"].map(str::to_string));
        }
        Some(SearchProfile::Test) => {
            query_words.extend(["test", "failure", "flaky", "reproduce"].map(str::to_string));
        }
        Some(SearchProfile::Terminal) => {
            query_words.extend(["terminal", "command", "shell", "exit"].map(str::to_string));
        }
        Some(SearchProfile::Git) => {
            query_words.extend(["git", "commit", "branch", "diff"].map(str::to_string));
        }
        Some(SearchProfile::Ci) => {
            if profile_kinds.is_empty() {
                profile_kinds.push(MemoryKind::Bug);
            }
            query_words.extend(["ci", "workflow", "build", "failure"].map(str::to_string));
        }
        None => {}
    }

    (query_words, profile_kinds, profile_tags)
}

fn recall_command(engine: &MemoryEngine, options: RecallCommandOptions<'_>) -> Result<()> {
    let (profile_words, profile_kinds, profile_tags) =
        apply_search_profile(options.query, options.kinds, options.tags, options.profile);
    let mut recall_query = build_recall_query(
        &profile_words,
        options.workspace,
        &profile_kinds,
        &profile_tags,
        options.limit,
        options.include_content,
        options.include_global,
        engine,
    )?;
    recall_query = recall_query.include_inactive(options.include_inactive);
    let memories = engine.search(recall_query)?;
    if options.json_output {
        if options.explain || options.profile.is_some() {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "profile": options.profile.map(|profile| format!("{profile:?}").to_ascii_lowercase()),
                    "expanded_query": profile_words.join(" "),
                    "kinds": profile_kinds,
                    "tags": profile_tags,
                    "results": memories,
                }))?
            );
        } else {
            println!("{}", serde_json::to_string_pretty(&memories)?);
        }
    } else if memories.is_empty() {
        println!("no memories found");
    } else {
        if options.explain || options.profile.is_some() {
            println!(
                "search profile: {}",
                options
                    .profile
                    .map(|profile| format!("{profile:?}").to_ascii_lowercase())
                    .unwrap_or_else(|| "default".to_string())
            );
            println!("expanded query: {}", profile_words.join(" "));
        }
        for (index, item) in memories.iter().enumerate() {
            println!(
                "{}. [{:.3} | {} | {}] {}",
                index + 1,
                item.score,
                item.memory.kind,
                item.memory.scope,
                item.memory.summary
            );
            println!(
                "   why: semantic={:.3}, keyword={:.3}, entity={:.3}, confidence={:.3}",
                item.similarity, item.keyword_score, item.entity_score, item.confidence_score
            );
            if options.include_content {
                println!("   {}", item.memory.content);
            }
        }
    }
    Ok(())
}

fn explain_command(
    engine: &MemoryEngine,
    query: &[String],
    workspace: Option<&String>,
    limit: usize,
    last: bool,
    json_output: bool,
) -> Result<()> {
    if last {
        let trace = engine.last_explain(workspace.map(String::as_str))?;
        match trace {
            Some(trace) if json_output => println!("{}", serde_json::to_string_pretty(&trace)?),
            Some(trace) => {
                println!("last recall query: {}", trace.query);
                println!("retrieved at: {}", trace.retrieved_at);
                for item in trace.memories {
                    println!(
                        "{} [{}] {}",
                        item.memory.id, item.memory.kind, item.memory.summary
                    );
                    println!("   {}", item.reason);
                }
            }
            None => println!("no recall trace found"),
        }
        return Ok(());
    }

    let memories = engine.explain(build_recall_query(
        query,
        workspace,
        &[],
        &[],
        limit,
        true,
        true,
        engine,
    )?)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else {
        for (index, item) in memories.iter().enumerate() {
            println!("{}. [{:.3}] {}", index + 1, item.score, item.memory.summary);
            println!("   {}", item.reason);
        }
    }
    Ok(())
}

fn forget_command(engine: &MemoryEngine, id: &str, reason: &str, json_output: bool) -> Result<()> {
    let memory = engine.forget(id, reason)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!("forgot {}", memory.id);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn edit_command(
    engine: &MemoryEngine,
    id: &str,
    content: &Option<String>,
    kind: Option<MemoryKind>,
    tags: &[String],
    metadata: &Option<String>,
    importance: Option<f32>,
    confidence: Option<f32>,
    source: Option<&str>,
    source_type: Option<&str>,
    source_file: Option<&str>,
    source_line: Option<u64>,
    source_commit: Option<&str>,
    source_conversation: Option<&str>,
    created_by: Option<&str>,
    status: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let edit = MemoryEdit {
        content: content.clone(),
        kind,
        importance,
        confidence,
        tags: (!tags.is_empty()).then(|| tags.to_vec()),
        metadata: if metadata.is_some() {
            Some(parse_metadata(metadata.as_deref())?)
        } else {
            None
        },
        source: build_memory_source(
            source,
            source_type,
            source_file,
            source_line,
            source_commit,
            source_conversation,
            created_by,
            confidence,
            None,
        ),
        status: status.map(parse_status).transpose()?,
    };

    let memory = engine.edit_memory(id, edit)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!("edited {}", memory.id);
        println!("{}", memory.summary);
    }
    Ok(())
}

fn restore_command(engine: &MemoryEngine, id: &str, json_output: bool) -> Result<()> {
    let memory = engine.restore_memory(id)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memory)?);
    } else {
        println!("restored {}", memory.id);
        println!("{}", memory.summary);
    }
    Ok(())
}

struct PatchCommandOptions<'a> {
    id: &'a str,
    content: &'a [String],
    kind: MemoryKind,
    workspace: Option<&'a String>,
    tags: &'a [String],
    confidence: Option<f32>,
    json_output: bool,
}

fn patch_command(engine: &MemoryEngine, options: PatchCommandOptions<'_>) -> Result<()> {
    let scope = required_workspace(engine, options.workspace)?;
    let mut replacement = NewMemory::new(options.content.join(" "))
        .scope(scope)
        .try_kind(options.kind.as_str())?;
    for tag in options.tags {
        replacement = replacement.tag(tag.clone());
    }
    if let Some(confidence) = options.confidence {
        replacement = replacement.confidence(confidence);
    }

    let result = engine.patch(options.id, replacement)?;
    if options.json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "patched {} -> {}",
            result.old_memory.id, result.new_memory.id
        );
    }
    Ok(())
}

fn context_command(
    engine: &MemoryEngine,
    query: &[String],
    workspace: Option<&String>,
    limit: usize,
    tokens: usize,
) -> Result<()> {
    let context = engine.context(
        build_recall_query(query, workspace, &[], &[], limit, false, true, engine)?,
        tokens,
    )?;
    println!("{}", context.text);
    Ok(())
}

fn compile_command(
    engine: &MemoryEngine,
    query: &[String],
    workspace: Option<&String>,
    target: &CompileTarget,
    limit: usize,
    tokens: usize,
) -> Result<()> {
    let context = engine.context(
        build_recall_query(query, workspace, &[], &[], limit, false, true, engine)?,
        tokens,
    )?;

    let compiled = match target {
        CompileTarget::Cursor | CompileTarget::Codex => format!(
            "Long-term memory for this coding task:\n{}\n\nUse it only when relevant.",
            context.text
        ),
        CompileTarget::Claude => format!(
            "<long_term_memory>\n{}\n</long_term_memory>\nUse this only when it improves the answer.",
            context.text
        ),
        CompileTarget::Ollama => format!(
            "SYSTEM MEMORY:\n{}\n\nFollow the memory only when it clearly applies.",
            context.text
        ),
    };

    println!("{compiled}");
    Ok(())
}

struct ImportCommandOptions<'a> {
    path: &'a Path,
    workspace: Option<&'a String>,
    kind: MemoryKind,
    format: ImportFormat,
    chunk_chars: usize,
    recursive: bool,
    preview_redactions: bool,
    json_output: bool,
}

fn import_command(engine: &MemoryEngine, options: ImportCommandOptions<'_>) -> Result<()> {
    let cli_options = options;
    if cli_options.preview_redactions {
        let hits = preview_redactions(cli_options.path, cli_options.recursive)?;
        let report = json!({
            "path": cli_options.path,
            "hits": hits,
        });
        if cli_options.json_output {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else if hits.is_empty() {
            println!("no likely secret material detected in the import set");
        } else {
            println!("detected possible secrets before import:");
            for hit in hits {
                println!(
                    "  - {} [{}] {}",
                    hit.path,
                    hit.reason,
                    hit.preview
                        .unwrap_or_else(|| "redacted content".to_string())
                );
            }
            println!("skipped by default; remove or redact them before import if needed.");
        }
        return Ok(());
    }

    let options = ImportOptions {
        scope: required_workspace(engine, cli_options.workspace)?,
        kind: cli_options.kind,
        format: cli_options.format,
        chunk_chars: cli_options.chunk_chars,
        recursive: cli_options.recursive,
    };
    let report = import_path(engine, cli_options.path, &options)?;
    if cli_options.json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "imported {} memories from {} file(s), skipped {}",
            report.imported, report.files, report.skipped
        );
    }
    Ok(())
}

fn preview_redactions(path: &Path, recursive: bool) -> Result<Vec<RedactionPreviewHit>> {
    let files = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        collect_importable_files(path, recursive)?
    };
    let mut hits = Vec::new();
    for file in files {
        let raw = match fs::read_to_string(&file) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for line in raw.lines() {
            let trimmed = line.trim();
            if let Some(reason) = detect_sensitive_reason(trimmed) {
                let preview = trimmed
                    .split_once(':')
                    .map(|(prefix, _)| format!("{prefix}: [REDACTED]"))
                    .or_else(|| {
                        trimmed
                            .split_once('=')
                            .map(|(prefix, _)| format!("{prefix}=[REDACTED]"))
                    })
                    .or(Some("[REDACTED]".to_string()));
                hits.push(RedactionPreviewHit {
                    path: file.display().to_string(),
                    reason: reason.to_string(),
                    preview,
                });
                break;
            }
        }
    }
    Ok(hits)
}

fn watch_command(
    engine: &MemoryEngine,
    path: &Path,
    workspace: Option<&String>,
    kind: MemoryKind,
    interval_secs: u64,
    chunk_chars: usize,
    once: bool,
) -> Result<()> {
    let options = ImportOptions {
        scope: required_workspace(engine, workspace)?,
        kind,
        format: ImportFormat::Auto,
        chunk_chars,
        recursive: true,
    };
    let mut seen = HashMap::<PathBuf, SystemTime>::new();

    loop {
        let mut imported = 0;
        for file in collect_watch_files(path)? {
            let modified = fs::metadata(&file)?
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if seen.get(&file).is_some_and(|old| *old >= modified) {
                continue;
            }

            for mut memory in parse_file(&file, &options)? {
                memory.scope = options.scope.clone();
                memory.kind = options.kind;
                memory = memory.tag("watch".to_string()).source(MemorySource {
                    source_type: Some("watch".to_string()),
                    source_app: None,
                    source: Some(file.to_string_lossy().to_string()),
                    source_file: Some(file.to_string_lossy().to_string()),
                    source_line: None,
                    source_commit: None,
                    source_conversation_id: None,
                    source_message_id: None,
                    created_by: Some("watcher".to_string()),
                    reliability: Some(0.85),
                });
                if engine
                    .remember_candidate(memory, "watch candidate memory requires review")?
                    .is_some()
                {
                    imported += 1;
                }
            }
            seen.insert(file, modified);
        }

        if imported > 0 {
            println!("imported {imported} updated memories");
        }

        if once {
            break;
        }

        thread::sleep(Duration::from_secs(interval_secs.max(1)));
    }

    Ok(())
}

fn sleep_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let report = engine.sleep(&required_workspace(engine, workspace)?)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "sleep complete for {}: duplicates={}, conflicts={}, decayed={}",
            report.workspace,
            report.duplicates_superseded,
            report.conflicts_detected,
            report.stale_memories_decayed
        );
        if let Some(summary) = report.summary_memory_id {
            println!("summary memory: {summary}");
        }
    }
    Ok(())
}

fn timeline_command(
    engine: &MemoryEngine,
    query: &[String],
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let workspace_record = engine.current_workspace()?;
    let scope = workspace
        .map(String::as_str)
        .or(workspace_record.as_ref().map(|value| value.name.as_str()));
    let query_text = (!query.is_empty()).then(|| query.join(" "));
    let timeline = engine.timeline(scope, query_text.as_deref(), limit)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&timeline)?);
    } else {
        for event in timeline {
            println!(
                "{} [{}] {}",
                event.created_at.format("%Y-%m-%d %H:%M"),
                event.event_type,
                event.body
            );
        }
    }
    Ok(())
}

fn replay_command(
    engine: &MemoryEngine,
    query: &[String],
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let steps = engine.replay(
        &query.join(" "),
        workspace.map(String::as_str).or(engine
            .current_workspace()?
            .as_ref()
            .map(|value| value.name.as_str())),
        limit,
    )?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&steps)?);
    } else {
        for step in steps {
            println!("{}. [{}] {}", step.index, step.event, step.detail);
        }
    }
    Ok(())
}

fn graph_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    entity: &Option<String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let workspace_record = engine.current_workspace()?;
    let scope = workspace
        .map(String::as_str)
        .or(workspace_record.as_ref().map(|value| value.name.as_str()));

    if let Some(entity) = entity {
        let links = engine.related_entity(entity, scope, limit)?;
        if json_output {
            println!("{}", serde_json::to_string_pretty(&links)?);
        } else {
            for link in links {
                println!(
                    "{} [{} | {}] {}",
                    link.entity.name,
                    link.entity.kind.as_str(),
                    link.scope,
                    link.memory_summary
                );
            }
        }
        return Ok(());
    }

    let graph = engine.entity_graph(scope, limit)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&graph)?);
    } else {
        for node in graph.entities.into_iter().take(limit) {
            println!(
                "{} [{}] memories={} weight={:.2}",
                node.entity.name,
                node.entity.kind.as_str(),
                node.memories,
                node.weight
            );
        }
    }
    Ok(())
}

fn eval_command(engine: &MemoryEngine, file: &Path, limit: usize, json_output: bool) -> Result<()> {
    let cases = read_eval_cases(file)?;
    let report = evaluate(engine, &cases, limit)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "recall@{} {:.3} | mrr {:.3} | hits {}/{}",
            limit, report.recall_at_k, report.mean_reciprocal_rank, report.hits, report.cases
        );
    }
    Ok(())
}

fn export_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    format: &ExportFormat,
    output: &Path,
) -> Result<()> {
    let workspace_record = engine.current_workspace()?;
    let scope = workspace
        .map(String::as_str)
        .or(workspace_record.as_ref().map(|value| value.name.as_str()));
    let memories = engine.all_memories(scope, true)?;

    match format {
        ExportFormat::Jsonl => {
            let mut lines = Vec::new();
            for memory in memories {
                lines.push(serde_json::to_string(&memory)?);
            }
            fs::write(output, lines.join("\n"))?;
        }
        ExportFormat::Markdown => {
            let mut markdown = String::new();
            for memory in memories {
                markdown.push_str(&format!(
                    "## {} [{} | {}]\n\n{}\n\n",
                    memory.id, memory.kind, memory.scope, memory.summary
                ));
            }
            fs::write(output, markdown)?;
        }
        ExportFormat::Graphml => {
            let graph = engine.entity_graph(scope, 500)?;
            let mut graphml = String::from(
                r#"<?xml version="1.0" encoding="UTF-8"?><graphml><graph edgedefault="undirected">"#,
            );
            for (index, node) in graph.entities.iter().enumerate() {
                graphml.push_str(&format!(
                    r#"<node id="n{index}"><data key="name">{}</data><data key="kind">{}</data></node>"#,
                    xml_escape(&node.entity.name),
                    xml_escape(node.entity.kind.as_str())
                ));
            }
            graphml.push_str("</graph></graphml>");
            fs::write(output, graphml)?;
        }
        ExportFormat::Sqlite => {
            fs::copy(engine.store_path(), output)?;
        }
    }

    println!("exported memory to {}", output.display());
    Ok(())
}

fn persona_command(engine: &MemoryEngine, command: &PersonaCommand) -> Result<()> {
    match command {
        PersonaCommand::Export {
            workspace,
            name,
            output,
        } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let persona = engine.export_persona(&scope, name)?;
            fs::write(output, serde_json::to_string_pretty(&persona)?)?;
            println!("exported persona to {}", output.display());
        }
        PersonaCommand::Import { file, workspace } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let persona: PersonaProfile = serde_json::from_str(&fs::read_to_string(file)?)?;
            let imported = engine.import_persona(&scope, persona)?;
            println!("imported {} persona memories", imported);
        }
    }
    Ok(())
}

fn workspace_command(engine: &MemoryEngine, command: &WorkspaceCommand) -> Result<()> {
    match command {
        WorkspaceCommand::Create {
            name,
            description,
            category,
            activate,
        } => {
            let workspace = engine.create_workspace(name, description, category, *activate)?;
            if *activate {
                set_default_workspace(engine.store_path(), &workspace.name)?;
            }
            println!("workspace ready: {}", workspace.name);
        }
        WorkspaceCommand::Switch { name } => {
            let workspace = engine.switch_workspace(name)?;
            set_default_workspace(engine.store_path(), &workspace.name)?;
            println!("active workspace: {}", workspace.name);
        }
        WorkspaceCommand::List { json } => {
            let workspaces = engine.list_workspaces()?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&workspaces)?);
            } else {
                for workspace in workspaces {
                    println!(
                        "{} [{}] {}",
                        workspace.name,
                        if workspace.active { "active" } else { "idle" },
                        workspace.description
                    );
                }
            }
        }
        WorkspaceCommand::Current { json } => {
            let workspace = engine.current_workspace()?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&workspace)?);
            } else if let Some(workspace) = workspace {
                println!("{}", workspace.name);
            } else {
                println!("no active workspace");
            }
        }
    }
    Ok(())
}

fn policy_command(engine: &MemoryEngine, command: &PolicyCommand) -> Result<()> {
    match command {
        PolicyCommand::Set {
            workspace,
            memory_type,
            mode,
            retain_days,
            metadata,
        } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let policy = engine.set_policy(
                &scope,
                memory_type.clone(),
                mode.clone().into(),
                *retain_days,
                parse_metadata(metadata.as_deref())?,
            )?;
            println!(
                "policy set: {} {}",
                policy.scope,
                serde_json::to_string(&policy.mode)?
            );
        }
        PolicyCommand::List { workspace, json } => {
            let policies = engine.list_policies(workspace.as_deref())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&policies)?);
            } else {
                for policy in policies {
                    println!(
                        "{} {:?} retain_days={:?}",
                        policy.scope, policy.mode, policy.retain_days
                    );
                }
            }
        }
    }
    Ok(())
}

fn snapshot_command(engine: &MemoryEngine, command: &SnapshotCommand) -> Result<()> {
    match command {
        SnapshotCommand::Save {
            name,
            workspace,
            json,
        } => {
            let snapshot =
                engine.save_snapshot(&required_workspace(engine, workspace.as_ref())?, name)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
            } else {
                println!("saved snapshot {}", snapshot.name);
            }
        }
        SnapshotCommand::Restore { name, workspace } => {
            let restored =
                engine.restore_snapshot(&required_workspace(engine, workspace.as_ref())?, name)?;
            println!("restored {} memories", restored);
        }
        SnapshotCommand::List { workspace, json } => {
            let snapshots = engine.list_snapshots(workspace.as_deref(), 50)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&snapshots)?);
            } else {
                for snapshot in snapshots {
                    println!(
                        "{} [{}] {} memories",
                        snapshot.name,
                        snapshot.scope,
                        snapshot.memories.len()
                    );
                }
            }
        }
    }
    Ok(())
}

fn diff_command(
    engine: &MemoryEngine,
    left: &str,
    right: &str,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let diff = engine.diff_snapshots(&required_workspace(engine, workspace)?, left, right)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else {
        println!(
            "added={} removed={} changed={}",
            diff.added.len(),
            diff.removed.len(),
            diff.changed.len()
        );
    }
    Ok(())
}

fn inbox_command(engine: &MemoryEngine, command: &InboxCommand) -> Result<()> {
    match command {
        InboxCommand::List {
            workspace,
            status,
            simple,
            important,
            risky,
            json,
        } => {
            let mut items = engine.inbox(workspace.as_deref(), status.as_deref())?;
            if *important {
                items.retain(|item| inbox_confidence(item) >= 0.8);
            }
            if *risky {
                items.retain(|item| detect_sensitive_reason(&item.content).is_some());
            }
            if *json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                if items.is_empty() {
                    println!("inbox is clear");
                } else {
                    for item in items {
                        if *simple {
                            println!(
                                "{} [{} {:.2}] {}",
                                item.id,
                                item.status,
                                inbox_confidence(&item),
                                item.content
                            );
                        } else {
                            print_inbox_item(&item, false);
                        }
                    }
                }
            }
        }
        InboxCommand::Stats { workspace, json } => {
            let items = engine.inbox(workspace.as_deref(), None)?;
            let stats = inbox_stats(&items);
            if *json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("candidate inbox stats");
                println!("total: {}", stats["total"].as_u64().unwrap_or(0));
                println!(
                    "pending: {} | approved: {} | rejected: {}",
                    stats["pending"].as_u64().unwrap_or(0),
                    stats["approved"].as_u64().unwrap_or(0),
                    stats["rejected"].as_u64().unwrap_or(0)
                );
                println!(
                    "average confidence: {:.2}",
                    stats["average_confidence"].as_f64().unwrap_or(0.0)
                );
                println!(
                    "sensitive/risky: {}",
                    stats["sensitive"].as_u64().unwrap_or(0)
                );
            }
        }
        InboxCommand::Review { workspace, json } => {
            let item = engine
                .inbox(workspace.as_deref(), Some("pending"))?
                .into_iter()
                .max_by(|left, right| inbox_confidence(left).total_cmp(&inbox_confidence(right)));
            if *json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else if let Some(item) = item {
                println!("candidate review");
                print_inbox_item(&item, true);
                println!("actions:");
                println!("  approve: memory inbox approve {}", item.id);
                println!("  edit:    memory inbox edit {} \"new text\"", item.id);
                println!(
                    "  reject:  memory inbox reject {} --reason duplicate",
                    item.id
                );
                println!("  skip:    memory inbox snooze {}", item.id);
            } else {
                println!("inbox is clear");
            }
        }
        InboxCommand::Explain { id, json } => {
            let item = find_inbox_item(engine, id)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&inbox_explanation(&item))?
                );
            } else {
                print_inbox_item(&item, true);
            }
        }
        InboxCommand::Edit {
            id,
            content,
            reason,
            kind,
            confidence,
            tags,
            source_file,
            source_commit,
            status,
            json,
        } => {
            let mut item = find_inbox_item(engine, id)?;
            if let Some(content) = content {
                item.content = content.clone();
            }
            if let Some(reason) = reason {
                item.reason = reason.clone();
            }
            if let Some(status) = status {
                item.status = status.clone();
            }
            update_inbox_metadata(
                &mut item.metadata,
                *kind,
                *confidence,
                tags,
                source_file.as_deref(),
                source_commit.as_deref(),
            );
            engine.update_inbox_entry(&item)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("updated inbox item {}", item.id);
                print_inbox_item(&item, true);
            }
        }
        InboxCommand::Approve { id } => match approve_inbox_item(engine, id, false)? {
            Some(memory_id) => println!("approved {id} -> remembered {memory_id}"),
            None => println!("inbox item not found: {}", id),
        },
        InboxCommand::Reject { id, reason } => {
            if engine.review_inbox(id, "rejected")? {
                if let Some(reason) = reason {
                    println!("rejected {} ({})", id, reason);
                } else {
                    println!("rejected {}", id);
                }
            } else {
                println!("inbox item not found: {}", id);
            }
        }
        InboxCommand::RejectAll {
            workspace,
            yes,
            json,
        } => {
            let items = engine.inbox(workspace.as_deref(), Some("pending"))?;
            if !*yes {
                println!(
                    "{} pending item(s) would be rejected. Re-run with --yes.",
                    items.len()
                );
                return Ok(());
            }
            let mut rejected = 0usize;
            for item in &items {
                if engine.review_inbox(&item.id, "rejected")? {
                    rejected += 1;
                }
            }
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"rejected": rejected}))?
                );
            } else {
                println!("rejected {rejected} inbox item(s)");
            }
        }
        InboxCommand::Snooze { id } => {
            if engine.review_inbox(id, "snoozed")? {
                println!("snoozed {id}");
            } else {
                println!("inbox item not found: {id}");
            }
        }
        InboxCommand::Merge { a, b } => {
            let mut first = find_inbox_item(engine, a)?;
            let second = find_inbox_item(engine, b)?;
            first.content = format!("{}\n\n{}", first.content, second.content);
            first.reason = format!("merged candidates: {}; {}", first.reason, second.reason);
            engine.update_inbox_entry(&first)?;
            let _ = engine.review_inbox(b, "merged");
            println!("merged {b} into {a}");
        }
        InboxCommand::Similar { id, json } => {
            let item = find_inbox_item(engine, id)?;
            let all = engine.inbox(Some(&item.scope), None)?;
            let needle = item.content.to_ascii_lowercase();
            let similar = all
                .into_iter()
                .filter(|candidate| candidate.id != item.id)
                .filter(|candidate| {
                    let lower = candidate.content.to_ascii_lowercase();
                    lower.split_whitespace().any(|word| needle.contains(word))
                })
                .take(10)
                .collect::<Vec<_>>();
            if *json {
                println!("{}", serde_json::to_string_pretty(&similar)?);
            } else {
                println!("similar candidates:");
                for candidate in similar {
                    println!("  - {} {}", candidate.id, candidate.content);
                }
            }
        }
        InboxCommand::Source { id } => {
            let item = find_inbox_item(engine, id)?;
            let explanation = inbox_explanation(&item);
            println!(
                "source file: {}",
                explanation["source_file"].as_str().unwrap_or("unknown")
            );
            println!(
                "source commit: {}",
                explanation["source_commit"].as_str().unwrap_or("unknown")
            );
            println!("why captured: {}", item.reason);
        }
        InboxCommand::Preview { id } => {
            let item = find_inbox_item(engine, id)?;
            print_inbox_item(&item, true);
            println!("what this helps with: future search, dev resume, maps, and AI context packs");
            println!(
                "safe to store: {}",
                if detect_sensitive_reason(&item.content).is_some() {
                    "review first"
                } else {
                    "likely yes"
                }
            );
        }
        InboxCommand::Rules { command } => inbox_rules_command(engine, command)?,
        InboxCommand::Export { output, workspace } => {
            let items = engine.inbox(workspace.as_deref(), None)?;
            fs::write(output, serde_json::to_string_pretty(&items)?)?;
            println!("exported inbox to {}", output.display());
        }
        InboxCommand::ClearRejected { yes } => {
            if !*yes {
                println!("Rejected entries are kept as audit history. Re-run with --yes to mark this acknowledged.");
            } else {
                println!("Rejected entries are retained for auditability in this release.");
            }
        }
        InboxCommand::ApproveAll {
            workspace,
            confidence_above,
            dry_run,
            json,
        } => {
            let items = engine.inbox(workspace.as_deref(), Some("pending"))?;
            let eligible = items
                .into_iter()
                .filter(|item| inbox_confidence(item) >= *confidence_above)
                .collect::<Vec<_>>();
            let mut approved = Vec::new();
            if !*dry_run {
                for item in &eligible {
                    if let Some(memory_id) = approve_inbox_item(engine, &item.id, true)? {
                        approved.push(json!({ "inbox_id": item.id, "memory_id": memory_id }));
                    }
                }
            }
            let report = json!({
                "threshold": confidence_above,
                "dry_run": dry_run,
                "eligible": eligible,
                "approved": approved,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if *dry_run {
                println!(
                    "{} inbox item(s) would be approved at confidence >= {:.2}",
                    report["eligible"].as_array().map(Vec::len).unwrap_or(0),
                    confidence_above
                );
            } else {
                println!("approved {} inbox item(s)", approved.len());
            }
        }
    }
    Ok(())
}

fn inbox_rules_command(engine: &MemoryEngine, command: &Option<InboxRulesCommand>) -> Result<()> {
    let path = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("inbox-rules.json");
    match command {
        Some(InboxRulesCommand::Add {
            pattern,
            action,
            confidence_above,
        }) => {
            let mut rules = load_inbox_rules(&path)?;
            let id = format!("rule-{}", rules.len() + 1);
            rules.push(json!({
                "id": id,
                "pattern": pattern,
                "action": action,
                "confidence_above": confidence_above,
                "created_at": Utc::now(),
            }));
            save_inbox_rules(&path, &rules)?;
            println!("added inbox rule for pattern {pattern}");
        }
        Some(InboxRulesCommand::List { json }) => {
            let rules = load_inbox_rules(&path)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&rules)?);
            } else if rules.is_empty() {
                println!("no custom inbox rules yet");
                println!("defaults: never store secrets, review sensitive candidates, approve only by policy.");
            } else {
                println!("inbox rules:");
                for rule in rules {
                    println!(
                        "  - {}: {} -> {}",
                        rule["id"].as_str().unwrap_or("rule"),
                        rule["pattern"].as_str().unwrap_or("*"),
                        rule["action"].as_str().unwrap_or("review")
                    );
                }
            }
        }
        Some(InboxRulesCommand::Remove { id }) => {
            let mut rules = load_inbox_rules(&path)?;
            let before = rules.len();
            rules.retain(|rule| rule["id"].as_str() != Some(id.as_str()));
            save_inbox_rules(&path, &rules)?;
            println!(
                "removed {} inbox rule(s)",
                before.saturating_sub(rules.len())
            );
        }
        None => {
            println!("inbox rules:");
            println!("- high-confidence low-risk candidates can be approved with memory inbox approve-all --confidence-above 0.9");
            println!("- sensitive-looking candidates should be edited or rejected");
            println!("- secrets and ignored paths should never be stored");
            println!("- automatic writes stay approval-gated unless policy allows them");
            println!("custom rules: memory inbox rules add \"docs/**\" --action review");
        }
    }
    Ok(())
}

fn load_inbox_rules(path: &Path) -> Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_inbox_rules(path: &Path, rules: &[Value]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(rules)?)?;
    Ok(())
}

fn find_inbox_item(engine: &MemoryEngine, id: &str) -> Result<memory_core::InboxEntry> {
    engine
        .inbox(None, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| anyhow!("inbox item not found: {id}"))
}

fn inbox_stats(items: &[memory_core::InboxEntry]) -> Value {
    let mut by_status = HashMap::<String, usize>::new();
    let mut confidence_sum = 0.0f64;
    let mut confidence_count = 0usize;
    let mut sensitive = 0usize;
    for item in items {
        *by_status.entry(item.status.clone()).or_default() += 1;
        let confidence = inbox_confidence(item);
        if confidence > 0.0 {
            confidence_sum += confidence as f64;
            confidence_count += 1;
        }
        if detect_sensitive_reason(&item.content).is_some()
            || item
                .metadata
                .pointer("/memory_cpp/sensitivity")
                .and_then(Value::as_str)
                .is_some_and(|value| value != "low")
        {
            sensitive += 1;
        }
    }
    json!({
        "total": items.len(),
        "pending": by_status.get("pending").copied().unwrap_or(0),
        "approved": by_status.get("approved").copied().unwrap_or(0),
        "rejected": by_status.get("rejected").copied().unwrap_or(0),
        "by_status": by_status,
        "average_confidence": if confidence_count == 0 { 0.0 } else { confidence_sum / confidence_count as f64 },
        "sensitive": sensitive,
    })
}

fn inbox_explanation(item: &memory_core::InboxEntry) -> Value {
    json!({
        "id": item.id,
        "workspace": item.scope,
        "status": item.status,
        "suggested_memory": item.content,
        "why_captured": item.reason,
        "confidence": inbox_confidence(item),
        "kind": inbox_kind(item).as_str(),
        "source_file": item.metadata.pointer("/memory_cpp/source/source_file").and_then(Value::as_str)
            .or_else(|| item.metadata.pointer("/memory_cpp/source_file").and_then(Value::as_str)),
        "source_commit": item.metadata.pointer("/memory_cpp/source/source_commit").and_then(Value::as_str)
            .or_else(|| item.metadata.pointer("/memory_cpp/source_commit").and_then(Value::as_str)),
        "risk_or_sensitivity": detect_sensitive_reason(&item.content).unwrap_or("low"),
        "recommended_action": if inbox_confidence(item) >= 0.9 {
            "approve"
        } else if detect_sensitive_reason(&item.content).is_some() {
            "edit or reject"
        } else {
            "review"
        },
        "metadata": item.metadata,
    })
}

fn print_inbox_item(item: &memory_core::InboxEntry, verbose: bool) {
    let explanation = inbox_explanation(item);
    println!(
        "{} [{}] {}",
        item.id,
        item.status,
        explanation["suggested_memory"].as_str().unwrap_or("")
    );
    println!(
        "  why: {} | confidence {:.2} | kind {}",
        explanation["why_captured"]
            .as_str()
            .unwrap_or("captured candidate"),
        explanation["confidence"].as_f64().unwrap_or(0.0),
        explanation["kind"].as_str().unwrap_or("note")
    );
    if verbose {
        println!(
            "  source: file={} commit={}",
            explanation["source_file"].as_str().unwrap_or("unknown"),
            explanation["source_commit"].as_str().unwrap_or("unknown")
        );
        println!(
            "  risk: {} | recommended action: {}",
            explanation["risk_or_sensitivity"].as_str().unwrap_or("low"),
            explanation["recommended_action"]
                .as_str()
                .unwrap_or("review")
        );
    }
}

fn inbox_confidence(item: &memory_core::InboxEntry) -> f32 {
    item.metadata
        .pointer("/memory_cpp/confidence")
        .and_then(Value::as_f64)
        .or_else(|| {
            item.metadata
                .pointer("/memory_cpp/candidate/confidence")
                .and_then(Value::as_f64)
        })
        .unwrap_or(0.5) as f32
}

fn inbox_kind(item: &memory_core::InboxEntry) -> MemoryKind {
    item.metadata
        .pointer("/memory_cpp/candidate_kind")
        .and_then(Value::as_str)
        .or_else(|| {
            item.metadata
                .pointer("/memory_cpp/candidate/kind")
                .and_then(Value::as_str)
        })
        .and_then(|value| MemoryKind::from_str(value).ok())
        .unwrap_or_else(|| classify_memory_kind(&item.content))
}

fn classify_memory_kind(content: &str) -> MemoryKind {
    let lower = content.to_ascii_lowercase();
    if ["bug", "fix", "failure", "failed", "error", "regression"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Bug
    } else if ["decision", "because", "chosen", "default", "alternative"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Decision
    } else if ["todo", "next", "fixme", "plan"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Task
    } else if ["run ", "command", "workflow", "build", "test"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Workflow
    } else {
        MemoryKind::Note
    }
}

fn update_inbox_metadata(
    metadata: &mut Value,
    kind: Option<MemoryKind>,
    confidence: Option<f32>,
    tags: &[String],
    source_file: Option<&str>,
    source_commit: Option<&str>,
) {
    if !metadata.is_object() {
        *metadata = json!({});
    }
    if metadata.get("memory_cpp").is_none() {
        metadata["memory_cpp"] = json!({});
    }
    if let Some(kind) = kind {
        metadata["memory_cpp"]["candidate_kind"] = json!(kind.as_str());
    }
    if let Some(confidence) = confidence {
        metadata["memory_cpp"]["confidence"] = json!(confidence.clamp(0.0, 1.0));
    }
    if !tags.is_empty() {
        metadata["memory_cpp"]["tags"] = json!(tags);
    }
    if source_file.is_some() || source_commit.is_some() {
        if metadata["memory_cpp"].get("source").is_none() {
            metadata["memory_cpp"]["source"] = json!({});
        }
        if let Some(source_file) = source_file {
            metadata["memory_cpp"]["source"]["source_file"] = json!(source_file);
        }
        if let Some(source_commit) = source_commit {
            metadata["memory_cpp"]["source"]["source_commit"] = json!(source_commit);
        }
    }
}

fn approve_inbox_item(engine: &MemoryEngine, id: &str, missing_ok: bool) -> Result<Option<String>> {
    let item = match find_inbox_item(engine, id) {
        Ok(item) => item,
        Err(err) if missing_ok => {
            let _ = err;
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let kind = inbox_kind(&item);
    let mut memory = NewMemory::new(item.content.clone())
        .scope(item.scope.clone())
        .kind(kind.as_str())
        .confidence(inbox_confidence(&item))
        .metadata(json!({
            "approved_from_inbox": item.id,
            "candidate_reason": item.reason,
            "candidate_metadata": item.metadata,
        }))
        .status(MemoryStatus::Active)
        .source(MemorySource {
            source_type: Some("inbox_candidate".to_string()),
            source_app: Some("memory.cpp".to_string()),
            source: Some(item.reason.clone()),
            source_file: item
                .metadata
                .pointer("/memory_cpp/source/source_file")
                .and_then(Value::as_str)
                .map(str::to_string),
            source_line: None,
            source_commit: item
                .metadata
                .pointer("/memory_cpp/source/source_commit")
                .and_then(Value::as_str)
                .map(str::to_string),
            source_conversation_id: None,
            source_message_id: None,
            created_by: Some("inbox".to_string()),
            reliability: Some(inbox_confidence(&item)),
        });
    if let Some(tags) = item
        .metadata
        .pointer("/memory_cpp/tags")
        .and_then(Value::as_array)
    {
        memory = memory.tags(
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>(),
        );
    }
    let stored = engine.remember(memory)?;
    engine.review_inbox(id, "approved")?;
    Ok(Some(stored.id))
}

fn embeddings_command(
    engine: &MemoryEngine,
    options: &EngineOptions,
    command: &EmbeddingsCommand,
) -> Result<()> {
    match command {
        EmbeddingsCommand::Status { json } => {
            let config = load_app_config(engine.store_path())?;
            let report = json!({
                "active_provider": engine.embedder_name(),
                "configured_provider": config.embedding.provider,
                "endpoint": config.embedding.endpoint,
                "model": config.embedding.model,
                "dimensions": config.embedding.dimensions.unwrap_or(options.dimensions),
                "store": engine.store_path(),
                "migrated_at": config.embedding.migrated_at,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("embedding status");
                println!(
                    "active provider: {}",
                    report["active_provider"].as_str().unwrap_or("hash")
                );
                println!(
                    "configured provider: {}",
                    report["configured_provider"].as_str().unwrap_or("hash")
                );
                println!(
                    "dimensions: {}",
                    report["dimensions"].as_u64().unwrap_or(384)
                );
            }
        }
        EmbeddingsCommand::List { json } => {
            let providers = embedding_provider_registry();
            if *json {
                println!("{}", serde_json::to_string_pretty(&providers)?);
            } else {
                println!("embedding providers:");
                for provider in providers.as_array().cloned().unwrap_or_default() {
                    println!(
                        "  - {} ({}) {}",
                        provider["name"].as_str().unwrap_or("provider"),
                        provider["status"].as_str().unwrap_or("available"),
                        provider["description"].as_str().unwrap_or("")
                    );
                }
            }
        }
        EmbeddingsCommand::Set {
            provider,
            endpoint,
            model,
            dimensions,
        } => {
            let mut config = load_app_config(engine.store_path())?;
            config.embedding.provider = Some(provider.provider_name().to_string());
            if let Some(endpoint) = endpoint {
                config.embedding.endpoint = Some(endpoint.clone());
            }
            if let Some(model) = model {
                config.embedding.model = Some(model.clone());
            }
            if let Some(dimensions) = dimensions {
                config.embedding.dimensions = Some(*dimensions);
            }
            save_app_config(engine.store_path(), &config)?;
            println!(
                "embedding provider set to {} for future commands",
                provider.provider_name()
            );
        }
        EmbeddingsCommand::Migrate {
            provider,
            dry_run,
            json,
        } => {
            let memories = engine.all_memories(None, true)?;
            let report = json!({
                "to": provider.provider_name(),
                "dry_run": dry_run,
                "memories_seen": memories.len(),
                "note": "local stores keep old vectors until memories are rewritten; this migration switches the active provider and future writes use it",
            });
            if !*dry_run {
                let mut config = load_app_config(engine.store_path())?;
                config.embedding.provider = Some(provider.provider_name().to_string());
                config.embedding.dimensions = Some(options.dimensions);
                config.embedding.migrated_at = Some(Utc::now());
                save_app_config(engine.store_path(), &config)?;
            }
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "embedding migration {} to {} across {} memory record(s)",
                    if *dry_run { "planned" } else { "recorded" },
                    provider.provider_name(),
                    report["memories_seen"].as_u64().unwrap_or(0)
                );
                println!("{}", report["note"].as_str().unwrap_or(""));
            }
        }
        EmbeddingsCommand::Doctor { json } => {
            let config = load_app_config(engine.store_path())?;
            let provider = config
                .embedding
                .provider
                .clone()
                .unwrap_or_else(|| engine.embedder_name().to_string());
            let ollama_ok = check_ollama("http://localhost:11434").unwrap_or(false);
            let report = json!({
                "provider": provider,
                "dimensions": config.embedding.dimensions.unwrap_or(options.dimensions),
                "ollama_reachable": ollama_ok,
                "low_ram_safe": true,
                "warnings": embedding_warnings(&config, options, ollama_ok),
                "recommendation": "Use hash for lowest RAM, fastembed for local semantic recall, ollama when a local model server is already running."
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("embedding doctor");
                println!(
                    "provider: {}",
                    report["provider"].as_str().unwrap_or("hash")
                );
                println!("low-RAM safe: yes");
                for warning in report["warnings"].as_array().into_iter().flatten() {
                    println!("- {}", warning.as_str().unwrap_or("warning"));
                }
                println!("{}", report["recommendation"].as_str().unwrap_or(""));
            }
        }
        EmbeddingsCommand::Refresh { dry_run, json } => {
            let count = engine.all_memories(None, true)?.len();
            let report = json!({
                "dry_run": dry_run,
                "memories_seen": count,
                "note": "refresh is lightweight in this release; rewrite or migration keeps old vectors until memories are touched"
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "embedding refresh {} for {} memory record(s)",
                    if *dry_run { "planned" } else { "checked" },
                    count
                );
                println!("{}", report["note"].as_str().unwrap_or(""));
            }
        }
        EmbeddingsCommand::Benchmark { json } => {
            let sample = "memory.cpp helps your repo remember what happened";
            let started = std::time::Instant::now();
            let vector = HashEmbedder::new(options.dimensions).embed(sample)?;
            let elapsed = started.elapsed().as_millis();
            let report = json!({
                "provider": engine.embedder_name(),
                "sample": sample,
                "dimensions": vector.len(),
                "elapsed_ms": elapsed,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "{} dimensions from {} in {}ms",
                    vector.len(),
                    engine.embedder_name(),
                    elapsed
                );
            }
        }
        EmbeddingsCommand::Compare { left, right, json } => {
            let left = left.as_ref().unwrap_or(&EmbedderChoice::Hash);
            let right = right.as_ref().unwrap_or(&EmbedderChoice::Fastembed);
            let report = json!({
                "left": left.provider_name(),
                "right": right.provider_name(),
                "note": "compare records provider intent in this release; run migrate --dry-run before switching stores"
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{} vs {}", left.provider_name(), right.provider_name());
                println!("{}", report["note"].as_str().unwrap_or(""));
            }
        }
        EmbeddingsCommand::Explain => {
            println!("embedding providers");
            println!("hash: stable, tiny, offline, low-RAM default.");
            println!("ollama: beta, local server required, useful when Ollama is already running.");
            println!("openai: beta, OpenAI-compatible endpoint, opt-in API key.");
            println!("fastembed/fastembed-onnx: experimental provider intent in this CLI; no bundled ONNX Runtime is claimed here.");
            println!("try: memory embeddings status");
        }
    }
    Ok(())
}

fn embedding_warnings(config: &AppConfig, options: &EngineOptions, ollama_ok: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    if config.embedding.provider.as_deref() == Some("ollama") && !ollama_ok {
        warnings
            .push("Ollama provider is configured but localhost:11434 is not reachable".to_string());
    }
    if let Some(dimensions) = config.embedding.dimensions {
        if dimensions != options.dimensions {
            warnings.push(format!(
                "configured dimensions ({dimensions}) differ from active dimensions ({})",
                options.dimensions
            ));
        }
    }
    if config.embedding.migrated_at.is_none() {
        warnings.push("no embedding migration timestamp recorded yet".to_string());
    }
    warnings
}

fn embedding_provider_registry() -> Value {
    json!([
        {
            "name": "hash",
            "status": "built-in",
            "description": "deterministic offline lexical vectors; default and zero setup"
        },
        {
            "name": "fastembed",
            "status": "built-in-local",
            "description": "local FastEmbed/ONNX-style semantic hashing backend for zero-key semantic recall"
        },
        {
            "name": "ollama",
            "status": "http",
            "description": "uses local Ollama embeddings such as nomic-embed-text"
        },
        {
            "name": "openai",
            "status": "http",
            "description": "OpenAI-compatible embedding API using MEMORY_CPP_OPENAI_API_KEY by default"
        }
    ])
}

fn terminal_command(engine: &MemoryEngine, command: &TerminalCommand) -> Result<()> {
    match command {
        TerminalCommand::Status { json } => {
            let path = terminal_log_path(engine)?;
            let paused = terminal_paused(engine)?;
            let entries = read_terminal_entries(engine, 200)?;
            let failures = entries.iter().filter(|entry| entry.exit_code != 0).count();
            let report = json!({
                "enabled": path.exists(),
                "paused": paused,
                "log": path,
                "commands": entries.len(),
                "failures": failures,
                "last_success": entries.iter().find(|entry| entry.exit_code == 0),
                "last_failure": entries.iter().find(|entry| entry.exit_code != 0),
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("terminal memory status");
                println!("enabled: {}", report["enabled"]);
                println!("paused: {paused}");
                println!("commands recorded: {}", entries.len());
                println!("failures recorded: {failures}");
                println!("next: memory terminal search \"how did I run tests?\"");
            }
        }
        TerminalCommand::Enable { shell, json } => {
            let path = terminal_log_path(engine)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            OpenOptions::new().create(true).append(true).open(&path)?;
            let shell_name = shell.clone().unwrap_or_else(|| "powershell".to_string());
            set_terminal_paused(engine, false)?;
            let hook = terminal_shell_hook(&shell_name);
            let report = json!({ "log": path, "shell": shell_name, "hook": hook });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("terminal memory enabled at {}", path.display());
                println!("optional shell hook:");
                println!("{hook}");
            }
        }
        TerminalCommand::Record {
            command,
            exit_code,
            cwd,
            duration_ms,
        } => {
            if terminal_paused(engine)? {
                println!("terminal memory is paused; command not recorded");
                return Ok(());
            }
            let path = terminal_log_path(engine)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let cwd_path = cwd.clone().unwrap_or(env::current_dir()?);
            let git_branch = git_repo_root(&cwd_path)
                .and_then(|root| git_stdout(&root, &["branch", "--show-current"]).ok())
                .filter(|branch| !branch.trim().is_empty());
            let entry = TerminalEntry {
                recorded_at: Utc::now(),
                command: redact_command_line(command),
                exit_code: *exit_code,
                cwd: cwd_path.display().to_string(),
                git_branch,
                duration_ms: *duration_ms,
            };
            let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
            writeln!(file, "{}", serde_json::to_string(&entry)?)?;
            println!("recorded terminal command");
        }
        TerminalCommand::Commands { limit, json } => {
            let entries = read_terminal_entries(engine, *limit)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("no terminal commands recorded yet");
            } else {
                for entry in entries {
                    println!(
                        "{} [{}{}] {}",
                        entry.recorded_at.to_rfc3339(),
                        entry.exit_code,
                        entry
                            .git_branch
                            .as_ref()
                            .map(|branch| format!(" {branch}"))
                            .unwrap_or_default(),
                        entry.command
                    );
                }
            }
        }
        TerminalCommand::LastError { json } => {
            let entry = read_terminal_entries(engine, 200)?
                .into_iter()
                .find(|entry| entry.exit_code != 0);
            if *json {
                println!("{}", serde_json::to_string_pretty(&entry)?);
            } else if let Some(entry) = entry {
                println!(
                    "last failed command [{}]: {}",
                    entry.exit_code, entry.command
                );
                println!("cwd: {}", entry.cwd);
                if let Some(branch) = entry.git_branch {
                    println!("branch: {branch}");
                }
            } else {
                println!("no failed terminal command recorded");
            }
        }
        TerminalCommand::Search { query, limit, json } => {
            let entries = terminal_query_entries(engine, query, *limit)?
                .into_iter()
                .take(*limit)
                .collect::<Vec<_>>();
            if *json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("no terminal command matched {query}");
            } else {
                for entry in entries {
                    println!(
                        "{} [{}{}] {}",
                        entry.recorded_at,
                        entry.exit_code,
                        entry
                            .git_branch
                            .as_ref()
                            .map(|branch| format!(" {branch}"))
                            .unwrap_or_default(),
                        entry.command
                    );
                }
            }
        }
        TerminalCommand::Suggest { query, limit, json } => {
            let query = query
                .clone()
                .unwrap_or_else(|| "tests build dev server".to_string());
            let entries = terminal_query_entries(engine, &query, *limit)?;
            let suggestions = if entries.is_empty() {
                infer_run_commands(&env::current_dir()?)
                    .into_iter()
                    .take(*limit)
                    .collect::<Vec<_>>()
            } else {
                entries
                    .into_iter()
                    .map(|entry| entry.command)
                    .take(*limit)
                    .collect::<Vec<_>>()
            };
            if *json {
                println!("{}", serde_json::to_string_pretty(&suggestions)?);
            } else if suggestions.is_empty() {
                println!("no command suggestions yet");
                println!("try: memory terminal record --command \"cargo test\" --exit-code 0");
            } else {
                println!("terminal command suggestions:");
                for command in suggestions {
                    println!("  - {command}");
                }
            }
        }
        TerminalCommand::Pause { json } => {
            set_terminal_paused(engine, true)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"paused": true}))?
                );
            } else {
                println!("terminal memory paused");
            }
        }
        TerminalCommand::Resume { json } => {
            set_terminal_paused(engine, false)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"paused": false}))?
                );
            } else {
                println!("terminal memory resumed");
            }
        }
        TerminalCommand::Purge { yes } => {
            if !*yes {
                println!("This deletes terminal command memory. Re-run with --yes to confirm.");
                return Ok(());
            }
            let path = terminal_log_path(engine)?;
            if path.exists() {
                fs::remove_file(&path)?;
            }
            println!("terminal command memory purged");
        }
        TerminalCommand::Export { output } => {
            let entries = read_terminal_entries(engine, usize::MAX)?;
            fs::write(output, serde_json::to_string_pretty(&entries)?)?;
            println!("exported terminal memory to {}", output.display());
        }
        TerminalCommand::InstallShell { shell, json } => {
            let shell = shell.clone().unwrap_or_else(|| "powershell".to_string());
            let hook = terminal_shell_hook(&shell);
            let report = json!({
                "shell": shell,
                "hook": hook,
                "note": "opt-in shell integration; paste into your shell profile if you want command capture"
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("shell integration snippet for {shell}:");
                println!("{hook}");
                println!("Terminal memory is opt-in and stays local.");
            }
        }
        TerminalCommand::Privacy { json } => {
            let path = terminal_log_path(engine)?;
            let report = json!({
                "opt_in": true,
                "log": path,
                "paused": terminal_paused(engine)?,
                "redaction": "secret-looking arguments are replaced before writing",
                "purge": "memory terminal purge --yes",
                "export": "memory terminal export terminal-memory.json",
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("terminal memory privacy");
                println!("opt-in: yes");
                println!("stored locally at: {}", path.display());
                println!("redaction: secret-looking arguments are replaced");
                println!("pause: memory terminal pause");
                println!("purge: memory terminal purge --yes");
            }
        }
    }
    Ok(())
}

fn terminal_shell_hook(shell_name: &str) -> &'static str {
    let shell = shell_name.to_ascii_lowercase();
    if shell.contains("power") {
        "function Invoke-MemoryCommand { param([string]$Command) $started=Get-Date; iex $Command; $code=$LASTEXITCODE; memory terminal record --command $Command --exit-code $code --cwd (Get-Location).Path }"
    } else if shell.contains("fish") {
        "function memory_record_last --on-event fish_postexec; memory terminal record --command \"$argv\" --exit-code $status --cwd \"$PWD\"; end"
    } else {
        "memory terminal record --command \"$BASH_COMMAND\" --exit-code \"$?\" --cwd \"$PWD\""
    }
}

fn redact_command_line(command: &str) -> String {
    command
        .split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.contains("token=")
                || lower.contains("password=")
                || lower.contains("secret=")
                || lower.starts_with("sk-")
                || lower.starts_with("ghp_")
            {
                "[REDACTED]".to_string()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn terminal_log_path(engine: &MemoryEngine) -> Result<PathBuf> {
    Ok(engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("terminal")
        .join("commands.jsonl"))
}

fn terminal_state_path(engine: &MemoryEngine) -> Result<PathBuf> {
    Ok(engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("terminal")
        .join("state.json"))
}

fn terminal_paused(engine: &MemoryEngine) -> Result<bool> {
    let path = terminal_state_path(engine)?;
    if !path.exists() {
        return Ok(false);
    }
    let state: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok(state
        .get("paused")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

fn set_terminal_paused(engine: &MemoryEngine, paused: bool) -> Result<()> {
    let path = terminal_state_path(engine)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(&json!({
            "paused": paused,
            "updated_at": Utc::now(),
        }))?,
    )?;
    Ok(())
}

fn read_terminal_entries(engine: &MemoryEngine, limit: usize) -> Result<Vec<TerminalEntry>> {
    let path = terminal_log_path(engine)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut entries = io::BufReader::new(file)
        .lines()
        .map_while(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<TerminalEntry>(&line).ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.recorded_at));
    entries.truncate(limit.max(1));
    Ok(entries)
}

fn terminal_query_entries(
    engine: &MemoryEngine,
    query: &str,
    limit: usize,
) -> Result<Vec<TerminalEntry>> {
    let lower = query.to_ascii_lowercase();
    let expanded = if lower.contains("run tests") || lower.contains("test") {
        vec!["test", "cargo test", "npm test", "pytest", "go test"]
    } else if lower.contains("dev server") || lower.contains("start dev") {
        vec!["dev", "serve", "start", "run"]
    } else if lower.contains("build") || lower.contains("release") {
        vec!["build", "release", "cargo build", "npm run build"]
    } else {
        vec![lower.as_str()]
    };
    Ok(read_terminal_entries(engine, 500)?
        .into_iter()
        .filter(|entry| {
            let command = entry.command.to_ascii_lowercase();
            expanded.iter().any(|needle| command.contains(needle))
        })
        .take(limit.max(1))
        .collect())
}

fn ci_command(engine: &MemoryEngine, command: &CiCommand) -> Result<()> {
    match command {
        CiCommand::Ingest {
            path,
            workspace,
            json,
        } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let raw = fs::read_to_string(path)
                .with_context(|| format!("failed to read CI log {}", path.display()))?;
            let failures = parse_ci_failures(&raw);
            let mut stored = Vec::new();
            for failure in &failures {
                let memory = engine.remember(
                    NewMemory::new(failure.clone())
                        .scope(scope.clone())
                        .kind("bug")
                        .confidence(0.82)
                        .tag("ci")
                        .tag("test-failure")
                        .metadata(json!({ "source": path, "importer": "ci-ingest" }))
                        .source(MemorySource {
                            source_type: Some("ci_log".to_string()),
                            source_app: Some("memory.cpp".to_string()),
                            source: Some(path.display().to_string()),
                            source_file: Some(path.display().to_string()),
                            source_line: None,
                            source_commit: None,
                            source_conversation_id: None,
                            source_message_id: None,
                            created_by: Some("ci".to_string()),
                            reliability: Some(0.82),
                        }),
                )?;
                stored.push(memory.id);
            }
            let report =
                json!({ "workspace": scope, "path": path, "failures": failures, "stored": stored });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("ingested {} CI failure memory item(s)", stored.len());
            }
        }
        CiCommand::ExplainFailure {
            query,
            workspace,
            limit,
            json,
        } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let query = query
                .clone()
                .unwrap_or_else(|| "ci failure test error previous fix".to_string());
            let memories = engine.search(
                RecallQuery::new(query)
                    .workspace(scope)
                    .kind(MemoryKind::Bug)
                    .limit(*limit)
                    .include_content(true),
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&memories)?);
            } else if memories.is_empty() {
                println!("no CI failure memory found");
            } else {
                println!("CI failure explanation:");
                for item in memories {
                    println!("  - {}", item.memory.summary);
                    println!("    {}", item.reason);
                }
            }
        }
        CiCommand::Last { workspace, json } => {
            let memories = ci_memory_search(engine, workspace.as_ref(), "ci failure", 1)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&memories)?);
            } else if let Some(item) = memories.first() {
                println!("last CI failure: {}", item.memory.summary);
                println!("next: memory ci explain-failure");
            } else {
                println!("no CI failure memory recorded yet");
                println!("try: memory ci ingest ./ci.log");
            }
        }
        CiCommand::Similar {
            query,
            workspace,
            limit,
            json,
        } => {
            let query = query.clone().unwrap_or_else(|| "ci failure".to_string());
            let memories = ci_memory_search(engine, workspace.as_ref(), &query, *limit)?;
            emit_memory_search("similar CI failures", &memories, *json)?;
        }
        CiCommand::Flaky { workspace, json } => {
            let memories =
                ci_memory_search(engine, workspace.as_ref(), "flaky intermittent timeout", 12)?;
            emit_memory_search("flaky CI memory", &memories, *json)?;
        }
        CiCommand::KnownFailures { workspace, json } => {
            let memories = ci_memory_search(
                engine,
                workspace.as_ref(),
                "known failure ci test error",
                12,
            )?;
            emit_memory_search("known CI failures", &memories, *json)?;
        }
        CiCommand::FixHistory {
            query,
            workspace,
            json,
        } => {
            let query = query
                .clone()
                .unwrap_or_else(|| "previous fix ci failure".to_string());
            let memories = ci_memory_search(engine, workspace.as_ref(), &query, 12)?;
            emit_memory_search("CI fix history", &memories, *json)?;
        }
        CiCommand::Health { workspace, json } => {
            let memories =
                ci_memory_search(engine, workspace.as_ref(), "ci failure test error", 64)?;
            let report = json!({
                "known_failures": memories.len(),
                "health": if memories.len() > 8 { "watch" } else { "ok" },
                "next_command": "memory ci explain-failure",
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "CI memory health: {}",
                    report["health"].as_str().unwrap_or("ok")
                );
                println!("known failure memories: {}", memories.len());
                println!("next: memory ci explain-failure");
            }
        }
        CiCommand::Export { output, workspace } => {
            let memories =
                ci_memory_search(engine, workspace.as_ref(), "ci failure test error", 128)?;
            let markdown = render_ci_markdown(&memories, false);
            fs::write(output, markdown)?;
            println!("exported CI report to {}", output.display());
        }
        CiCommand::Report { output, workspace } => {
            let memories =
                ci_memory_search(engine, workspace.as_ref(), "ci failure test error", 32)?;
            let markdown = render_ci_markdown(&memories, false);
            if let Some(output) = output {
                fs::write(output, markdown)?;
                println!("wrote CI report to {}", output.display());
            } else {
                println!("{markdown}");
            }
        }
        CiCommand::PrComment { output, workspace } => {
            let memories =
                ci_memory_search(engine, workspace.as_ref(), "ci failure test error", 12)?;
            let markdown = render_ci_markdown(&memories, true);
            if let Some(output) = output {
                fs::write(output, markdown)?;
                println!("wrote CI PR comment to {}", output.display());
            } else {
                println!("{markdown}");
            }
        }
    }
    Ok(())
}

fn render_ci_markdown(memories: &[memory_core::RetrievedMemory], pr_comment: bool) -> String {
    let mut markdown = if pr_comment {
        String::from("## memory.cpp CI notes\n\n")
    } else {
        String::from("# CI memory report\n\n")
    };
    if memories.is_empty() {
        markdown.push_str("No CI failure memory has been recorded yet.\n\n");
        markdown.push_str("Try: `memory ci ingest ./ci.log`\n");
        return markdown;
    }
    markdown.push_str("Known failures and previous fixes:\n\n");
    for item in memories {
        markdown.push_str(&format!("- {}\n", item.memory.summary));
    }
    markdown.push_str("\nSuggested next step: `memory ci explain-failure`\n");
    markdown
}

fn ci_memory_search(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    query: &str,
    limit: usize,
) -> Result<Vec<memory_core::RetrievedMemory>> {
    let scope = required_workspace(engine, workspace)?;
    Ok(engine.search(
        RecallQuery::new(query)
            .workspace(scope)
            .kind(MemoryKind::Bug)
            .limit(limit)
            .include_content(true),
    )?)
}

fn emit_memory_search(
    title: &str,
    memories: &[memory_core::RetrievedMemory],
    json_output: bool,
) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(memories)?);
    } else if memories.is_empty() {
        println!("no {title} found");
    } else {
        println!("{title}:");
        for item in memories {
            println!("  - {}", item.memory.summary);
        }
    }
    Ok(())
}

fn parse_ci_failures(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.len() > 8
            && ["failed", "failure", "error:", "panic", "assertion", "flaky"]
                .iter()
                .any(|needle| lower.contains(needle))
        {
            out.push(trimmed.chars().take(280).collect());
        }
        if out.len() >= 32 {
            break;
        }
    }
    out.sort();
    out.dedup();
    out
}

fn welcome_command(engine: &MemoryEngine) -> Result<()> {
    println!("Welcome to memory.cpp");
    println!("Your repo can remember what happened, why it changed, and what to do next.");
    println!();
    println!("Nothing scary happened: this command only explains the tool.");
    println!(
        "Data stays local by default at {}",
        engine.store_path().display()
    );
    println!();
    println!("Try these next:");
    println!("1. memory setup --developer");
    println!("2. memory dev morning");
    println!("3. memory show-map");
    Ok(())
}

fn setup_command(engine: &MemoryEngine, args: &ManualSetupCli) -> Result<()> {
    if args.reset {
        println!("setup reset requested");
        println!("Run `memory privacy purge --yes` to delete local memory data.");
        return Ok(());
    }

    let profile = if args.minimal {
        "minimal"
    } else if args.ai_coding {
        "ai-coding"
    } else if args.private {
        "private"
    } else if args.offline {
        "offline"
    } else {
        "developer"
    };
    let workspace = args
        .workspace
        .clone()
        .or_else(|| {
            env::current_dir().ok().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
        })
        .unwrap_or_else(|| "default".to_string());

    if args.interactive && !args.yes {
        println!("Interactive setup");
        println!("Suggested workspace: {workspace}");
        if !ask_yes_no("Create or activate this workspace?", true)? {
            println!("No changes made.");
            return Ok(());
        }
    }

    engine.create_workspace(&workspace, "developer workspace", "project", true)?;
    set_default_workspace(engine.store_path(), &workspace)?;

    let base_dir = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    fs::create_dir_all(base_dir)?;
    fs::create_dir_all(base_dir.join("runtime"))?;
    fs::create_dir_all(base_dir.join("audit"))?;
    fs::create_dir_all(base_dir.join("terminal"))?;

    let cwd = env::current_dir()?;
    let ignore_path = cwd.join(".memoryignore");
    if !ignore_path.exists() && !args.minimal {
        fs::write(&ignore_path, DEFAULT_MEMORYIGNORE)?;
    } else if ignore_path.exists() && args.interactive && !args.yes {
        println!(".memoryignore already exists; keeping it unchanged");
    }

    let config_file = config_path(engine.store_path());
    let config_exists = config_file.exists();
    let mut config = load_app_config(engine.store_path())?;
    config.default_workspace = Some(workspace.clone());
    config.profile = Some(profile.to_string());
    config.mcp.read_only = true;
    config.mcp.redact_sensitive = true;
    if args.offline || args.private {
        config.embedding.provider = Some("hash".to_string());
    }
    let save_config = !config_exists
        || args.yes
        || (args.interactive && ask_yes_no("Update existing memory config?", true)?);
    if save_config {
        save_app_config(engine.store_path(), &config)?;
    }

    let detections = setup_detections(&cwd);
    let report = json!({
        "profile": profile,
        "workspace": workspace,
        "database": engine.store_path(),
        "config": config_file,
        "config_updated": save_config,
        "memoryignore": ignore_path,
        "detections": detections,
        "next_commands": [
            "memory dev morning",
            "memory dev context --for codex",
            "memory show-map"
        ],
    });

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Welcome to memory.cpp");
        println!("Profile: {profile}");
        println!(
            "Workspace: {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        println!("Database: {}", engine.store_path().display());
        println!("Saved locally. Not uploaded anywhere.");
        if !save_config {
            println!("Existing config kept unchanged. Re-run with --yes to update it.");
        }
        if !args.minimal {
            println!(
                "Safety: .memoryignore is ready at {}",
                ignore_path.display()
            );
        }
        println!("Detected:");
        for (key, value) in detections.as_object().into_iter().flatten() {
            println!("  - {key}: {}", value.as_str().unwrap_or("unknown"));
        }
        println!("Recommended setup:");
        println!("  - terminal memory: opt-in with memory terminal enable");
        println!("  - git watch: try memory git watch --once --dry-run");
        println!("  - AI context: try memory dev context --for codex");
        println!("Next three commands:");
        println!("1. memory dev morning");
        println!("2. memory dev context --for codex");
        println!("3. memory show-map");
        println!("Delete/reset later: memory privacy purge --yes");
    }
    Ok(())
}

fn setup_detections(root: &Path) -> Value {
    let package_manager = if root.join("Cargo.toml").exists() {
        "cargo"
    } else if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("package-lock.json").exists() {
        "npm"
    } else {
        "unknown"
    };
    let language = if root.join("Cargo.toml").exists() {
        "rust"
    } else if root.join("package.json").exists() {
        "javascript"
    } else if root.join("pyproject.toml").exists() {
        "python"
    } else if root.join("go.mod").exists() {
        "go"
    } else {
        "unknown"
    };
    let test_command = infer_test_command(root).unwrap_or_else(|| "unknown".to_string());
    let build_command = infer_build_command(root).unwrap_or_else(|| "unknown".to_string());
    json!({
        "git_repo": if git_repo_root(root).is_some() { "yes" } else { "no" },
        "cursor": if root.join(".cursor").exists() { "yes" } else { "no" },
        "vscode": if root.join(".vscode").exists() { "yes" } else { "no" },
        "claude": if root.join(".claude").exists() { "yes" } else { "no" },
        "ollama": if check_ollama("http://localhost:11434").unwrap_or(false) { "yes" } else { "no" },
        "package_manager": package_manager,
        "language": language,
        "test_command": test_command,
        "build_command": build_command,
        "readme": if root.join("README.md").exists() { "yes" } else { "no" },
        "memoryignore": if root.join(".memoryignore").exists() { "yes" } else { "no" },
        "memory_dir": if root.join(".memory.cpp").exists() { "yes" } else { "no" },
        "ci": if root.join(".github").join("workflows").exists() { "yes" } else { "no" },
        "docs": if root.join("docs").exists() { "yes" } else { "no" },
    })
}

fn infer_test_command(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").exists() {
        Some("cargo test".to_string())
    } else if root.join("package.json").exists() {
        Some("npm test".to_string())
    } else if root.join("pyproject.toml").exists() {
        Some("pytest".to_string())
    } else if root.join("go.mod").exists() {
        Some("go test ./...".to_string())
    } else {
        None
    }
}

fn infer_build_command(root: &Path) -> Option<String> {
    if root.join("Cargo.toml").exists() {
        Some("cargo build".to_string())
    } else if root.join("package.json").exists() {
        Some("npm run build".to_string())
    } else if root.join("go.mod").exists() {
        Some("go build ./...".to_string())
    } else {
        None
    }
}

fn ask_yes_no(prompt: &str, default_yes: bool) -> Result<bool> {
    print!("{} [{}] ", prompt, if default_yes { "Y/n" } else { "y/N" });
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let trimmed = line.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(trimmed.as_str(), "y" | "yes"))
}

fn tutorial_command(engine: &MemoryEngine, command: &Option<TutorialCommand>) -> Result<()> {
    match command {
        Some(TutorialCommand::Start { workspace, json }) => {
            let cards = json!([
                { "step": 1, "title": "Find the memory", "command": "memory search SQLite --profile decision" },
                { "step": 2, "title": "Approve a candidate", "command": "memory inbox explain <id>" },
                { "step": 3, "title": "Generate your first map", "command": "memory show-map" },
                { "step": 4, "title": "Create an AI context pack", "command": "memory dev context --for codex" },
                { "step": 5, "title": "Recover a forgotten command", "command": "memory terminal search \"test\"" },
                { "step": 6, "title": "Fix a fake CI failure", "command": "memory ci explain-failure" }
            ]);
            if *json {
                println!("{}", serde_json::to_string_pretty(&cards)?);
            } else {
                println!("memory.cpp tutorial");
                println!("Workspace: {}", workspace.as_deref().unwrap_or("current"));
                for card in cards.as_array().cloned().unwrap_or_default() {
                    println!(
                        "{}. {} -> {}",
                        card["step"].as_u64().unwrap_or(0),
                        card["title"].as_str().unwrap_or("step"),
                        card["command"].as_str().unwrap_or("memory help-me")
                    );
                }
                println!("Completion badge: local memory scout");
            }
        }
        None => {
            let _ = engine;
            println!("Start the tutorial with `memory tutorial start`.");
        }
    }
    Ok(())
}

fn what_command(engine: &MemoryEngine) -> Result<()> {
    println!(
        "memory.cpp helps your repo remember what happened, why it changed, and what to do next."
    );
    println!("- watches useful repo activity when you ask it to");
    println!("- remembers decisions, errors, commands, and context");
    println!("- asks before saving uncertain or important candidates");
    println!("- helps you resume work later with dev morning/resume");
    println!("- generates project maps and AI context packs");
    println!(
        "- stores local project memory in {}",
        engine.store_path().display()
    );
    println!("Nothing is uploaded by default.");
    println!("Next: memory setup --developer");
    Ok(())
}

fn where_command(engine: &MemoryEngine) -> Result<()> {
    let db = engine.store_path();
    let base = db.parent().unwrap_or_else(|| Path::new(".memory.cpp"));
    println!("memory.cpp local paths");
    println!("database: {}", db.display());
    println!("config: {}", config_path(db).display());
    println!("runtime: {}", base.join("runtime").display());
    println!("audit: {}", base.join("audit").display());
    println!("logs: {}", base.join("runtime").display());
    println!("terminal: {}", base.join("terminal").display());
    println!("maps: {}", base.join("demo").display());
    println!(
        ".memoryignore: {}",
        env::current_dir()?.join(".memoryignore").display()
    );
    println!("delete everything: memory privacy purge --yes");
    Ok(())
}

fn day_recap_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    days_ago: i64,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    let scope = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .or_else(|| {
            load_app_config(engine.store_path())
                .ok()
                .and_then(|config| config.default_workspace)
        });
    let start = Utc::now() - ChronoDuration::days(days_ago + 1);
    let end = Utc::now() - ChronoDuration::days(days_ago);
    let mut events = engine.timeline(scope.as_deref(), None, 80)?;
    events.retain(|event| event.created_at >= start && event.created_at <= end);
    let event_count = events.len();
    let cwd = env::current_dir()?;
    let repo = git_repo_root(&cwd);
    let branch = repo
        .as_ref()
        .and_then(|root| git_stdout(root, &["branch", "--show-current"]).ok())
        .unwrap_or_else(|| "not a git repo".to_string());
    let dirty = repo
        .as_ref()
        .map(|root| repo_status_report(root)["dirty_count"].clone())
        .unwrap_or_else(|| json!(0));
    let recent_commands = read_terminal_entries(engine, 5).unwrap_or_default();
    let pending = engine
        .inbox(scope.as_deref(), Some("pending"))
        .unwrap_or_default()
        .len();
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let latest_map = newest_file(&[base.join("demo"), base.to_path_buf()], "html");
    let report = json!({
        "workspace": scope,
        "branch": branch,
        "uncommitted_changes": dirty,
        "events": events,
        "recent_commands": recent_commands,
        "pending_candidates": pending,
        "latest_map": latest_map,
        "next_command": "memory dev next",
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!(
        "{} recap",
        if days_ago == 0 { "today" } else { "yesterday" }
    );
    println!(
        "workspace: {}",
        report["workspace"].as_str().unwrap_or("default")
    );
    println!("branch: {}", report["branch"].as_str().unwrap_or("unknown"));
    println!("uncommitted changes: {}", report["uncommitted_changes"]);
    println!("pending candidates: {pending}");
    if event_count == 0 {
        println!("No local memory events found. Nothing scary happened.");
        println!("Try: memory dev morning");
    } else {
        println!("recent memory events:");
        for event in engine
            .timeline(scope.as_deref(), None, 80)?
            .into_iter()
            .take(if verbose { 12 } else { 5 })
        {
            println!("- {} ({})", event.body, event.event_type);
        }
    }
    if verbose && !recent_commands.is_empty() {
        println!("recent terminal commands:");
        for entry in recent_commands {
            println!("- [{}] {}", entry.exit_code, entry.command);
        }
    }
    println!(
        "last map: {}",
        report["latest_map"].as_str().unwrap_or("not generated yet")
    );
    println!("next: memory dev next");
    Ok(())
}

fn open_command(args: &ManualOpenCli) -> Result<()> {
    let target = args
        .print_target
        .as_deref()
        .or(args.target.as_deref())
        .unwrap_or("dashboard");
    let value = open_target_value(target, args.host.as_str(), args.port)?;
    if args.print_target.is_some() {
        println!("{value}");
        return Ok(());
    }
    if open_with_os(&value).is_ok() {
        println!("opened {target}: {value}");
    } else {
        println!("{target}: {value}");
        println!("Could not open automatically in this environment.");
    }
    if target == "dashboard" {
        println!("If it is not running yet: memory start");
    }
    Ok(())
}

fn open_target_value(target: &str, host: &str, port: u16) -> Result<String> {
    let cwd = env::current_dir()?;
    let value = match target {
        "dashboard" => format!("http://{host}:{port}/"),
        "map" => newest_file(
            &[
                cwd.join(".memory.cpp").join("demo"),
                cwd.join(".memory.cpp"),
            ],
            "html",
        )
        .unwrap_or_else(|| {
            cwd.join(".memory.cpp/demo/evolution.html")
                .display()
                .to_string()
        }),
        "docs" => cwd.join("docs/quickstart.md").display().to_string(),
        "privacy" => cwd.join("docs/privacy.md").display().to_string(),
        "folder" => cwd.join(".memory.cpp").display().to_string(),
        "website" => cwd.join("website/index.html").display().to_string(),
        other => other.to_string(),
    };
    Ok(value)
}

fn open_with_os(target: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        ProcessCommand::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map(|_| ())
            .context("failed to open target")
    }
    #[cfg(target_os = "macos")]
    {
        ProcessCommand::new("open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .context("failed to open target")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        ProcessCommand::new("xdg-open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .context("failed to open target")
    }
}

fn clean_command(engine: &MemoryEngine) -> Result<()> {
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let runtime = base.join("runtime");
    let mut removed = 0usize;
    if runtime.exists() {
        for file in runtime_state_files(&runtime).unwrap_or_default() {
            if fs::remove_file(file).is_ok() {
                removed += 1;
            }
        }
    }
    println!("Cleaned {removed} safe runtime state file(s).");
    println!("Durable memories were not touched.");
    Ok(())
}

fn help_me_command(engine: &MemoryEngine) -> Result<()> {
    println!("Here is the shortest path:");
    println!("1. memory what");
    println!("2. memory where");
    println!("3. memory dev morning");
    println!("4. memory doctor");
    println!("Store: {}", engine.store_path().display());
    Ok(())
}

fn explain_this_command(command: &str) -> Result<()> {
    let command = command.trim();
    if command.is_empty() {
        println!("Tell me a command, for example: memory explain-this \"memory dev morning\"");
        return Ok(());
    }
    let explanation = if command.contains("dev morning") {
        "Shows what changed recently, what broke, open TODOs, and the next command."
    } else if command.contains("dev context") {
        "Builds a clean repo context block for an AI assistant."
    } else if command.contains("map") {
        "Builds a local project map from memories, citations, and optional Git signals."
    } else if command.contains("doctor") {
        "Checks local setup, safety defaults, ports, and integration config."
    } else if command.contains("privacy") {
        "Shows or deletes local memory data."
    } else {
        "This looks like a memory.cpp command. Run it with --help or try memory help-me."
    };
    println!("{command}");
    println!("{explanation}");
    Ok(())
}

fn show_map_command(engine: &MemoryEngine, workspace: Option<&String>, save: &Path) -> Result<()> {
    map_command(
        engine,
        Some(&env::current_dir()?),
        None,
        workspace,
        CliMapType::Evolution,
        CliMapOutput::Html,
        None,
        None,
        true,
        false,
        None,
        None,
        None,
        Some(save),
    )?;
    println!("saved locally: {}", save.display());
    println!("not uploaded anywhere");
    Ok(())
}

fn privacy_command(engine: &MemoryEngine, command: &Option<PrivacyCommand>) -> Result<()> {
    match command {
        Some(PrivacyCommand::Status { json }) => {
            let base = engine
                .store_path()
                .parent()
                .unwrap_or_else(|| Path::new(".memory.cpp"));
            let report = json!({
                "database": engine.store_path(),
                "config": config_path(engine.store_path()),
                "local_first": true,
                "mcp_read_only_default": load_app_config(engine.store_path()).unwrap_or_default().mcp.read_only,
                "redaction_default": load_app_config(engine.store_path()).unwrap_or_default().mcp.redact_sensitive,
                "cloud_used": false,
                "memoryignore": env::current_dir()?.join(".memoryignore"),
                "audit": base.join("audit"),
                "terminal": base.join("terminal"),
                "terminal_paused": terminal_paused(engine).unwrap_or(false),
                "git_watch_state": base.join("git-watch").join("state.json"),
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("privacy status");
                println!("local-only database: {}", engine.store_path().display());
                println!("MCP read-only by default: yes");
                println!("redaction default: yes");
                println!("cloud upload: no");
                println!(
                    "terminal memory paused: {}",
                    report["terminal_paused"].as_bool().unwrap_or(false)
                );
                println!(
                    ".memoryignore: {}",
                    report["memoryignore"].as_str().unwrap_or("")
                );
                println!("delete everything: memory privacy purge --yes");
            }
        }
        Some(PrivacyCommand::Explain) | None => {
            println!("memory.cpp stores data locally by default.");
            println!("It does not upload your repo.");
            println!("Terminal memory is opt-in.");
            println!("MCP is read-only unless you pass --allow-writes.");
            println!("Use .memoryignore to keep files out of imports and watch flows.");
        }
        Some(PrivacyCommand::Purge { yes }) | Some(PrivacyCommand::Reset { yes }) => {
            if !yes {
                println!("Refusing to purge without --yes.");
                println!("Run: memory privacy purge --yes");
                return Ok(());
            }
            let base = engine
                .store_path()
                .parent()
                .unwrap_or_else(|| Path::new(".memory.cpp"))
                .to_path_buf();
            println!("Purging local memory files under {}", base.display());
            println!("If Windows keeps the open database locked, close running memory processes and remove the folder manually.");
            if base.exists() {
                match fs::remove_dir_all(&base) {
                    Ok(()) => println!("purged {}", base.display()),
                    Err(err) => println!("could not remove {}: {err}", base.display()),
                }
            }
        }
        Some(PrivacyCommand::Export { output }) => {
            export_command(engine, None, &ExportFormat::Jsonl, output)?;
            println!("exported local memories to {}", output.display());
        }
        Some(PrivacyCommand::Receipts { json }) => {
            let base = engine
                .store_path()
                .parent()
                .unwrap_or_else(|| Path::new(".memory.cpp"));
            let receipts = json!({
                "mcp_audit_log": base.join("audit").join("mcp-access.jsonl"),
                "terminal_log": base.join("terminal").join("commands.jsonl"),
                "candidate_review": "memory inbox list",
                "local_only": true,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&receipts)?);
            } else {
                println!("privacy receipts");
                println!(
                    "MCP audit log: {}",
                    base.join("audit").join("mcp-access.jsonl").display()
                );
                println!(
                    "terminal log: {}",
                    base.join("terminal").join("commands.jsonl").display()
                );
                println!("candidate review: memory inbox list");
                println!("cloud upload: no");
            }
        }
    }
    Ok(())
}

fn explain_or_topic_command(engine: &MemoryEngine, args: &ManualExplainCli) -> Result<()> {
    let topic = args.query.join(" ").trim().to_ascii_lowercase();
    if let Some(explanation) = beginner_explanation(&topic) {
        if args.json {
            println!("{}", serde_json::to_string_pretty(&explanation)?);
        } else {
            println!("{}", explanation["title"].as_str().unwrap_or("memory.cpp"));
            println!(
                "what it means: {}",
                explanation["meaning"].as_str().unwrap_or("")
            );
            println!("why useful: {}", explanation["why"].as_str().unwrap_or(""));
            println!(
                "local by default: {}",
                explanation["local"].as_str().unwrap_or("yes")
            );
            println!(
                "try: {}",
                explanation["command"].as_str().unwrap_or("memory what")
            );
        }
        return Ok(());
    }
    if !topic.is_empty() && args.workspace.is_none() && !args.last && args.query.len() <= 3 {
        let suggestions = suggest_explain_topics(&topic);
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "topic": topic,
                    "known": false,
                    "suggestions": suggestions,
                    "examples": ["memory explain memory", "memory explain candidate", "memory explain dev context"]
                }))?
            );
        } else {
            println!("I do not have a beginner card for '{topic}' yet.");
            if !suggestions.is_empty() {
                println!("nearby topics: {}", suggestions.join(", "));
            }
            println!("examples: memory explain memory | memory explain candidate | memory explain dev context");
        }
        return Ok(());
    }
    explain_command(
        engine,
        &args.query,
        args.workspace.as_ref(),
        args.limit,
        args.last,
        args.json,
    )
}

fn beginner_explanation(topic: &str) -> Option<Value> {
    let normalized = topic.trim().replace('-', " ");
    let (title, meaning, why, command) = match normalized.as_str() {
        "" | "memory" | "memory.cpp" => (
            "memory.cpp",
            "A local repo memory tool that stores useful decisions, fixes, commands, and context.",
            "It helps you resume work and gives AI assistants better project context.",
            "memory what",
        ),
        "workspace" => (
            "workspace",
            "A named scope for memories, usually one repo or project.",
            "It keeps project memory separate from other projects.",
            "memory workspace current",
        ),
        "candidate" | "inbox candidate" => (
            "candidate",
            "A suggested memory waiting for review.",
            "It lets memory.cpp be helpful without silently storing uncertain facts.",
            "memory inbox stats",
        ),
        "inbox" => (
            "inbox",
            "The review queue for candidate memories.",
            "You approve useful memories and reject noisy or sensitive ones.",
            "memory show-inbox",
        ),
        "provenance" => (
            "provenance",
            "The source trail for a memory: file, commit, command, chat, or importer.",
            "It makes memory explainable instead of magical.",
            "memory explain why SQLite --workspace demo",
        ),
        "map" | "memory map" => (
            "map",
            "A visual project story built from memories, citations, and optional Git signals.",
            "It shows what changed, why it changed, and what depends on it.",
            "memory show-map",
        ),
        "context" | "context pack" => (
            "context pack",
            "A short briefing generated for Cursor, Codex, Claude, or another assistant.",
            "It gives AI tools repo facts without pasting the whole project.",
            "memory dev context --for codex",
        ),
        "git watch" => (
            "git watch",
            "A local observer for branch and commit changes.",
            "It turns meaningful Git activity into candidate memories.",
            "memory git watch --once --dry-run",
        ),
        "terminal memory" | "terminal" => (
            "terminal memory",
            "Opt-in command history for this repo.",
            "It remembers how you ran tests, builds, servers, and fixes.",
            "memory terminal enable",
        ),
        "doctor" => (
            "doctor",
            "A setup checker for database, workspace, privacy, Git, maps, and integrations.",
            "It gives exact fix commands instead of vague errors.",
            "memory doctor",
        ),
        "privacy" => (
            "privacy",
            "The local-first safety surface: paths, redaction, purge, and MCP write policy.",
            "It shows what is stored and how to delete it.",
            "memory privacy status",
        ),
        "mcp" => (
            "MCP",
            "A protocol that lets coding assistants call memory tools.",
            "memory.cpp defaults MCP to read-only and redacted.",
            "memory attach cursor",
        ),
        "proxy" => (
            "proxy",
            "A local OpenAI-compatible proxy that can inject memory context.",
            "It lets local model workflows benefit from project memory.",
            "memory proxy --learn --approval-required",
        ),
        "embeddings" | "embedding" | "semantic search" => (
            "embeddings",
            "Small numeric vectors used for semantic recall.",
            "They help find related memories even when wording differs.",
            "memory embeddings status",
        ),
        "dev morning" => (
            "dev morning",
            "A daily recap command for where you left off and what to do next.",
            "It is the everyday habit command.",
            "memory dev morning",
        ),
        "dev resume" => (
            "dev resume",
            "A command that reconstructs interrupted work from memory, Git, TODOs, and commands.",
            "It helps after context switches and weekends.",
            "memory dev resume",
        ),
        "dev context" => (
            "dev context",
            "A repo briefing for AI coding tools.",
            "It makes assistants more accurate without cloud sync.",
            "memory dev context --for cursor",
        ),
        "ci" => (
            "CI memory",
            "Lightweight memory for imported CI failures and previous fixes.",
            "It helps you remember how a build failed before.",
            "memory ci ingest ./ci.log",
        ),
        "redaction" => (
            "redaction",
            "Replacing detected secrets with safe placeholders before recall or preview.",
            "It reduces the chance of exposing tokens or private keys.",
            "memory redact test .env",
        ),
        ".memoryignore" | "memoryignore" => (
            ".memoryignore",
            "A local ignore file for memory import and watch flows.",
            "It keeps secrets and noisy folders out of memory.",
            "memory ignore init",
        ),
        _ => return None,
    };
    Some(json!({
        "title": title,
        "meaning": meaning,
        "why": why,
        "local": "yes",
        "command": command,
    }))
}

fn suggest_explain_topics(topic: &str) -> Vec<&'static str> {
    let topics = [
        "memory",
        "workspace",
        "candidate",
        "inbox",
        "provenance",
        "map",
        "context",
        "git watch",
        "terminal memory",
        "doctor",
        "privacy",
        "mcp",
        "proxy",
        "embeddings",
        "dev morning",
        "dev resume",
        "dev context",
        "ci",
        "redaction",
        ".memoryignore",
    ];
    topics
        .into_iter()
        .filter(|candidate| {
            candidate.contains(topic)
                || topic.contains(*candidate)
                || candidate
                    .split_whitespace()
                    .any(|part| topic.contains(part))
        })
        .take(5)
        .collect()
}

fn examples_command(area: Option<&str>, json_output: bool) -> Result<()> {
    let area = area.unwrap_or("all").to_ascii_lowercase();
    let workflows = example_workflows();
    let selected = workflows
        .iter()
        .filter(|workflow| area == "all" || workflow["area"].as_str() == Some(area.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if json_output {
        println!("{}", serde_json::to_string_pretty(&selected)?);
    } else {
        let selected = if selected.is_empty() {
            workflows
                .iter()
                .filter(|workflow| workflow["area"].as_str() == Some("dev"))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            selected
        };
        for workflow in selected {
            println!("{}", workflow["title"].as_str().unwrap_or("workflow"));
            for command in workflow["commands"].as_array().into_iter().flatten() {
                println!("  {}", command.as_str().unwrap_or(""));
            }
            println!();
        }
    }
    Ok(())
}

fn example_workflows() -> Vec<Value> {
    vec![
        json!({"area": "dev", "title": "Daily developer loop", "commands": ["memory setup --developer", "memory dev morning", "memory dev next", "memory show-map"]}),
        json!({"area": "ai", "title": "Give an assistant repo context", "commands": ["memory dev explain-repo", "memory dev context --for cursor", "memory dev context --for codex"]}),
        json!({"area": "privacy", "title": "Check and reset local data", "commands": ["memory privacy status", "memory where", "memory redact test .env", "memory privacy purge --yes"]}),
        json!({"area": "map", "title": "Generate a project map", "commands": ["memory demo seed", "memory map --type evolution --output html --save .memory.cpp/demo/evolution.html", "memory map latest"]}),
        json!({"area": "terminal", "title": "Remember useful commands", "commands": ["memory terminal enable", "memory terminal record --command \"cargo test\" --exit-code 0", "memory terminal search test"]}),
        json!({"area": "git", "title": "Turn Git activity into memory candidates", "commands": ["memory git summary --since 7d", "memory git watch --once --dry-run", "memory git ingest --dry-run"]}),
        json!({"area": "ci", "title": "Recall a CI failure", "commands": ["memory ci ingest ./ci.log", "memory ci explain-failure", "memory ci health"]}),
    ]
}

fn product_status_command(
    engine: &MemoryEngine,
    options: &EngineOptions,
    json_output: bool,
    verbose: bool,
) -> Result<()> {
    let stats = engine.stats()?;
    let workspace = current_workspace_name(engine)?.unwrap_or_else(|| "default".to_string());
    let pending = engine
        .inbox(Some(&workspace), Some("pending"))
        .unwrap_or_default()
        .len();
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let runtime_files = runtime_state_files(&runtime_dir(options)?).unwrap_or_default();
    let latest_map = newest_file(&[base.join("demo"), base.to_path_buf()], "html");
    let report = json!({
        "workspace": workspace,
        "database": engine.store_path(),
        "memory_count": stats.memories,
        "candidate_count": pending,
        "git_watch_state": base.join("git-watch").join("state.json").exists(),
        "terminal_memory": terminal_log_path(engine).map(|path| path.exists()).unwrap_or(false),
        "terminal_paused": terminal_paused(engine).unwrap_or(false),
        "privacy_redaction": load_app_config(engine.store_path()).unwrap_or_default().mcp.redact_sensitive,
        "runtime_state_files": runtime_files.len(),
        "ai_context_ready": stats.memories > 0,
        "last_map": latest_map,
        "next_command": if pending > 0 { "memory show-inbox" } else { "memory dev morning" },
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("memory.cpp status");
        println!(
            "workspace: {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        println!("database: {}", engine.store_path().display());
        println!("memories: {}", stats.memories);
        println!("pending candidates: {pending}");
        println!(
            "terminal memory: {}{}",
            if report["terminal_memory"].as_bool().unwrap_or(false) {
                "enabled"
            } else {
                "not enabled"
            },
            if report["terminal_paused"].as_bool().unwrap_or(false) {
                " (paused)"
            } else {
                ""
            }
        );
        println!(
            "git watch: {}",
            if report["git_watch_state"].as_bool().unwrap_or(false) {
                "baseline recorded"
            } else {
                "not started"
            }
        );
        println!("privacy/redaction: on");
        println!(
            "AI context: {}",
            if report["ai_context_ready"].as_bool().unwrap_or(false) {
                "ready"
            } else {
                "needs memories"
            }
        );
        println!(
            "last map: {}",
            report["last_map"].as_str().unwrap_or("not generated yet")
        );
        if verbose {
            println!("runtime files: {}", runtime_files.len());
            println!("embedding provider: {}", stats.embedding_model);
            println!("stale memories: {}", stats.stale_memories);
        }
        println!(
            "next: {}",
            report["next_command"]
                .as_str()
                .unwrap_or("memory dev morning")
        );
    }
    Ok(())
}

fn fix_command(
    engine: &MemoryEngine,
    options: &EngineOptions,
    apply: bool,
    json_output: bool,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let mut issues = Vec::new();
    if !base.exists() {
        issues.push(
            json!({"issue": "missing .memory.cpp directory", "fix": "memory setup --developer"}),
        );
        if apply {
            fs::create_dir_all(base)?;
        }
    }
    let ignore = cwd.join(".memoryignore");
    if !ignore.exists() {
        issues.push(json!({"issue": "missing .memoryignore", "fix": "memory ignore init"}));
        if apply {
            fs::write(&ignore, DEFAULT_MEMORYIGNORE)?;
        }
    }
    let config = config_path(engine.store_path());
    if !config.exists() {
        issues.push(json!({"issue": "missing starter config", "fix": "memory setup --developer"}));
        if apply {
            save_app_config(engine.store_path(), &AppConfig::default())?;
        }
    }
    let mut stale_pid_files = 0usize;
    for state_file in runtime_state_files(&runtime_dir(options)?).unwrap_or_default() {
        if let Ok(raw) = fs::read_to_string(&state_file) {
            if let Ok(state) = serde_json::from_str::<RuntimeState>(&raw) {
                if !pid_is_alive(state.pid).unwrap_or(false) {
                    stale_pid_files += 1;
                    issues.push(json!({"issue": format!("stale runtime state: {}", state_file.display()), "fix": "memory clean"}));
                    if apply {
                        let _ = fs::remove_file(state_file);
                    }
                }
            }
        }
    }
    let report = json!({
        "applied": apply,
        "issues": issues,
        "stale_pid_files": stale_pid_files,
        "safe_fixes_only": true,
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if issues.is_empty() {
        println!("No obvious setup issues found.");
        println!("next: memory doctor");
    } else {
        println!("memory fix found {} issue(s)", issues.len());
        for issue in issues {
            println!("- {}", issue["issue"].as_str().unwrap_or("issue"));
            println!(
                "  fix: {}",
                issue["fix"].as_str().unwrap_or("memory doctor")
            );
        }
        if !apply {
            println!("No files changed. Re-run with --apply for safe fixes.");
        }
    }
    Ok(())
}

fn redact_command(command: &RedactCommand) -> Result<()> {
    match command {
        RedactCommand::Preview { path, json } => {
            let hits = redact_preview(path)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else if hits.is_empty() {
                println!("no obvious secrets detected");
            } else {
                println!("redaction preview:");
                for hit in hits {
                    println!("- {}: {}", hit.path, hit.reason);
                    if let Some(preview) = hit.preview {
                        println!("  {preview}");
                    }
                }
            }
        }
        RedactCommand::Test { file, json } => {
            let raw = fs::read_to_string(file)
                .with_context(|| format!("failed to read {}", file.display()))?;
            let mut hits = Vec::new();
            for (line, text) in raw.lines().enumerate() {
                if let Some(reason) = detect_sensitive_reason(text) {
                    hits.push(json!({
                        "line": line + 1,
                        "reason": reason,
                        "redacted": redact_line(text),
                    }));
                }
            }
            if *json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else if hits.is_empty() {
                println!("no obvious secrets detected in {}", file.display());
            } else {
                println!("redaction test for {}", file.display());
                for hit in hits {
                    println!(
                        "- line {} {} -> {}",
                        hit["line"],
                        hit["reason"].as_str().unwrap_or("sensitive"),
                        hit["redacted"].as_str().unwrap_or("[REDACTED]")
                    );
                }
            }
        }
    }
    Ok(())
}

fn redact_preview(path: &Path) -> Result<Vec<RedactionPreviewHit>> {
    let files = if path.is_dir() {
        collect_importable_files(path, true)?
    } else {
        vec![path.to_path_buf()]
    };
    let mut hits = Vec::new();
    for file in files.into_iter().take(200) {
        let Ok(raw) = fs::read_to_string(&file) else {
            continue;
        };
        for line in raw.lines().take(2000) {
            if let Some(reason) = detect_sensitive_reason(line) {
                hits.push(RedactionPreviewHit {
                    path: file.display().to_string(),
                    reason: reason.to_string(),
                    preview: Some(redact_line(line)),
                });
                break;
            }
        }
        if hits.len() >= 64 {
            break;
        }
    }
    Ok(hits)
}

fn redact_line(line: &str) -> String {
    if detect_sensitive_reason(line).is_some() {
        "[REDACTED sensitive value]".to_string()
    } else {
        line.to_string()
    }
}

fn config_command(engine: &MemoryEngine, command: &Option<ConfigCommand>) -> Result<()> {
    if command.is_none() {
        return config_command(engine, &Some(ConfigCommand::Show { json: false }));
    }
    match command.as_ref().expect("checked above") {
        ConfigCommand::Show { json } => {
            let config = load_app_config(engine.store_path())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                println!("config: {}", config_path(engine.store_path()).display());
                println!(
                    "workspace: {}",
                    config.default_workspace.as_deref().unwrap_or("not set")
                );
                println!(
                    "profile: {}",
                    config.profile.as_deref().unwrap_or("developer")
                );
                println!("mcp read-only: {}", config.mcp.read_only);
                println!(
                    "embedding provider: {}",
                    config.embedding.provider.as_deref().unwrap_or("hash")
                );
            }
        }
        ConfigCommand::Get { key } => {
            let value = config_get(engine, key)?;
            println!("{value}");
        }
        ConfigCommand::Set { key, value } => {
            config_set(engine, key, value)?;
            println!("config set: {key}={value}");
        }
        ConfigCommand::Edit => {
            println!("{}", config_path(engine.store_path()).display());
            println!("Open this file in your editor, then run: memory config doctor");
        }
        ConfigCommand::Doctor { json } => {
            let config = load_app_config(engine.store_path())?;
            let checks = json!({
                "has_workspace": config.default_workspace.is_some(),
                "mcp_read_only": config.mcp.read_only,
                "redaction": config.mcp.redact_sensitive,
                "provider": config.embedding.provider.unwrap_or_else(|| "hash".to_string()),
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&checks)?);
            } else {
                println!("config doctor");
                println!("workspace set: {}", checks["has_workspace"]);
                println!("MCP read-only: {}", checks["mcp_read_only"]);
                println!("redaction: {}", checks["redaction"]);
                println!(
                    "embedding provider: {}",
                    checks["provider"].as_str().unwrap_or("hash")
                );
            }
        }
        ConfigCommand::Reset { yes } => {
            if !*yes {
                println!("Run memory config reset --yes to replace local config with defaults.");
                return Ok(());
            }
            save_app_config(engine.store_path(), &AppConfig::default())?;
            println!("config reset");
        }
        ConfigCommand::Export { output } => {
            fs::copy(config_path(engine.store_path()), output)?;
            println!("exported config to {}", output.display());
        }
        ConfigCommand::Import { input } => {
            let config: AppConfig = serde_json::from_str(&fs::read_to_string(input)?)?;
            save_app_config(engine.store_path(), &config)?;
            println!("imported config from {}", input.display());
        }
        ConfigCommand::Path => {
            println!("{}", config_path(engine.store_path()).display());
        }
        ConfigCommand::Profiles => {
            println!("available profiles:");
            for profile in [
                "beginner",
                "developer",
                "ai-coding",
                "private",
                "offline",
                "low-ram",
                "power-user",
            ] {
                println!("  - {profile}");
            }
            println!("set one with: memory config set profile developer");
        }
    }
    Ok(())
}

fn config_get(engine: &MemoryEngine, key: &str) -> Result<String> {
    let config = load_app_config(engine.store_path())?;
    let value = match key {
        "workspace" | "default_workspace" => config.default_workspace.unwrap_or_default(),
        "profile" => config.profile.unwrap_or_else(|| "developer".to_string()),
        "mcp.read_only" => config.mcp.read_only.to_string(),
        "mcp.redact_sensitive" => config.mcp.redact_sensitive.to_string(),
        "embedding.provider" => config
            .embedding
            .provider
            .unwrap_or_else(|| "hash".to_string()),
        "embedding.model" => config.embedding.model.unwrap_or_default(),
        "embedding.endpoint" => config.embedding.endpoint.unwrap_or_default(),
        other => return Err(anyhow!("unknown config key: {other}")),
    };
    Ok(value)
}

fn config_set(engine: &MemoryEngine, key: &str, value: &str) -> Result<()> {
    let mut config = load_app_config(engine.store_path())?;
    match key {
        "workspace" | "default_workspace" => config.default_workspace = Some(value.to_string()),
        "profile" => config.profile = Some(value.to_string()),
        "mcp.read_only" => config.mcp.read_only = parse_bool(value)?,
        "mcp.redact_sensitive" => config.mcp.redact_sensitive = parse_bool(value)?,
        "embedding.provider" => config.embedding.provider = Some(value.to_string()),
        "embedding.model" => config.embedding.model = Some(value.to_string()),
        "embedding.endpoint" => config.embedding.endpoint = Some(value.to_string()),
        other => return Err(anyhow!("unknown config key: {other}")),
    }
    save_app_config(engine.store_path(), &config)
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        other => Err(anyhow!("expected boolean value, got {other}")),
    }
}

fn map_latest_command(engine: &MemoryEngine, open: bool) -> Result<()> {
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let Some(path) = newest_file(&[base.join("demo"), base.to_path_buf()], "html") else {
        println!("no generated HTML map found yet");
        println!("try: memory show-map");
        return Ok(());
    };
    println!("{path}");
    if open {
        let _ = open_with_os(&path);
    } else {
        println!("open it with: memory map open");
    }
    Ok(())
}

fn map_status_command(engine: &MemoryEngine) -> Result<()> {
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let latest_html = newest_file(
        &[base.join("demo"), base.join("maps"), base.to_path_buf()],
        "html",
    );
    let latest_md = newest_file(&[base.join("maps"), base.to_path_buf()], "md");
    println!("map status");
    println!("latest html: {}", latest_html.as_deref().unwrap_or("none"));
    println!(
        "latest markdown: {}",
        latest_md.as_deref().unwrap_or("none")
    );
    println!("refresh: memory map refresh");
    Ok(())
}

fn map_refresh_command(engine: &MemoryEngine) -> Result<()> {
    let save = PathBuf::from(".memory.cpp/maps/evolution.html");
    map_command(
        engine,
        None,
        None,
        None,
        CliMapType::Evolution,
        CliMapOutput::Html,
        None,
        None,
        true,
        false,
        None,
        None,
        None,
        Some(&save),
    )
}

fn map_export_markdown_command(engine: &MemoryEngine, title: &str, save: &str) -> Result<()> {
    let save = PathBuf::from(save);
    map_command(
        engine,
        None,
        Some(&title.to_string()),
        None,
        CliMapType::Evolution,
        CliMapOutput::Markdown,
        None,
        None,
        true,
        true,
        None,
        None,
        None,
        Some(&save),
    )
}

fn map_changed_command(engine: &MemoryEngine, rest: &[String]) -> Result<()> {
    let since = rest
        .windows(2)
        .find(|pair| pair[0] == "--since")
        .map(|pair| pair[1].as_str())
        .unwrap_or("7d");
    println!("map changes since {since}");
    map_command(
        engine,
        None,
        None,
        None,
        CliMapType::Evolution,
        CliMapOutput::Markdown,
        None,
        None,
        true,
        true,
        None,
        None,
        None,
        None,
    )
}

fn newest_file(dirs: &[PathBuf], extension: &str) -> Option<String> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
            {
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                    newest = Some((modified, path));
                }
            }
        }
    }
    newest.map(|(_, path)| path.display().to_string())
}

#[allow(clippy::too_many_arguments)]
fn attach_command(
    engine: &MemoryEngine,
    target: &AttachTarget,
    host: &str,
    port: u16,
    upstream: &str,
    start_proxy: bool,
    workspace: Option<&String>,
    dry_run: bool,
    _yes: bool,
    print_config: bool,
) -> Result<()> {
    let exe = env::current_exe().context("could not locate current memory executable")?;
    let db = engine
        .store_path()
        .canonicalize()
        .unwrap_or_else(|_| engine.store_path().to_path_buf());
    let root = env::current_dir()?;
    let scoped_workspace = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .or(load_app_config(engine.store_path())?.mcp.workspace);
    if !dry_run && !print_config {
        let mut app_config = load_app_config(engine.store_path())?;
        app_config.mcp.workspace = scoped_workspace.clone();
        app_config.mcp.read_only = true;
        app_config.mcp.redact_sensitive = true;
        if app_config.mcp.audit_log.is_none() {
            app_config.mcp.audit_log = Some(
                engine
                    .store_path()
                    .parent()
                    .unwrap_or_else(|| Path::new(".memory.cpp"))
                    .join("audit")
                    .join("mcp-access.jsonl")
                    .display()
                    .to_string(),
            );
        }
        save_app_config(engine.store_path(), &app_config)?;
    }

    for target in expand_attach_targets(target) {
        if matches!(target, AttachTarget::Ollama) {
            let path = attach_config_path(&root, &target)?;
            let proxy_info = json!({
                "base_url": format!("http://{host}:7332/v1"),
                "upstream": upstream,
                "db": db,
                "workspace": scoped_workspace.clone(),
                "note": "Start explicitly with `memory proxy`; attach does not auto-run long-lived services unless --start-proxy is passed.",
            });
            emit_attach_plan(&target, &path, &proxy_info, dry_run, print_config)?;
            if !dry_run && !print_config {
                write_json_with_backup(&path, &proxy_info)?;
                if start_proxy {
                    let mut child = ProcessCommand::new(&exe);
                    child.args([
                        "--db",
                        &db.to_string_lossy(),
                        "proxy",
                        "--listen",
                        &format!("{host}:7332"),
                        "--upstream",
                        upstream,
                        "--learn",
                        "--approval-required",
                    ]);
                    if let Some(workspace) = &scoped_workspace {
                        child.args(["--workspace", workspace.as_str()]);
                    }
                    let _child = child.spawn().context("failed to start background proxy")?;
                    println!("started proxy on http://{}:7332/v1", host);
                }
            }
            continue;
        }

        let config = build_attach_config(&exe, &db, scoped_workspace.as_ref());
        let path = attach_config_path(&root, &target)?;
        emit_attach_plan(&target, &path, &config, dry_run, print_config)?;
        if !dry_run && !print_config {
            write_json_with_backup(&path, &config)?;
        }
    }

    if print_config {
        return Ok(());
    }

    if let Some(workspace) = scoped_workspace {
        println!("workspace scope: {workspace}");
    }
    println!("health endpoint: http://{}:{}/health", host, port);
    println!("MCP safety: read-only tools are enabled by default; memory writes require explicit approval.");
    Ok(())
}

fn expand_attach_targets(target: &AttachTarget) -> Vec<AttachTarget> {
    match target {
        AttachTarget::All => vec![
            AttachTarget::Cursor,
            AttachTarget::Claude,
            AttachTarget::Vscode,
            AttachTarget::Codex,
            AttachTarget::Continue,
            AttachTarget::Ollama,
        ],
        other => vec![other.clone()],
    }
}

fn attach_config_path(root: &Path, target: &AttachTarget) -> Result<PathBuf> {
    let path = match target {
        AttachTarget::Cursor => root.join(".cursor").join("mcp.json"),
        AttachTarget::Vscode => root.join(".vscode").join("mcp.json"),
        AttachTarget::Codex => root.join(".codex").join("mcp.json"),
        AttachTarget::Claude => root.join(".claude").join("claude_desktop_config.json"),
        AttachTarget::Continue => root.join(".continue").join("mcp.json"),
        AttachTarget::Ollama => root
            .join(".memory.cpp")
            .join("attach")
            .join("ollama-proxy.json"),
        AttachTarget::All => return Err(anyhow!("all expands to concrete attach targets")),
    };
    Ok(path)
}

fn build_attach_config(exe: &Path, db: &Path, workspace: Option<&String>) -> Value {
    let mut args = vec![
        "--db".to_string(),
        db.to_string_lossy().to_string(),
        "mcp".to_string(),
    ];
    if let Some(workspace) = workspace {
        args.push("--workspace".to_string());
        args.push(workspace.clone());
    }
    json!({
        "mcpServers": {
            "memory-cpp": {
                "command": exe,
                "args": args,
                "description": "Read-only local repo memory context for AI coding tools"
            }
        }
    })
}

fn emit_attach_plan(
    target: &AttachTarget,
    path: &Path,
    config: &Value,
    dry_run: bool,
    print_config: bool,
) -> Result<()> {
    if print_config {
        println!("{}", serde_json::to_string_pretty(config)?);
        return Ok(());
    }
    if dry_run {
        println!("would attach {:?} at {}", target, path.display());
    } else {
        println!("attached {:?} using {}", target, path.display());
    }
    println!("undo: memory detach {:?}", target);
    Ok(())
}

fn write_json_with_backup(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let backup = path.with_extension(format!("{}.bak", Utc::now().format("%Y%m%d%H%M%S")));
        fs::copy(path, &backup)?;
        println!("backup: {}", backup.display());
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn detach_command(target: &AttachTarget, dry_run: bool, _yes: bool) -> Result<()> {
    let root = env::current_dir()?;
    for target in expand_attach_targets(target) {
        let path = attach_config_path(&root, &target)?;
        if dry_run {
            println!("would detach {:?} from {}", target, path.display());
            continue;
        }
        if !path.exists() {
            println!("{:?} is not attached at {}", target, path.display());
            continue;
        }
        let backup = path.with_extension(format!(
            "{}.detached.bak",
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        fs::copy(&path, &backup)?;
        fs::remove_file(&path)?;
        println!("detached {:?}; backup kept at {}", target, backup.display());
    }
    Ok(())
}

fn attach_status_command(engine: &MemoryEngine, json_output: bool) -> Result<()> {
    let root = env::current_dir()?;
    let targets = expand_attach_targets(&AttachTarget::All)
        .into_iter()
        .map(|target| {
            let path = attach_config_path(&root, &target).unwrap_or_default();
            json!({
                "target": format!("{target:?}").to_ascii_lowercase(),
                "path": path,
                "attached": path.exists(),
            })
        })
        .collect::<Vec<_>>();
    let config = load_app_config(engine.store_path())?;
    let report = json!({
        "read_only": config.mcp.read_only,
        "redaction": config.mcp.redact_sensitive,
        "targets": targets,
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("attach status");
        println!("MCP read-only: {}", config.mcp.read_only);
        println!("redaction: {}", config.mcp.redact_sensitive);
        for target in report["targets"].as_array().into_iter().flatten() {
            println!(
                "  - {}: {} ({})",
                target["target"].as_str().unwrap_or("target"),
                if target["attached"].as_bool().unwrap_or(false) {
                    "attached"
                } else {
                    "not attached"
                },
                target["path"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}

fn attach_doctor_command(engine: &MemoryEngine) -> Result<()> {
    attach_status_command(engine, false)?;
    println!("doctor:");
    println!("  - read-only MCP default should be true");
    println!("  - detach with: memory detach cursor --dry-run");
    println!("  - print config with: memory attach --print-config cursor");
    Ok(())
}

fn attach_list_command() -> Result<()> {
    println!("attach targets:");
    for target in [
        "cursor", "claude", "vscode", "codex", "continue", "ollama", "all",
    ] {
        println!("  - {target}");
    }
    Ok(())
}

fn public_watch_command(engine: &MemoryEngine, args: &ManualWatchCli) -> Result<()> {
    let default_action = if args.once {
        WatchAction::Once
    } else if args.foreground {
        WatchAction::Start
    } else {
        WatchAction::Status
    };
    let action = args.action.as_ref().unwrap_or(&default_action);
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let state_path = base.join("runtime").join("watch-state.json");
    let cwd = env::current_dir()?;
    let repo_root = resolve_repo_root(&cwd);

    match action {
        WatchAction::Start => {
            fs::create_dir_all(state_path.parent().unwrap_or(base))?;
            let state = json!({
                "running": true,
                "foreground": args.foreground,
                "interval": args.interval,
                "dry_run": args.dry_run,
                "updated_at": Utc::now(),
                "note": "lightweight coordinator; use --foreground or watch once for active observation"
            });
            fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
            println!("memory watch marked active");
            println!("state: {}", state_path.display());
            if args.foreground {
                if let Some(repo_root) = repo_root.as_ref() {
                    git_watch_command(
                        engine,
                        repo_root,
                        args.workspace.as_ref(),
                        args.interval,
                        args.once,
                        32,
                        args.dry_run,
                        args.json,
                    )?;
                } else {
                    println!("no git repository detected; watch will only report local status");
                }
            } else {
                println!("run foreground loop with: memory watch start --foreground");
            }
        }
        WatchAction::Stop => {
            fs::create_dir_all(state_path.parent().unwrap_or(base))?;
            fs::write(
                &state_path,
                serde_json::to_string_pretty(&json!({
                    "running": false,
                    "updated_at": Utc::now(),
                }))?,
            )?;
            println!("memory watch stopped");
        }
        WatchAction::Status => {
            let state = fs::read_to_string(&state_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .unwrap_or_else(|| json!({"running": false}));
            let git_state = repo_root.as_ref().map(|root| {
                root.join(".memory.cpp")
                    .join("git-watch")
                    .join("state.json")
            });
            let report = json!({
                "state": state,
                "state_path": state_path,
                "git_repo": repo_root,
                "git_watch_state": git_state,
                "terminal_paused": terminal_paused(engine).unwrap_or(false),
                "local_only": true,
            });
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("memory watch status");
                println!(
                    "running: {}",
                    report["state"]["running"].as_bool().unwrap_or(false)
                );
                println!("local-only: yes");
                println!("terminal paused: {}", report["terminal_paused"]);
                println!("next: memory watch once --dry-run");
            }
        }
        WatchAction::Once => {
            if let Some(repo_root) = repo_root.as_ref() {
                git_watch_command(
                    engine,
                    repo_root,
                    args.workspace.as_ref(),
                    args.interval,
                    true,
                    32,
                    args.dry_run,
                    args.json,
                )?;
            } else {
                println!("no git repository detected from {}", cwd.display());
                println!("watch once can still inspect terminal/status, but no git candidates were created");
            }
        }
        WatchAction::Pause => {
            fs::create_dir_all(state_path.parent().unwrap_or(base))?;
            fs::write(
                &state_path,
                serde_json::to_string_pretty(&json!({
                    "running": true,
                    "paused": true,
                    "updated_at": Utc::now(),
                }))?,
            )?;
            if let Some(repo_root) = repo_root.as_ref() {
                git_watch_action_command(repo_root, &GitWatchAction::Pause)?;
            }
            println!("memory watch paused");
        }
        WatchAction::Resume => {
            fs::create_dir_all(state_path.parent().unwrap_or(base))?;
            fs::write(
                &state_path,
                serde_json::to_string_pretty(&json!({
                    "running": true,
                    "paused": false,
                    "updated_at": Utc::now(),
                }))?,
            )?;
            if let Some(repo_root) = repo_root.as_ref() {
                git_watch_action_command(repo_root, &GitWatchAction::Resume)?;
            }
            println!("memory watch resumed");
        }
        WatchAction::Doctor => {
            println!("memory watch doctor");
            println!("local-only: yes");
            println!("network required: no");
            println!(
                "git repo: {}",
                repo_root
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "not detected".to_string())
            );
            println!(
                "terminal memory paused: {}",
                terminal_paused(engine).unwrap_or(false)
            );
            println!("try: memory watch once --dry-run");
        }
    }
    Ok(())
}

fn serve_command(engine: MemoryEngine, host: &str, port: u16, dashboard: bool) -> Result<()> {
    let address = format!("{host}:{port}");
    let server = Server::http(&address).map_err(|err| anyhow!(err.to_string()))?;
    println!("memory.cpp server listening on http://{address}");

    for request in server.incoming_requests() {
        if let Err(err) = handle_api_request(&engine, request, dashboard) {
            eprintln!("request error: {err:#}");
        }
    }

    Ok(())
}

fn proxy_command(
    engine: MemoryEngine,
    listen: &str,
    upstream: &str,
    workspace: Option<&String>,
    limit: usize,
    tokens: usize,
    learning: ProxyLearningConfig,
) -> Result<()> {
    let server = Server::http(listen).map_err(|err| anyhow!(err.to_string()))?;
    println!("memory.cpp proxy listening on http://{listen}");
    println!(
        "forwarding chat completions to {}",
        upstream.trim_end_matches('/')
    );
    if learning.enabled {
        println!(
            "proxy learning: enabled (min_confidence={:.2}, mode={})",
            learning.min_confidence,
            if learning.approval_required {
                "approval-required"
            } else {
                "auto-store when safe"
            }
        );
        if learning.dry_run {
            println!("proxy learning is running in dry-run mode");
        }
    } else {
        println!("proxy learning: disabled (use --learn to capture candidate memory)");
    }

    let scope = workspace
        .cloned()
        .or(current_workspace_name(&engine)?)
        .unwrap_or_else(|| "default".to_string());

    for request in server.incoming_requests() {
        if let Err(err) =
            handle_proxy_request(&engine, request, upstream, &scope, limit, tokens, &learning)
        {
            eprintln!("proxy error: {err:#}");
        }
    }

    Ok(())
}

fn mcp_command(engine: &MemoryEngine, config: &McpRuntimeConfig) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value = serde_json::from_str(&line)?;
        let response = handle_mcp_message(engine, request, config);
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }

    Ok(())
}

fn list_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let memories = engine.list_recent(workspace.map(String::as_str), limit)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else {
        for memory in memories {
            println!(
                "{} [{} | {} | {:.2} | {}] {}",
                memory.id,
                memory.kind,
                memory.scope,
                memory.importance,
                memory.attributes.status.as_str(),
                memory.summary
            );
        }
    }
    Ok(())
}

fn compact_command(engine: &MemoryEngine, workspace: Option<&String>, limit: usize) -> Result<()> {
    let memory = engine.compact_scope(&required_workspace(engine, workspace)?, limit)?;
    println!("compacted into {}", memory.id);
    Ok(())
}

fn stats_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    show_conflicts: bool,
    json_output: bool,
) -> Result<()> {
    let stats = engine.stats()?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("memories: {}", stats.memories);
    println!("workspaces: {}", stats.workspaces);
    println!("stored bytes: {}", stats.bytes);
    println!("embedding model: {}", stats.embedding_model);
    println!(
        "avg recall latency ms: {:.2}",
        stats.average_recall_latency_ms
    );
    println!("stale memories: {}", stats.stale_memories);
    println!("conflicts: {}", stats.conflicts);
    if let Some(workspace) = workspace {
        println!("current workspace query: {}", workspace);
    }
    for entity in stats.top_entities {
        println!("entity: {} [{}] {}", entity.name, entity.kind, entity.count);
    }

    if show_conflicts {
        for conflict in engine.conflicts(workspace.map(String::as_str), 10)? {
            println!(
                "conflict: {} {} -> {} ({})",
                conflict.id, conflict.old_memory_id, conflict.new_memory_id, conflict.reason
            );
        }
    }

    Ok(())
}

fn dev_command(engine: &MemoryEngine, command: &DevCommand) -> Result<()> {
    match command {
        DevCommand::Watch {
            path,
            workspace,
            interval_secs,
            chunk_chars,
            once,
        } => watch_command(
            engine,
            path,
            workspace.as_ref(),
            MemoryKind::Code,
            *interval_secs,
            *chunk_chars,
            *once,
        ),
        DevCommand::Morning {
            workspace,
            limit,
            json,
        } => dev_morning_command(engine, workspace.as_ref(), *limit, *json),
        DevCommand::Resume {
            query,
            workspace,
            limit,
            tokens,
            json,
        } => dev_resume_command(engine, query, workspace.as_ref(), *limit, *tokens, *json),
        DevCommand::ExplainRepo {
            path,
            workspace,
            json,
        } => dev_explain_repo_command(engine, path.as_deref(), workspace.as_ref(), *json),
        DevCommand::Next {
            workspace,
            limit,
            json,
        } => dev_next_command(engine, workspace.as_ref(), *limit, *json),
        DevCommand::RecallError {
            error,
            workspace,
            limit,
            json,
        } => dev_recall_error_command(engine, error, workspace.as_ref(), *limit, *json),
        DevCommand::TestFailures {
            workspace,
            limit,
            json,
        } => dev_test_failures_command(engine, workspace.as_ref(), *limit, *json),
        DevCommand::RecallTest {
            test,
            workspace,
            limit,
            json,
        } => dev_recall_test_command(engine, test, workspace.as_ref(), *limit, *json),
        DevCommand::Context {
            workspace,
            target,
            limit,
            tokens,
            verbose,
            json,
        } => dev_context_command(
            engine,
            workspace.as_ref(),
            target,
            *limit,
            *tokens,
            *verbose,
            *json,
        ),
        DevCommand::Onboard {
            workspace,
            output,
            save,
        } => dev_onboard_command(engine, workspace.as_ref(), output, save.as_deref()),
        DevCommand::ReadmeSuggest { workspace, json } => {
            dev_readme_suggest_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::Changelog {
            workspace,
            since,
            json,
        } => dev_changelog_command(engine, workspace.as_ref(), since.as_deref(), *json),
        DevCommand::Health { workspace, json } => {
            dev_health_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::PrSummary { workspace, json } => {
            dev_pr_summary_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::Review { workspace, json } => {
            dev_review_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::Evening {
            workspace,
            verbose,
            json,
        } => dev_period_command(engine, workspace.as_ref(), "evening", 0, *verbose, *json),
        DevCommand::Today {
            workspace,
            verbose,
            json,
        } => dev_period_command(engine, workspace.as_ref(), "today", 0, *verbose, *json),
        DevCommand::Yesterday {
            workspace,
            verbose,
            json,
        } => dev_period_command(engine, workspace.as_ref(), "yesterday", 1, *verbose, *json),
        DevCommand::Week {
            workspace,
            verbose,
            json,
        } => dev_week_command(engine, workspace.as_ref(), *verbose, *json),
        DevCommand::Focus { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "current task next focus",
            "focus",
            *json,
        ),
        DevCommand::Tasks { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "todo task next plan",
            "tasks",
            *json,
        ),
        DevCommand::Blockers { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "blocker blocked failing error",
            "blockers",
            *json,
        ),
        DevCommand::Risks { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "risk risky limitation debt",
            "risks",
            *json,
        ),
        DevCommand::Cleanup { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "cleanup refactor stale debt",
            "cleanup",
            *json,
        ),
        DevCommand::DocsGap { workspace, json } => {
            dev_docs_gap_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::StaleDecisions { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "stale decision old alternative",
            "stale decisions",
            *json,
        ),
        DevCommand::StaleTodos { workspace, json } => {
            dev_stale_todos_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::ChangedFiles { workspace, json } => {
            dev_changed_files_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::HotFiles { workspace, json } => {
            dev_hot_files_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::CommonErrors { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "error failed panic exception",
            "common errors",
            *json,
        ),
        DevCommand::CommonCommands { workspace, json } => {
            dev_common_commands_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::Roadmap { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "roadmap next planned future",
            "roadmap",
            *json,
        ),
        DevCommand::ReleaseNotes {
            workspace,
            since,
            json,
        } => dev_changelog_command(engine, workspace.as_ref(), since.as_deref(), *json),
        DevCommand::SetupGuide { workspace, json } => {
            dev_setup_guide_command(engine, workspace.as_ref(), *json)
        }
        DevCommand::Architecture { workspace, json } => dev_focus_query(
            engine,
            workspace.as_ref(),
            "architecture module storage command",
            "architecture",
            *json,
        ),
        DevCommand::ExplainCommand { cmd, json } => {
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "command": cmd,
                        "explanation": command_explanation(cmd),
                    }))?
                );
                Ok(())
            } else {
                println!("{cmd}");
                println!("{}", command_explanation(cmd));
                Ok(())
            }
        }
    }
}

fn dev_morning_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let since = Utc::now() - ChronoDuration::days(1);
    let recent_events = engine
        .timeline(Some(&scope), None, limit.max(8) * 4)?
        .into_iter()
        .filter(|event| event.created_at >= since)
        .collect::<Vec<_>>();
    let recent_memories = engine
        .list_recent(Some(&scope), limit.max(8))?
        .into_iter()
        .filter(|memory| memory.updated_at >= since || memory.created_at >= since)
        .collect::<Vec<_>>();
    let decisions = recent_memories
        .iter()
        .filter(|memory| matches!(memory.kind, MemoryKind::Decision))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let bug_fixes = recent_memories
        .iter()
        .filter(|memory| {
            matches!(memory.kind, MemoryKind::Bug)
                || memory.summary.to_ascii_lowercase().contains("fix")
        })
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let conflicts = engine.conflicts(Some(&scope), limit.min(10))?;
    let inbox = engine.inbox(Some(&scope), Some("pending"))?;
    let repo_root = resolve_repo_root(&env::current_dir()?);
    let repo_status = repo_root.as_deref().map(repo_status_report);
    let todos = repo_root
        .as_deref()
        .map(|root| collect_todos(root, limit.max(8)))
        .unwrap_or_default();
    let commits = repo_root
        .as_deref()
        .and_then(|root| git_commit_records(root, Some("24h"), limit).ok())
        .unwrap_or_default();
    let failed_tests = recent_memories
        .iter()
        .filter(|memory| {
            let lower = memory.summary.to_ascii_lowercase();
            lower.contains("test") && (lower.contains("fail") || lower.contains("flaky"))
        })
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let next_step = recent_memories
        .iter()
        .find(|memory| matches!(memory.kind, MemoryKind::Task | MemoryKind::Decision))
        .map(|memory| memory.summary.clone())
        .or_else(|| conflicts.first().map(|conflict| conflict.reason.clone()))
        .or_else(|| inbox.first().map(|entry| entry.reason.clone()))
        .or_else(|| {
            repo_status.as_ref().and_then(|status| {
                (status["dirty_count"].as_u64().unwrap_or(0) > 0)
                    .then(|| "Review and commit the current uncommitted repo changes.".to_string())
            })
        })
        .unwrap_or_else(|| {
            "Review the latest project decisions and consolidate any pending review memories."
                .to_string()
        });
    let next_command = if !failed_tests.is_empty() {
        "memory dev test-failures".to_string()
    } else if !inbox.is_empty() {
        "memory inbox".to_string()
    } else if repo_status
        .as_ref()
        .and_then(|status| status["dirty_count"].as_u64())
        .unwrap_or(0)
        > 0
    {
        "git diff --stat".to_string()
    } else {
        "memory dev next".to_string()
    };

    let report = json!({
        "workspace": scope,
        "since": since,
        "what_was_i_doing": recent_memories.first().map(|memory| memory.summary.clone()),
        "last_session_summary": recent_events.first().map(|event| event.body.clone()),
        "major_changes": recent_events,
        "recent_commits": commits,
        "open_todos": todos,
        "recent_decisions": decisions,
        "recent_bugs_and_fixes": bug_fixes,
        "failed_tests": failed_tests,
        "repo_status": repo_status,
        "open_conflicts": conflicts,
        "inbox": inbox,
        "suggested_next_work": next_step,
        "next_recommended_command": next_command,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "morning recap for {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        if let Some(summary) = report["what_was_i_doing"].as_str() {
            println!("what you were doing: {summary}");
        }
        if let Some(session) = report["last_session_summary"].as_str() {
            println!("last session: {session}");
        }
        println!("yesterday's major changes:");
        let changes = report["major_changes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if changes.is_empty() {
            println!("  none recorded in the last 24 hours");
        } else {
            for event in changes.iter().take(limit) {
                println!(
                    "  - {} ({})",
                    event["body"].as_str().unwrap_or("event"),
                    event["event_type"].as_str().unwrap_or("timeline")
                );
            }
        }
        println!("recent decisions:");
        let recent_decisions = report["recent_decisions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if recent_decisions.is_empty() {
            println!("  none captured");
        } else {
            for memory in recent_decisions.iter().take(limit) {
                println!("  - {}", memory["summary"].as_str().unwrap_or("decision"));
            }
        }
        println!("recent bugs/fixes:");
        let recent_bugs = report["recent_bugs_and_fixes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if recent_bugs.is_empty() {
            println!("  none captured");
        } else {
            for memory in recent_bugs.iter().take(limit) {
                println!("  - {}", memory["summary"].as_str().unwrap_or("memory"));
            }
        }
        println!(
            "open conflicts: {} | inbox: {}",
            report["open_conflicts"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0),
            report["inbox"].as_array().map(Vec::len).unwrap_or(0)
        );
        if let Some(status) = report["repo_status"].as_object() {
            println!(
                "branch: {} | uncommitted files: {}",
                status
                    .get("branch")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                status
                    .get("dirty_count")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            );
        }
        let todos = report["open_todos"].as_array().cloned().unwrap_or_default();
        if !todos.is_empty() {
            println!("open TODOs:");
            for todo in todos.iter().take(limit.min(5)) {
                println!(
                    "  - {}:{} {}",
                    todo["path"].as_str().unwrap_or("file"),
                    todo["line"].as_u64().unwrap_or(0),
                    todo["text"].as_str().unwrap_or("")
                );
            }
        }
        let failed_tests = report["failed_tests"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !failed_tests.is_empty() {
            println!("failed/flaky tests:");
            for memory in failed_tests.iter().take(limit.min(5)) {
                println!(
                    "  - {}",
                    memory["summary"].as_str().unwrap_or("test failure")
                );
            }
        }
        println!(
            "suggested next work: {}",
            report["suggested_next_work"]
                .as_str()
                .unwrap_or("review project memory")
        );
        println!(
            "next recommended command: {}",
            report["next_recommended_command"]
                .as_str()
                .unwrap_or("memory dev next")
        );
    }

    Ok(())
}

fn dev_resume_command(
    engine: &MemoryEngine,
    query: &Option<String>,
    workspace: Option<&String>,
    limit: usize,
    tokens: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let resume_query = if let Some(query) = query {
        query.clone()
    } else {
        engine
            .list_recent(Some(&scope), 1)?
            .first()
            .map(|memory| memory.summary.clone())
            .unwrap_or_else(|| "resume the most recent interrupted task".to_string())
    };

    let replay = engine.replay(&resume_query, Some(&scope), limit.max(6))?;
    let context = engine.context(
        RecallQuery::new(resume_query.clone())
            .workspace(scope.clone())
            .limit(limit.max(6)),
        tokens,
    )?;
    let repo_root = resolve_repo_root(&env::current_dir()?);
    let relevant_files = repo_root
        .as_deref()
        .map(|root| {
            git_stdout(root, &["log", "--name-only", "--pretty=format:", "-n", "8"])
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .take(limit.max(6))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let related_commits = repo_root
        .as_deref()
        .and_then(|root| git_commit_records(root, Some("30d"), limit).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|commit| {
            let haystack = format!(
                "{} {} {}",
                commit.subject,
                commit.body,
                commit.files.join(" ")
            )
            .to_ascii_lowercase();
            resume_query
                .split_whitespace()
                .any(|token| haystack.contains(&token.to_ascii_lowercase()))
        })
        .take(limit)
        .collect::<Vec<_>>();
    let todos = repo_root
        .as_deref()
        .map(|root| collect_todos(root, limit.max(8)))
        .unwrap_or_default();
    let terminal_entries = read_terminal_entries(engine, 60).unwrap_or_default();
    let failed_commands = terminal_entries
        .iter()
        .filter(|entry| entry.exit_code != 0)
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    let successful_commands = terminal_entries
        .iter()
        .filter(|entry| entry.exit_code == 0)
        .take(5)
        .cloned()
        .collect::<Vec<_>>();

    let response = json!({
        "workspace": scope,
        "query": resume_query,
        "replay": replay,
        "last_relevant_files_touched": relevant_files,
        "related_commits": related_commits,
        "related_todos": todos,
        "failed_commands": failed_commands,
        "successful_commands": successful_commands,
        "related_memories": context.memories,
        "context": context,
        "recommended_next_step": context
            .memories
            .first()
            .map(|memory| memory.reason.clone())
            .unwrap_or_else(|| "continue from the most recent project milestone".to_string()),
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("resume query: {}", response["query"].as_str().unwrap_or(""));
        println!("recent workflow replay:");
        for step in response["replay"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(limit)
        {
            println!(
                "  {}. {} - {}",
                step["index"].as_u64().unwrap_or(0),
                step["event"].as_str().unwrap_or("step"),
                step["detail"].as_str().unwrap_or("")
            );
        }
        println!(
            "\nrecommended next step: {}",
            response["recommended_next_step"]
                .as_str()
                .unwrap_or("continue from the current context")
        );
        let files = response["last_relevant_files_touched"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !files.is_empty() {
            println!("\nlast relevant files touched:");
            for file in files.iter().take(limit.min(8)) {
                println!("  - {}", file.as_str().unwrap_or(""));
            }
        }
        let commits = response["related_commits"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !commits.is_empty() {
            println!("\nrelated commits:");
            for commit in commits.iter().take(limit.min(5)) {
                println!(
                    "  - {} {}",
                    commit["short_sha"].as_str().unwrap_or("commit"),
                    commit["subject"].as_str().unwrap_or("")
                );
            }
        }
        let failed = response["failed_commands"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !failed.is_empty() {
            println!("\nrecent failed commands:");
            for command in failed.iter().take(3) {
                println!(
                    "  - [{}] {}",
                    command["exit_code"].as_i64().unwrap_or(1),
                    command["command"].as_str().unwrap_or("")
                );
            }
        }
        println!("\ncontext block:\n{}", context.text);
    }

    Ok(())
}

fn dev_explain_repo_command(
    engine: &MemoryEngine,
    path: Option<&Path>,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let requested_path = path.map(Path::to_path_buf).unwrap_or(env::current_dir()?);
    let repo_root = resolve_repo_root(&requested_path).unwrap_or(requested_path.clone());
    let outline = collect_repo_outline(&repo_root)?;
    let recent_memories = engine.list_recent(Some(&scope), 18)?;
    let recent_decisions = recent_memories
        .iter()
        .filter(|memory| matches!(memory.kind, MemoryKind::Decision | MemoryKind::Workflow))
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    let recent_bugs = recent_memories
        .iter()
        .filter(|memory| {
            matches!(memory.kind, MemoryKind::Bug)
                || memory.summary.to_ascii_lowercase().contains("fix")
        })
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    let recent_commits = git_commit_records(&repo_root, Some("14d"), 5).unwrap_or_default();
    let readme_brief = read_readme_brief(&repo_root)
        .unwrap_or_else(|| "No README summary was detected yet.".to_string());
    let important = important_files(&repo_root);
    let commands = infer_run_commands(&repo_root);
    let todos = collect_todos(&repo_root, 12);
    let roadmap = recent_memories
        .iter()
        .filter(|memory| {
            matches!(memory.kind, MemoryKind::Task)
                || memory.summary.to_ascii_lowercase().contains("roadmap")
                || memory.summary.to_ascii_lowercase().contains("next")
        })
        .take(6)
        .cloned()
        .collect::<Vec<_>>();
    let report = json!({
        "workspace": scope,
        "path": requested_path,
        "repo_root": repo_root,
        "what_this_repo_does": readme_brief,
        "outline": outline,
        "main_modules": outline,
        "important_files": important,
        "how_to_run_or_test": commands,
        "data_storage": "local SQLite database under .memory.cpp/memory.db unless --db is provided",
        "command_structure": "memory-cli uses a small manual pre-parser for launch commands plus Clap subcommands for the stable core",
        "recent_decisions": recent_decisions,
        "recent_bugs_and_fixes": recent_bugs,
        "recent_commits": recent_commits,
        "known_risks": todos,
        "current_roadmap": roadmap,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "repo explanation for {}",
            report["repo_root"].as_str().unwrap_or(".")
        );
        println!(
            "workspace: {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        println!("shape:");
        for entry in report["outline"].as_array().cloned().unwrap_or_default() {
            println!("  - {}", entry.as_str().unwrap_or("item"));
        }
        println!(
            "what this repo does: {}",
            report["what_this_repo_does"]
                .as_str()
                .unwrap_or("README summary unavailable")
        );
        println!("important files:");
        for file in report["important_files"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            println!("  - {}", file.as_str().unwrap_or(""));
        }
        println!("how to run/test:");
        for command in report["how_to_run_or_test"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            println!("  - {}", command.as_str().unwrap_or(""));
        }
        println!(
            "data storage: {}",
            report["data_storage"].as_str().unwrap_or("local SQLite")
        );
        println!(
            "command structure: {}",
            report["command_structure"]
                .as_str()
                .unwrap_or("CLI subcommands")
        );
        println!("recent decisions:");
        let decisions = report["recent_decisions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if decisions.is_empty() {
            println!("  none captured yet");
        } else {
            for item in decisions {
                println!("  - {}", item["summary"].as_str().unwrap_or("decision"));
            }
        }
        println!("recent bugs/fixes:");
        let bugs = report["recent_bugs_and_fixes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if bugs.is_empty() {
            println!("  none captured yet");
        } else {
            for item in bugs {
                println!("  - {}", item["summary"].as_str().unwrap_or("bug/fix"));
            }
        }
        if let Some(commits) = report["recent_commits"].as_array() {
            if !commits.is_empty() {
                println!("recent git activity:");
                for commit in commits {
                    println!(
                        "  - {} {}",
                        commit["short_sha"].as_str().unwrap_or("commit"),
                        commit["subject"].as_str().unwrap_or("")
                    );
                }
            }
        }
        let risks = report["known_risks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !risks.is_empty() {
            println!("known risks / TODOs:");
            for risk in risks.iter().take(6) {
                println!(
                    "  - {}:{} {}",
                    risk["path"].as_str().unwrap_or("file"),
                    risk["line"].as_u64().unwrap_or(0),
                    risk["text"].as_str().unwrap_or("")
                );
            }
        }
    }

    Ok(())
}

fn dev_next_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let inbox = engine.inbox(Some(&scope), Some("pending"))?;
    let conflicts = engine.conflicts(Some(&scope), limit.max(5))?;
    let recent = engine.list_recent(Some(&scope), limit.max(8))?;
    let mut suggestions = Vec::new();
    let repo_root = resolve_repo_root(&env::current_dir()?);
    let repo_status = repo_root.as_deref().map(repo_status_report);
    let todos = repo_root
        .as_deref()
        .map(|root| collect_todos(root, limit.max(8)))
        .unwrap_or_default();

    if recent.iter().any(|memory| {
        let lower = memory.summary.to_ascii_lowercase();
        lower.contains("test") && (lower.contains("fail") || lower.contains("flaky"))
    }) {
        suggestions.push(
            "Fix or explain the latest failing test with `memory dev test-failures`.".to_string(),
        );
    }
    if repo_status
        .as_ref()
        .and_then(|status| status["dirty_count"].as_u64())
        .unwrap_or(0)
        > 0
    {
        suggestions.push(
            "Review uncommitted changes with `git diff --stat`, then commit the coherent slice."
                .to_string(),
        );
    }
    if let Some(item) = inbox.first() {
        suggestions.push(format!(
            "Review candidate inbox items with `memory inbox explain {}`: {}",
            item.id, item.reason
        ));
    }
    if let Some(conflict) = conflicts.first() {
        suggestions.push(format!("Resolve memory conflict: {}", conflict.reason));
    }
    if let Some(memory) = recent.iter().find(|memory| {
        matches!(
            memory.kind,
            MemoryKind::Task | MemoryKind::Decision | MemoryKind::Workflow
        )
    }) {
        suggestions.push(format!(
            "Continue the latest tracked thread: {}",
            memory.summary
        ));
    }
    if let Some(memory) = recent.iter().find(|memory| {
        matches!(memory.kind, MemoryKind::Bug)
            || memory.summary.to_ascii_lowercase().contains("fix")
    }) {
        suggestions.push(format!(
            "Verify the latest bug/fix memory: {}",
            memory.summary
        ));
    }
    if !todos.is_empty() {
        let todo = &todos[0];
        suggestions.push(format!(
            "Close or clarify TODO at {}:{}: {}",
            todo.path, todo.line, todo.text
        ));
    }
    if repo_root.is_some() {
        suggestions.push(
            "Run `memory git watch --once` to capture new repo changes automatically.".to_string(),
        );
    }
    suggestions.push("Run `memory map --type evolution --output html --save .memory.cpp/demo/evolution.html` for a shareable project evolution map.".to_string());
    if suggestions.is_empty() {
        suggestions.push(
            "Run `memory dev morning` and seed or import a few project memories to build momentum."
                .to_string(),
        );
    }
    suggestions.truncate(limit.max(1));

    let report = json!({
        "workspace": scope,
        "suggestions": suggestions,
        "pending_inbox": inbox.len(),
        "conflicts": conflicts.len(),
        "repo_status": repo_status,
        "open_todos": todos,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "recommended next tasks for {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        for (index, suggestion) in report["suggestions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            println!(
                "{}. {}",
                index + 1,
                suggestion.as_str().unwrap_or("review project memory")
            );
        }
        println!(
            "signals: {} pending inbox item(s), {} conflict(s)",
            report["pending_inbox"].as_u64().unwrap_or(0),
            report["conflicts"].as_u64().unwrap_or(0)
        );
    }

    Ok(())
}

fn dev_recall_error_command(
    engine: &MemoryEngine,
    error: &str,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let query_words = vec![error.to_string()];
    let memories = engine.search(
        build_recall_query(
            &query_words,
            Some(&scope),
            &[MemoryKind::Bug],
            &[],
            limit,
            true,
            true,
            engine,
        )?
        .include_inactive(true),
    )?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else if memories.is_empty() {
        println!("no previous fix memory found for {error}");
        println!("tip: after fixing it, run `memory remember \"{error}: fixed by ...\" --kind bug --tags error,fix`");
    } else {
        println!("previous fixes for {error}:");
        for item in memories {
            println!("  - {}", item.memory.summary);
            if !item.memory.content.is_empty() {
                println!("    {}", item.memory.content);
            }
        }
    }
    Ok(())
}

fn dev_test_failures_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let query_words = vec!["test failure flaky regression reproduce".to_string()];
    let memories = engine.search(build_recall_query(
        &query_words,
        Some(&scope),
        &[MemoryKind::Bug],
        &[],
        limit,
        true,
        true,
        engine,
    )?)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else if memories.is_empty() {
        println!("no test failure memories found");
    } else {
        println!("known test failures:");
        for item in memories {
            println!("  - {}", item.memory.summary);
            println!("    score {:.2} | {}", item.score, item.reason);
        }
    }
    Ok(())
}

fn dev_recall_test_command(
    engine: &MemoryEngine,
    test: &str,
    workspace: Option<&String>,
    limit: usize,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let query_words = vec![format!("{test} test failure flaky fix reproduce")];
    let memories = engine.search(build_recall_query(
        &query_words,
        Some(&scope),
        &[],
        &[],
        limit,
        true,
        true,
        engine,
    )?)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else if memories.is_empty() {
        println!("no memory found for test {test}");
    } else {
        println!("recall for test {test}:");
        for item in memories {
            println!("  - {}", item.memory.summary);
        }
    }
    Ok(())
}

fn dev_context_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    target: &DevContextTarget,
    limit: usize,
    tokens: usize,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let repo_summary =
        read_readme_brief(&repo_root).unwrap_or_else(|| "Repo summary unavailable.".to_string());
    let status = repo_status_report(&repo_root);
    let context = engine.context(
        RecallQuery::new(
            "current task recent decisions coding style important files known pitfalls commands",
        )
        .workspace(scope.clone())
        .limit(limit),
        tokens,
    )?;
    let commands = infer_run_commands(&repo_root);
    let important = important_files(&repo_root);
    let todos = collect_todos(&repo_root, if verbose { 16 } else { 6 });
    let pitfalls = engine.search(
        RecallQuery::new("pitfall bug error workaround risk")
            .workspace(scope.clone())
            .limit(if verbose { 8 } else { 4 })
            .include_content(false),
    )?;
    let citations = context
        .memories
        .iter()
        .take(if verbose { 10 } else { 5 })
        .map(|item| {
            let source = item.memory.attributes.source.as_ref();
            json!({
                "id": item.memory.id,
                "summary": item.memory.summary,
                "source_file": source.and_then(|source| source.source_file.clone()),
                "source_commit": source.and_then(|source| source.source_commit.clone()),
                "reason": item.reason,
            })
        })
        .collect::<Vec<_>>();
    let header = match target {
        DevContextTarget::Cursor => "Cursor context pack",
        DevContextTarget::Codex => "Codex context pack",
        DevContextTarget::Claude => "Claude context pack",
        DevContextTarget::Vscode => "VS Code context pack",
        DevContextTarget::Continue => "Continue context pack",
        DevContextTarget::Aider => "Aider context pack",
        DevContextTarget::Copilot => "Copilot context pack",
        DevContextTarget::Ollama => "Ollama context pack",
        DevContextTarget::Openai => "OpenAI context pack",
        DevContextTarget::SmallModel => "Small-model context pack",
        DevContextTarget::LargeModel => "Large-model context pack",
        DevContextTarget::Generic => "AI assistant context pack",
    };
    let todo_block = if todos.is_empty() {
        "- No TODO/FIXME comments detected.".to_string()
    } else {
        todos
            .iter()
            .map(|todo| format!("- {}:{} {}", todo.path, todo.line, todo.text))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let pitfall_block = if pitfalls.is_empty() {
        "- No pitfall memories found yet.".to_string()
    } else {
        pitfalls
            .iter()
            .map(|item| format!("- {}", item.memory.summary))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let citation_block = if citations.is_empty() {
        "- No citations yet.".to_string()
    } else {
        citations
            .iter()
            .map(|item| {
                format!(
                    "- {} ({})",
                    item["summary"].as_str().unwrap_or("memory"),
                    item["source_file"].as_str().unwrap_or("memory store")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let extra = if verbose {
        format!("\nKnown pitfalls:\n{pitfall_block}\n\nCitations:\n{citation_block}\n")
    } else {
        String::new()
    };
    let block = format!(
        "{header}\n\nRepo summary:\n{repo_summary}\n\nCurrent branch: {}\nDirty files: {}\n\nImportant files:\n{}\n\nCommands to run:\n{}\n\nOpen TODOs:\n{}\n\nPrivacy/safety note:\n- memory.cpp stays local by default. Do not paste secrets into prompts.\n{extra}\nMemory context:\n{}",
        status["branch"].as_str().unwrap_or("unknown"),
        status["dirty_count"].as_u64().unwrap_or(0),
        important.iter().map(|item| format!("- {item}")).collect::<Vec<_>>().join("\n"),
        commands.iter().map(|item| format!("- {item}")).collect::<Vec<_>>().join("\n"),
        todo_block,
        context.text
    );
    let report = json!({
        "workspace": scope,
        "target": format!("{target:?}").to_ascii_lowercase(),
        "repo_summary": repo_summary,
        "status": status,
        "important_files": important,
        "commands": commands,
        "todos": todos,
        "pitfalls": pitfalls,
        "citations": citations,
        "verbose": verbose,
        "context": context,
        "block": block,
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report["block"].as_str().unwrap_or(""));
    }
    Ok(())
}

fn public_context_command(engine: &MemoryEngine, args: &ManualContextCli) -> Result<()> {
    let default_action = ContextAction::Build;
    let action = args.action.as_ref().unwrap_or(&default_action);
    let base = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"));
    let context_dir = base.join("context");
    match action {
        ContextAction::Build | ContextAction::Refresh => {
            if args.output.is_some() {
                let rendered = context_pack_text(
                    engine,
                    args.workspace.as_ref(),
                    &args.target,
                    args.limit,
                    args.budget,
                    args.verbose,
                    &args.format,
                )?;
                emit_or_save(&rendered, args.output.as_deref())?;
            } else {
                dev_context_command(
                    engine,
                    args.workspace.as_ref(),
                    &args.target,
                    args.limit,
                    args.budget,
                    args.verbose,
                    args.json,
                )?;
            }
            if args.copy {
                println!("clipboard copy is not automatic in this release; select the output above or use --output.");
            }
        }
        ContextAction::Write => {
            let output = args.output.clone().unwrap_or_else(|| {
                context_dir.join(format!(
                    "{}.md",
                    format!("{:?}", args.target).to_ascii_lowercase()
                ))
            });
            let rendered = context_pack_text(
                engine,
                args.workspace.as_ref(),
                &args.target,
                args.limit,
                args.budget,
                args.verbose,
                &args.format,
            )?;
            emit_or_save(&rendered, Some(&output))?;
        }
        ContextAction::Open => {
            if let Some(path) = newest_file(std::slice::from_ref(&context_dir), "md") {
                println!("{path}");
                let _ = open_with_os(&path);
            } else {
                println!("no context pack found yet");
                println!("try: memory context write --for cursor");
            }
        }
        ContextAction::Status => {
            let latest = newest_file(std::slice::from_ref(&context_dir), "md");
            let report = json!({
                "context_dir": context_dir,
                "latest": latest,
                "freshness": if latest.is_some() { "available" } else { "missing" },
                "next": "memory context write --for cursor",
            });
            if args.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("context pack status");
                println!("folder: {}", report["context_dir"].as_str().unwrap_or(""));
                println!("latest: {}", report["latest"].as_str().unwrap_or("none"));
                println!("next: {}", report["next"].as_str().unwrap_or(""));
            }
        }
        ContextAction::Diff => {
            println!("context diff is lightweight in this release.");
            println!("Write a pack, then compare it with your editor or git:");
            println!("memory context write --for cursor --output .memory.cpp/context/cursor.md");
        }
        ContextAction::Explain => {
            println!("context packs are short local briefings for AI coding tools.");
            println!("They include repo summary, recent decisions, important files, commands, TODOs, and safety notes.");
            println!("They stay local unless you paste or attach them yourself.");
            println!("try: memory context write --for codex");
        }
    }
    Ok(())
}

fn context_pack_text(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    target: &DevContextTarget,
    limit: usize,
    tokens: usize,
    verbose: bool,
    format: &str,
) -> Result<String> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let status = repo_status_report(&repo_root);
    let repo_summary =
        read_readme_brief(&repo_root).unwrap_or_else(|| "Repo summary unavailable.".to_string());
    let context = engine.context(
        RecallQuery::new("current task decisions architecture commands errors fixes")
            .workspace(scope.clone())
            .limit(limit),
        tokens,
    )?;
    let commands = infer_run_commands(&repo_root);
    let todos = collect_todos(&repo_root, if verbose { 16 } else { 6 });
    let important = important_files(&repo_root);
    if format.eq_ignore_ascii_case("json") {
        return Ok(serde_json::to_string_pretty(&json!({
            "target": format!("{target:?}").to_ascii_lowercase(),
            "workspace": scope,
            "repo_summary": repo_summary,
            "status": status,
            "commands": commands,
            "todos": todos,
            "important_files": important,
            "context": context,
            "local_only": true,
        }))?);
    }
    let mut out = String::new();
    out.push_str(&format!(
        "# {} context pack\n\n",
        format!("{target:?}").to_ascii_lowercase()
    ));
    out.push_str("Local-first note: generated from local repo memory. Do not paste secrets.\n\n");
    out.push_str("## Project summary\n");
    out.push_str(&repo_summary);
    out.push_str("\n\n## Current repo state\n");
    out.push_str(&format!(
        "- Branch: {}\n- Dirty files: {}\n",
        status["branch"].as_str().unwrap_or("unknown"),
        status["dirty_count"].as_u64().unwrap_or(0)
    ));
    out.push_str("\n## Important files\n");
    for file in important {
        out.push_str(&format!("- `{file}`\n"));
    }
    out.push_str("\n## Commands to run\n");
    for command in commands {
        out.push_str(&format!("- `{command}`\n"));
    }
    out.push_str("\n## Open TODOs\n");
    if todos.is_empty() {
        out.push_str("- No TODO/FIXME comments detected.\n");
    } else {
        for todo in todos {
            out.push_str(&format!("- `{}`:{} {}\n", todo.path, todo.line, todo.text));
        }
    }
    out.push_str("\n## Memory context\n");
    out.push_str(&context.text);
    out.push_str("\n\n## What not to do\n");
    out.push_str("- Do not assume network or cloud sync is enabled.\n");
    out.push_str("- Do not store secrets; use `.memoryignore` and redaction previews.\n");
    Ok(out)
}

fn dev_onboard_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    output: &DevOnboardOutput,
    save: Option<&Path>,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let decisions = engine.search(
        RecallQuery::new("architecture decision why because")
            .workspace(scope.clone())
            .kind(MemoryKind::Decision)
            .limit(8)
            .include_content(true),
    )?;
    let bugs = engine.search(
        RecallQuery::new("common error bug fix workaround")
            .workspace(scope.clone())
            .kind(MemoryKind::Bug)
            .limit(8)
            .include_content(true),
    )?;
    let report = json!({
        "workspace": scope,
        "repo_root": repo_root,
        "overview": read_readme_brief(&repo_root),
        "architecture": collect_repo_outline(&repo_root)?,
        "important_files": important_files(&repo_root),
        "commands": infer_run_commands(&repo_root),
        "important_decisions": decisions,
        "common_errors": bugs,
        "known_risks": collect_todos(&repo_root, 12),
        "next_tasks": engine.inbox(Some(&scope), Some("pending"))?,
    });
    let rendered = if matches!(output, DevOnboardOutput::Json) {
        serde_json::to_string_pretty(&report)?
    } else {
        render_onboarding_markdown(&report)
    };
    emit_or_save(&rendered, save)?;
    Ok(())
}

fn render_onboarding_markdown(report: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Project onboarding\n\n");
    out.push_str(&format!(
        "Workspace: `{}`\n\n",
        report["workspace"].as_str().unwrap_or("default")
    ));
    out.push_str("## Overview\n");
    out.push_str(
        report["overview"]
            .as_str()
            .unwrap_or("No overview detected."),
    );
    out.push_str("\n\n## How to run and test\n");
    for command in report["commands"].as_array().cloned().unwrap_or_default() {
        out.push_str(&format!("- `{}`\n", command.as_str().unwrap_or("")));
    }
    out.push_str("\n## Important files\n");
    for file in report["important_files"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!("- `{}`\n", file.as_str().unwrap_or("")));
    }
    out.push_str("\n## Architecture\n");
    for item in report["architecture"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push_str("\n## Important decisions\n");
    for item in report["important_decisions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!(
            "- {}\n",
            item["memory"]["summary"].as_str().unwrap_or("decision")
        ));
    }
    out.push_str("\n## Common errors\n");
    for item in report["common_errors"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!(
            "- {}\n",
            item["memory"]["summary"].as_str().unwrap_or("error")
        ));
    }
    out.push_str("\n## Known risks and TODOs\n");
    for item in report["known_risks"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!(
            "- `{}`:{} {}\n",
            item["path"].as_str().unwrap_or("file"),
            item["line"].as_u64().unwrap_or(0),
            item["text"].as_str().unwrap_or("")
        ));
    }
    out
}

fn dev_readme_suggest_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let commands = infer_run_commands(&repo_root);
    let mut suggestions = Vec::new();
    if !readme.to_ascii_lowercase().contains("architecture") {
        suggestions.push("Add or refresh an Architecture section.".to_string());
    }
    if !commands.iter().all(|cmd| readme.contains(cmd)) {
        suggestions
            .push("Document the current run/test commands detected from the repo.".to_string());
    }
    if repo_root.join(".memory.cpp").exists() && !readme.contains("memory dev morning") {
        suggestions.push("Mention `memory dev morning` as the daily resume command.".to_string());
    }
    if suggestions.is_empty() {
        suggestions.push("README looks aligned with the current developer workflow.".to_string());
    }
    let report = json!({ "repo_root": repo_root, "suggestions": suggestions });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("README suggestions:");
        for suggestion in report["suggestions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            println!("  - {}", suggestion.as_str().unwrap_or(""));
        }
    }
    Ok(())
}

fn dev_changelog_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    since: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let commits = git_commit_records(&repo_root, since.or(Some("30d")), 80).unwrap_or_default();
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut fixed = Vec::new();
    let mut docs = Vec::new();
    for commit in &commits {
        let lower = commit.subject.to_ascii_lowercase();
        if lower.contains("fix") || lower.contains("bug") {
            fixed.push(commit.subject.clone());
        } else if commit.files.iter().any(|file| file.ends_with(".md")) {
            docs.push(commit.subject.clone());
        } else if lower.contains("add") || lower.contains("introduce") {
            added.push(commit.subject.clone());
        } else {
            changed.push(commit.subject.clone());
        }
    }
    let report = json!({
        "workspace": scope,
        "since": since.unwrap_or("30d"),
        "added": added,
        "changed": changed,
        "fixed": fixed,
        "docs": docs,
        "breaking_changes": [],
        "internal": commits.iter().map(|commit| commit.subject.clone()).collect::<Vec<_>>(),
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "# Changelog since {}",
            report["since"].as_str().unwrap_or("30d")
        );
        for section in ["added", "changed", "fixed", "docs", "internal"] {
            println!("\n## {}", section);
            let items = report[section].as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                println!("- none");
            } else {
                for item in items.iter().take(20) {
                    println!("- {}", item.as_str().unwrap_or(""));
                }
            }
        }
    }
    Ok(())
}

fn dev_health_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let inbox = engine.inbox(Some(&scope), Some("pending"))?;
    let decisions = engine.search(
        RecallQuery::new("decision stale architecture")
            .workspace(scope.clone())
            .kind(MemoryKind::Decision)
            .limit(8),
    )?;
    let test_failures = engine.search(
        RecallQuery::new("test failure flaky")
            .workspace(scope.clone())
            .kind(MemoryKind::Bug)
            .limit(8),
    )?;
    let docs_freshness = if repo_root.join("README.md").exists() {
        "README present"
    } else {
        "README missing"
    };
    let report = json!({
        "workspace": scope,
        "repo_status": repo_status_report(&repo_root),
        "docs_freshness": docs_freshness,
        "test_status": if test_failures.is_empty() { "no remembered failures" } else { "remembered failures present" },
        "known_flaky_areas": test_failures,
        "unreviewed_memory_candidates": inbox.len(),
        "stale_decisions": decisions,
        "open_todos": collect_todos(&repo_root, 20),
        "architecture_drift": "run `memory map --type architecture` and compare with the README architecture section",
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("repo health for {scope}");
        println!("docs: {}", report["docs_freshness"].as_str().unwrap_or(""));
        println!("tests: {}", report["test_status"].as_str().unwrap_or(""));
        println!(
            "unreviewed candidates: {} | open TODOs: {}",
            report["unreviewed_memory_candidates"].as_u64().unwrap_or(0),
            report["open_todos"].as_array().map(Vec::len).unwrap_or(0)
        );
        println!("{}", report["architecture_drift"].as_str().unwrap_or(""));
    }
    Ok(())
}

fn dev_pr_summary_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let diff_stat = git_stdout(&repo_root, &["diff", "--stat", "HEAD"]).unwrap_or_default();
    let decisions = engine.search(
        RecallQuery::new("related decision why changed")
            .workspace(scope.clone())
            .kind(MemoryKind::Decision)
            .limit(6),
    )?;
    let report = json!({
        "workspace": scope,
        "what_changed": diff_stat,
        "why_it_changed": decisions,
        "risky_areas": collect_todos(&repo_root, 8),
        "tests_to_run": infer_run_commands(&repo_root).into_iter().filter(|cmd| cmd.contains("test")).collect::<Vec<_>>(),
        "docs_to_update": dev_readme_suggestions_value(&repo_root),
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("PR summary");
        println!(
            "what changed:\n{}",
            report["what_changed"].as_str().unwrap_or("no diff")
        );
        println!("tests to run:");
        for command in report["tests_to_run"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            println!("  - {}", command.as_str().unwrap_or(""));
        }
    }
    Ok(())
}

fn dev_readme_suggestions_value(repo_root: &Path) -> Vec<String> {
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let mut suggestions = Vec::new();
    if !readme.to_ascii_lowercase().contains("architecture") {
        suggestions.push("architecture section may need an update".to_string());
    }
    if !readme.contains("memory dev") {
        suggestions.push("developer workflow commands are not documented".to_string());
    }
    suggestions
}

fn dev_review_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let memories = engine.search(
        RecallQuery::new("code review style preference risk owner common mistake")
            .workspace(scope.clone())
            .limit(10)
            .include_content(true),
    )?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else {
        println!("review memory for {scope}:");
        if memories.is_empty() {
            println!("  no review-specific memories yet");
        } else {
            for item in memories {
                println!("  - {}", item.memory.summary);
            }
        }
    }
    Ok(())
}

fn dev_period_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    label: &str,
    days_ago: i64,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    day_recap_command(engine, workspace, days_ago, verbose, json_output)?;
    if !json_output && label == "evening" {
        println!("evening wrap-up: run memory dev changelog --since 1d if you want release notes.");
    }
    Ok(())
}

fn dev_week_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let since = Utc::now() - ChronoDuration::days(7);
    let events = engine
        .timeline(Some(&scope), None, 200)?
        .into_iter()
        .filter(|event| event.created_at >= since)
        .collect::<Vec<_>>();
    let report = json!({
        "workspace": scope,
        "events": events,
        "next": "memory dev next",
    });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "week recap for {}",
            report["workspace"].as_str().unwrap_or("default")
        );
        for event in report["events"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(if verbose { 20 } else { 8 })
        {
            println!(
                "- {} ({})",
                event["body"].as_str().unwrap_or("event"),
                event["event_type"].as_str().unwrap_or("memory")
            );
        }
        println!("next: memory dev next");
    }
    Ok(())
}

fn dev_focus_query(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    query: &str,
    title: &str,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let memories = engine.search(
        RecallQuery::new(query)
            .workspace(scope.clone())
            .limit(10)
            .include_content(true),
    )?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else if memories.is_empty() {
        println!("no {title} memories found yet");
        println!("try: memory dev morning");
    } else {
        println!("{title} for {scope}:");
        for item in memories {
            println!("  - {}", item.memory.summary);
        }
    }
    Ok(())
}

fn dev_docs_gap_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let suggestions = dev_readme_suggestions_value(&repo_root);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
    } else {
        println!("docs gaps:");
        for suggestion in suggestions {
            println!("  - {suggestion}");
        }
    }
    Ok(())
}

fn dev_stale_todos_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let todos = collect_todos(&repo_root, 24);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&todos)?);
    } else if todos.is_empty() {
        println!("no TODO/FIXME comments found");
    } else {
        println!("TODO/FIXME comments:");
        for todo in todos {
            println!("  - {}:{} {}", todo.path, todo.line, todo.text);
        }
    }
    Ok(())
}

fn dev_changed_files_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let status = repo_status_report(&repo_root);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("changed files:");
        for file in status["dirty_files"].as_array().into_iter().flatten() {
            println!("  - {}", file.as_str().unwrap_or(""));
        }
    }
    Ok(())
}

fn dev_hot_files_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let repo_root = resolve_repo_root(&env::current_dir()?).unwrap_or(env::current_dir()?);
    let files = git_hot_files(&repo_root, 12).unwrap_or_default();
    if json_output {
        println!("{}", serde_json::to_string_pretty(&files)?);
    } else {
        println!("hot files:");
        for file in files {
            println!("  - {file}");
        }
    }
    Ok(())
}

fn dev_common_commands_command(
    engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let entries = read_terminal_entries(engine, 200)?;
    let mut counts = HashMap::<String, usize>::new();
    for entry in entries {
        *counts.entry(entry.command).or_default() += 1;
    }
    let mut common = counts.into_iter().collect::<Vec<_>>();
    common.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    common.truncate(12);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&common)?);
    } else if common.is_empty() {
        println!("no terminal command memory yet");
        println!("try: memory terminal enable");
    } else {
        println!("common commands:");
        for (command, count) in common {
            println!("  - {command} ({count}x)");
        }
    }
    Ok(())
}

fn dev_setup_guide_command(
    _engine: &MemoryEngine,
    _workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let root = env::current_dir()?;
    let detections = setup_detections(&root);
    let commands = json!([
        "memory setup --developer",
        detections["test_command"]
            .as_str()
            .unwrap_or("memory dev morning"),
        "memory doctor",
        "memory dev context --for codex"
    ]);
    let report = json!({ "detections": detections, "commands": commands });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("setup guide:");
        for command in report["commands"].as_array().into_iter().flatten() {
            println!("  - {}", command.as_str().unwrap_or(""));
        }
    }
    Ok(())
}

fn command_explanation(command: &str) -> &'static str {
    if command.contains("dev morning") {
        "Shows where you left off, recent work, open candidates, branch state, and a next command."
    } else if command.contains("dev context") {
        "Builds a local AI context pack for a coding assistant."
    } else if command.contains("git watch") {
        "Observes local Git changes and creates candidate memories with provenance."
    } else if command.contains("terminal") {
        "Manages opt-in local terminal command memory."
    } else if command.contains("privacy") {
        "Shows local storage, redaction, and purge controls."
    } else if command.contains("map") {
        "Builds a project map from memories, citations, and optional Git signals."
    } else {
        "A memory.cpp command. Try `memory examples` for short workflows."
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_command(
    engine: &MemoryEngine,
    path: Option<&Path>,
    workspace: Option<&String>,
    kind: Option<MemoryKind>,
    from_git: bool,
    since: Option<&str>,
    limit: usize,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let source_path = path.map(Path::to_path_buf).unwrap_or(env::current_dir()?);
    let candidates = if from_git {
        extract_candidates_from_git(&source_path, kind, since, limit)?
    } else {
        extract_candidates_from_path(&source_path, kind, limit)?
    };

    let mut stored = 0usize;
    let mut queued = 0usize;
    if !dry_run {
        for candidate in &candidates {
            let memory = extracted_candidate_to_memory(candidate, &scope, true);
            if engine
                .remember_candidate(memory, &candidate.reason)?
                .is_some()
            {
                stored += 1;
            } else {
                queued += 1;
            }
        }
    }

    let report = json!({
        "workspace": scope,
        "source": source_path,
        "mode": if from_git { "git" } else { "files" },
        "dry_run": dry_run,
        "stored": stored,
        "queued": queued,
        "candidates": candidates,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "extracted {} candidate memory item(s) from {}",
            report["candidates"].as_array().map(Vec::len).unwrap_or(0),
            report["source"].as_str().unwrap_or(".")
        );
        if dry_run {
            println!("dry run only, nothing was stored");
        } else {
            println!("stored immediately: {stored} | queued for review: {queued}");
        }
        for candidate in report["candidates"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(limit.min(8))
        {
            println!(
                "  - [{} {:.2}] {}",
                candidate["kind"].as_str().unwrap_or("note"),
                candidate["confidence"].as_f64().unwrap_or(0.0),
                candidate["content"].as_str().unwrap_or("candidate")
            );
        }
    }

    Ok(())
}

fn git_command(engine: &MemoryEngine, command: &GitCommand) -> Result<()> {
    let cwd = env::current_dir()?;
    let Some(repo_root) = resolve_repo_root(&cwd) else {
        println!("no git repository detected from {}", cwd.display());
        return Ok(());
    };

    match command {
        GitCommand::Ingest {
            workspace,
            since,
            limit,
            dry_run,
            json,
        } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let candidates =
                extract_candidates_from_git(&repo_root, None, since.as_deref(), *limit)?;
            let mut stored = 0usize;
            let mut queued = 0usize;
            if !*dry_run {
                for candidate in &candidates {
                    let memory = extracted_candidate_to_memory(candidate, &scope, false);
                    if engine
                        .remember_candidate(memory, &candidate.reason)?
                        .is_some()
                    {
                        stored += 1;
                    } else {
                        queued += 1;
                    }
                }
            }
            let report = json!({
                "repo_root": repo_root,
                "workspace": scope,
                "dry_run": dry_run,
                "stored": stored,
                "queued": queued,
                "candidates": candidates,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "git ingest found {} candidate memory item(s)",
                    report["candidates"].as_array().map(Vec::len).unwrap_or(0)
                );
                if *dry_run {
                    println!("dry run only, nothing was stored");
                } else {
                    println!("stored immediately: {stored} | queued for review: {queued}");
                }
            }
        }
        GitCommand::Summary {
            since, limit, json, ..
        } => {
            let commits = git_commit_records(&repo_root, since.as_deref(), *limit)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&commits)?);
            } else if commits.is_empty() {
                println!("no commits matched the requested window");
            } else {
                println!("recent git summary for {}", repo_root.display());
                for commit in commits {
                    println!("  - {} {}", commit.short_sha, commit.subject);
                }
            }
        }
        GitCommand::Decisions {
            since, limit, json, ..
        } => {
            let candidates = extract_candidates_from_git(
                &repo_root,
                Some(MemoryKind::Decision),
                since.as_deref(),
                *limit,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
            } else if candidates.is_empty() {
                println!("no decision-flavored git memories detected");
            } else {
                for candidate in candidates {
                    println!(
                        "  - [{} {:.2}] {}",
                        candidate.kind, candidate.confidence, candidate.content
                    );
                }
            }
        }
        GitCommand::Bugs {
            since, limit, json, ..
        } => {
            let candidates = extract_candidates_from_git(
                &repo_root,
                Some(MemoryKind::Bug),
                since.as_deref(),
                *limit,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
            } else if candidates.is_empty() {
                println!("no bug/fix git memories detected");
            } else {
                for candidate in candidates {
                    println!(
                        "  - [{} {:.2}] {}",
                        candidate.kind, candidate.confidence, candidate.content
                    );
                }
            }
        }
        GitCommand::Map {
            workspace,
            output,
            save,
            json,
        } => {
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "repo_root": repo_root,
                        "workspace": workspace,
                        "type": "evolution",
                        "output": format!("{output:?}").to_ascii_lowercase(),
                        "save": save,
                    }))?
                );
            } else {
                map_command(
                    engine,
                    Some(&repo_root),
                    None,
                    workspace.as_ref(),
                    CliMapType::Evolution,
                    output.clone(),
                    None,
                    None,
                    true,
                    false,
                    None,
                    None,
                    None,
                    save.as_deref(),
                )?;
            }
        }
        GitCommand::Watch {
            action,
            workspace,
            interval_secs,
            daemon,
            once,
            limit,
            dry_run,
            json,
        } => {
            if let Some(action) = action {
                git_watch_action_command(&repo_root, action)?;
            } else {
                git_watch_command(
                    engine,
                    &repo_root,
                    workspace.as_ref(),
                    *interval_secs,
                    *once || !*daemon,
                    *limit,
                    *dry_run,
                    *json,
                )?;
            }
        }
        GitCommand::Today { json } => git_period_command(&repo_root, "24h", *json)?,
        GitCommand::Yesterday { json } => git_period_command(&repo_root, "48h", *json)?,
        GitCommand::Week { json } => git_period_command(&repo_root, "7d", *json)?,
        GitCommand::Branch { branch, json } => {
            git_branch_command(&repo_root, branch.as_deref(), *json)?
        }
        GitCommand::DiffMemory { workspace, json } => {
            let scope = required_workspace(engine, workspace.as_ref())?;
            let status = git_stdout(&repo_root, &["diff", "--stat"]).unwrap_or_default();
            let report = json!({"workspace": scope, "diff_stat": status, "next": "memory git ingest --dry-run"});
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report["diff_stat"].as_str().unwrap_or("no diff"));
                println!("next: memory git ingest --dry-run");
            }
        }
        GitCommand::ReleaseNotes { since, json } => {
            git_release_notes_command(&repo_root, since.as_deref(), *json)?
        }
        GitCommand::WhyFileChanged { file, json } => {
            git_why_file_changed_command(&repo_root, file, *json)?
        }
        GitCommand::HotFiles { json } => {
            let files = git_hot_files(&repo_root, 20)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&files)?);
            } else {
                for file in files {
                    println!("{file}");
                }
            }
        }
        GitCommand::DependencyChanges { json } => {
            git_filtered_changes(&repo_root, "dependency", *json)?
        }
        GitCommand::TestChanges { json } => git_filtered_changes(&repo_root, "test", *json)?,
        GitCommand::DocsChanges { json } => git_filtered_changes(&repo_root, "docs", *json)?,
        GitCommand::RiskyChanges { json } => git_filtered_changes(&repo_root, "risk", *json)?,
        GitCommand::ForgottenChanges { json } => git_forgotten_changes(&repo_root, *json)?,
        GitCommand::SummarizeCommit { sha, json } => git_summarize_commit(&repo_root, sha, *json)?,
        GitCommand::SummarizeBranch { branch, json } => {
            git_summarize_branch(&repo_root, branch, *json)?
        }
        GitCommand::CompareBranches { left, right, json } => {
            git_compare_branches(&repo_root, left, right, *json)?
        }
        GitCommand::MapBranch {
            branch,
            workspace,
            save,
        } => {
            let branch = branch
                .clone()
                .or_else(|| git_stdout(&repo_root, &["branch", "--show-current"]).ok());
            println!("branch map: {}", branch.as_deref().unwrap_or("current"));
            map_command(
                engine,
                Some(&repo_root),
                branch.as_ref(),
                workspace.as_ref(),
                CliMapType::Evolution,
                CliMapOutput::Html,
                None,
                None,
                true,
                true,
                None,
                None,
                None,
                save.as_deref(),
            )?;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn git_watch_command(
    engine: &MemoryEngine,
    repo_root: &Path,
    workspace: Option<&String>,
    interval_secs: u64,
    once: bool,
    limit: usize,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let scope = required_workspace(engine, workspace)?;
    let watch_dir = repo_root.join(".memory.cpp").join("git-watch");
    fs::create_dir_all(&watch_dir)?;
    let state_file = watch_dir.join("state.json");
    let mut previous = load_git_watch_state(&state_file).unwrap_or_else(|| json!({}));

    loop {
        let current_head = git_stdout(repo_root, &["rev-parse", "HEAD"]).unwrap_or_default();
        let current_branch = git_stdout(repo_root, &["branch", "--show-current"])
            .unwrap_or_else(|_| "unknown".to_string());
        let previous_head = previous
            .get("head")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let previous_branch = previous
            .get("branch")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let mut observations = Vec::new();
        if !previous_branch.is_empty() && previous_branch != current_branch {
            observations.push(json!({
                "kind": "branch_change",
                "summary": format!("branch changed from {previous_branch} to {current_branch}"),
            }));
        }
        if !previous_head.is_empty() && previous_head != current_head {
            let range = format!("{previous_head}..{current_head}");
            let commits = git_commit_records_for_range(repo_root, &range, limit)?;
            for commit in commits {
                observations.push(json!({
                    "kind": "commit",
                    "summary": commit.subject,
                    "commit": commit.sha,
                    "files": commit.files,
                }));
            }
        }
        if previous_head.is_empty() {
            observations.push(json!({
                "kind": "initialized",
                "summary": "git watch baseline recorded",
                "head": current_head,
                "branch": current_branch,
            }));
        }

        let candidates = observations
            .iter()
            .filter_map(|observation| {
                build_git_watch_candidate(observation, repo_root, &current_head)
            })
            .collect::<Vec<_>>();
        let mut stored = 0usize;
        let mut queued = 0usize;
        if !dry_run {
            for candidate in &candidates {
                let memory = extracted_candidate_to_memory(candidate, &scope, false);
                if engine
                    .remember_candidate(memory, &candidate.reason)?
                    .is_some()
                {
                    stored += 1;
                } else {
                    queued += 1;
                }
            }
            previous = json!({
                "head": current_head,
                "branch": current_branch,
                "recorded_at": Utc::now(),
            });
            fs::write(&state_file, serde_json::to_string_pretty(&previous)?)?;
        }

        let report = json!({
            "repo_root": repo_root,
            "workspace": scope,
            "dry_run": dry_run,
            "observations": observations,
            "candidates": candidates,
            "stored": stored,
            "queued": queued,
            "state_file": state_file,
        });
        if json_output {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!(
                "git watch observed {} change(s); stored={}, queued={}",
                report["observations"].as_array().map(Vec::len).unwrap_or(0),
                stored,
                queued
            );
            println!("state: {}", report["state_file"].as_str().unwrap_or(""));
        }

        if once {
            break;
        }
        thread::sleep(Duration::from_secs(interval_secs.max(2)));
    }
    Ok(())
}

fn git_watch_action_command(repo_root: &Path, action: &GitWatchAction) -> Result<()> {
    let watch_dir = repo_root.join(".memory.cpp").join("git-watch");
    fs::create_dir_all(&watch_dir)?;
    let state_file = watch_dir.join("state.json");
    let pause_file = watch_dir.join("paused");
    match action {
        GitWatchAction::Status { json } => {
            let state = load_git_watch_state(&state_file).unwrap_or_else(|| json!({}));
            let report = json!({
                "state_file": state_file,
                "paused": pause_file.exists(),
                "state": state,
            });
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("git watch status");
                println!("paused: {}", report["paused"]);
                println!("state: {}", report["state_file"].as_str().unwrap_or(""));
            }
        }
        GitWatchAction::Pause => {
            fs::write(&pause_file, Utc::now().to_rfc3339())?;
            println!("git watch paused");
        }
        GitWatchAction::Resume => {
            if pause_file.exists() {
                fs::remove_file(&pause_file)?;
            }
            println!("git watch resumed");
        }
        GitWatchAction::ResetBaseline { json } => {
            let head = git_stdout(repo_root, &["rev-parse", "HEAD"]).unwrap_or_default();
            let branch = git_stdout(repo_root, &["branch", "--show-current"]).unwrap_or_default();
            let state = json!({
                "head": head,
                "branch": branch,
                "recorded_at": Utc::now(),
            });
            fs::write(&state_file, serde_json::to_string_pretty(&state)?)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                println!("git watch baseline reset");
            }
        }
    }
    Ok(())
}

fn git_period_command(repo_root: &Path, since: &str, json_output: bool) -> Result<()> {
    let commits = git_commit_records(repo_root, Some(since), 32)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&commits)?);
    } else if commits.is_empty() {
        println!("no commits found for {since}");
    } else {
        for commit in commits {
            println!("{} {}", commit.short_sha, commit.subject);
        }
    }
    Ok(())
}

fn git_branch_command(repo_root: &Path, branch: Option<&str>, json_output: bool) -> Result<()> {
    let branch = branch.map(str::to_string).unwrap_or_else(|| {
        git_stdout(repo_root, &["branch", "--show-current"]).unwrap_or_default()
    });
    let commits = git_commit_records_for_range(repo_root, &branch, 12).unwrap_or_default();
    let report = json!({ "branch": branch, "commits": commits });
    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("branch: {}", report["branch"].as_str().unwrap_or("current"));
        for commit in report["commits"].as_array().into_iter().flatten() {
            println!("  - {}", commit["subject"].as_str().unwrap_or("commit"));
        }
    }
    Ok(())
}

fn git_release_notes_command(
    repo_root: &Path,
    since: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let commits = git_commit_records(repo_root, since.or(Some("30d")), 80)?;
    let mut markdown = String::from("# Release notes\n\n");
    for heading in ["Added", "Changed", "Fixed", "Docs"] {
        markdown.push_str(&format!("## {heading}\n"));
        for commit in &commits {
            let lower = commit.subject.to_ascii_lowercase();
            let include = match heading {
                "Fixed" => lower.contains("fix") || lower.contains("bug"),
                "Docs" => commit.files.iter().any(|file| file.ends_with(".md")),
                "Added" => lower.contains("add") || lower.contains("introduce"),
                _ => true,
            };
            if include {
                markdown.push_str(&format!("- {}\n", commit.subject));
            }
        }
        markdown.push('\n');
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"markdown": markdown, "commits": commits}))?
        );
    } else {
        print!("{markdown}");
    }
    Ok(())
}

fn git_why_file_changed_command(repo_root: &Path, file: &Path, json_output: bool) -> Result<()> {
    let file_arg = file.to_string_lossy().to_string();
    let output = ProcessCommand::new("git")
        .current_dir(repo_root)
        .args(["log", "--follow", "--pretty=format:%h %s", "--", &file_arg])
        .output()
        .context("failed to run git log for file")?;
    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .take(12)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"file": file, "commits": lines}))?
        );
    } else if lines.is_empty() {
        println!("no git history found for {}", file.display());
    } else {
        println!("why {} changed:", file.display());
        for line in lines {
            println!("  - {line}");
        }
    }
    Ok(())
}

fn git_hot_files(repo_root: &Path, limit: usize) -> Result<Vec<String>> {
    let output = ProcessCommand::new("git")
        .current_dir(repo_root)
        .args(["log", "--name-only", "--pretty=format:"])
        .output()
        .context("failed to run git log for hot files")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let mut counts = HashMap::<String, usize>::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            *counts.entry(trimmed.to_string()).or_default() += 1;
        }
    }
    let mut files = counts.into_iter().collect::<Vec<_>>();
    files.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    Ok(files
        .into_iter()
        .take(limit)
        .map(|(file, count)| format!("{file} ({count} changes)"))
        .collect())
}

fn git_filtered_changes(repo_root: &Path, mode: &str, json_output: bool) -> Result<()> {
    let commits = git_commit_records(repo_root, Some("30d"), 80)?;
    let filtered = commits
        .into_iter()
        .filter(|commit| {
            let joined =
                format!("{} {}", commit.subject, commit.files.join(" ")).to_ascii_lowercase();
            match mode {
                "dependency" => {
                    joined.contains("cargo.toml")
                        || joined.contains("package.json")
                        || joined.contains("lock")
                }
                "test" => joined.contains("test") || joined.contains("spec"),
                "docs" => {
                    joined.contains(".md") || joined.contains("readme") || joined.contains("docs")
                }
                "risk" => {
                    joined.contains("unsafe")
                        || joined.contains("auth")
                        || joined.contains("schema")
                        || joined.contains("migration")
                }
                _ => true,
            }
        })
        .collect::<Vec<_>>();
    if json_output {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else if filtered.is_empty() {
        println!("no {mode} changes found");
    } else {
        for commit in filtered {
            println!("{} {}", commit.short_sha, commit.subject);
        }
    }
    Ok(())
}

fn git_forgotten_changes(repo_root: &Path, json_output: bool) -> Result<()> {
    let status = repo_status_report(repo_root);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("uncommitted or untracked changes:");
        for file in status["dirty_files"].as_array().into_iter().flatten() {
            println!("  - {}", file.as_str().unwrap_or(""));
        }
    }
    Ok(())
}

fn git_summarize_commit(repo_root: &Path, sha: &str, json_output: bool) -> Result<()> {
    let commits = git_commit_records_for_range(repo_root, sha, 1)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&commits)?);
    } else if let Some(commit) = commits.first() {
        println!("{} {}", commit.short_sha, commit.subject);
        if !commit.files.is_empty() {
            println!("files: {}", commit.files.join(", "));
        }
    } else {
        println!("commit not found: {sha}");
    }
    Ok(())
}

fn git_summarize_branch(repo_root: &Path, branch: &str, json_output: bool) -> Result<()> {
    let commits = git_commit_records_for_range(repo_root, branch, 20)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&commits)?);
    } else {
        println!("branch {branch}: {} commit(s)", commits.len());
        for commit in commits {
            println!("  - {} {}", commit.short_sha, commit.subject);
        }
    }
    Ok(())
}

fn git_compare_branches(
    repo_root: &Path,
    left: &str,
    right: &str,
    json_output: bool,
) -> Result<()> {
    let range = format!("{left}..{right}");
    let commits = git_commit_records_for_range(repo_root, &range, 64)?;
    let diff = git_stdout(repo_root, &["diff", "--stat", &range]).unwrap_or_default();
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"range": range, "commits": commits, "diff": diff})
            )?
        );
    } else {
        println!("{range}");
        println!("{diff}");
        for commit in commits {
            println!("  - {} {}", commit.short_sha, commit.subject);
        }
    }
    Ok(())
}

fn load_git_watch_state(path: &Path) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn build_git_watch_candidate(
    observation: &Value,
    repo_root: &Path,
    head: &str,
) -> Option<ExtractedCandidate> {
    let summary = observation.get("summary")?.as_str()?;
    let files = observation
        .get("files")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let lower = format!(
        "{} {}",
        summary.to_ascii_lowercase(),
        files.join(" ").to_ascii_lowercase()
    );
    let kind = if ["fix", "bug", "fail", "error", "regression"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Bug
    } else if ["architecture", "schema", "parser", "storage", "mcp"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        MemoryKind::Decision
    } else {
        MemoryKind::Workflow
    };
    let reason = format!(
        "git watch observed {}",
        observation["kind"].as_str().unwrap_or("change")
    );
    let mut tags = vec!["git-watch".to_string(), "git".to_string()];
    if files
        .iter()
        .any(|file| file.eq_ignore_ascii_case("README.md"))
    {
        tags.push("docs".to_string());
    }
    if files
        .iter()
        .any(|file| file.contains("Cargo.toml") || file.contains("package.json"))
    {
        tags.push("dependency".to_string());
    }
    if files
        .iter()
        .any(|file| file.contains("test") || file.contains("spec"))
    {
        tags.push("test".to_string());
    }
    Some(ExtractedCandidate {
        content: format!("Git watch: {summary}"),
        kind,
        confidence: 0.83,
        reason,
        tags,
        source_file: files
            .first()
            .map(|file| repo_root.join(file).display().to_string()),
        source_commit: (!head.is_empty()).then(|| head.to_string()),
    })
}

fn git_commit_records_for_range(
    root: &Path,
    range: &str,
    limit: usize,
) -> Result<Vec<GitCommitRecord>> {
    let output = ProcessCommand::new("git")
        .current_dir(root)
        .args([
            "log",
            range,
            "--name-only",
            "--pretty=format:%x1e%H%x1f%cI%x1f%s%x1f%b",
            &format!("-n{}", limit.max(1)),
        ])
        .output()
        .context("failed to run git log for watch range")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    parse_git_log_records(&String::from_utf8_lossy(&output.stdout))
}

fn ignore_command(command: &IgnoreCommand) -> Result<()> {
    match command {
        IgnoreCommand::Init { root, force } => {
            let root = root.clone().unwrap_or(env::current_dir()?);
            let path = root.join(".memoryignore");
            if path.exists() && !*force {
                return Err(anyhow!(
                    "{} already exists; use --force to overwrite it",
                    path.display()
                ));
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, DEFAULT_MEMORYIGNORE)?;
            println!("wrote {}", path.display());
        }
        IgnoreCommand::Check { path, root, json } => {
            let root = root.clone().unwrap_or(env::current_dir()?);
            let target = if path.is_absolute() {
                path.clone()
            } else {
                root.join(path)
            };
            let ignored = check_ignored_path(&root, &target)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "root": root,
                        "path": target,
                        "ignored": ignored,
                    }))?
                );
            } else {
                println!(
                    "{} -> {}",
                    target.display(),
                    if ignored { "ignored" } else { "included" }
                );
            }
        }
        IgnoreCommand::List { root, json } => {
            let root = root.clone().unwrap_or(env::current_dir()?);
            let path = root.join(".memoryignore");
            let lines = fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string)
                .collect::<Vec<_>>();
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "path": path,
                        "patterns": lines,
                    }))?
                );
            } else {
                println!("{}", path.display());
                for line in lines {
                    println!("  - {line}");
                }
            }
        }
        IgnoreCommand::Explain => {
            println!(".memoryignore keeps files out of memory import and watch flows.");
            println!(
                "Use it for secrets, generated files, dependency folders, and noisy build output."
            );
            println!("try: memory ignore init");
        }
        IgnoreCommand::Add { pattern, root } => {
            let root = root.clone().unwrap_or(env::current_dir()?);
            let path = root.join(".memoryignore");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let existing = fs::read_to_string(&path).unwrap_or_default();
            if existing.lines().any(|line| line.trim() == pattern) {
                println!("{pattern} is already in {}", path.display());
            } else {
                let mut updated = existing;
                if !updated.ends_with('\n') && !updated.is_empty() {
                    updated.push('\n');
                }
                updated.push_str(pattern);
                updated.push('\n');
                fs::write(&path, updated)?;
                println!("added {pattern} to {}", path.display());
            }
        }
        IgnoreCommand::Remove { pattern, root } => {
            let root = root.clone().unwrap_or(env::current_dir()?);
            let path = root.join(".memoryignore");
            let existing = fs::read_to_string(&path).unwrap_or_default();
            let lines = existing
                .lines()
                .filter(|line| line.trim() != pattern)
                .map(str::to_string)
                .collect::<Vec<_>>();
            fs::write(&path, format!("{}\n", lines.join("\n")))?;
            println!("removed {pattern} from {}", path.display());
        }
    }

    Ok(())
}

fn collect_repo_outline(root: &Path) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(root)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && name != ".memory.cpp" {
                return None;
            }
            let suffix = if entry.path().is_dir() { "/" } else { "" };
            Some(format!("{name}{suffix}"))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.truncate(10);

    for focus in ["crates", "docs"] {
        let dir = root.join(focus);
        if !dir.is_dir() {
            continue;
        }
        let mut children = fs::read_dir(&dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    return None;
                }
                Some(format!("{focus}/{name}"))
            })
            .collect::<Vec<_>>();
        children.sort();
        children.truncate(6);
        entries.extend(children);
    }

    entries.truncate(18);
    Ok(entries)
}

fn repo_status_report(root: &Path) -> Value {
    let branch = git_stdout(root, &["branch", "--show-current"]).ok();
    let status = git_stdout(root, &["status", "--short"]).unwrap_or_default();
    let dirty_files = status
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>();
    let ahead_behind = git_stdout(root, &["status", "--short", "--branch"])
        .ok()
        .and_then(|value| value.lines().next().map(str::to_string));
    let current_commit = git_stdout(root, &["rev-parse", "--short", "HEAD"]).ok();
    json!({
        "root": root,
        "branch": branch.filter(|value| !value.is_empty()).unwrap_or_else(|| "unknown".to_string()),
        "current_commit": current_commit,
        "ahead_behind": ahead_behind,
        "dirty_count": dirty_files.len(),
        "dirty_files": dirty_files,
    })
}

fn collect_todos(root: &Path, limit: usize) -> Vec<TodoHit> {
    let files = collect_importable_files(root, true).unwrap_or_default();
    let mut hits = Vec::new();
    for file in files {
        if hits.len() >= limit {
            break;
        }
        let path = file.to_string_lossy();
        if path.contains("\\target\\")
            || path.contains("/target/")
            || path.contains("\\.git\\")
            || path.contains("/.git/")
        {
            continue;
        }
        let extension = file
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let source_file = matches!(
            extension.as_str(),
            "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "c" | "cpp" | "h" | "hpp"
        );
        let Ok(raw) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in raw.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            let trimmed = line.trim_start();
            if source_file
                && !trimmed.starts_with("//")
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("/*")
                && !trimmed.starts_with('*')
            {
                continue;
            }
            if line.contains("TODO:")
                || line.contains("TODO ")
                || line.contains("FIXME:")
                || line.contains("FIXME ")
                || lower.contains("todo:")
                || lower.contains("fixme:")
            {
                hits.push(TodoHit {
                    path: file
                        .strip_prefix(root)
                        .unwrap_or(&file)
                        .display()
                        .to_string(),
                    line: index + 1,
                    text: line.trim().to_string(),
                });
                if hits.len() >= limit {
                    break;
                }
            }
        }
    }
    hits
}

fn important_files(root: &Path) -> Vec<String> {
    [
        "README.md",
        "Cargo.toml",
        "package.json",
        "Makefile",
        "justfile",
        "scripts/smoke.sh",
        "scripts/smoke.ps1",
        ".github/workflows/ci.yml",
        "crates/memory-cli/src/main.rs",
        "crates/memory-core/src/lib.rs",
    ]
    .into_iter()
    .filter(|path| root.join(path).exists())
    .map(str::to_string)
    .collect()
}

fn infer_run_commands(root: &Path) -> Vec<String> {
    let mut commands = Vec::new();
    if root.join("Cargo.toml").exists() {
        commands.push("cargo run -- --help".to_string());
        commands.push("cargo test".to_string());
    }
    if root.join("package.json").exists() {
        commands.push("npm test".to_string());
        commands.push("npm run dev".to_string());
    }
    if root.join("scripts/smoke.ps1").exists() {
        commands.push("powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1".to_string());
    }
    if root.join("scripts/smoke.sh").exists() {
        commands.push("bash scripts/smoke.sh".to_string());
    }
    commands
}

fn read_readme_brief(root: &Path) -> Option<String> {
    let readme = root.join("README.md");
    let raw = fs::read_to_string(readme).ok()?;
    let brief = raw
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with('!')
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    (!brief.is_empty()).then_some(brief)
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn resolve_repo_root(path: &Path) -> Option<PathBuf> {
    let workdir = if path.is_file() {
        path.parent().unwrap_or_else(|| Path::new("."))
    } else {
        path
    };
    git_repo_root(workdir)
}

fn extract_candidates_from_path(
    path: &Path,
    kind_hint: Option<MemoryKind>,
    limit: usize,
) -> Result<Vec<ExtractedCandidate>> {
    let files = if path.is_file() {
        vec![path.to_path_buf()]
    } else {
        collect_importable_files(path, true)?
    };
    let mut files = files;
    files.sort();

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for file in files.into_iter().take(limit.max(8) * 4) {
        let raw = match fs::read_to_string(&file) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for line in extract_candidate_lines(&file, &raw) {
            let Some(candidate) = build_extracted_candidate(
                &line,
                kind_hint,
                Some(file.to_string_lossy().to_string()),
                None,
                "repo extraction".to_string(),
            ) else {
                continue;
            };
            let key = candidate.content.to_ascii_lowercase();
            if seen.insert(key) {
                out.push(candidate);
            }
            if out.len() >= limit {
                return Ok(out);
            }
        }
    }

    Ok(out)
}

fn extract_candidates_from_git(
    path: &Path,
    kind_hint: Option<MemoryKind>,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<ExtractedCandidate>> {
    let Some(repo_root) = resolve_repo_root(path) else {
        return Ok(Vec::new());
    };
    let commits = git_commit_records(&repo_root, since, limit.max(8))?;
    let mut out = Vec::new();
    for commit in commits {
        let mut content = commit.subject.trim().to_string();
        if !commit.body.trim().is_empty() {
            let body = commit
                .body
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or_default()
                .trim();
            if !body.is_empty() {
                content = format!("{content}. {body}");
            }
        }
        let Some(candidate) = build_extracted_candidate(
            &content,
            kind_hint,
            commit.files.first().cloned(),
            Some(commit.sha.clone()),
            format!("git commit {}", commit.short_sha),
        ) else {
            continue;
        };
        out.push(candidate);
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

fn extract_candidate_lines(path: &Path, raw: &str) -> Vec<String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    let treat_as_source = matches!(
        extension.as_str(),
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "c" | "cpp" | "h" | "hpp"
    );

    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("```") {
                return None;
            }
            if treat_as_source
                && !trimmed.starts_with("//")
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("/*")
                && !trimmed.starts_with('*')
                && !trimmed.contains("TODO")
                && !trimmed.contains("FIXME")
            {
                return None;
            }
            let normalized = sanitize_candidate_text(trimmed);
            if normalized.len() < 24 {
                None
            } else {
                Some(normalized)
            }
        })
        .collect()
}

fn sanitize_candidate_text(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('/')
        .trim_start_matches('*')
        .trim_start_matches('-')
        .trim_start_matches(':')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_extracted_candidate(
    content: &str,
    kind_hint: Option<MemoryKind>,
    source_file: Option<String>,
    source_commit: Option<String>,
    reason: String,
) -> Option<ExtractedCandidate> {
    let content = sanitize_candidate_text(content);
    if content.len() < 24 || content.len() > 320 || detect_sensitive_reason(&content).is_some() {
        return None;
    }

    let lower = content.to_ascii_lowercase();
    let (mut kind, mut confidence, mut tags): (MemoryKind, f32, Vec<String>) = if lower
        .contains("todo")
        || lower.contains("fixme")
        || lower.starts_with("next ")
        || lower.starts_with("next:")
    {
        (MemoryKind::Task, 0.84, vec!["task".to_string()])
    } else if [
        "bug",
        "fix",
        "regression",
        "timeout",
        "crash",
        "error",
        "failure",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        (MemoryKind::Bug, 0.82, vec!["bug".to_string()])
    } else if [
        "decision",
        "because",
        "chosen",
        "default",
        "local-first",
        "read-only",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        (MemoryKind::Decision, 0.79, vec!["decision".to_string()])
    } else if ["prefer", "always", "never", "should"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        (MemoryKind::Preference, 0.76, vec!["preference".to_string()])
    } else if [
        "workflow", "run ", "use ", "attach", "proxy", "watch", "command",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        (MemoryKind::Workflow, 0.72, vec!["workflow".to_string()])
    } else if ["roadmap", "milestone", "release"]
        .iter()
        .any(|needle| lower.contains(needle))
    {
        (MemoryKind::Fact, 0.68, vec!["roadmap".to_string()])
    } else {
        return None;
    };

    if let Some(expected) = kind_hint {
        if expected != kind {
            if expected == MemoryKind::Decision
                && matches!(
                    kind,
                    MemoryKind::Preference | MemoryKind::Workflow | MemoryKind::Fact
                )
            {
                kind = MemoryKind::Decision;
                confidence = (confidence - 0.06f32).clamp(0.55f32, 1.0f32);
            } else {
                return None;
            }
        }
    }

    if source_commit.is_some() {
        tags.push("git".to_string());
    } else if source_file.is_some() {
        tags.push("extract".to_string());
    }
    tags.sort();
    tags.dedup();

    Some(ExtractedCandidate {
        content,
        kind,
        confidence,
        reason,
        tags,
        source_file,
        source_commit,
    })
}

fn extracted_candidate_to_memory(
    candidate: &ExtractedCandidate,
    workspace: &str,
    force_pending_review: bool,
) -> NewMemory {
    let status = if force_pending_review || candidate.confidence < 0.8 {
        MemoryStatus::PendingReview
    } else {
        MemoryStatus::Active
    };
    let importance = match candidate.kind {
        MemoryKind::Decision | MemoryKind::Bug | MemoryKind::Workflow => 0.78,
        MemoryKind::Task => 0.74,
        MemoryKind::Preference => 0.7,
        _ => 0.62,
    };
    NewMemory::new(candidate.content.clone())
        .scope(workspace.to_string())
        .kind(candidate.kind.as_str())
        .importance(importance)
        .confidence(candidate.confidence)
        .tags(candidate.tags.clone())
        .status(status)
        .source(MemorySource {
            source_type: Some(if candidate.source_commit.is_some() {
                "git_extract".to_string()
            } else {
                "repo_extract".to_string()
            }),
            source_app: Some("memory.cpp".to_string()),
            source: Some(candidate.reason.clone()),
            source_file: candidate.source_file.clone(),
            source_line: None,
            source_commit: candidate.source_commit.clone(),
            source_conversation_id: None,
            source_message_id: None,
            created_by: Some("extract".to_string()),
            reliability: Some(candidate.confidence),
        })
}

fn git_commit_records(
    root: &Path,
    since: Option<&str>,
    limit: usize,
) -> Result<Vec<GitCommitRecord>> {
    let mut command = ProcessCommand::new("git");
    command.current_dir(root).args([
        "log",
        "--name-only",
        "--pretty=format:%x1e%H%x1f%cI%x1f%s%x1f%b",
        &format!("-n{}", limit.max(1)),
    ]);
    if let Some(since) = since {
        command.arg(format!("--since={}", normalize_since_arg(since)));
    }
    let output = command.output().context("failed to run git log")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git log failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    parse_git_log_records(&String::from_utf8_lossy(&output.stdout))
}

fn parse_git_log_records(raw: &str) -> Result<Vec<GitCommitRecord>> {
    let mut commits = Vec::new();
    for chunk in raw.split('\u{1e}') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        let mut lines = chunk.lines();
        let Some(header) = lines.next() else {
            continue;
        };
        let mut parts = header.split('\u{1f}');
        let sha = parts.next().unwrap_or_default().trim().to_string();
        if sha.is_empty() {
            continue;
        }
        let committed_at = parts.next().unwrap_or_default().trim().to_string();
        let subject = parts.next().unwrap_or_default().trim().to_string();
        let body = parts.next().unwrap_or_default().trim().to_string();
        let files = lines
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        commits.push(GitCommitRecord {
            short_sha: sha.chars().take(7).collect(),
            sha,
            committed_at,
            subject,
            body,
            files,
        });
    }
    Ok(commits)
}

fn normalize_since_arg(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(days) = trimmed.strip_suffix('d') {
        if days.parse::<u64>().is_ok() {
            return format!("{days} days ago");
        }
    }
    if let Some(hours) = trimmed.strip_suffix('h') {
        if hours.parse::<u64>().is_ok() {
            return format!("{hours} hours ago");
        }
    }
    if let Some(weeks) = trimmed.strip_suffix('w') {
        if weeks.parse::<u64>().is_ok() {
            return format!("{weeks} weeks ago");
        }
    }
    trimmed.to_string()
}

#[allow(clippy::too_many_arguments)]
fn map_command(
    engine: &MemoryEngine,
    path: Option<&Path>,
    project: Option<&String>,
    workspace: Option<&String>,
    map_type: CliMapType,
    output: CliMapOutput,
    from: Option<&str>,
    to: Option<&str>,
    chronological: bool,
    why: bool,
    impact: Option<&str>,
    compare_left: Option<&str>,
    compare_right: Option<&str>,
    save: Option<&Path>,
) -> Result<()> {
    let request = build_map_request(
        path,
        project,
        workspace,
        map_type,
        output.clone(),
        from,
        to,
        chronological,
        why,
        impact,
    )?;

    if let (Some(left), Some(right)) = (compare_left, compare_right) {
        let diff = engine.compare_maps(&request, left, right)?;
        let rendered = if matches!(request.output, MapOutputFormat::Json) {
            serde_json::to_string_pretty(&diff)?
        } else {
            diff.render_markdown()
        };
        emit_or_save(&rendered, save)?;
        return Ok(());
    }

    let map = engine.build_map(&request)?;
    emit_or_save(&map.render(request.output)?, save)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeState {
    name: String,
    pid: u32,
    health_url: Option<String>,
    log_out: String,
    log_err: String,
    workspace: Option<String>,
    db: String,
    started_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
fn start_command(
    options: &EngineOptions,
    host: &str,
    port: u16,
    proxy: bool,
    proxy_listen: &str,
    upstream: &str,
    workspace: Option<&String>,
    limit: usize,
    tokens: usize,
) -> Result<()> {
    let runtime_dir = runtime_dir(options)?;
    fs::create_dir_all(&runtime_dir)?;
    let exe = env::current_exe()?;
    let global_args = build_global_args(options);

    spawn_runtime_process(
        &runtime_dir,
        "server",
        &exe,
        &global_args,
        &[
            "dashboard".to_string(),
            "--host".to_string(),
            host.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
        Some(format!("http://{}:{}/health", host, port)),
        workspace.cloned(),
        options,
    )?;

    if proxy {
        let mut args = vec![
            "proxy".to_string(),
            "--listen".to_string(),
            proxy_listen.to_string(),
            "--upstream".to_string(),
            upstream.to_string(),
            "--limit".to_string(),
            limit.to_string(),
            "--tokens".to_string(),
            tokens.to_string(),
            "--learn".to_string(),
            "--approval-required".to_string(),
        ];
        if let Some(workspace) = workspace {
            args.push("--workspace".to_string());
            args.push(workspace.clone());
        }
        spawn_runtime_process(
            &runtime_dir,
            "proxy",
            &exe,
            &global_args,
            &args,
            Some(format!("http://{}/health", proxy_listen)),
            workspace.cloned(),
            options,
        )?;
    }

    println!("runtime started in {}", runtime_dir.display());
    status_command(options)
}

fn stop_command(options: &EngineOptions) -> Result<()> {
    let runtime_dir = runtime_dir(options)?;
    if !runtime_dir.exists() {
        println!("no runtime directory at {}", runtime_dir.display());
        return Ok(());
    }

    let mut stopped = 0usize;
    for state_file in runtime_state_files(&runtime_dir)? {
        let state: RuntimeState = serde_json::from_str(&fs::read_to_string(&state_file)?)?;
        if pid_is_alive(state.pid)? {
            terminate_pid(state.pid)?;
            stopped += 1;
            println!("stopped {} (pid {})", state.name, state.pid);
        } else {
            println!("removed stale runtime state for {}", state.name);
        }
        fs::remove_file(state_file)?;
    }

    if stopped == 0 {
        println!("no active memory.cpp runtime processes found");
    }
    Ok(())
}

fn status_command(options: &EngineOptions) -> Result<()> {
    let runtime_dir = runtime_dir(options)?;
    if !runtime_dir.exists() {
        println!("runtime: stopped");
        return Ok(());
    }

    let state_files = runtime_state_files(&runtime_dir)?;
    if state_files.is_empty() {
        println!("runtime: stopped");
        return Ok(());
    }

    for state_file in state_files {
        let state: RuntimeState = serde_json::from_str(&fs::read_to_string(&state_file)?)?;
        let alive = pid_is_alive(state.pid)?;
        let health = match &state.health_url {
            Some(url) if alive => ureq::get(url)
                .call()
                .ok()
                .map(|response| response.status().to_string())
                .unwrap_or_else(|| "unreachable".to_string()),
            _ => "unavailable".to_string(),
        };
        println!(
            "{}: {} | pid={} | health={} | db={}{}",
            state.name,
            if alive { "running" } else { "stale" },
            state.pid,
            health,
            state.db,
            state
                .workspace
                .as_ref()
                .map(|workspace| format!(" | workspace={workspace}"))
                .unwrap_or_default()
        );
    }

    Ok(())
}

fn demo_seed_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    path: Option<&PathBuf>,
    json_output: bool,
) -> Result<()> {
    let workspace = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .unwrap_or_else(|| "demo".to_string());
    let existing = engine
        .list_workspaces()?
        .into_iter()
        .find(|entry| entry.name == workspace);
    if existing.is_some() {
        engine.switch_workspace(&workspace)?;
    } else {
        engine.create_workspace(
            &workspace,
            "launch-ready demo workspace for memory.cpp",
            "project",
            true,
        )?;
    }
    set_default_workspace(engine.store_path(), &workspace)?;

    let now = Utc::now();
    let repo_root = path
        .cloned()
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut seen = engine
        .list_recent(Some(&workspace), 512)?
        .into_iter()
        .map(|memory| memory.content)
        .collect::<std::collections::HashSet<_>>();
    let mut seeded = Vec::new();

    let seeds = vec![
        demo_memory_seed(
            "memory.cpp aims to be SQLite for engineering memory: one local memory layer for developers and AI apps.",
            MemoryKind::Decision,
            &workspace,
            0.98,
            0.97,
            now - ChronoDuration::days(18),
            &["vision", "launch"],
            Some("README.md"),
        ),
        demo_memory_seed(
            "Use SQLite as the core store so memory stays local-first, portable, auditable, and easy to back up.",
            MemoryKind::Decision,
            &workspace,
            0.97,
            0.96,
            now - ChronoDuration::days(17),
            &["storage", "sqlite", "local-first"],
            Some("crates/memory-core/src/storage.rs"),
        ),
        demo_memory_seed(
            "Hybrid retrieval mixes semantic similarity, keyword matching, entity overlap, recency, importance, and confidence.",
            MemoryKind::Fact,
            &workspace,
            0.90,
            0.93,
            now - ChronoDuration::days(15),
            &["retrieval", "hybrid", "ranking"],
            Some("crates/memory-core/src/ranker.rs"),
        ),
        demo_memory_seed(
            "Expose memory through MCP so Cursor, Claude, Codex, and VS Code can use memory.cpp without custom integrations.",
            MemoryKind::Decision,
            &workspace,
            0.96,
            0.95,
            now - ChronoDuration::days(13),
            &["mcp", "integrations", "agents"],
            Some("crates/memory-cli/src/main.rs"),
        ),
        demo_memory_seed(
            "The viral demo is memory proxy plus memory map evolution: every local chat remembers and every repo can explain itself.",
            MemoryKind::Decision,
            &workspace,
            0.99,
            0.94,
            now - ChronoDuration::days(11),
            &["proxy", "map", "demo"],
            Some("README.md"),
        ),
        demo_memory_seed(
            "Bug: the expanded Clap command tree hit a stack-overflow edge case, so v0.2.1 keeps a documented pre-parser with dedicated tests.",
            MemoryKind::Bug,
            &workspace,
            0.89,
            0.92,
            now - ChronoDuration::hours(16),
            &["cli", "parser", "bug"],
            Some("crates/memory-cli/src/main.rs"),
        ),
        demo_memory_seed(
            "Fix: split launch-only commands behind a small manual parser, then cover them with smoke tests and parser unit tests.",
            MemoryKind::Workflow,
            &workspace,
            0.87,
            0.90,
            now - ChronoDuration::hours(14),
            &["cli", "tests", "workflow"],
            Some("crates/memory-cli/src/main.rs"),
        ),
        demo_memory_seed(
            "memory dev morning should summarize yesterday's work, open conflicts, recent decisions, recent bugs, and the next recommended action.",
            MemoryKind::Workflow,
            &workspace,
            0.93,
            0.94,
            now - ChronoDuration::hours(11),
            &["dev", "recap", "workflow"],
            Some("crates/memory-cli/src/main.rs"),
        ),
        demo_memory_seed(
            "memory map evolution should show idea, storage, retrieval, MCP, proxy, attach, and launch polish as a chronological project story.",
            MemoryKind::Decision,
            &workspace,
            0.97,
            0.95,
            now - ChronoDuration::hours(9),
            &["map", "evolution", "chronology"],
            Some("crates/memory-core/src/map.rs"),
        ),
        demo_memory_seed(
            "Attach helpers should default to read-only MCP with workspace scoping and credential redaction so the first integration feels trustworthy.",
            MemoryKind::Decision,
            &workspace,
            0.94,
            0.92,
            now - ChronoDuration::hours(7),
            &["attach", "safety", "mcp"],
            Some("crates/memory-cli/src/main.rs"),
        ),
        demo_memory_seed(
            "memory doctor should verify database health, runtime state, map exportability, MCP safety defaults, and Ollama reachability before launch.",
            MemoryKind::Task,
            &workspace,
            0.78,
            0.88,
            now - ChronoDuration::hours(5),
            &["doctor", "launch", "ops"],
            Some("crates/memory-cli/src/main.rs"),
        ),
    ];

    for seed in seeds {
        if seen.insert(seed.content.clone()) {
            seeded.push(engine.remember(seed)?);
        }
    }

    let _ = engine.remember_candidate(
        NewMemory::new(
            "Consider adding Homebrew packaging and a public binary release before calling the launch path complete.",
        )
        .scope(workspace.clone())
        .kind("task")
        .confidence(0.42)
        .tag("candidate")
        .tag("release")
        .source(MemorySource {
            source_type: Some("demo_seed".to_string()),
            source_app: Some("memory.cpp".to_string()),
            source: Some("launch readiness".to_string()),
            source_file: Some("README.md".to_string()),
            source_line: None,
            source_commit: None,
            source_conversation_id: None,
            source_message_id: None,
            created_by: Some("demo-seed".to_string()),
            reliability: Some(0.42),
        })
        .status(MemoryStatus::PendingReview),
        "demo seed candidate memory",
    )?;

    let _ = engine.snapshot_named("demo-foundation", &workspace);
    let _ = engine.snapshot_named("demo-launch-core", &workspace);

    let demo_dir = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("demo");
    fs::create_dir_all(&demo_dir)?;

    for (name, request) in [
        (
            "evolution.html",
            MapRequest {
                path: Some(repo_root.clone()),
                project: Some("memory.cpp".to_string()),
                workspace: Some(workspace.clone()),
                map_type: MapType::Evolution,
                output: MapOutputFormat::Html,
                chronological: true,
                why: true,
                limit: 64,
                ..Default::default()
            },
        ),
        (
            "evolution.mmd",
            MapRequest {
                path: Some(repo_root.clone()),
                project: Some("memory.cpp".to_string()),
                workspace: Some(workspace.clone()),
                map_type: MapType::Evolution,
                output: MapOutputFormat::Mermaid,
                chronological: true,
                why: true,
                limit: 64,
                ..Default::default()
            },
        ),
        (
            "decisions.md",
            MapRequest {
                path: Some(repo_root.clone()),
                project: Some("memory.cpp".to_string()),
                workspace: Some(workspace.clone()),
                map_type: MapType::Decisions,
                output: MapOutputFormat::Markdown,
                why: true,
                limit: 48,
                ..Default::default()
            },
        ),
        (
            "architecture.mmd",
            MapRequest {
                path: Some(repo_root.clone()),
                project: Some("memory.cpp".to_string()),
                workspace: Some(workspace.clone()),
                map_type: MapType::Architecture,
                output: MapOutputFormat::Mermaid,
                limit: 48,
                ..Default::default()
            },
        ),
    ] {
        let map = engine.build_map(&request)?;
        fs::write(demo_dir.join(name), map.render(request.output)?)?;
    }

    let report = json!({
        "workspace": workspace,
        "seeded": seeded.len(),
        "demo_dir": demo_dir,
        "repo_path": repo_root,
        "next_commands": [
            format!("memory --db {} dev morning --workspace {}", engine.store_path().display(), current_workspace_name(engine)?.unwrap_or_else(|| "demo".to_string())),
            format!("memory --db {} map {} --workspace {} --type evolution --output html --save {}", engine.store_path().display(), repo_root.display(), current_workspace_name(engine)?.unwrap_or_else(|| "demo".to_string()), demo_dir.join("evolution.html").display()),
            format!("memory --db {} attach cursor --workspace {}", engine.store_path().display(), current_workspace_name(engine)?.unwrap_or_else(|| "demo".to_string())),
        ],
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "seeded demo workspace: {}",
            report["workspace"].as_str().unwrap_or("demo")
        );
        println!("new memories: {}", report["seeded"].as_u64().unwrap_or(0));
        println!("demo artifacts:");
        println!("  - {}", demo_dir.join("evolution.html").display());
        println!("  - {}", demo_dir.join("evolution.mmd").display());
        println!("  - {}", demo_dir.join("decisions.md").display());
        println!("  - {}", demo_dir.join("architecture.mmd").display());
        println!("next:");
        for command in report["next_commands"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if let Some(command) = command.as_str() {
                println!("  - {command}");
            }
        }
    }

    Ok(())
}

fn demo_reset_command(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    path: Option<&PathBuf>,
    json_output: bool,
) -> Result<()> {
    let workspace = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .unwrap_or_else(|| "demo".to_string());
    let repo_root = path
        .cloned()
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let demo_dir = engine
        .store_path()
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("demo");

    let removed = if demo_dir.exists() {
        fs::remove_dir_all(&demo_dir)?;
        true
    } else {
        false
    };

    let report = json!({
        "workspace": workspace,
        "repo_path": repo_root,
        "removed_demo_dir": if removed { Some(demo_dir.to_string_lossy().to_string()) } else { None::<String> },
        "note": "Removed generated demo artifacts only. Stored demo memories remain in the database so reseeding stays idempotent and existing demo workspaces keep working."
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "demo reset for workspace `{}`",
            report["workspace"].as_str().unwrap_or("demo")
        );
        println!(
            "removed artifacts: {}",
            report["removed_demo_dir"].as_str().unwrap_or("none")
        );
        println!(
            "{}",
            report["note"]
                .as_str()
                .unwrap_or("Stored demo memories remain unchanged.")
        );
    }

    Ok(())
}

fn doctor_command(
    engine: &MemoryEngine,
    options: &EngineOptions,
    workspace: Option<&String>,
    json_output: bool,
) -> Result<()> {
    let workspace = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .or(load_app_config(engine.store_path())?.default_workspace);
    let mut checks = Vec::new();

    let db_path = engine.store_path();
    checks.push(if db_path.exists() {
        ok_check("database", format!("found {}", db_path.display()))
    } else {
        warn_check(
            "database",
            format!("expected {}", db_path.display()),
            "run `memory init` to create the local store",
        )
    });

    checks.push(match engine.stats() {
        Ok(stats) => ok_check(
            "schema",
            format!(
                "opened successfully with {} memories and {} workspaces",
                stats.memories, stats.workspaces
            ),
        ),
        Err(err) => error_check(
            "schema",
            format!("failed to read stats: {err}"),
            "run `memory doctor` after recreating the database or restoring a backup",
        ),
    });

    checks.push(match workspace.clone() {
        Some(workspace) => ok_check("workspace", format!("active workspace is {workspace}")),
        None => warn_check(
            "workspace",
            "no workspace selected".to_string(),
            "run `memory init --workspace demo` or `memory workspace switch <name>`",
        ),
    });

    let cwd = env::current_dir()?;
    let repo_root = git_repo_root(cwd.as_path());
    checks.push(match repo_root.clone() {
        Some(root) => {
            let status = repo_status_report(&root);
            ok_check(
                "git",
                format!(
                    "detected git repository at {}; branch {}; dirty files {}",
                    root.display(),
                    status["branch"].as_str().unwrap_or("unknown"),
                    status["dirty_count"].as_u64().unwrap_or(0)
                ),
            )
        }
        None => warn_check(
            "git",
            "no git repository detected from the current directory".to_string(),
            "run memory.cpp from a repo root to enrich maps and dev workflows",
        ),
    });

    let cursor_config = cwd.join(".cursor").join("mcp.json");
    checks.push(if cursor_config.exists() {
        ok_check(
            "cursor-config",
            format!("found {}", cursor_config.display()),
        )
    } else {
        warn_check(
            "cursor-config",
            "Cursor MCP config not found in this repo".to_string(),
            "run `memory attach cursor --workspace <name>`",
        )
    });

    let claude_config = cwd.join(".claude").join("claude_desktop_config.json");
    checks.push(if claude_config.exists() {
        ok_check(
            "claude-config",
            format!("found {}", claude_config.display()),
        )
    } else {
        warn_check(
            "claude-config",
            "Claude config not found in this repo".to_string(),
            "run `memory attach claude --workspace <name>` if you use Claude Desktop",
        )
    });

    let mcp = resolve_mcp_runtime_config(engine, workspace.as_ref(), false, false, None)?;
    checks.push(if mcp.allow_writes {
        warn_check(
            "mcp-safety",
            "MCP write operations are enabled".to_string(),
            "prefer read-only MCP by default and only enable writes intentionally",
        )
    } else {
        ok_check(
            "mcp-safety",
            format!(
                "read-only by default, workspace scoped to {}, audit log at {}",
                mcp.workspace
                    .clone()
                    .unwrap_or_else(|| "current/default".to_string()),
                mcp.audit_log.display()
            ),
        )
    });

    checks.push(if mcp.redact_sensitive {
        ok_check(
            "redaction",
            "secret redaction is enabled for MCP responses".to_string(),
        )
    } else {
        warn_check(
            "redaction",
            "MCP redaction is disabled".to_string(),
            "restart `memory mcp` without `--no-redaction` for safer defaults",
        )
    });

    let ollama_endpoint = options
        .endpoint
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    checks.push(match check_ollama(&ollama_endpoint) {
        Ok(true) => ok_check("ollama", format!("reachable at {ollama_endpoint}")),
        Ok(false) => warn_check(
            "ollama",
            format!("not reachable at {ollama_endpoint}"),
            "start Ollama or use `memory attach ollama --start-proxy` later",
        ),
        Err(err) => warn_check(
            "ollama",
            format!("could not probe {ollama_endpoint}: {err}"),
            "verify the endpoint or ignore this if you are using offline hash embeddings",
        ),
    });

    let demo_dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .join("demo");
    checks.push(match ensure_writable_dir(&demo_dir) {
        Ok(()) => ok_check(
            "map-output",
            format!(
                "demo/export directory is writable at {}",
                demo_dir.display()
            ),
        ),
        Err(err) => error_check(
            "map-output",
            format!("{} is not writable: {err}", demo_dir.display()),
            "fix filesystem permissions before generating HTML map exports",
        ),
    });

    let runtime_dir = runtime_dir(options)?;
    let runtime_state_count = if runtime_dir.exists() {
        runtime_state_files(&runtime_dir)?.len()
    } else {
        0
    };
    checks.push(ok_check(
        "runtime",
        if runtime_state_count == 0 {
            "no background runtime processes are active".to_string()
        } else {
            format!("{runtime_state_count} runtime state file(s) found")
        },
    ));

    checks.push(match port_available("127.0.0.1:7331") {
        Ok(true) => ok_check("api-port", "127.0.0.1:7331 is available".to_string()),
        Ok(false) => warn_check(
            "api-port",
            "127.0.0.1:7331 is already in use".to_string(),
            "stop the current runtime or use `memory serve --port <port>`",
        ),
        Err(err) => warn_check(
            "api-port",
            format!("could not test port availability: {err}"),
            "check local firewall or socket permissions",
        ),
    });

    checks.push(match port_available("127.0.0.1:7332") {
        Ok(true) => ok_check("proxy-port", "127.0.0.1:7332 is available".to_string()),
        Ok(false) => warn_check(
            "proxy-port",
            "127.0.0.1:7332 is already in use".to_string(),
            "run `memory status` to see whether the memory proxy is active",
        ),
        Err(err) => warn_check(
            "proxy-port",
            format!("could not test proxy port: {err}"),
            "check local firewall or socket permissions",
        ),
    });

    if let Some(root) = repo_root {
        let state = root
            .join(".memory.cpp")
            .join("git-watch")
            .join("state.json");
        checks.push(if state.exists() {
            ok_check(
                "git-watch",
                format!("watch baseline found at {}", state.display()),
            )
        } else {
            warn_check(
                "git-watch",
                "no git watch baseline found".to_string(),
                "run `memory git watch --once` to start automatic repo memory",
            )
        });
    }

    let smoke_ps1 = cwd.join("scripts").join("smoke.ps1");
    let smoke_sh = cwd.join("scripts").join("smoke.sh");
    checks.push(if smoke_ps1.exists() || smoke_sh.exists() {
        ok_check(
            "smoke-tests",
            format!(
                "found {}{}",
                if smoke_ps1.exists() {
                    "scripts/smoke.ps1"
                } else {
                    ""
                },
                if smoke_sh.exists() {
                    " scripts/smoke.sh"
                } else {
                    ""
                }
            ),
        )
    } else {
        warn_check(
            "smoke-tests",
            "no smoke-test script detected".to_string(),
            "add scripts/smoke.ps1 or scripts/smoke.sh",
        )
    });

    let report = DoctorReport {
        store: db_path.display().to_string(),
        workspace,
        checks,
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("memory.cpp doctor");
        println!("store: {}", report.store);
        if let Some(workspace) = &report.workspace {
            println!("workspace: {workspace}");
        }
        for check in &report.checks {
            let icon = match check.status.as_str() {
                "ok" => "[ok]",
                "warn" => "[warn]",
                _ => "[error]",
            };
            println!("{icon} {}: {}", check.name, check.detail);
            if let Some(suggestion) = &check.suggestion {
                println!("  suggestion: {suggestion}");
            }
        }
    }

    Ok(())
}

fn audit_log_command(
    engine: &MemoryEngine,
    limit: usize,
    explicit_path: Option<&Path>,
    json_output: bool,
) -> Result<()> {
    let audit_path = explicit_path.map(Path::to_path_buf).unwrap_or_else(|| {
        engine
            .store_path()
            .parent()
            .unwrap_or_else(|| Path::new(".memory.cpp"))
            .join("audit")
            .join("mcp-access.jsonl")
    });

    if !audit_path.exists() {
        if json_output {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "path": audit_path,
                    "entries": [],
                    "note": "No audit log has been recorded yet."
                }))?
            );
        } else {
            println!("no audit log found at {}", audit_path.display());
            println!("run `memory mcp` or use an attached client to generate access receipts.");
        }
        return Ok(());
    }

    let file = File::open(&audit_path)?;
    let mut entries = io::BufReader::new(file)
        .lines()
        .map_while(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<StoredAuditLogEntry>(&line).ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.recorded_at));
    entries.truncate(limit.max(1));

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "path": audit_path,
                "entries": entries,
            }))?
        );
    } else {
        println!("audit log: {}", audit_path.display());
        for entry in entries {
            println!(
                "{} | {} | {} | workspace={} | {}",
                entry.recorded_at.to_rfc3339(),
                entry.channel,
                entry.action,
                entry.workspace.unwrap_or_else(|| "default".to_string()),
                if entry.allowed { "allowed" } else { "blocked" }
            );
            println!("  {}", entry.detail);
        }
    }

    Ok(())
}

fn build_global_args(options: &EngineOptions) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(db) = &options.db {
        args.push("--db".to_string());
        args.push(db.display().to_string());
    }
    args.push("--embedder".to_string());
    args.push(
        match options.embedder {
            EmbedderChoice::Hash => "hash",
            EmbedderChoice::Ollama => "ollama",
            EmbedderChoice::Openai => "openai",
            EmbedderChoice::Fastembed => "fastembed",
            EmbedderChoice::Onnx => "onnx",
        }
        .to_string(),
    );
    if let Some(endpoint) = &options.endpoint {
        args.push("--endpoint".to_string());
        args.push(endpoint.clone());
    }
    if let Some(model) = &options.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    if options.dimensions != 384 {
        args.push("--dimensions".to_string());
        args.push(options.dimensions.to_string());
    }
    if options.api_key_env != "MEMORY_CPP_OPENAI_API_KEY" {
        args.push("--api-key-env".to_string());
        args.push(options.api_key_env.clone());
    }
    args
}

#[allow(clippy::too_many_arguments)]
fn spawn_runtime_process(
    runtime_dir: &Path,
    name: &str,
    exe: &Path,
    global_args: &[String],
    command_args: &[String],
    health_url: Option<String>,
    workspace: Option<String>,
    options: &EngineOptions,
) -> Result<()> {
    let state_path = runtime_dir.join(format!("{name}.json"));
    if state_path.exists() {
        let state: RuntimeState = serde_json::from_str(&fs::read_to_string(&state_path)?)?;
        if pid_is_alive(state.pid)? {
            println!("{name} already running on pid {}", state.pid);
            return Ok(());
        }
        fs::remove_file(&state_path).ok();
    }

    let stdout_path = runtime_dir.join(format!("{name}.out.log"));
    let stderr_path = runtime_dir.join(format!("{name}.err.log"));
    let stdout = File::create(&stdout_path)?;
    let stderr = File::create(&stderr_path)?;

    let mut command = ProcessCommand::new(exe);
    command
        .args(global_args)
        .args(command_args)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    let child = command
        .spawn()
        .with_context(|| format!("failed to start {name}"))?;
    let state = RuntimeState {
        name: name.to_string(),
        pid: child.id(),
        health_url,
        log_out: stdout_path.display().to_string(),
        log_err: stderr_path.display().to_string(),
        workspace,
        db: options
            .db
            .clone()
            .unwrap_or_else(|| PathBuf::from(".memory.cpp/memory.db"))
            .display()
            .to_string(),
        started_at: Utc::now(),
    };
    fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;
    Ok(())
}

fn runtime_dir(options: &EngineOptions) -> Result<PathBuf> {
    let db = options
        .db
        .clone()
        .or_else(|| env::var_os("MEMORY_CPP_DB").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".memory.cpp/memory.db"));
    let parent = db
        .parent()
        .unwrap_or_else(|| Path::new(".memory.cpp"))
        .to_path_buf();
    Ok(parent.join("runtime"))
}

fn runtime_state_files(runtime_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(runtime_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn pid_is_alive(pid: u32) -> Result<bool> {
    #[cfg(target_os = "windows")]
    {
        let output = ProcessCommand::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(output.status.success() && text.contains(&pid.to_string()))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = ProcessCommand::new("kill")
            .args(["-0", &pid.to_string()])
            .status()?;
        Ok(status.success())
    }
}

fn terminate_pid(pid: u32) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let status = ProcessCommand::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()?;
        if !status.success() {
            if !pid_is_alive(pid)? {
                return Ok(());
            }
            return Err(anyhow!("failed to stop pid {pid}"));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = ProcessCommand::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()?;
        if !status.success() {
            return Err(anyhow!("failed to stop pid {pid}"));
        }
        Ok(())
    }
}

fn handle_api_request(engine: &MemoryEngine, mut request: Request, dashboard: bool) -> Result<()> {
    let method = request.method().clone();
    let url = request.url().to_string();

    if method == Method::Options {
        return respond_raw(request, 204, String::new(), "application/json");
    }

    if dashboard && method == Method::Get && (url == "/" || url == "/dashboard") {
        return respond_raw(request, 200, dashboard_html(), "text/html; charset=utf-8");
    }

    if dashboard && method == Method::Get && url.starts_with("/dashboard/map") {
        let params = query_params(&url);
        let path = params.get("path").cloned().or_else(|| {
            env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
        });
        let request_body = build_map_request(
            path.as_deref().map(Path::new),
            params.get("project"),
            params.get("workspace"),
            params
                .get("type")
                .map(|value| parse_map_type_value(value))
                .transpose()?
                .unwrap_or(CliMapType::Evolution),
            CliMapOutput::Html,
            params.get("from").map(String::as_str),
            params.get("to").map(String::as_str),
            params
                .get("chronological")
                .is_some_and(|value| value == "true"),
            params.get("why").is_some_and(|value| value == "true"),
            params.get("impact").map(String::as_str),
        )?;
        let map = engine.build_map(&request_body)?;
        return respond_raw(
            request,
            200,
            map.render(MapOutputFormat::Html)?,
            "text/html; charset=utf-8",
        );
    }

    if method == Method::Get && url == "/health" {
        return respond_json(request, 200, json!({ "ok": true, "service": "memory.cpp" }));
    }

    if method == Method::Get && url == "/v1/stats" {
        return respond_json(request, 200, engine.stats()?);
    }

    if method == Method::Get && url.starts_with("/v1/map") {
        let params = query_params(&url);
        let request_body = build_map_request(
            params.get("path").map(|value| Path::new(value.as_str())),
            params.get("project"),
            params.get("workspace"),
            params
                .get("type")
                .map(|value| parse_map_type_value(value))
                .transpose()?
                .unwrap_or(CliMapType::Evolution),
            params
                .get("output")
                .map(|value| parse_map_output_value(value))
                .transpose()?
                .unwrap_or(CliMapOutput::Json),
            params.get("from").map(String::as_str),
            params.get("to").map(String::as_str),
            params
                .get("chronological")
                .is_some_and(|value| value == "true"),
            params.get("why").is_some_and(|value| value == "true"),
            params.get("impact").map(String::as_str),
        )?;
        let map = engine.build_map(&request_body)?;
        if matches!(request_body.output, MapOutputFormat::Json) {
            return respond_json(request, 200, map);
        }
        return respond_raw(
            request,
            200,
            map.render(request_body.output)?,
            request_body.output.content_type(),
        );
    }

    if method == Method::Get && url.starts_with("/v1/memories/search") {
        let params = query_params(&url);
        let query = params.get("q").cloned().unwrap_or_else(|| "".to_string());
        let scope = params.get("scope").cloned();
        let limit = params
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(8);
        let memories = engine.search(
            RecallQuery::new(query)
                .limit(limit)
                .workspace(scope.unwrap_or_else(|| "default".to_string())),
        )?;
        return respond_json(request, 200, memories);
    }

    if method == Method::Get && url.starts_with("/v1/memories/graph") {
        let params = query_params(&url);
        let scope = params.get("scope").map(String::as_str);
        let entity = params.get("entity");
        let limit = params
            .get("limit")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(50);
        if let Some(entity) = entity {
            return respond_json(request, 200, engine.related_entity(entity, scope, limit)?);
        }
        return respond_json(request, 200, engine.entity_graph(scope, limit)?);
    }

    if method == Method::Post && url == "/v1/memories" {
        let body = read_request_body(&mut request)?;
        let input: RememberRequest = serde_json::from_str(&body)?;
        let memory = engine.remember(input.into_memory("default")?)?;
        return respond_json(request, 200, memory);
    }

    if method == Method::Post && url == "/v1/memories/compact" {
        let body = read_request_body(&mut request)?;
        let value: Value = serde_json::from_str(&body)?;
        let scope = value
            .get("workspace")
            .or_else(|| value.get("scope"))
            .and_then(Value::as_str)
            .unwrap_or("default");
        let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize;
        let memory = engine.compact_scope(scope, limit)?;
        return respond_json(request, 200, memory);
    }

    if method == Method::Post && url == "/v1/recall" {
        let body = read_request_body(&mut request)?;
        let input: RecallRequest = serde_json::from_str(&body)?;
        let memories = engine.search(input.into_query("default"))?;
        return respond_json(request, 200, memories);
    }

    if method == Method::Post && url == "/v1/context" {
        let body = read_request_body(&mut request)?;
        let input: ContextRequest = serde_json::from_str(&body)?;
        let context = engine.context(
            input.recall.into_query("default"),
            input.tokens.unwrap_or(1_200),
        )?;
        return respond_json(request, 200, context);
    }

    if method == Method::Post && url == "/v1/timeline" {
        let body = read_request_body(&mut request)?;
        let value: Value = serde_json::from_str(&body)?;
        let scope = value
            .get("workspace")
            .or_else(|| value.get("scope"))
            .and_then(Value::as_str);
        let query = value.get("query").and_then(Value::as_str);
        let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
        let timeline = engine.timeline(scope, query, limit)?;
        return respond_json(request, 200, timeline);
    }

    if method == Method::Post && url == "/v1/map" {
        let body = read_request_body(&mut request)?;
        let input: MapApiRequest = serde_json::from_str(&body)?;
        let request_body = input.into_request()?;
        let map = engine.build_map(&request_body)?;
        if matches!(request_body.output, MapOutputFormat::Json) {
            return respond_json(request, 200, map);
        }
        return respond_raw(
            request,
            200,
            map.render(request_body.output)?,
            request_body.output.content_type(),
        );
    }

    respond_json(request, 404, json!({ "error": "not found" }))
}

fn handle_proxy_request(
    engine: &MemoryEngine,
    mut request: Request,
    upstream: &str,
    workspace: &str,
    limit: usize,
    tokens: usize,
    learning: &ProxyLearningConfig,
) -> Result<()> {
    if request.method() == &Method::Options {
        return respond_raw(request, 204, String::new(), "application/json");
    }

    if request.method() == &Method::Get && request.url() == "/health" {
        return respond_json(request, 200, json!({ "ok": true, "mode": "proxy" }));
    }

    if request.method() != &Method::Post || request.url() != "/v1/chat/completions" {
        return respond_json(
            request,
            404,
            json!({ "error": "proxy only supports POST /v1/chat/completions" }),
        );
    }

    let body = read_request_body(&mut request)?;
    let mut payload: Value = serde_json::from_str(&body)?;
    let query = extract_chat_query(&payload).unwrap_or_default();
    if !query.is_empty() {
        let context = engine.context(
            RecallQuery::new(query.clone())
                .workspace(workspace.to_string())
                .limit(limit),
            tokens,
        )?;
        let mut safe_context = serde_json::to_value(&context.text)?;
        redact_json_value(&mut safe_context);
        let safe_context = safe_context
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| context.text.clone());
        inject_memory_context(&mut payload, &safe_context);
    }

    let target = format!("{}/v1/chat/completions", upstream.trim_end_matches('/'));
    let response = ureq::post(&target).send_json(payload.clone());
    match response {
        Ok(response) => {
            let status = response.status();
            let text = response.into_string()?;
            observe_proxy_response(engine, workspace, &query, &text, learning)?;
            respond_raw(request, status, text, "application/json")
        }
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_default();
            observe_proxy_response(engine, workspace, &query, &text, learning).ok();
            respond_raw(request, status, text, "application/json")
        }
        Err(err) => respond_json(request, 502, json!({ "error": err.to_string() })),
    }
}

fn handle_mcp_message(engine: &MemoryEngine, request: Value, config: &McpRuntimeConfig) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "memory.cpp", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": {} }
        }),
        "tools/list" => json!({ "tools": mcp_tools(config) }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            match call_mcp_tool(engine, params.clone(), config) {
                Ok(value) => value,
                Err(err) => {
                    let tool_name = params
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let _ = write_mcp_audit(
                        engine.store_path(),
                        config,
                        tool_name,
                        false,
                        err.to_string(),
                    );
                    return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": err.to_string() } });
                }
            }
        }
        _ => {
            return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "method not found" } })
        }
    };

    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn call_mcp_tool(engine: &MemoryEngine, params: Value, config: &McpRuntimeConfig) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing tool name"))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let outcome = match name {
        "memory_add_candidate" => {
            let input: RememberRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(
                config,
                input.workspace.as_deref().or(input.scope.as_deref()),
            )?;
            let candidate = input.into_memory(&workspace)?;
            let stored = engine.remember_candidate(candidate, "mcp candidate memory")?;
            json!({
                "queued": stored.is_none(),
                "memory": stored,
                "workspace": workspace,
                "mode": "candidate"
            })
        }
        "memory_add" => {
            ensure_mcp_write_allowed(config, name)?;
            let input: RememberRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(
                config,
                input.workspace.as_deref().or(input.scope.as_deref()),
            )?;
            let memory = engine.remember(input.into_memory(&workspace)?)?;
            serde_json::to_value(memory)?
        }
        "memory_search" => {
            let input: RecallRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(
                config,
                input.workspace.as_deref().or(input.scope.as_deref()),
            )?;
            let memories = engine.search(input.into_query(&workspace))?;
            serde_json::to_value(memories)?
        }
        "memory_context" => {
            let input: ContextRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(
                config,
                input
                    .recall
                    .workspace
                    .as_deref()
                    .or(input.recall.scope.as_deref()),
            )?;
            let context = engine.context(
                input.recall.into_query(&workspace),
                input.tokens.unwrap_or(1_200),
            )?;
            serde_json::to_value(context)?
        }
        "memory_update" => {
            ensure_mcp_write_allowed(config, name)?;
            let input: UpdateToolRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(config, input.workspace.as_deref())?;
            let patch = engine.patch(
                &input.id,
                NewMemory::new(input.content)
                    .scope(workspace)
                    .kind(input.kind.unwrap_or_else(|| "note".to_string())),
            )?;
            serde_json::to_value(patch)?
        }
        "memory_forget" => {
            ensure_mcp_write_allowed(config, name)?;
            let input: ForgetToolRequest = serde_json::from_value(args)?;
            let memory =
                engine.forget(&input.id, input.reason.as_deref().unwrap_or("mcp forget"))?;
            serde_json::to_value(memory)?
        }
        "memory_timeline" => {
            let input: TimelineToolRequest = serde_json::from_value(args)?;
            let workspace = input
                .workspace
                .as_deref()
                .map(|value| mcp_workspace(config, Some(value)))
                .transpose()?;
            let timeline = engine.timeline(
                workspace.as_deref(),
                input.query.as_deref(),
                input.limit.unwrap_or(20),
            )?;
            serde_json::to_value(timeline)?
        }
        "memory_explain" => {
            let input: ExplainToolRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(config, input.workspace.as_deref())?;
            if input.last.unwrap_or(false) {
                let trace = engine.last_explain(Some(&workspace))?;
                serde_json::to_value(trace)?
            } else {
                let query = input.query.unwrap_or_default();
                let explain = engine.explain(
                    RecallQuery::new(query)
                        .workspace(workspace)
                        .limit(input.limit.unwrap_or(8)),
                )?;
                serde_json::to_value(explain)?
            }
        }
        "memory_graph" => {
            let input: GraphRequest = serde_json::from_value(args)?;
            let workspace = input
                .workspace
                .as_deref()
                .map(|value| mcp_workspace(config, Some(value)))
                .transpose()?;
            if let Some(entity) = input.entity {
                serde_json::to_value(engine.related_entity(
                    &entity,
                    workspace.as_deref(),
                    input.limit.unwrap_or(50),
                )?)?
            } else {
                serde_json::to_value(
                    engine.entity_graph(workspace.as_deref(), input.limit.unwrap_or(50))?,
                )?
            }
        }
        "memory_compact" => {
            ensure_mcp_write_allowed(config, name)?;
            let input: CompactToolRequest = serde_json::from_value(args)?;
            let workspace = mcp_workspace(config, input.workspace.as_deref())?;
            let compact = engine.compact_scope(&workspace, input.limit.unwrap_or(200))?;
            serde_json::to_value(compact)?
        }
        "memory_map" => {
            let input: MapApiRequest = serde_json::from_value(args)?;
            let mut request = input.into_request()?;
            request.workspace = Some(mcp_workspace(config, request.workspace.as_deref())?);
            let map = engine.build_map(&request)?;
            if matches!(request.output, MapOutputFormat::Json) {
                serde_json::to_value(map)?
            } else {
                json!({ "rendered": map.render(request.output)? })
            }
        }
        _ => return Err(anyhow!("unknown tool: {name}")),
    };

    write_mcp_audit(engine.store_path(), config, name, true, outcome.to_string())?;
    let mut safe_outcome = outcome;
    if config.redact_sensitive {
        redact_json_value(&mut safe_outcome);
    }
    Ok(mcp_text(serde_json::to_string_pretty(&safe_outcome)?))
}

fn mcp_tools(config: &McpRuntimeConfig) -> Vec<Value> {
    let mut tools = vec![
        mcp_tool(
            "memory_search",
            "Hybrid search across long-term memory.",
            json_schema(
                &["query"],
                &[
                    ("query", "string"),
                    ("workspace", "string"),
                    ("limit", "integer"),
                ],
            ),
        ),
        mcp_tool(
            "memory_context",
            "Build a model-ready context pack from recalled memory.",
            json_schema(
                &["query"],
                &[
                    ("query", "string"),
                    ("workspace", "string"),
                    ("limit", "integer"),
                    ("tokens", "integer"),
                ],
            ),
        ),
        mcp_tool(
            "memory_timeline",
            "Inspect memory events over time.",
            json_schema(
                &[],
                &[
                    ("workspace", "string"),
                    ("query", "string"),
                    ("limit", "integer"),
                ],
            ),
        ),
        mcp_tool(
            "memory_explain",
            "Explain why memories were recalled.",
            json_schema(
                &[],
                &[
                    ("query", "string"),
                    ("workspace", "string"),
                    ("limit", "integer"),
                    ("last", "boolean"),
                ],
            ),
        ),
        mcp_tool(
            "memory_graph",
            "Inspect the entity graph.",
            json_schema(
                &[],
                &[
                    ("workspace", "string"),
                    ("entity", "string"),
                    ("limit", "integer"),
                ],
            ),
        ),
        mcp_tool(
            "memory_map",
            "Generate a shareable project memory map.",
            json_schema(
                &[],
                &[
                    ("path", "string"),
                    ("project", "string"),
                    ("workspace", "string"),
                    ("type", "string"),
                    ("output", "string"),
                    ("from", "string"),
                    ("to", "string"),
                    ("chronological", "boolean"),
                    ("why", "boolean"),
                    ("impact", "string"),
                ],
            ),
        ),
        mcp_tool(
            "memory_add_candidate",
            "Queue candidate memory for later approval instead of writing directly.",
            json_schema(
                &["content"],
                &[
                    ("content", "string"),
                    ("workspace", "string"),
                    ("kind", "string"),
                    ("importance", "number"),
                    ("confidence", "number"),
                ],
            ),
        ),
    ];

    if config.allow_writes {
        tools.extend([
            mcp_tool(
                "memory_add",
                "Store durable long-term memory.",
                json_schema(
                    &["content"],
                    &[
                        ("content", "string"),
                        ("workspace", "string"),
                        ("kind", "string"),
                        ("importance", "number"),
                        ("confidence", "number"),
                    ],
                ),
            ),
            mcp_tool(
                "memory_update",
                "Patch a memory with newer information.",
                json_schema(
                    &["id", "content"],
                    &[
                        ("id", "string"),
                        ("content", "string"),
                        ("kind", "string"),
                        ("workspace", "string"),
                    ],
                ),
            ),
            mcp_tool(
                "memory_forget",
                "Mark a memory as forgotten.",
                json_schema(&["id"], &[("id", "string"), ("reason", "string")]),
            ),
            mcp_tool(
                "memory_compact",
                "Compact workspace memory into summary form.",
                json_schema(&[], &[("workspace", "string"), ("limit", "integer")]),
            ),
        ]);
    }

    tools
}

fn mcp_tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

fn json_schema(required: &[&str], properties: &[(&str, &str)]) -> Value {
    let mut map = serde_json::Map::new();
    for (name, ty) in properties {
        map.insert((*name).to_string(), json!({ "type": ty }));
    }
    json!({
        "type": "object",
        "properties": map,
        "required": required
    })
}

fn mcp_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn build_engine(cli: &Cli) -> Result<MemoryEngine> {
    build_engine_from_options(&EngineOptions::from(cli))
}

fn build_engine_from_options(options: &EngineOptions) -> Result<MemoryEngine> {
    let db = options
        .db
        .clone()
        .or_else(|| env::var_os("MEMORY_CPP_DB").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".memory.cpp/memory.db"));
    let config = load_app_config(&db).unwrap_or_default();
    let configured_provider = config
        .embedding
        .provider
        .as_deref()
        .and_then(parse_embedder_choice);
    let effective_embedder = if matches!(options.embedder, EmbedderChoice::Hash) {
        configured_provider.unwrap_or_else(|| options.embedder.clone())
    } else {
        options.embedder.clone()
    };
    let effective_endpoint = options
        .endpoint
        .clone()
        .or_else(|| config.embedding.endpoint.clone());
    let effective_model = options
        .model
        .clone()
        .or_else(|| config.embedding.model.clone());
    let effective_dimensions = config
        .embedding
        .dimensions
        .unwrap_or(options.dimensions)
        .max(32);

    let embedder: SharedEmbedder = match effective_embedder {
        EmbedderChoice::Hash => Arc::new(HashEmbedder::new(effective_dimensions)),
        EmbedderChoice::Fastembed | EmbedderChoice::Onnx => {
            Arc::new(FastEmbedOnnxEmbedder::new(effective_dimensions))
        }
        EmbedderChoice::Ollama => Arc::new(OllamaEmbedder::new(
            effective_endpoint.unwrap_or_else(|| "http://localhost:11434".to_string()),
            effective_model.unwrap_or_else(|| "nomic-embed-text".to_string()),
            effective_dimensions,
        )),
        EmbedderChoice::Openai => {
            let api_key = env::var(&options.api_key_env).ok();
            Arc::new(OpenAiCompatibleEmbedder::new(
                effective_endpoint.unwrap_or_else(|| "https://api.openai.com".to_string()),
                api_key,
                effective_model.unwrap_or_else(|| "text-embedding-3-small".to_string()),
                effective_dimensions,
            ))
        }
    };

    MemoryEngine::open_with_embedder(db, embedder).context("failed to open memory engine")
}

fn parse_embedder_choice(value: &str) -> Option<EmbedderChoice> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hash" => Some(EmbedderChoice::Hash),
        "ollama" => Some(EmbedderChoice::Ollama),
        "openai" | "openai-compatible" => Some(EmbedderChoice::Openai),
        "fastembed" => Some(EmbedderChoice::Fastembed),
        "onnx" | "fastembed-onnx" => Some(EmbedderChoice::Onnx),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_recall_query(
    words: &[String],
    workspace: Option<&String>,
    kinds: &[MemoryKind],
    tags: &[String],
    limit: usize,
    include_content: bool,
    include_global: bool,
    engine: &MemoryEngine,
) -> Result<RecallQuery> {
    let mut query = RecallQuery::new(words.join(" "))
        .limit(limit)
        .include_content(include_content)
        .include_global(include_global);

    if let Some(scope) = workspace.cloned().or(current_workspace_name(engine)?) {
        query = query.workspace(scope);
    }

    for kind in kinds {
        query = query.kind(*kind);
    }
    for tag in tags {
        query = query.tag(tag.clone());
    }

    Ok(query)
}

#[derive(Debug, Deserialize)]
struct RememberRequest {
    content: String,
    kind: Option<String>,
    workspace: Option<String>,
    scope: Option<String>,
    tags: Option<Vec<String>>,
    metadata: Option<Value>,
    importance: Option<f32>,
    confidence: Option<f32>,
}

impl RememberRequest {
    fn into_memory(self, default_scope: &str) -> Result<NewMemory> {
        let mut memory = NewMemory::new(self.content)
            .scope(
                self.workspace
                    .or(self.scope)
                    .unwrap_or_else(|| default_scope.to_string()),
            )
            .kind(self.kind.unwrap_or_else(|| "note".to_string()))
            .metadata(self.metadata.unwrap_or_else(|| json!({})));

        if let Some(importance) = self.importance {
            memory = memory.importance(importance);
        }
        if let Some(confidence) = self.confidence {
            memory = memory.confidence(confidence);
        }
        for tag in self.tags.unwrap_or_default() {
            memory = memory.tag(tag);
        }
        Ok(memory)
    }
}

#[derive(Debug, Deserialize)]
struct RecallRequest {
    #[serde(alias = "text")]
    query: String,
    workspace: Option<String>,
    scope: Option<String>,
    limit: Option<usize>,
    include_content: Option<bool>,
}

impl RecallRequest {
    fn into_query(self, default_scope: &str) -> RecallQuery {
        let mut query = RecallQuery::new(self.query)
            .limit(self.limit.unwrap_or(8))
            .include_content(self.include_content.unwrap_or(false));
        if let Some(scope) = self.workspace.or(self.scope) {
            query = query.scope(scope);
        } else {
            query = query.scope(default_scope.to_string());
        }
        query
    }
}

#[derive(Debug, Deserialize)]
struct ContextRequest {
    #[serde(flatten)]
    recall: RecallRequest,
    tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GraphRequest {
    #[serde(alias = "scope")]
    workspace: Option<String>,
    entity: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct MapApiRequest {
    path: Option<String>,
    project: Option<String>,
    workspace: Option<String>,
    #[serde(rename = "type")]
    map_type: Option<String>,
    output: Option<String>,
    from: Option<String>,
    to: Option<String>,
    chronological: Option<bool>,
    why: Option<bool>,
    impact: Option<String>,
}

impl MapApiRequest {
    fn into_request(self) -> Result<MapRequest> {
        Ok(MapRequest {
            path: self.path.map(PathBuf::from),
            project: self.project,
            workspace: self.workspace,
            map_type: self
                .map_type
                .as_deref()
                .map(parse_map_type_core)
                .transpose()?
                .unwrap_or(MapType::Evolution),
            output: self
                .output
                .as_deref()
                .map(parse_map_output_core)
                .transpose()?
                .unwrap_or(MapOutputFormat::Json),
            from: self
                .from
                .as_deref()
                .map(|value| parse_date(value, false))
                .transpose()?,
            to: self
                .to
                .as_deref()
                .map(|value| parse_date(value, true))
                .transpose()?,
            chronological: self.chronological.unwrap_or(false),
            why: self.why.unwrap_or(false),
            impact: self.impact,
            limit: 48,
        })
    }
}

#[derive(Debug, Deserialize)]
struct UpdateToolRequest {
    id: String,
    content: String,
    kind: Option<String>,
    workspace: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForgetToolRequest {
    id: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TimelineToolRequest {
    workspace: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ExplainToolRequest {
    query: Option<String>,
    workspace: Option<String>,
    limit: Option<usize>,
    last: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CompactToolRequest {
    workspace: Option<String>,
    limit: Option<usize>,
}

fn parse_metadata(value: Option<&str>) -> Result<Value> {
    match value {
        Some(raw) => serde_json::from_str(raw).context("metadata must be valid JSON"),
        None => Ok(Value::Object(Default::default())),
    }
}

fn parse_kind(value: &str) -> std::result::Result<MemoryKind, String> {
    MemoryKind::from_str(value).map_err(|err| err.to_string())
}

fn parse_permission(value: &str) -> Result<MemoryPermission> {
    match value.trim().to_ascii_lowercase().as_str() {
        "private" => Ok(MemoryPermission::Private),
        "workspace_only" | "workspace" => Ok(MemoryPermission::WorkspaceOnly),
        "agent_specific" | "agent" => Ok(MemoryPermission::AgentSpecific),
        "shareable" | "shared" => Ok(MemoryPermission::Shareable),
        "encrypted" => Ok(MemoryPermission::Encrypted),
        "ephemeral" => Ok(MemoryPermission::Ephemeral),
        other => Err(anyhow!("unknown permission: {other}")),
    }
}

fn parse_layer(value: &str) -> Result<MemoryLayer> {
    match value.trim().to_ascii_lowercase().as_str() {
        "working" => Ok(MemoryLayer::Working),
        "session" => Ok(MemoryLayer::Session),
        "episodic" => Ok(MemoryLayer::Episodic),
        "semantic" => Ok(MemoryLayer::Semantic),
        "procedural" => Ok(MemoryLayer::Procedural),
        "identity" => Ok(MemoryLayer::Identity),
        "project" => Ok(MemoryLayer::Project),
        "archival" => Ok(MemoryLayer::Archival),
        other => Err(anyhow!("unknown layer: {other}")),
    }
}

fn parse_status(value: &str) -> Result<MemoryStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "active" => Ok(MemoryStatus::Active),
        "archived" => Ok(MemoryStatus::Archived),
        "superseded" => Ok(MemoryStatus::Superseded),
        "contradicted" => Ok(MemoryStatus::Contradicted),
        "forgotten" => Ok(MemoryStatus::Forgotten),
        "ephemeral" => Ok(MemoryStatus::Ephemeral),
        "pending_review" | "pending-review" | "pending" => Ok(MemoryStatus::PendingReview),
        other => Err(anyhow!("unknown status: {other}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_memory_source(
    source: Option<&str>,
    source_type: Option<&str>,
    source_file: Option<&str>,
    source_line: Option<u64>,
    source_commit: Option<&str>,
    source_conversation: Option<&str>,
    created_by: Option<&str>,
    reliability: Option<f32>,
    source_app: Option<&str>,
) -> Option<MemorySource> {
    if source.is_none()
        && source_type.is_none()
        && source_file.is_none()
        && source_line.is_none()
        && source_commit.is_none()
        && source_conversation.is_none()
        && created_by.is_none()
        && reliability.is_none()
        && source_app.is_none()
    {
        return None;
    }

    Some(MemorySource {
        source_type: source_type.map(str::to_string),
        source_app: source_app.map(str::to_string),
        source: source.map(str::to_string),
        source_file: source_file.map(str::to_string),
        source_line,
        source_commit: source_commit.map(str::to_string),
        source_conversation_id: source_conversation.map(str::to_string),
        source_message_id: None,
        created_by: created_by.map(str::to_string),
        reliability,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_map_request(
    path: Option<&Path>,
    project: Option<&String>,
    workspace: Option<&String>,
    map_type: CliMapType,
    output: CliMapOutput,
    from: Option<&str>,
    to: Option<&str>,
    chronological: bool,
    why: bool,
    impact: Option<&str>,
) -> Result<MapRequest> {
    Ok(MapRequest {
        path: path.map(Path::to_path_buf),
        project: project.cloned(),
        workspace: workspace.cloned(),
        map_type: map_type.into(),
        output: output.into(),
        from: from.map(|value| parse_date(value, false)).transpose()?,
        to: to.map(|value| parse_date(value, true)).transpose()?,
        chronological,
        why,
        impact: impact.map(str::to_string),
        limit: 48,
    })
}

fn parse_map_type_value(value: &str) -> Result<CliMapType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "evolution" => Ok(CliMapType::Evolution),
        "timeline" => Ok(CliMapType::Timeline),
        "decisions" | "decision" => Ok(CliMapType::Decisions),
        "architecture" | "arch" => Ok(CliMapType::Architecture),
        "bugs" | "bug" => Ok(CliMapType::Bugs),
        "dependencies" | "deps" => Ok(CliMapType::Dependencies),
        other => Err(anyhow!("unknown map type: {other}")),
    }
}

fn parse_map_type_core(value: &str) -> Result<MapType> {
    Ok(parse_map_type_value(value)?.into())
}

fn resolve_map_type(
    fallback: CliMapType,
    evolution: bool,
    timeline: bool,
    decisions: bool,
    architecture: bool,
    bugs: bool,
    dependencies: bool,
) -> CliMapType {
    if evolution {
        CliMapType::Evolution
    } else if timeline {
        CliMapType::Timeline
    } else if decisions {
        CliMapType::Decisions
    } else if architecture {
        CliMapType::Architecture
    } else if bugs {
        CliMapType::Bugs
    } else if dependencies {
        CliMapType::Dependencies
    } else {
        fallback
    }
}

fn parse_map_output_value(value: &str) -> Result<CliMapOutput> {
    match value.trim().to_ascii_lowercase().as_str() {
        "json" => Ok(CliMapOutput::Json),
        "markdown" | "md" => Ok(CliMapOutput::Markdown),
        "mermaid" | "mmd" => Ok(CliMapOutput::Mermaid),
        "html" => Ok(CliMapOutput::Html),
        other => Err(anyhow!("unknown map output: {other}")),
    }
}

fn parse_map_output_core(value: &str) -> Result<MapOutputFormat> {
    Ok(parse_map_output_value(value)?.into())
}

fn emit_or_save(rendered: &str, save: Option<&Path>) -> Result<()> {
    if let Some(path) = save {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, rendered)?;
        println!("wrote {}", path.display());
        println!("open it with: memory open --print map");
        println!("next: memory map why \"MCP integration\"");
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn parse_date(value: &str, end_of_day: bool) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .with_context(|| format!("invalid date '{value}', expected YYYY-MM-DD"))?;
    let datetime = if end_of_day {
        date.and_hms_opt(23, 59, 59)
            .ok_or_else(|| anyhow!("invalid end-of-day timestamp"))?
    } else {
        date.and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid start-of-day timestamp"))?
    };
    Ok(DateTime::from_naive_utc_and_offset(datetime, Utc))
}

fn read_request_body(request: &mut Request) -> Result<String> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    Ok(body)
}

fn respond_json<T: Serialize>(request: Request, status: u16, value: T) -> Result<()> {
    let body = serde_json::to_string(&value)?;
    respond_raw(request, status, body, "application/json")
}

fn respond_raw(request: Request, status: u16, text: String, content_type: &str) -> Result<()> {
    let response = Response::from_string(text)
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", content_type))
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
        .with_header(header(
            "Access-Control-Allow-Headers",
            "content-type, authorization",
        ));
    request.respond(response)?;
    Ok(())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid static header")
}

fn extract_chat_query(payload: &Value) -> Option<String> {
    payload
        .get("messages")?
        .as_array()?
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))?
        .get("content")?
        .as_str()
        .map(|value| value.to_string())
}

fn inject_memory_context(payload: &mut Value, context: &str) {
    let Some(messages) = payload.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };

    messages.insert(
        0,
        json!({
            "role": "system",
            "content": format!("Use this durable local memory when it is relevant. Do not mention it unless useful.\n\n{context}")
        }),
    );
}

fn observe_proxy_response(
    engine: &MemoryEngine,
    workspace: &str,
    query: &str,
    raw_response: &str,
    learning: &ProxyLearningConfig,
) -> Result<()> {
    if !learning.enabled {
        return Ok(());
    }

    let payload: Value = serde_json::from_str(raw_response).unwrap_or_else(|_| json!({}));
    let content = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if content.is_empty() {
        return Ok(());
    }

    for candidate in proxy_candidates(content, workspace, query, learning) {
        if learning.dry_run {
            eprintln!("proxy candidate: {}", serde_json::to_string(&candidate)?);
            continue;
        }

        let _ = engine.remember_candidate(candidate, "proxy-observed memory")?;
    }
    Ok(())
}

fn proxy_candidates(
    text: &str,
    workspace: &str,
    query: &str,
    learning: &ProxyLearningConfig,
) -> Vec<NewMemory> {
    let mut candidates = Vec::new();
    for sentence in text.split_terminator(['.', '!', '?']) {
        let trimmed = sentence.trim();
        if trimmed.len() < 20 {
            continue;
        }
        if detect_sensitive_reason(trimmed).is_some() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if [
            "prefer",
            "using",
            "working on",
            "stack",
            "build",
            "decision",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
        {
            let confidence = if lower.contains("decision") {
                0.7
            } else if lower.contains("prefer") {
                0.68
            } else {
                0.62
            };
            if confidence < learning.min_confidence {
                continue;
            }
            candidates.push(
                NewMemory::new(trimmed.to_string())
                    .scope(workspace.to_string())
                    .kind(if lower.contains("prefer") {
                        "preference"
                    } else if lower.contains("decision") {
                        "decision"
                    } else {
                        "fact"
                    })
                    .confidence(confidence)
                    .tag("proxy".to_string())
                    .source(MemorySource {
                        source_type: Some("proxy_response".to_string()),
                        source_app: Some("openai-compatible".to_string()),
                        source: Some(query.to_string()),
                        source_file: None,
                        source_line: None,
                        source_commit: None,
                        source_conversation_id: None,
                        source_message_id: None,
                        created_by: Some("proxy".to_string()),
                        reliability: Some(confidence),
                    })
                    .status(if learning.approval_required || confidence < 0.8 {
                        MemoryStatus::PendingReview
                    } else {
                        MemoryStatus::Active
                    }),
            );
        }
    }
    candidates
}

fn read_eval_cases(file: &Path) -> Result<Vec<EvalCase>> {
    let raw = fs::read_to_string(file)?;
    if file.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
        return raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(Into::into))
            .collect();
    }

    serde_json::from_str(&raw).map_err(Into::into)
}

fn collect_watch_files(path: &Path) -> Result<Vec<PathBuf>> {
    collect_importable_files(path, true).map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn demo_memory_seed(
    content: &str,
    kind: MemoryKind,
    workspace: &str,
    importance: f32,
    confidence: f32,
    created_at: DateTime<Utc>,
    tags: &[&str],
    source_file: Option<&str>,
) -> NewMemory {
    let mut memory = NewMemory::new(content.to_string())
        .scope(workspace.to_string())
        .kind(kind.as_str())
        .importance(importance)
        .confidence(confidence)
        .created_at(created_at)
        .source(MemorySource {
            source_type: Some("demo_seed".to_string()),
            source_app: Some("memory.cpp".to_string()),
            source: Some("demo workspace".to_string()),
            source_file: source_file.map(str::to_string),
            source_line: None,
            source_commit: None,
            source_conversation_id: None,
            source_message_id: None,
            created_by: Some("demo-seed".to_string()),
            reliability: Some(confidence),
        });
    for tag in tags {
        memory = memory.tag((*tag).to_string());
    }
    memory
}

fn ok_check(name: &str, detail: String) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: "ok".to_string(),
        detail,
        suggestion: None,
    }
}

fn warn_check(name: &str, detail: String, suggestion: &str) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: "warn".to_string(),
        detail,
        suggestion: Some(suggestion.to_string()),
    }
}

fn error_check(name: &str, detail: String, suggestion: &str) -> DoctorCheck {
    DoctorCheck {
        name: name.to_string(),
        status: "error".to_string(),
        detail,
        suggestion: Some(suggestion.to_string()),
    }
}

fn git_repo_root(path: &Path) -> Option<PathBuf> {
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(PathBuf::from(root))
    }
}

fn check_ollama(endpoint: &str) -> Result<bool> {
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
    match ureq::get(&url).call() {
        Ok(response) => Ok(response.status() < 500),
        Err(ureq::Error::Status(status, _)) => Ok(status < 500),
        Err(ureq::Error::Transport(_)) => Ok(false),
    }
}

fn ensure_writable_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let probe = path.join(".write-test.tmp");
    fs::write(&probe, "ok")?;
    fs::remove_file(probe)?;
    Ok(())
}

fn port_available(addr: &str) -> Result<bool> {
    match TcpListener::bind(addr) {
        Ok(listener) => {
            drop(listener);
            Ok(true)
        }
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => Ok(false),
        Err(err) => Err(anyhow!(err.to_string())),
    }
}

fn current_workspace_name(engine: &MemoryEngine) -> Result<Option<String>> {
    Ok(engine
        .current_workspace()?
        .map(|workspace| workspace.name)
        .or(load_app_config(engine.store_path())?.default_workspace))
}

fn required_workspace(engine: &MemoryEngine, workspace: Option<&String>) -> Result<String> {
    workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .or(Some("default".to_string()))
        .ok_or_else(|| anyhow!("no workspace available"))
}

fn resolve_mcp_runtime_config(
    engine: &MemoryEngine,
    workspace: Option<&String>,
    allow_writes: bool,
    no_redaction: bool,
    audit_log: Option<&PathBuf>,
) -> Result<McpRuntimeConfig> {
    let config = load_app_config(engine.store_path())?;
    let workspace = workspace
        .cloned()
        .or(config.mcp.workspace.clone())
        .or(current_workspace_name(engine)?)
        .or(config.default_workspace.clone());
    let audit_log = audit_log
        .cloned()
        .or_else(|| config.mcp.audit_log.map(PathBuf::from))
        .unwrap_or_else(|| {
            engine
                .store_path()
                .parent()
                .unwrap_or_else(|| Path::new(".memory.cpp"))
                .join("audit")
                .join("mcp-access.jsonl")
        });

    Ok(McpRuntimeConfig {
        workspace,
        allow_writes: allow_writes || !config.mcp.read_only,
        redact_sensitive: !no_redaction && config.mcp.redact_sensitive,
        audit_log,
    })
}

fn mcp_workspace(config: &McpRuntimeConfig, requested: Option<&str>) -> Result<String> {
    if let Some(scoped) = &config.workspace {
        if let Some(requested) = requested {
            if requested != scoped {
                return Err(anyhow!(
                    "MCP access is scoped to workspace '{}' and cannot access '{}'",
                    scoped,
                    requested
                ));
            }
        }
        return Ok(scoped.clone());
    }

    Ok(requested.unwrap_or("default").to_string())
}

fn ensure_mcp_write_allowed(config: &McpRuntimeConfig, tool: &str) -> Result<()> {
    if config.allow_writes {
        Ok(())
    } else {
        Err(anyhow!(
            "{tool} is disabled because MCP is running in read-only mode; restart with --allow-writes if you want direct mutation tools"
        ))
    }
}

fn write_mcp_audit(
    db_path: &Path,
    config: &McpRuntimeConfig,
    action: &str,
    allowed: bool,
    detail: String,
) -> Result<()> {
    if let Some(parent) = config.audit_log.parent() {
        fs::create_dir_all(parent)?;
    }
    let entry = AuditLogEntry {
        recorded_at: Utc::now(),
        channel: "mcp",
        action,
        workspace: config.workspace.as_deref(),
        allowed,
        detail: truncate_detail(&detail, 800),
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.audit_log)?;
    let _ = db_path;
    writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    Ok(())
}

fn truncate_detail(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        let mut output = value.chars().take(max_chars).collect::<String>();
        output.push_str("...");
        output
    }
}

fn load_app_config(db_path: &Path) -> Result<AppConfig> {
    let path = config_path(db_path);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn save_app_config(db_path: &Path, config: &AppConfig) -> Result<()> {
    let path = config_path(db_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

fn set_default_workspace(db_path: &Path, workspace: &str) -> Result<()> {
    let mut config = load_app_config(db_path)?;
    config.default_workspace = Some(workspace.to_string());
    save_app_config(db_path, &config)
}

fn config_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("memory-config.json")
}

fn detect_sensitive_reason(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    let patterns = [
        ("api_key", "api key material"),
        ("-----begin", "private key material"),
        ("authorization: bearer", "bearer token"),
        ("password=", "password-like secret"),
        ("secret=", "secret-like value"),
        ("cookie:", "cookie material"),
        ("ghp_", "github token"),
        ("sk-", "openai-style secret"),
        ("xoxb-", "slack token"),
    ];
    patterns
        .into_iter()
        .find(|(needle, _)| lower.contains(needle))
        .map(|(_, reason)| reason)
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            if let Some(reason) = detect_sensitive_reason(text) {
                *text = format!("[REDACTED: {reason}]");
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                redact_json_value(value);
            }
        }
        _ => {}
    }
}

fn dashboard_html() -> String {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>memory.cpp dashboard</title>
  <style>
    :root {
      --bg: #f6f1e8;
      --ink: #1f2421;
      --accent: #146356;
      --accent-2: #d97706;
      --line: #d6ccb9;
      --panel: rgba(255,255,255,0.82);
      --mono: "IBM Plex Mono", "Cascadia Code", Consolas, monospace;
      --sans: "Space Grotesk", "Segoe UI", sans-serif;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: var(--sans);
      color: var(--ink);
      background:
        radial-gradient(circle at top right, rgba(20,99,86,0.16), transparent 30%),
        linear-gradient(180deg, #f8f3eb 0%, #efe7da 100%);
    }
    main {
      max-width: 1180px;
      margin: 0 auto;
      padding: 24px;
    }
    h1 {
      margin: 0 0 8px;
      font-size: clamp(2rem, 4vw, 3.4rem);
      letter-spacing: 0;
    }
    p {
      margin: 0;
      color: rgba(31,36,33,0.78);
    }
    .hero {
      display: grid;
      gap: 18px;
      padding: 20px 0 28px;
    }
    .grid {
      display: grid;
      gap: 18px;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    }
    section {
      border: 1px solid var(--line);
      background: var(--panel);
      border-radius: 8px;
      padding: 16px;
      backdrop-filter: blur(12px);
    }
    input, button, select {
      font: inherit;
      padding: 10px 12px;
      border-radius: 6px;
      border: 1px solid var(--line);
    }
    button {
      cursor: pointer;
      background: var(--accent);
      color: white;
      border: none;
    }
    button.secondary {
      background: var(--accent-2);
    }
    .toolbar {
      display: flex;
      gap: 10px;
      flex-wrap: wrap;
      margin-top: 12px;
    }
    pre {
      font-family: var(--mono);
      white-space: pre-wrap;
      word-break: break-word;
      font-size: 0.88rem;
      margin: 0;
    }
    .pill {
      display: inline-block;
      margin-right: 8px;
      padding: 4px 8px;
      border-radius: 999px;
      background: rgba(20,99,86,0.12);
      color: var(--accent);
      font-size: 0.82rem;
    }
  </style>
</head>
<body>
  <main>
    <div class="hero">
      <div>
        <h1>memory.cpp</h1>
        <p>One file. Local. Fast. Persistent memory for every AI app.</p>
      </div>
      <div class="toolbar">
        <input id="query" placeholder="Search long-term memory" style="min-width:280px;flex:1" />
        <button onclick="search()">Search</button>
        <button class="secondary" onclick="loadStats()">Stats</button>
        <button class="secondary" onclick="loadGraph()">Graph</button>
        <button class="secondary" onclick="loadTimeline()">Timeline</button>
      </div>
    </div>
    <div class="grid">
      <section>
        <div class="pill">Search</div>
        <pre id="searchResult">Run a query to inspect memory retrieval.</pre>
      </section>
      <section>
        <div class="pill">Stats</div>
        <pre id="statsResult">Loading is manual so you can inspect the system when you want to.</pre>
      </section>
      <section>
        <div class="pill">Graph</div>
        <pre id="graphResult">Entity graph appears here.</pre>
      </section>
      <section>
        <div class="pill">Timeline</div>
        <pre id="timelineResult">Workspace activity appears here.</pre>
      </section>
    </div>
  </main>
  <script>
    async function loadJson(url, options) {
      const res = await fetch(url, options);
      return await res.json();
    }
    async function search() {
      const query = document.getElementById('query').value || 'project memory';
      const data = await loadJson('/v1/memories/search?q=' + encodeURIComponent(query));
      document.getElementById('searchResult').textContent = JSON.stringify(data, null, 2);
    }
    async function loadStats() {
      const data = await loadJson('/v1/stats');
      document.getElementById('statsResult').textContent = JSON.stringify(data, null, 2);
    }
    async function loadGraph() {
      const data = await loadJson('/v1/memories/graph');
      document.getElementById('graphResult').textContent = JSON.stringify(data, null, 2);
    }
    async function loadTimeline() {
      const data = await loadJson('/v1/timeline', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ limit: 20 })
      });
      document.getElementById('timelineResult').textContent = JSON.stringify(data, null, 2);
    }
  </script>
</body>
</html>"#.to_string()
}

fn query_params(url: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let Some((_, query)) = url.split_once('?') else {
        return params;
    };
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            params.insert(key.to_string(), value.replace('+', " "));
        }
    }
    params
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_manual_args_detects_demo_command() {
        let raw = vec![
            "memory".to_string(),
            "--db".to_string(),
            ".memory.cpp/memory.db".to_string(),
            "demo".to_string(),
            "--workspace".to_string(),
            "demo".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (options, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "demo");
        assert_eq!(
            options.db.as_deref(),
            Some(Path::new(".memory.cpp/memory.db"))
        );
        assert_eq!(rest, vec!["--workspace".to_string(), "demo".to_string()]);
    }

    #[test]
    fn split_manual_args_detects_audit_log_command() {
        let raw = vec![
            "memory".to_string(),
            "--db".to_string(),
            ".memory.cpp/memory.db".to_string(),
            "audit-log".to_string(),
            "--limit".to_string(),
            "5".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "audit-log");
        assert_eq!(rest, vec!["--limit".to_string(), "5".to_string()]);
    }

    #[test]
    fn split_manual_args_detects_extract_command() {
        let raw = vec![
            "memory".to_string(),
            "--embedder".to_string(),
            "hash".to_string(),
            "extract".to_string(),
            ".".to_string(),
            "--dry-run".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "extract");
        assert_eq!(rest, vec![".".to_string(), "--dry-run".to_string()]);
    }

    #[test]
    fn split_manual_args_detects_init_command() {
        let raw = vec![
            "memory".to_string(),
            "init".to_string(),
            "--workspace".to_string(),
            "demo".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "init");
        assert_eq!(rest, vec!["--workspace".to_string(), "demo".to_string()]);
    }

    #[test]
    fn split_manual_args_detects_import_command() {
        let raw = vec![
            "memory".to_string(),
            "import".to_string(),
            ".".to_string(),
            "--preview-redactions".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "import");
        assert_eq!(
            rest,
            vec![".".to_string(), "--preview-redactions".to_string()]
        );
    }

    #[test]
    fn split_manual_args_detects_developer_fame_commands() {
        for command in [
            "add",
            "search",
            "inbox",
            "embeddings",
            "terminal",
            "ci",
            "welcome",
            "setup",
            "what",
            "where",
            "today",
            "yesterday",
            "week",
            "next",
            "status",
            "explain",
            "examples",
            "open",
            "fix",
            "redact",
            "config",
            "attach",
            "detach",
            "watch",
            "context",
            "show-map",
            "show-brain",
            "show-timeline",
            "show-context",
            "show-inbox",
            "privacy",
            "ignore",
        ] {
            let raw = vec!["memory".to_string(), command.to_string()];
            let parsed = split_manual_args(&raw).expect("split should succeed");
            let (_, parsed_command, _) = parsed.expect("manual command should be detected");
            assert_eq!(parsed_command, command);
        }
    }

    #[test]
    fn split_manual_args_detects_git_command() {
        let raw = vec![
            "memory".to_string(),
            "git".to_string(),
            "summary".to_string(),
            "--since".to_string(),
            "7d".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "git");
        assert_eq!(
            rest,
            vec![
                "summary".to_string(),
                "--since".to_string(),
                "7d".to_string()
            ]
        );
    }

    #[test]
    fn split_manual_args_detects_ignore_command() {
        let raw = vec![
            "memory".to_string(),
            "ignore".to_string(),
            "check".to_string(),
            "README.md".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "ignore");
        assert_eq!(rest, vec!["check".to_string(), "README.md".to_string()]);
    }

    #[test]
    fn split_manual_args_detects_mcp_command() {
        let raw = vec![
            "memory".to_string(),
            "mcp".to_string(),
            "--workspace".to_string(),
            "demo".to_string(),
        ];
        let parsed = split_manual_args(&raw).expect("split should succeed");
        let (_, command, rest) = parsed.expect("manual command should be detected");
        assert_eq!(command, "mcp");
        assert_eq!(rest, vec!["--workspace".to_string(), "demo".to_string()]);
    }

    #[test]
    fn resolve_map_type_prefers_shortcut_flags() {
        let map_type = resolve_map_type(
            CliMapType::Evolution,
            false,
            false,
            true,
            false,
            false,
            false,
        );
        assert!(matches!(map_type, CliMapType::Decisions));
    }

    #[test]
    fn manual_map_focus_parses_why_command() {
        let parsed =
            ManualMapFocusCli::try_parse_from(["map", "MCP integration", "--output", "markdown"])
                .expect("parse should succeed");
        assert_eq!(parsed.target, "MCP integration");
        assert!(matches!(parsed.output, CliMapOutput::Markdown));
    }

    #[test]
    fn manual_map_parses_save_and_shortcut_flags() {
        let parsed = ManualMapCli::try_parse_from([
            "map",
            ".",
            "--evolution",
            "--output",
            "html",
            "--save",
            "demo.html",
        ])
        .expect("parse should succeed");
        assert_eq!(parsed.path.as_deref(), Some(Path::new(".")));
        assert!(parsed.evolution);
        assert!(matches!(parsed.output, CliMapOutput::Html));
        assert_eq!(parsed.save.as_deref(), Some(Path::new("demo.html")));
    }

    #[test]
    fn demo_command_parses_reset_action() {
        let parsed =
            ManualDemoCli::try_parse_from(["demo", "reset", "--workspace", "demo", "--json"])
                .expect("parse should succeed");
        assert_eq!(parsed.action, "reset");
        assert_eq!(parsed.workspace.as_deref(), Some("demo"));
        assert!(parsed.json);
    }

    #[test]
    fn manual_git_summary_parses_since_and_limit() {
        let parsed =
            ManualGitCli::try_parse_from(["git", "summary", "--since", "14d", "--limit", "6"])
                .expect("parse should succeed");
        match parsed.command {
            GitCommand::Summary { since, limit, .. } => {
                assert_eq!(since.as_deref(), Some("14d"));
                assert_eq!(limit, 6);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn manual_git_watch_parses_once_and_dry_run() {
        let parsed =
            ManualGitCli::try_parse_from(["git", "watch", "--once", "--dry-run", "--limit", "3"])
                .expect("parse should succeed");
        match parsed.command {
            GitCommand::Watch {
                once,
                dry_run,
                limit,
                ..
            } => {
                assert!(once);
                assert!(dry_run);
                assert_eq!(limit, 3);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn manual_ignore_check_parses_json_flag() {
        let parsed = ManualIgnoreCli::try_parse_from(["ignore", "check", "README.md", "--json"])
            .expect("parse should succeed");
        match parsed.command {
            IgnoreCommand::Check { path, json, .. } => {
                assert_eq!(path, PathBuf::from("README.md"));
                assert!(json);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn manual_dev_parses_explain_repo() {
        let parsed = ManualDevCli::try_parse_from([
            "dev",
            "explain-repo",
            ".",
            "--workspace",
            "demo",
            "--json",
        ])
        .expect("parse should succeed");
        match parsed.command {
            DevCommand::ExplainRepo {
                path,
                workspace,
                json,
            } => {
                assert_eq!(path.as_deref(), Some(Path::new(".")));
                assert_eq!(workspace.as_deref(), Some("demo"));
                assert!(json);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn manual_dev_parses_context_target() {
        let parsed =
            ManualDevCli::try_parse_from(["dev", "context", "--for", "codex", "--tokens", "900"])
                .expect("parse should succeed");
        match parsed.command {
            DevCommand::Context { target, tokens, .. } => {
                assert!(matches!(target, DevContextTarget::Codex));
                assert_eq!(tokens, 900);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn manual_inbox_and_embeddings_parse_polish_commands() {
        let inbox =
            ManualInboxCli::try_parse_from(["inbox", "approve-all", "--confidence-above", "0.91"])
                .expect("parse should succeed");
        match inbox.command.expect("subcommand") {
            InboxCommand::ApproveAll {
                confidence_above, ..
            } => assert!((confidence_above - 0.91).abs() < f32::EPSILON),
            other => panic!("unexpected inbox command: {other:?}"),
        }

        let embeddings = ManualEmbeddingsCli::try_parse_from(["embeddings", "set", "fastembed"])
            .expect("parse should succeed");
        match embeddings.command {
            EmbeddingsCommand::Set { provider, .. } => {
                assert!(matches!(provider, EmbedderChoice::Fastembed));
            }
            other => panic!("unexpected embeddings command: {other:?}"),
        }
    }

    #[test]
    fn manual_setup_parses_understandability_profiles() {
        let parsed = ManualSetupCli::try_parse_from([
            "setup",
            "--developer",
            "--ai-coding",
            "--offline",
            "--workspace",
            "demo",
            "--json",
        ])
        .expect("parse should succeed");
        assert!(parsed.developer);
        assert!(parsed.ai_coding);
        assert!(parsed.offline);
        assert_eq!(parsed.workspace.as_deref(), Some("demo"));
        assert!(parsed.json);
    }

    #[test]
    fn manual_privacy_parses_purge_and_status() {
        let status = ManualPrivacyCli::try_parse_from(["privacy", "status", "--json"])
            .expect("parse should succeed");
        match status.command {
            Some(PrivacyCommand::Status { json }) => assert!(json),
            other => panic!("unexpected privacy command: {other:?}"),
        }

        let purge = ManualPrivacyCli::try_parse_from(["privacy", "purge", "--yes"])
            .expect("parse should succeed");
        match purge.command {
            Some(PrivacyCommand::Purge { yes }) => assert!(yes),
            other => panic!("unexpected privacy command: {other:?}"),
        }
    }

    #[test]
    fn manual_show_map_parses_save_shortcut() {
        let parsed = ManualShowMapCli::try_parse_from([
            "show-map",
            "--workspace",
            "demo",
            "--save",
            "map.html",
        ])
        .expect("parse should succeed");
        assert_eq!(parsed.workspace.as_deref(), Some("demo"));
        assert_eq!(parsed.save.as_path(), Path::new("map.html"));
    }

    #[test]
    fn manual_import_parses_preview_redactions() {
        let parsed = ManualImportCli::try_parse_from([
            "import",
            ".",
            "--workspace",
            "demo",
            "--preview-redactions",
            "--json",
        ])
        .expect("parse should succeed");
        assert_eq!(parsed.path, PathBuf::from("."));
        assert_eq!(parsed.workspace.as_deref(), Some("demo"));
        assert!(parsed.preview_redactions);
        assert!(parsed.json);
    }

    #[test]
    fn manual_proxy_parses_learning_flags() {
        let parsed = ManualProxyCli::try_parse_from([
            "proxy",
            "--learn",
            "--approval-required",
            "--min-confidence",
            "0.7",
        ])
        .expect("parse should succeed");
        assert!(parsed.learn);
        assert!(parsed.approval_required);
        assert!((parsed.min_confidence - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn normalize_since_arg_expands_shortcuts() {
        assert_eq!(normalize_since_arg("7d"), "7 days ago");
        assert_eq!(normalize_since_arg("12h"), "12 hours ago");
        assert_eq!(normalize_since_arg("2w"), "2 weeks ago");
        assert_eq!(normalize_since_arg("2026-05-01"), "2026-05-01");
    }

    #[test]
    fn build_extracted_candidate_classifies_decisions() {
        let candidate = build_extracted_candidate(
            "Use SQLite as the default local-first storage engine because portability matters.",
            Some(MemoryKind::Decision),
            Some("README.md".to_string()),
            None,
            "repo extraction".to_string(),
        )
        .expect("candidate should be extracted");
        assert!(matches!(candidate.kind, MemoryKind::Decision));
        assert!(candidate.confidence >= 0.7);
        assert!(candidate.tags.contains(&"decision".to_string()));
    }

    #[test]
    fn manual_status_open_fix_redact_and_config_parse_release_flags() {
        let status = ManualStatusCli::try_parse_from(["status", "--json", "--verbose"])
            .expect("status parse should succeed");
        assert!(status.json);
        assert!(status.verbose);
        assert!(!status.runtime);

        let runtime = ManualStatusCli::try_parse_from(["status", "--runtime"])
            .expect("runtime status parse should succeed");
        assert!(runtime.runtime);

        let open = ManualOpenCli::try_parse_from(["open", "--print", "docs"])
            .expect("open parse should succeed");
        assert_eq!(open.print_target.as_deref(), Some("docs"));

        let fix = ManualFixCli::try_parse_from(["fix", "--apply", "--json"])
            .expect("fix parse should succeed");
        assert!(fix.apply);
        assert!(fix.json);

        let redact = ManualRedactCli::try_parse_from(["redact", "preview", "README.md", "--json"])
            .expect("redact parse should succeed");
        match redact.command {
            RedactCommand::Preview { path, json } => {
                assert_eq!(path, PathBuf::from("README.md"));
                assert!(json);
            }
            other => panic!("unexpected redact command: {other:?}"),
        }

        let config = ManualConfigCli::try_parse_from(["config", "set", "profile", "developer"])
            .expect("config parse should succeed");
        match config.command.expect("config subcommand") {
            ConfigCommand::Set { key, value } => {
                assert_eq!(key, "profile");
                assert_eq!(value, "developer");
            }
            other => panic!("unexpected config command: {other:?}"),
        }
    }

    #[test]
    fn manual_git_watch_lifecycle_actions_parse() {
        let status = ManualGitCli::try_parse_from(["git", "watch", "status", "--json"])
            .expect("git watch status parse should succeed");
        match status.command {
            GitCommand::Watch {
                action: Some(GitWatchAction::Status { json }),
                ..
            } => assert!(json),
            other => panic!("unexpected git watch command: {other:?}"),
        }

        for args in [
            ["git", "watch", "pause"].as_slice(),
            ["git", "watch", "resume"].as_slice(),
            ["git", "watch", "reset-baseline"].as_slice(),
        ] {
            let parsed = ManualGitCli::try_parse_from(args).expect("git watch action should parse");
            match parsed.command {
                GitCommand::Watch {
                    action: Some(GitWatchAction::Pause),
                    ..
                }
                | GitCommand::Watch {
                    action: Some(GitWatchAction::Resume),
                    ..
                }
                | GitCommand::Watch {
                    action: Some(GitWatchAction::ResetBaseline { .. }),
                    ..
                } => {}
                other => panic!("unexpected git watch action: {other:?}"),
            }
        }
    }

    #[test]
    fn manual_terminal_ci_inbox_and_embeddings_parse_release_variants() {
        let terminal_status = ManualTerminalCli::try_parse_from(["terminal", "status", "--json"])
            .expect("terminal status parse should succeed");
        assert!(matches!(
            terminal_status.command,
            TerminalCommand::Status { json: true }
        ));

        let record = ManualTerminalCli::try_parse_from([
            "terminal",
            "record",
            "--command",
            "cargo test -p memory-cli",
            "--exit-code",
            "1",
            "--duration-ms",
            "120",
        ])
        .expect("terminal record parse should succeed");
        match record.command {
            TerminalCommand::Record {
                command,
                exit_code,
                duration_ms,
                ..
            } => {
                assert_eq!(command, "cargo test -p memory-cli");
                assert_eq!(exit_code, 1);
                assert_eq!(duration_ms, Some(120));
            }
            other => panic!("unexpected terminal command: {other:?}"),
        }

        let shell = ManualTerminalCli::try_parse_from([
            "terminal",
            "install-shell",
            "powershell",
            "--json",
        ])
        .expect("terminal install-shell parse should succeed");
        match shell.command {
            TerminalCommand::InstallShell { shell, json } => {
                assert_eq!(shell.as_deref(), Some("powershell"));
                assert!(json);
            }
            other => panic!("unexpected terminal shell command: {other:?}"),
        }

        let suggest =
            ManualTerminalCli::try_parse_from(["terminal", "suggest", "how did I run tests?"])
                .expect("terminal suggest parse should succeed");
        assert!(matches!(suggest.command, TerminalCommand::Suggest { .. }));

        let privacy = ManualTerminalCli::try_parse_from(["terminal", "privacy", "--json"])
            .expect("terminal privacy parse should succeed");
        assert!(matches!(
            privacy.command,
            TerminalCommand::Privacy { json: true }
        ));

        let ci = ManualCiCli::try_parse_from([
            "ci",
            "explain-failure",
            "auth_refresh_retries",
            "--workspace",
            "demo",
            "--json",
        ])
        .expect("ci explain-failure parse should succeed");
        match ci.command {
            CiCommand::ExplainFailure {
                query,
                workspace,
                json,
                ..
            } => {
                assert_eq!(query.as_deref(), Some("auth_refresh_retries"));
                assert_eq!(workspace.as_deref(), Some("demo"));
                assert!(json);
            }
            other => panic!("unexpected ci command: {other:?}"),
        }

        let ci_report = ManualCiCli::try_parse_from(["ci", "report", "--output", "ci.md"])
            .expect("ci report parse should succeed");
        assert!(matches!(ci_report.command, CiCommand::Report { .. }));

        let ci_comment =
            ManualCiCli::try_parse_from(["ci", "pr-comment", "--output", "comment.md"])
                .expect("ci pr-comment parse should succeed");
        assert!(matches!(ci_comment.command, CiCommand::PrComment { .. }));

        let reject = ManualInboxCli::try_parse_from([
            "inbox",
            "reject",
            "candidate-1",
            "--reason",
            "duplicate",
        ])
        .expect("inbox reject parse should succeed");
        match reject.command.expect("inbox subcommand") {
            InboxCommand::Reject { id, reason } => {
                assert_eq!(id, "candidate-1");
                assert_eq!(reason.as_deref(), Some("duplicate"));
            }
            other => panic!("unexpected inbox reject command: {other:?}"),
        }

        for args in [
            ["inbox", "list", "--simple"].as_slice(),
            ["inbox", "list", "--important"].as_slice(),
            ["inbox", "list", "--risky"].as_slice(),
        ] {
            let parsed = ManualInboxCli::try_parse_from(args).expect("inbox list should parse");
            assert!(matches!(parsed.command, Some(InboxCommand::List { .. })));
        }

        let review = ManualInboxCli::try_parse_from(["inbox", "review", "--json"])
            .expect("inbox review should parse");
        assert!(matches!(
            review.command,
            Some(InboxCommand::Review { json: true, .. })
        ));

        let rule = ManualInboxCli::try_parse_from([
            "inbox",
            "rules",
            "add",
            "docs/**",
            "--action",
            "review",
            "--confidence-above",
            "0.8",
        ])
        .expect("inbox rules add should parse");
        assert!(matches!(
            rule.command,
            Some(InboxCommand::Rules {
                command: Some(InboxRulesCommand::Add { .. })
            })
        ));

        let migrate = ManualEmbeddingsCli::try_parse_from([
            "embeddings",
            "migrate",
            "--to",
            "fastembed",
            "--dry-run",
        ])
        .expect("embeddings migrate parse should succeed");
        match migrate.command {
            EmbeddingsCommand::Migrate {
                provider, dry_run, ..
            } => {
                assert!(matches!(provider, EmbedderChoice::Fastembed));
                assert!(dry_run);
            }
            other => panic!("unexpected embeddings command: {other:?}"),
        }

        let explain = ManualEmbeddingsCli::try_parse_from(["embeddings", "explain"])
            .expect("embeddings explain parse should succeed");
        assert!(matches!(explain.command, EmbeddingsCommand::Explain));
    }

    #[test]
    fn manual_public_adoption_commands_parse() {
        let attach =
            ManualAttachCli::try_parse_from(["attach", "--dry-run", "--print-config", "continue"])
                .expect("attach parse should succeed");
        assert!(matches!(attach.target, AttachTarget::Continue));
        assert!(attach.dry_run);
        assert!(attach.print_config);

        let detach = ManualDetachCli::try_parse_from(["detach", "all", "--dry-run"])
            .expect("detach parse should succeed");
        assert!(matches!(detach.target, AttachTarget::All));
        assert!(detach.dry_run);

        let watch = ManualWatchCli::try_parse_from(["watch", "once", "--dry-run", "--json"])
            .expect("watch parse should succeed");
        assert!(matches!(watch.action, Some(WatchAction::Once)));
        assert!(watch.dry_run);
        assert!(watch.json);

        let context = ManualContextCli::try_parse_from([
            "context",
            "write",
            "--for",
            "cursor",
            "--output",
            "cursor.md",
        ])
        .expect("context parse should succeed");
        assert!(matches!(context.action, Some(ContextAction::Write)));
        assert!(matches!(context.target, DevContextTarget::Cursor));
        assert_eq!(context.output.as_deref(), Some(Path::new("cursor.md")));
    }

    #[test]
    fn manual_privacy_config_ignore_and_search_parse_new_variants() {
        let privacy = ManualPrivacyCli::try_parse_from(["privacy", "receipts", "--json"])
            .expect("privacy receipts parse should succeed");
        assert!(matches!(
            privacy.command,
            Some(PrivacyCommand::Receipts { json: true })
        ));

        let config_path =
            ManualConfigCli::try_parse_from(["config", "path"]).expect("config path parses");
        assert!(matches!(config_path.command, Some(ConfigCommand::Path)));
        let config_profiles = ManualConfigCli::try_parse_from(["config", "profiles"])
            .expect("config profiles parses");
        assert!(matches!(
            config_profiles.command,
            Some(ConfigCommand::Profiles)
        ));

        let ignore_add = ManualIgnoreCli::try_parse_from(["ignore", "add", "*.secret"])
            .expect("ignore add parses");
        assert!(matches!(ignore_add.command, IgnoreCommand::Add { .. }));
        let ignore_remove = ManualIgnoreCli::try_parse_from(["ignore", "remove", "*.secret"])
            .expect("ignore remove parses");
        assert!(matches!(
            ignore_remove.command,
            IgnoreCommand::Remove { .. }
        ));

        let recall = ManualRecallCli::try_parse_from([
            "search",
            "workflow",
            "--profile",
            "terminal",
            "--explain",
            "--json",
        ])
        .expect("search profile parses");
        assert!(matches!(recall.profile, Some(SearchProfile::Terminal)));
        assert!(recall.explain);
        assert!(recall.json);
    }

    #[test]
    fn manual_dev_context_parses_generic_budget_and_verbose() {
        let parsed = ManualDevCli::try_parse_from([
            "dev",
            "context",
            "--for",
            "generic",
            "--budget",
            "2000",
            "--verbose",
        ])
        .expect("dev context parse should succeed");
        match parsed.command {
            DevCommand::Context {
                target,
                tokens,
                verbose,
                ..
            } => {
                assert!(matches!(target, DevContextTarget::Generic));
                assert_eq!(tokens, 2000);
                assert!(verbose);
            }
            other => panic!("unexpected dev context command: {other:?}"),
        }
    }

    #[test]
    fn developer_ready_docs_recipes_examples_and_website_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in [
            "docs/quickstart.md",
            "docs/install.md",
            "docs/uninstall.md",
            "docs/upgrade.md",
            "docs/first-five-minutes.md",
            "docs/core-concepts.md",
            "docs/dev-workflow.md",
            "docs/git-memory.md",
            "docs/terminal-memory.md",
            "docs/ai-context.md",
            "docs/context-packs.md",
            "docs/maps.md",
            "docs/inbox.md",
            "docs/doctor.md",
            "docs/privacy.md",
            "docs/safety.md",
            "docs/config.md",
            "docs/ci-memory.md",
            "docs/watch.md",
            "docs/faq.md",
            "docs/examples.md",
            "docs/troubleshooting.md",
            "docs/troubleshooting-install.md",
            "docs/architecture.md",
            "docs/roadmap.md",
            "docs/changelog.md",
            "docs/launch-checklist.md",
            "docs/integrations/cursor.md",
            "docs/integrations/claude.md",
            "docs/integrations/vscode.md",
            "docs/integrations/codex.md",
            "docs/integrations/continue.md",
            "docs/integrations/ollama.md",
            "docs/integrations/mcp.md",
            "docs/recipes/use-with-cursor.md",
            "docs/recipes/use-with-codex.md",
            "docs/recipes/use-with-claude.md",
            "docs/recipes/resume-work-after-weekend.md",
            "docs/recipes/generate-project-map.md",
            "docs/recipes/remember-terminal-commands.md",
            "docs/recipes/recover-a-fix.md",
            "docs/recipes/prepare-a-pr.md",
            "docs/recipes/private-local-setup.md",
            "docs/recipes/offline-setup.md",
            "docs/recipes/understand-a-new-repo.md",
            "docs/recipes/clean-up-memory.md",
            "docs/recipes/fix-ci-failure.md",
            "docs/recipes/write-release-notes.md",
            "docs/recipes/review-a-pr.md",
            "docs/recipes/explain-a-codebase.md",
            "docs/recipes/create-ai-context-pack.md",
            "docs/recipes/restore-after-interruption.md",
            "docs/recipes/automatic-repo-memory.md",
            "docs/recipes/review-memory-candidates.md",
            "examples/dev-morning.md",
            "examples/dev-evening.md",
            "examples/dev-next.md",
            "examples/yesterday.md",
            "examples/week.md",
            "examples/explain-repo.md",
            "examples/cursor-context.md",
            "examples/codex-context.md",
            "examples/claude-context.md",
            "examples/generic-context.md",
            "examples/project-map.html",
            "examples/project-map.md",
            "examples/project-map.mmd",
            "examples/privacy-status.md",
            "examples/doctor.md",
            "examples/fix.md",
            "examples/inbox-candidate.md",
            "examples/terminal-search.md",
            "examples/terminal-commands.md",
            "examples/ci-failure.md",
            "examples/git-summary.md",
            "examples/git-watch.md",
            "examples/readme-suggestion.md",
            "examples/changelog.md",
            "examples/pr-summary.md",
            "examples/review.md",
            "examples/health.md",
            "examples/attach-cursor.md",
            "examples/attach-ollama.md",
            "examples/status.md",
            "examples/today.md",
            "examples/next.md",
            "website/index.html",
            "website/styles.css",
            "website/app.js",
            "website/pages/integrations.html",
        ] {
            assert!(root.join(path).exists(), "missing {path}");
        }
    }
}
