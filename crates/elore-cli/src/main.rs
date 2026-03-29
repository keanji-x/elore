use std::path::PathBuf;
use clap::{Parser, Subcommand};

mod cmd;

#[derive(Parser)]
#[command(name = "elore", about = "EverLore v3 — Phase-Driven Narrative Compiler", version)]
pub struct Cli {
    /// Project directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    pub project: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    // ── Project setup ────────────────────────────────────────────
    /// 初始化项目
    Init,

    /// 创建实体 scaffold (人类使用)
    New {
        #[arg(value_name = "TYPE")]
        entity_type: String,
        #[arg(value_name = "ID")]
        id: String,
        #[arg(long)]
        name: Option<String>,
    },

    // ── AI write API ─────────────────────────────────────────────
    /// AI 注入结构化数据 (不碰文件系统)
    Add {
        #[command(subcommand)]
        action: AddAction,
    },

    // ── AI read API ──────────────────────────────────────────────
    /// AI 查询世界状态 (结构化输出)
    Read {
        #[command(subcommand)]
        action: ReadAction,
    },

    // ── Human pipeline ───────────────────────────────────────────
    /// 构建世界快照
    Snapshot { chapter: String },

    /// 校验 drama intent
    Validate { chapter: String },

    /// 生成 Author prompt
    Write {
        chapter: String,
        #[arg(long)]
        pov: Option<String>,
        #[arg(long)]
        outline: Option<PathBuf>,
    },

    /// 全管线: snapshot → validate → write
    Run {
        chapter: String,
        #[arg(long)]
        pov: Option<String>,
    },

    /// 态势面板
    Plan {
        #[arg(long)]
        chapter: Option<String>,
    },

    /// 章节间 diff
    Diff {
        from_chapter: String,
        to_chapter: String,
    },

    /// What-If 分析
    Whatif {
        chapter: String,
        #[arg(long)]
        effect: String,
    },

    /// 事件日志管理
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },

    /// Drama Node 管理
    Drama {
        #[command(subcommand)]
        action: DramaAction,
    },

    /// 将所有 Beats 编译成可读的 Markdown 文档
    Gen {
        /// 输出文件路径 (省略则打印到 stdout)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,

        /// 只包含指定的 Phase (可多次指定), 默认全部
        #[arg(long = "phase")]
        phases: Vec<String>,
    },

    /// 项目概览 (v3: 显示四层约束进度)
    Status {
        #[arg(long, default_value = "human")]
        format: String,
    },

    // ── v3: Phase lifecycle ───────────────────────────────────────
    /// 切换到指定 phase
    Checkout {
        phase_id: String,
    },

    /// 提交当前 phase 进入审阅
    Submit,

    /// 批准当前 phase
    Approve,

    /// 拒绝当前 phase (退回 active)
    Reject {
        reason: String,
    },
}

// ── Add subcommands ──────────────────────────────────────────────

#[derive(Subcommand)]
pub enum AddAction {
    /// 注入/更新实体 (部分字段用默认值填充, 引用必须存在)
    Entity {
        /// JSON 字符串, id 为必填
        json: String,
    },
    /// 注入/更新 drama node (chapter 为必填)
    Drama {
        /// JSON 字符串, chapter 为必填
        json: String,
    },
    /// 注入/更新秘密 (id 和 content 为必填)
    Secret {
        /// JSON 字符串
        json: String,
    },
    /// 提交单个 effect (等同于 history add)
    Effect {
        chapter: String,
        /// Effect DSL, 例如: move(kian, oasis_gate)
        dsl: String,
    },

    // ── v3 ──
    /// 创建/注册 Phase 定义
    Phase {
        /// JSON 字符串, id 为必填
        json: String,
    },
    /// 提交一个 Beat (text + effects)
    Beat {
        /// JSON 字符串, text 为必填
        json: String,
    },
    /// 标注一个 Beat 的质量
    Note {
        /// JSON 字符串, beat + score 为必填
        json: String,
    },
    /// 批量创建实体 (JSON array, 自动拓扑排序)
    Entities {
        /// JSON array 字符串, 每个元素的 id 为必填
        json: String,
    },
}

// ── Read subcommands ─────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ReadAction {
    /// 世界快照 (默认 human, --format json 机器可读)
    Snapshot {
        chapter: String,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// 编译好的 Author prompt (纯文本, 直连 LLM)
    Prompt {
        chapter: String,
        #[arg(long)]
        pov: Option<String>,
    },
    /// Drama node
    Drama {
        chapter: String,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// 事件历史
    History {
        #[arg(long)]
        chapter: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },
    // ── v3 ──
    /// 当前 Phase 定义和约束
    Phase {
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// 当前 Phase 的所有 beats
    Beats {
        #[arg(long)]
        phase: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },
}

// ── Existing subcommands ─────────────────────────────────────────

#[derive(Subcommand)]
pub enum HistoryAction {
    List {
        #[arg(long)]
        chapter: Option<String>,
    },
    Add {
        chapter: String,
        effect: String,
    },
    Rollback {
        chapter: String,
    },
}

#[derive(Subcommand)]
pub enum DramaAction {
    Show { chapter: String },
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if let Err(e) = cmd::dispatch(cli).await {
        eprintln!("\x1b[31m✗ Error:\x1b[0m {e}");
        std::process::exit(1);
    }
}

