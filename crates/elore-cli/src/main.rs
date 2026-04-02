use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cmd;

#[derive(Parser)]
#[command(
    name = "elore",
    about = "EverLore — Tree-Driven Narrative Compiler",
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
    // ── Project ──────────────────────────────────────────────────
    /// 初始化项目
    Init,

    /// 创建实体 scaffold
    New {
        #[arg(value_name = "TYPE")]
        entity_type: String,
        #[arg(value_name = "ID")]
        id: String,
        #[arg(long)]
        name: Option<String>,
    },

    /// 编译 cards/ → .everlore/
    Build,

    // ── Content tree ─────────────────────────────────────────────
    /// 查看内容树
    Tree {
        #[arg(long, default_value = "human")]
        format: String,
    },

    /// 查看节点详情 (省略 id 则用当前 active)
    Show {
        id: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },

    /// 移动光标到一个节点（不改状态）
    Activate {
        id: String,
    },

    /// 编辑节点（创建 pov 目录、初始化 progress，committed 节点自动 v+1）
    Edit {
        id: String,
    },

    /// 提交节点 (省略 id 则用当前 active)
    Commit {
        id: Option<String>,
    },

    /// 查看节点 effects 对世界状态的影响 (before → after diff)
    Diff {
        id: Option<String>,
    },

    /// 查看世界快照 (省略 id 则用当前 active)
    Snapshot {
        id: Option<String>,
        #[arg(long, default_value = "human")]
        format: String,
        #[arg(long)]
        before: bool,
    },

    /// 阅读内容（聚合视图）
    Read {
        #[command(subcommand)]
        mode: ReadMode,
    },

    /// 查看实体的叙事上下文（effects 关联 + 文本引用）
    Context {
        /// 实体 ID (character, location, secret, faction)。省略则用 main_role。
        id: Option<String>,
        /// 显示完整文本而非摘要
        #[arg(long)]
        full: bool,
    },

    // ── Output ───────────────────────────────────────────────────
    /// 将内容树编译为可读的 Markdown 文档
    Publish {
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    // ── AI API ───────────────────────────────────────────────────
    /// AI 注入结构化数据
    Add {
        #[command(subcommand)]
        action: AddAction,
    },

    /// 启发式断言分析，为 AI 提供建议
    Suggest,

    // ── Pack system ──────────────────────────────────────────────
    /// 扩展包管理
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
}

// ── Read subcommands ────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ReadMode {
    /// 按深度聚合 synopsis（默认 depth=1，章节大纲）
    Level {
        #[arg(default_value = "1")]
        depth: usize,
    },
    /// 叶子正文聚合，截止到指定节点（默认 active）
    Leaf {
        id: Option<String>,
    },
    /// 从根到指定节点的路径聚合（默认 active）
    Path {
        id: Option<String>,
    },
    /// 父节点概览：synopsis + 所有兄弟状态（默认 active）
    Parent {
        id: Option<String>,
    },
    /// 前序兄弟的尾部文本，用于衔接（默认 active）
    Sibling {
        id: Option<String>,
    },
    /// 查看 POV 草稿（默认 active 节点，可指定角色）
    Pov {
        /// 节点 ID（默认 active）
        id: Option<String>,
        /// 只看某个角色的草稿
        #[arg(long)]
        who: Option<String>,
    },
}

// ── Pack subcommands ────────────────────────────────────────────

#[derive(Subcommand)]
pub enum PackAction {
    List,
    Info { name: String },
    Install { name: String },
}

// ── Add subcommands ─────────────────────────────────────────────

#[derive(Subcommand)]
pub enum AddAction {
    /// 注入/更新实体
    Entity { json: String },
    /// 注入/更新秘密
    Secret { json: String },
    /// 批量创建实体
    Entities { json: String },
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
