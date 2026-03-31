use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cmd;

#[derive(Parser)]
#[command(
    name = "elore",
    about = "EverLore v3 — Phase-Driven Narrative Compiler",
    version
)]
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

    // ── Human workflow ───────────────────────────────────────────
    /// 态势面板。优先使用 phase / 当前 active phase
    Plan {
        #[arg(long)]
        phase: Option<String>,
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
        #[arg(long)]
        phase: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },

    // ── v3: Phase lifecycle ───────────────────────────────────────
    /// 切换到指定 phase
    Checkout { phase_id: String },

    /// 提交当前 phase 进入审阅
    Submit,

    /// 批准当前 phase
    Approve,

    /// 拒绝当前 phase (退回 active)
    Reject { reason: String },

    // ── v4: File-based workflow ───────────────────────────────────
    /// 编译 cards/ → .everlore/ (entity cache, history, snapshots)
    Build,

    /// 从 drafts/ 编译 Markdown 为正式 beats
    Ingest,

    /// 从文件系统重建 history.jsonl 和 state.json
    Sync,

    // ── v5: AI Copilot Workflow ───────────────────────────────────
    /// 对 drafts 里的 markdown 语法与前置 YAML 进行静态分析
    LintDrafts,

    /// 启发式断言分析，为 AI 提供下一步剧情动作和撰写建议
    Suggest,

    // ── v6: Pack system ──────────────────────────────────────────
    /// 扩展包管理 (list / info / install)
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
}

// ── Pack subcommands ────────────────────────────────────────────

#[derive(Subcommand)]
pub enum PackAction {
    /// 列出所有可用的扩展包
    List,
    /// 查看扩展包详情
    Info {
        /// 扩展包名称或路径
        name: String,
    },
    /// 安装扩展包到当前项目
    Install {
        /// 扩展包名称或路径
        name: String,
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
    /// 注入/更新秘密 (id 和 content 为必填)
    Secret {
        /// JSON 字符串
        json: String,
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
    /// 世界快照。参数语义为 phase_id，输出的是该 phase 截止当前的世界状态
    Snapshot {
        phase: String,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// 事件历史。参数语义为 phase_id
    History {
        #[arg(long)]
        phase: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },
    // ── v3 ──
    /// Phase 定义和约束。省略 --phase 时读取当前 active phase
    Phase {
        #[arg(long)]
        phase: Option<String>,
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
    // ── v4 ──
    /// 获取前一个 Beat 的结尾上下文片段，保持剧情连续性
    PreviousBeat {
        #[arg(long)]
        phase: Option<String>,
    },
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
