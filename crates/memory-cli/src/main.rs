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
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use memory_core::{
    check_ignored_path, collect_importable_files, evaluate, import_path, parse_file, EvalCase,
    HashEmbedder, ImportFormat, ImportOptions, MapOutputFormat, MapRequest, MapType, MemoryEdit,
    MemoryEngine, MemoryKind, MemoryLayer, MemoryPermission, MemorySource, MemoryStatus, NewMemory,
    OllamaEmbedder, OpenAiCompatibleEmbedder, PersonaProfile, PolicyMode, RecallQuery,
    SharedEmbedder, DEFAULT_MEMORYIGNORE,
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
    Ollama,
    Vscode,
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
        json: bool,
    },
    Approve {
        id: String,
    },
    Reject {
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
    #[arg(long)]
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
    #[arg(long)]
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
    #[arg(long)]
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

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct AppConfig {
    default_workspace: Option<String>,
    encrypted_requested: bool,
    mcp: McpPersistedConfig,
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
    let mut command = Cli::command();
    command.print_help()?;
    println!("\n\nAdditional v0.2 commands:");
    println!("  edit <id> [content]           Edit memory content/metadata in place");
    println!("  restore <id>                  Restore the latest active version of a memory");
    println!(
        "  demo --workspace demo         Seed a launch-ready demo workspace and sample map files"
    );
    println!(
        "  doctor                        Diagnose local setup, safety defaults, and runtime health"
    );
    println!("  audit-log                     Inspect recorded MCP agent access receipts");
    println!(
        "  extract [PATH]                Extract candidate memory from repo files or git history"
    );
    println!("  git ingest|summary|map        Git-aware project memory helpers");
    println!("  ignore init|check             Manage .memoryignore safety rules");
    println!("  dev watch|morning|resume      Solo-dev workflow helpers");
    println!(
        "  dev explain-repo|next         Explain repo structure and recommend the next actions"
    );
    println!("  map [PATH]                    Render evolution/decision/architecture maps");
    println!("  map why <topic>               Explain why a project decision or feature exists");
    println!("  map impact <topic>            Show what depends on a decision or component");
    println!("  map compare <left> <right>    Diff two map snapshots or exported map files");
    println!("  start | stop | status         Lightweight runtime management for server/proxy");
    println!("\nParser note: a few v0.2.1 commands are routed through a small pre-parser to avoid a Clap");
    println!("stack-overflow edge case from an oversized nested command tree. The behavior is tested and");
    println!("documented, and the tree can be simplified further in a future cleanup pass.");
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
            init_command(&engine, args.encrypted, args.workspace)?;
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
        "dev" => {
            let args = ManualDevCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            dev_command(&engine, &args.command)?;
        }
        "attach" => {
            let args = ManualAttachCli::parse_from(
                std::iter::once(command.clone()).chain(rest.iter().cloned()),
            );
            let engine = build_engine_from_options(&options)?;
            attach_command(
                &engine,
                &args.target,
                &args.host,
                args.port,
                &args.upstream,
                args.start_proxy,
                args.workspace.as_ref(),
            )?;
        }
        "map" => {
            let engine = build_engine_from_options(&options)?;
            if rest.first().is_some_and(|value| value == "compare") {
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
        "status" => status_command(&options)?,
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
        "dev",
        "attach",
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
    limit: usize,
    include_content: bool,
    include_inactive: bool,
    include_global: bool,
    json_output: bool,
}

fn recall_command(engine: &MemoryEngine, options: RecallCommandOptions<'_>) -> Result<()> {
    let mut recall_query = build_recall_query(
        options.query,
        options.workspace,
        options.kinds,
        options.tags,
        options.limit,
        options.include_content,
        options.include_global,
        engine,
    )?;
    recall_query = recall_query.include_inactive(options.include_inactive);
    let memories = engine.search(recall_query)?;
    if options.json_output {
        println!("{}", serde_json::to_string_pretty(&memories)?);
    } else if memories.is_empty() {
        println!("no memories found");
    } else {
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
            json,
        } => {
            let items = engine.inbox(workspace.as_deref(), status.as_deref())?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                for item in items {
                    println!("{} [{}] {}", item.id, item.status, item.reason);
                }
            }
        }
        InboxCommand::Approve { id } => {
            if engine.review_inbox(id, "approved")? {
                println!("approved {}", id);
            } else {
                println!("inbox item not found: {}", id);
            }
        }
        InboxCommand::Reject { id } => {
            if engine.review_inbox(id, "rejected")? {
                println!("rejected {}", id);
            } else {
                println!("inbox item not found: {}", id);
            }
        }
    }
    Ok(())
}

fn attach_command(
    engine: &MemoryEngine,
    target: &AttachTarget,
    host: &str,
    port: u16,
    upstream: &str,
    start_proxy: bool,
    workspace: Option<&String>,
) -> Result<()> {
    let exe = env::current_exe().context("could not locate current memory executable")?;
    let db = engine
        .store_path()
        .canonicalize()
        .unwrap_or_else(|_| engine.store_path().to_path_buf());
    let root = env::current_dir()?;
    let attach_dir = root.join(".memory.cpp").join("attach");
    fs::create_dir_all(&attach_dir)?;
    let scoped_workspace = workspace
        .cloned()
        .or(current_workspace_name(engine)?)
        .or(load_app_config(engine.store_path())?.mcp.workspace);
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

    match target {
        AttachTarget::Cursor
        | AttachTarget::Vscode
        | AttachTarget::Codex
        | AttachTarget::Claude => {
            let mut args = vec![
                "--db".to_string(),
                db.to_string_lossy().to_string(),
                "mcp".to_string(),
            ];
            if let Some(workspace) = &scoped_workspace {
                args.push("--workspace".to_string());
                args.push(workspace.clone());
            }
            let config = json!({
                "mcpServers": {
                    "memory-cpp": {
                        "command": exe,
                        "args": args
                    }
                }
            });

            let path = match target {
                AttachTarget::Cursor => root.join(".cursor").join("mcp.json"),
                AttachTarget::Vscode => root.join(".vscode").join("mcp.json"),
                AttachTarget::Codex => root.join(".codex").join("mcp.json"),
                AttachTarget::Claude => root.join(".claude").join("claude_desktop_config.json"),
                AttachTarget::Ollama => unreachable!(),
            };
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, serde_json::to_string_pretty(&config)?)?;
            println!("attached {:?} using {}", target, path.display());
        }
        AttachTarget::Ollama => {
            let proxy_info = json!({
                "base_url": format!("http://{host}:7332/v1"),
                "upstream": upstream,
                "db": db,
                "workspace": scoped_workspace.clone(),
            });
            let path = attach_dir.join("ollama-proxy.json");
            fs::write(&path, serde_json::to_string_pretty(&proxy_info)?)?;
            if start_proxy {
                let _child = ProcessCommand::new(exe)
                    .args([
                        "--db",
                        &db.to_string_lossy(),
                        "proxy",
                        "--listen",
                        &format!("{host}:7332"),
                        "--upstream",
                        upstream,
                        "--learn",
                        "--approval-required",
                    ])
                    .args(if let Some(workspace) = &scoped_workspace {
                        vec!["--workspace", workspace.as_str()]
                    } else {
                        Vec::new()
                    })
                    .spawn()
                    .context("failed to start background proxy")?;
                println!("started proxy on http://{}:7332/v1", host);
            }
            println!("attached Ollama using {}", path.display());
        }
    }

    if let Some(workspace) = scoped_workspace {
        println!("workspace scope: {workspace}");
    }
    println!("health endpoint: http://{}:{}/health", host, port);
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
    let next_step = recent_memories
        .iter()
        .find(|memory| matches!(memory.kind, MemoryKind::Task | MemoryKind::Decision))
        .map(|memory| memory.summary.clone())
        .or_else(|| conflicts.first().map(|conflict| conflict.reason.clone()))
        .or_else(|| inbox.first().map(|entry| entry.reason.clone()))
        .unwrap_or_else(|| {
            "Review the latest project decisions and consolidate any pending review memories."
                .to_string()
        });

    let report = json!({
        "workspace": scope,
        "since": since,
        "major_changes": recent_events,
        "recent_decisions": decisions,
        "recent_bugs_and_fixes": bug_fixes,
        "open_conflicts": conflicts,
        "inbox": inbox,
        "suggested_next_work": next_step,
    });

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "morning recap for {}",
            report["workspace"].as_str().unwrap_or("default")
        );
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
        println!(
            "suggested next work: {}",
            report["suggested_next_work"]
                .as_str()
                .unwrap_or("review project memory")
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

    let response = json!({
        "workspace": scope,
        "query": resume_query,
        "replay": replay,
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
    let report = json!({
        "workspace": scope,
        "path": requested_path,
        "repo_root": repo_root,
        "outline": outline,
        "recent_decisions": recent_decisions,
        "recent_bugs_and_fixes": recent_bugs,
        "recent_commits": recent_commits,
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

    if let Some(item) = inbox.first() {
        suggestions.push(format!(
            "Review pending inbox items starting with: {}",
            item.reason
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
    if resolve_repo_root(&env::current_dir()?).is_some() {
        suggestions.push("Refresh repo history with `memory git ingest --since 7d`.".to_string());
    }
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
    }

    Ok(())
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

    let raw = String::from_utf8_lossy(&output.stdout);
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

    checks.push(match git_repo_root(env::current_dir()?.as_path()) {
        Some(root) => ok_check(
            "git",
            format!("detected git repository at {}", root.display()),
        ),
        None => warn_check(
            "git",
            "no git repository detected from the current directory".to_string(),
            "run memory.cpp from a repo root to enrich maps and dev workflows",
        ),
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
                "ok" => "✓",
                "warn" => "⚠",
                _ => "✗",
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
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;
        if !status.success() {
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

    let embedder: SharedEmbedder = match options.embedder {
        EmbedderChoice::Hash => Arc::new(HashEmbedder::new(options.dimensions)),
        EmbedderChoice::Ollama => Arc::new(OllamaEmbedder::new(
            options
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            options
                .model
                .clone()
                .unwrap_or_else(|| "nomic-embed-text".to_string()),
            options.dimensions,
        )),
        EmbedderChoice::Openai => {
            let api_key = env::var(&options.api_key_env).ok();
            Arc::new(OpenAiCompatibleEmbedder::new(
                options
                    .endpoint
                    .clone()
                    .unwrap_or_else(|| "https://api.openai.com".to_string()),
                api_key,
                options
                    .model
                    .clone()
                    .unwrap_or_else(|| "text-embedding-3-small".to_string()),
                options.dimensions,
            ))
        }
    };

    MemoryEngine::open_with_embedder(db, embedder).context("failed to open memory engine")
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
}
