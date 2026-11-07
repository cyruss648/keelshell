//! Pure command-palette metadata and matching for the workspace.
//!
//! This module deliberately stops at data and ranking. A caller that renders
//! the palette owns action dispatch, modal policy, and the live application
//! state. Keeping those concerns outside this module makes it safe to query
//! from a UI without accidentally performing an SSH, filesystem, or settings
//! operation.

/// The interface language used when a caller renders one localized field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommandPaletteLanguage {
    /// Simplified Chinese, matching the application's default locale.
    #[default]
    ZhCn,
    /// English.
    En,
}

/// The smallest workspace snapshot needed to decide palette visibility.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteContext {
    /// Whether the workspace has a live or restorable session to act on.
    pub has_session: bool,
    /// Whether the AI assistant surface is currently open.
    pub assistant_open: bool,
    /// Whether a previous session tab exists from the active tab.
    pub can_previous_session: bool,
    /// Whether a next session tab exists from the active tab.
    pub can_next_session: bool,
}

/// The context required before an item can be shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandPaletteContextRequirement {
    /// The action can be offered in either workspace state.
    Any,
    /// The action is meaningful only when no session exists.
    #[allow(dead_code)]
    NoSession,
    /// The action needs a session-bound workspace surface.
    HasSession,
    /// The action needs a previous session tab.
    HasPreviousSession,
    /// The action needs a next session tab.
    HasNextSession,
}

impl CommandPaletteContextRequirement {
    /// Return whether this requirement is satisfied by the supplied snapshot.
    pub const fn matches(self, context: CommandPaletteContext) -> bool {
        match self {
            Self::Any => true,
            Self::NoSession => !context.has_session,
            Self::HasSession => context.has_session,
            Self::HasPreviousSession => context.can_previous_session,
            Self::HasNextSession => context.can_next_session,
        }
    }
}

/// An action identifier that the UI can map to an existing workspace entry.
///
/// These values describe intent only. They do not execute an action or retain
/// a GPUI entity, SSH session, file handle, or other mutable capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandPaletteAction {
    /// Open the saved connection manager.
    OpenConnectionManager,
    /// Open the existing quick-connect/new-session entry point.
    QuickConnect,
    /// Select the previous SSH session tab.
    PreviousSession,
    /// Select the next SSH session tab.
    NextSession,
    /// Toggle the AI assistant surface.
    ToggleAssistant,
    /// Open the remote file panel.
    OpenFilePanel,
    /// Select the host-monitor navigation panel.
    OpenHostMonitor,
    /// Open the SSH tunnel panel.
    OpenTunnels,
    /// Open the command library and snippet surface.
    OpenCommandSnippets,
    /// Open reviewed multi-host batch commands.
    OpenBatchCommands,
    /// Open the reviewed dependency workflow.
    OpenDependencyWorkflow,
    /// Open theme and appearance settings.
    ToggleAppearance,
    /// Open the existing credential-vault settings entry.
    OpenCredentialSettings,
    /// Open the existing AI settings entry.
    OpenAiSettings,
    /// Open the existing about and updates panel.
    OpenUpdates,
}

impl CommandPaletteAction {
    /// Return the stable UI route key associated with this action.
    ///
    /// The key follows existing element/action terminology where one exists,
    /// so a future dispatcher can map it without changing palette ranking.
    pub const fn route_key(self) -> &'static str {
        match self {
            Self::OpenConnectionManager => "open-connections",
            Self::QuickConnect => "new-session",
            Self::PreviousSession => "previous-session",
            Self::NextSession => "next-session",
            Self::ToggleAssistant => "toggle-assistant",
            Self::OpenFilePanel => "files",
            Self::OpenHostMonitor => "navigation-monitor",
            Self::OpenTunnels => "tunnels",
            Self::OpenCommandSnippets => "commands",
            Self::OpenBatchCommands => "command-batch",
            Self::OpenDependencyWorkflow => "command-workflow",
            Self::ToggleAppearance => "theme-appearance",
            Self::OpenCredentialSettings => "vault-settings",
            Self::OpenAiSettings => "ai-settings",
            Self::OpenUpdates => "about-updates",
        }
    }
}

/// A pair of static strings for one palette field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BilingualText {
    /// Simplified Chinese text.
    pub zh: &'static str,
    /// English text.
    pub en: &'static str,
}

impl BilingualText {
    /// Construct a static bilingual field.
    pub const fn new(zh: &'static str, en: &'static str) -> Self {
        Self { zh, en }
    }

    /// Select one translation without allocating.
    pub const fn get(self, language: CommandPaletteLanguage) -> &'static str {
        match language {
            CommandPaletteLanguage::ZhCn => self.zh,
            CommandPaletteLanguage::En => self.en,
        }
    }
}

/// Platform-specific display text for an existing keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutHint {
    /// macOS display text.
    pub macos: &'static str,
    /// Windows/Linux display text.
    pub other: &'static str,
}

impl ShortcutHint {
    /// Construct a platform-specific shortcut hint.
    pub const fn new(macos: &'static str, other: &'static str) -> Self {
        Self { macos, other }
    }
}

/// One immutable command-palette registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandPaletteItem {
    /// Intent to dispatch after the user selects this item.
    pub action: CommandPaletteAction,
    /// Localized visible title.
    pub title: BilingualText,
    /// Localized supporting description.
    pub description: BilingualText,
    /// Chinese search terms in addition to the title and description.
    pub keywords_zh: &'static [&'static str],
    /// English search terms in addition to the title and description.
    pub keywords_en: &'static [&'static str],
    /// Existing keyboard shortcut, when one is already registered.
    pub shortcut: Option<ShortcutHint>,
    /// Minimal context required before this item is visible.
    pub requirement: CommandPaletteContextRequirement,
}

impl CommandPaletteItem {
    /// Return the title for a locale, including the assistant's open state.
    pub fn localized_title(
        self,
        language: CommandPaletteLanguage,
        context: CommandPaletteContext,
    ) -> &'static str {
        if self.action == CommandPaletteAction::ToggleAssistant && context.assistant_open {
            match language {
                CommandPaletteLanguage::ZhCn => "关闭 AI 助手",
                CommandPaletteLanguage::En => "Close AI assistant",
            }
        } else {
            self.title.get(language)
        }
    }

    /// Return the description for a locale without allocating.
    pub const fn localized_description(self, language: CommandPaletteLanguage) -> &'static str {
        self.description.get(language)
    }

    /// Return whether the action is currently toggled on.
    pub fn is_active(self, context: CommandPaletteContext) -> bool {
        self.action == CommandPaletteAction::ToggleAssistant && context.assistant_open
    }

    /// Return whether this item can be shown in the supplied context.
    pub const fn is_available(self, context: CommandPaletteContext) -> bool {
        self.requirement.matches(context)
    }
}

/// Why a query matched a palette item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPaletteMatchKind {
    /// Returned for a blank query in registration/default order.
    Default,
    /// The query starts the title in either supported language.
    TitlePrefix,
    /// The query occurs somewhere in the title in either supported language.
    TitleContains,
    /// The query occurs in a keyword or description.
    MetadataContains,
}

/// A ranked result retaining the immutable item metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandPaletteMatch {
    /// The matched registration.
    pub item: CommandPaletteItem,
    /// The strongest reason the item matched.
    pub kind: CommandPaletteMatchKind,
}

/// Maximum number of results returned by the default palette query.
pub const DEFAULT_RESULT_LIMIT: usize = 12;

#[cfg(test)]
const NO_KEYWORDS: &[&str] = &[];
const OPEN_CONNECTION_KEYWORDS_ZH: &[&str] = &["连接", "连接管理", "主机", "配置"];
const OPEN_CONNECTION_KEYWORDS_EN: &[&str] = &["connection", "manager", "host", "profile"];
const QUICK_CONNECT_KEYWORDS_ZH: &[&str] = &["快速连接", "新会话", "新建", "ssh"];
const QUICK_CONNECT_KEYWORDS_EN: &[&str] = &["quick connect", "new session", "ssh"];
const PREVIOUS_SESSION_KEYWORDS_ZH: &[&str] = &["上一个", "上个会话", "左移"];
const PREVIOUS_SESSION_KEYWORDS_EN: &[&str] = &["previous", "back", "session"];
const NEXT_SESSION_KEYWORDS_ZH: &[&str] = &["下一个", "下个会话", "右移"];
const NEXT_SESSION_KEYWORDS_EN: &[&str] = &["next", "forward", "session"];
const ASSISTANT_KEYWORDS_ZH: &[&str] = &["ai", "助手", "侧栏", "问答"];
const ASSISTANT_KEYWORDS_EN: &[&str] = &["ai", "assistant", "chat", "side panel"];
const FILE_KEYWORDS_ZH: &[&str] = &["文件", "文件面板", "远程文件", "sftp"];
const FILE_KEYWORDS_EN: &[&str] = &["file", "files", "file panel", "remote files", "sftp"];
const MONITOR_KEYWORDS_ZH: &[&str] = &["监控", "主机监控", "主机信息", "资源"];
const MONITOR_KEYWORDS_EN: &[&str] = &["monitor", "host monitor", "host info", "resources"];
const TUNNEL_KEYWORDS_ZH: &[&str] = &["隧道", "端口转发", "转发", "代理"];
const TUNNEL_KEYWORDS_EN: &[&str] = &["tunnel", "tunnels", "port forwarding", "proxy"];
const SNIPPET_KEYWORDS_ZH: &[&str] = &["片段", "命令片段", "命令库", "历史"];
const SNIPPET_KEYWORDS_EN: &[&str] = &["snippet", "snippets", "command library", "history"];
const BATCH_KEYWORDS_ZH: &[&str] = &["批量", "批量执行", "多主机"];
const BATCH_KEYWORDS_EN: &[&str] = &["batch", "batch commands", "multi-host"];
const WORKFLOW_KEYWORDS_ZH: &[&str] = &["依赖", "工作流", "定时", "审阅"];
const WORKFLOW_KEYWORDS_EN: &[&str] = &["dependency", "workflow", "scheduled", "review"];
const APPEARANCE_KEYWORDS_ZH: &[&str] = &["外观", "主题", "颜色", "明暗"];
const APPEARANCE_KEYWORDS_EN: &[&str] = &["appearance", "theme", "color", "dark mode"];
const CREDENTIAL_KEYWORDS_ZH: &[&str] = &["设置", "凭据", "凭据库", "密码"];
const CREDENTIAL_KEYWORDS_EN: &[&str] = &["settings", "credentials", "vault", "password"];
const AI_SETTINGS_KEYWORDS_ZH: &[&str] = &["设置", "ai 设置", "模型", "提供商"];
const AI_SETTINGS_KEYWORDS_EN: &[&str] = &["settings", "ai settings", "model", "provider"];
const UPDATE_KEYWORDS_ZH: &[&str] = &["更新", "关于", "版本", "检查更新"];
const UPDATE_KEYWORDS_EN: &[&str] = &["update", "updates", "about", "check for updates"];

const CMD_T: ShortcutHint = ShortcutHint::new("⌘T", "Ctrl+Shift+T");
const CMD_J: ShortcutHint = ShortcutHint::new("⌘J", "Ctrl+Shift+J");
const CMD_ALT_LEFT: ShortcutHint = ShortcutHint::new("⌘⌥←", "Ctrl+Alt+Left");
const CMD_ALT_RIGHT: ShortcutHint = ShortcutHint::new("⌘⌥→", "Ctrl+Alt+Right");

const fn item(
    action: CommandPaletteAction,
    title: BilingualText,
    description: BilingualText,
    keywords_zh: &'static [&'static str],
    keywords_en: &'static [&'static str],
    shortcut: Option<ShortcutHint>,
    requirement: CommandPaletteContextRequirement,
) -> CommandPaletteItem {
    CommandPaletteItem {
        action,
        title,
        description,
        keywords_zh,
        keywords_en,
        shortcut,
        requirement,
    }
}

/// The registered actions in default-frequency order.
///
/// The first [`DEFAULT_RESULT_LIMIT`] entries are the blank-query defaults.
/// Registration order is also the tie-break order for equal query ranks.
pub const COMMAND_PALETTE_ITEMS: &[CommandPaletteItem] = &[
    item(
        CommandPaletteAction::OpenConnectionManager,
        BilingualText::new("打开连接管理器", "Open connection manager"),
        BilingualText::new(
            "浏览、筛选或编辑已保存的 SSH 连接",
            "Browse, filter, or edit saved SSH connections",
        ),
        OPEN_CONNECTION_KEYWORDS_ZH,
        OPEN_CONNECTION_KEYWORDS_EN,
        Some(CMD_T),
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::QuickConnect,
        BilingualText::new("快速连接 / 新建会话", "Quick connect / new session"),
        BilingualText::new(
            "打开一次性 SSH 连接表单，不自动保存连接配置",
            "Open a one-time SSH connection form without saving the profile",
        ),
        QUICK_CONNECT_KEYWORDS_ZH,
        QUICK_CONNECT_KEYWORDS_EN,
        Some(CMD_T),
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::PreviousSession,
        BilingualText::new("上一个会话", "Previous session"),
        BilingualText::new(
            "切换到前一个 SSH 会话标签",
            "Switch to the previous SSH session tab",
        ),
        PREVIOUS_SESSION_KEYWORDS_ZH,
        PREVIOUS_SESSION_KEYWORDS_EN,
        Some(CMD_ALT_LEFT),
        CommandPaletteContextRequirement::HasPreviousSession,
    ),
    item(
        CommandPaletteAction::NextSession,
        BilingualText::new("下一个会话", "Next session"),
        BilingualText::new(
            "切换到后一个 SSH 会话标签",
            "Switch to the next SSH session tab",
        ),
        NEXT_SESSION_KEYWORDS_ZH,
        NEXT_SESSION_KEYWORDS_EN,
        Some(CMD_ALT_RIGHT),
        CommandPaletteContextRequirement::HasNextSession,
    ),
    item(
        CommandPaletteAction::ToggleAssistant,
        BilingualText::new("切换 AI 助手", "Toggle AI assistant"),
        BilingualText::new(
            "打开或关闭 AI 助手侧栏",
            "Open or close the AI assistant side panel",
        ),
        ASSISTANT_KEYWORDS_ZH,
        ASSISTANT_KEYWORDS_EN,
        Some(CMD_J),
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::OpenFilePanel,
        BilingualText::new("打开文件面板", "Open file panel"),
        BilingualText::new(
            "查看远程目录并准备文件操作",
            "Browse the remote directory and prepare file operations",
        ),
        FILE_KEYWORDS_ZH,
        FILE_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::HasSession,
    ),
    item(
        CommandPaletteAction::OpenHostMonitor,
        BilingualText::new("打开主机监控", "Open host monitor"),
        BilingualText::new(
            "查看主机资源、网络和进程信息",
            "View host resources, network, and process information",
        ),
        MONITOR_KEYWORDS_ZH,
        MONITOR_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::HasSession,
    ),
    item(
        CommandPaletteAction::OpenTunnels,
        BilingualText::new("打开隧道", "Open tunnels"),
        BilingualText::new(
            "管理当前 SSH 会话的端口转发",
            "Manage port forwarding for the current SSH session",
        ),
        TUNNEL_KEYWORDS_ZH,
        TUNNEL_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::HasSession,
    ),
    item(
        CommandPaletteAction::OpenCommandSnippets,
        BilingualText::new("打开命令片段", "Open command snippets"),
        BilingualText::new(
            "搜索历史命令和已保存的命令片段",
            "Search command history and saved snippets",
        ),
        SNIPPET_KEYWORDS_ZH,
        SNIPPET_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::OpenBatchCommands,
        BilingualText::new("打开批量命令", "Open batch commands"),
        BilingualText::new(
            "审核后向多个已连接目标发送命令",
            "Review commands before sending them to connected targets",
        ),
        BATCH_KEYWORDS_ZH,
        BATCH_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::HasSession,
    ),
    item(
        CommandPaletteAction::OpenDependencyWorkflow,
        BilingualText::new("打开依赖工作流", "Open dependency workflow"),
        BilingualText::new(
            "审核依赖任务并按计划执行",
            "Review dependency tasks and run them on a schedule",
        ),
        WORKFLOW_KEYWORDS_ZH,
        WORKFLOW_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::HasSession,
    ),
    item(
        CommandPaletteAction::ToggleAppearance,
        BilingualText::new("切换外观", "Change appearance"),
        BilingualText::new(
            "选择主题并调整明暗模式",
            "Choose a theme and adjust the appearance mode",
        ),
        APPEARANCE_KEYWORDS_ZH,
        APPEARANCE_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::OpenCredentialSettings,
        BilingualText::new("打开凭据设置", "Open credential settings"),
        BilingualText::new(
            "管理受保护的本地凭据库",
            "Manage the protected local credential vault",
        ),
        CREDENTIAL_KEYWORDS_ZH,
        CREDENTIAL_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::OpenAiSettings,
        BilingualText::new("打开 AI 设置", "Open AI settings"),
        BilingualText::new(
            "配置 AI 模型、提供商和访问凭据",
            "Configure AI models, providers, and access credentials",
        ),
        AI_SETTINGS_KEYWORDS_ZH,
        AI_SETTINGS_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::Any,
    ),
    item(
        CommandPaletteAction::OpenUpdates,
        BilingualText::new("打开设置 / 更新", "Open about / updates"),
        BilingualText::new(
            "查看应用信息、检查并审核更新",
            "View app information and check or review updates",
        ),
        UPDATE_KEYWORDS_ZH,
        UPDATE_KEYWORDS_EN,
        None,
        CommandPaletteContextRequirement::Any,
    ),
];

/// Return the immutable default registration list.
pub const fn default_items() -> &'static [CommandPaletteItem] {
    COMMAND_PALETTE_ITEMS
}

/// Search the default registrations with the standard result cap.
pub fn search(query: &str, context: CommandPaletteContext) -> Vec<CommandPaletteMatch> {
    search_in(default_items(), query, context, DEFAULT_RESULT_LIMIT)
}

/// Search an explicit registration list.
///
/// Title prefixes rank ahead of title substrings, which rank ahead of keyword
/// and description substrings. The scan is stable: equal ranks retain the
/// input slice order, and duplicate actions retain their first registration.
/// A blank query skips matching and returns available items in registration
/// order, bounded by `limit`.
pub fn search_in(
    items: &[CommandPaletteItem],
    query: &str,
    context: CommandPaletteContext,
    limit: usize,
) -> Vec<CommandPaletteMatch> {
    if limit == 0 {
        return Vec::new();
    }

    let Some(query) = Query::new(query) else {
        return items
            .iter()
            .copied()
            .filter(|item| item.is_available(context))
            .map(|item| CommandPaletteMatch {
                item,
                kind: CommandPaletteMatchKind::Default,
            })
            .take(limit)
            .collect();
    };

    let mut matches = Vec::with_capacity(limit.min(items.len()));
    for item in items.iter().copied() {
        if !item.is_available(context)
            || matches
                .iter()
                .any(|matched: &CommandPaletteMatch| matched.item.action == item.action)
        {
            continue;
        }
        let Some(kind) = item.match_kind(&query) else {
            continue;
        };
        let candidate = CommandPaletteMatch { item, kind };
        let insertion = matches
            .iter()
            .position(|matched| matched.kind > candidate.kind)
            .unwrap_or(matches.len());
        if insertion < limit {
            matches.insert(insertion, candidate);
            matches.truncate(limit);
        }
    }
    matches
}

impl CommandPaletteItem {
    fn match_kind(self, query: &Query) -> Option<CommandPaletteMatchKind> {
        let title_rank = [self.title.zh, self.title.en]
            .into_iter()
            .filter_map(|title| query.rank(title))
            .min();
        match title_rank {
            Some(QueryMatchRank::Prefix) => return Some(CommandPaletteMatchKind::TitlePrefix),
            Some(QueryMatchRank::Contains) => return Some(CommandPaletteMatchKind::TitleContains),
            None => {}
        }

        let metadata = [self.description.zh, self.description.en]
            .into_iter()
            .chain(self.keywords_zh.iter().copied())
            .chain(self.keywords_en.iter().copied());
        metadata
            .filter_map(|field| query.rank(field))
            .next()
            .map(|_| CommandPaletteMatchKind::MetadataContains)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QueryMatchRank {
    Prefix,
    Contains,
}

struct Query {
    folded: Vec<char>,
    fallback: Vec<usize>,
}

impl Query {
    fn new(query: &str) -> Option<Self> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }

        let folded: Vec<_> = query.chars().flat_map(char::to_lowercase).collect();
        let mut fallback = vec![0; folded.len()];
        let mut matched = 0;
        for index in 1..folded.len() {
            while matched > 0 && folded[index] != folded[matched] {
                matched = fallback[matched - 1];
            }
            if folded[index] == folded[matched] {
                matched += 1;
            }
            fallback[index] = matched;
        }
        Some(Self { folded, fallback })
    }

    fn rank(&self, value: &str) -> Option<QueryMatchRank> {
        let mut matched = 0;
        for (index, character) in value.chars().flat_map(char::to_lowercase).enumerate() {
            while matched > 0 && self.folded[matched] != character {
                matched = self.fallback[matched - 1];
            }
            if self.folded[matched] == character {
                matched += 1;
                if matched == self.folded.len() {
                    return Some(if index + 1 == matched {
                        QueryMatchRank::Prefix
                    } else {
                        QueryMatchRank::Contains
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BilingualText, CommandPaletteAction, CommandPaletteContext,
        CommandPaletteContextRequirement, CommandPaletteItem, CommandPaletteLanguage,
        CommandPaletteMatchKind, DEFAULT_RESULT_LIMIT, NO_KEYWORDS, ShortcutHint, default_items,
        search, search_in,
    };

    fn actions(results: &[super::CommandPaletteMatch]) -> Vec<CommandPaletteAction> {
        results.iter().map(|result| result.item.action).collect()
    }

    #[test]
    fn chinese_and_english_queries_find_the_same_action() {
        let context = CommandPaletteContext {
            has_session: true,
            assistant_open: false,
            can_previous_session: true,
            can_next_session: true,
        };
        assert_eq!(
            actions(&search("主机监控", context)),
            vec![CommandPaletteAction::OpenHostMonitor]
        );
        assert_eq!(
            actions(&search("host monitor", context)),
            vec![CommandPaletteAction::OpenHostMonitor]
        );
    }

    #[test]
    fn title_prefix_precedes_title_and_metadata_contains() {
        let items = [
            CommandPaletteItem {
                action: CommandPaletteAction::OpenUpdates,
                title: BilingualText::new("查看版本", "View version"),
                description: BilingualText::new("检查更新", "Check for updates"),
                keywords_zh: &["更新"],
                keywords_en: &["update"],
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::OpenAiSettings,
                title: BilingualText::new("模型设置", "Model settings"),
                description: BilingualText::new("配置 AI 提供商", "Configure an AI provider"),
                keywords_zh: &["更新"],
                keywords_en: &["update"],
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::ToggleAppearance,
                title: BilingualText::new("更新外观", "Update appearance"),
                description: BilingualText::new("主题设置", "Theme settings"),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
        ];
        let results = search_in(
            &items,
            "update",
            CommandPaletteContext::default(),
            DEFAULT_RESULT_LIMIT,
        );
        assert_eq!(
            results.iter().map(|result| result.kind).collect::<Vec<_>>(),
            vec![
                CommandPaletteMatchKind::TitlePrefix,
                CommandPaletteMatchKind::MetadataContains,
                CommandPaletteMatchKind::MetadataContains,
            ]
        );
        assert_eq!(
            actions(&results),
            vec![
                CommandPaletteAction::ToggleAppearance,
                CommandPaletteAction::OpenUpdates,
                CommandPaletteAction::OpenAiSettings,
            ]
        );
    }

    #[test]
    fn duplicate_actions_are_removed_and_equal_ranks_keep_registration_order() {
        let items = [
            CommandPaletteItem {
                action: CommandPaletteAction::QuickConnect,
                title: BilingualText::new("快速连接", "Quick connect"),
                description: BilingualText::new("新会话", "New session"),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::OpenConnectionManager,
                title: BilingualText::new("连接管理器", "Connection manager"),
                description: BilingualText::new("快速连接", "Quick connect"),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::QuickConnect,
                title: BilingualText::new("快速连接别名", "Quick connect alias"),
                description: BilingualText::new("", ""),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
        ];
        let results = search_in(
            &items,
            "quick connect",
            CommandPaletteContext::default(),
            DEFAULT_RESULT_LIMIT,
        );
        assert_eq!(
            actions(&results),
            vec![
                CommandPaletteAction::QuickConnect,
                CommandPaletteAction::OpenConnectionManager,
            ]
        );
    }

    #[test]
    fn session_required_items_are_hidden_without_a_session() {
        let no_session = search(
            "",
            CommandPaletteContext {
                has_session: false,
                assistant_open: false,
                can_previous_session: false,
                can_next_session: false,
            },
        );
        assert!(!actions(&no_session).contains(&CommandPaletteAction::OpenFilePanel));
        assert!(!actions(&no_session).contains(&CommandPaletteAction::PreviousSession));
        assert!(!actions(&no_session).contains(&CommandPaletteAction::OpenBatchCommands));
        assert!(actions(&no_session).contains(&CommandPaletteAction::QuickConnect));

        let with_session = search(
            "",
            CommandPaletteContext {
                has_session: true,
                assistant_open: false,
                can_previous_session: true,
                can_next_session: true,
            },
        );
        assert!(actions(&with_session).contains(&CommandPaletteAction::OpenFilePanel));
        assert!(actions(&with_session).contains(&CommandPaletteAction::PreviousSession));
    }

    #[test]
    fn context_requirement_covers_no_session_any_and_has_session() {
        let items = [
            CommandPaletteItem {
                action: CommandPaletteAction::OpenUpdates,
                title: BilingualText::new("任意", "Any"),
                description: BilingualText::new("", ""),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::Any,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::QuickConnect,
                title: BilingualText::new("无会话", "No session"),
                description: BilingualText::new("", ""),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::NoSession,
            },
            CommandPaletteItem {
                action: CommandPaletteAction::OpenFilePanel,
                title: BilingualText::new("有会话", "Has session"),
                description: BilingualText::new("", ""),
                keywords_zh: NO_KEYWORDS,
                keywords_en: NO_KEYWORDS,
                shortcut: None,
                requirement: CommandPaletteContextRequirement::HasSession,
            },
        ];
        assert_eq!(
            actions(&search_in(
                &items,
                "",
                CommandPaletteContext::default(),
                DEFAULT_RESULT_LIMIT
            )),
            vec![
                CommandPaletteAction::OpenUpdates,
                CommandPaletteAction::QuickConnect
            ]
        );
        assert_eq!(
            actions(&search_in(
                &items,
                "",
                CommandPaletteContext {
                    has_session: true,
                    assistant_open: false,
                    can_previous_session: true,
                    can_next_session: true,
                },
                DEFAULT_RESULT_LIMIT
            )),
            vec![
                CommandPaletteAction::OpenUpdates,
                CommandPaletteAction::OpenFilePanel
            ]
        );
    }

    #[test]
    fn result_limit_is_respected_for_queries_and_blank_defaults() {
        assert_eq!(
            search(
                "",
                CommandPaletteContext {
                    has_session: true,
                    assistant_open: false,
                    can_previous_session: true,
                    can_next_session: true,
                }
            )
            .len(),
            DEFAULT_RESULT_LIMIT
        );
        assert_eq!(
            search_in(
                default_items(),
                "",
                CommandPaletteContext {
                    has_session: true,
                    assistant_open: false,
                    can_previous_session: true,
                    can_next_session: true,
                },
                3
            )
            .len(),
            3
        );
        assert!(
            search_in(
                default_items(),
                "session",
                CommandPaletteContext {
                    has_session: true,
                    assistant_open: false,
                    can_previous_session: false,
                    can_next_session: false,
                },
                0
            )
            .is_empty()
        );
    }

    #[test]
    fn blank_query_keeps_default_order_and_assistant_state_is_renderable() {
        let context = CommandPaletteContext {
            has_session: false,
            assistant_open: true,
            can_previous_session: false,
            can_next_session: false,
        };
        let results = search("", context);
        assert_eq!(
            results.first().map(|result| result.item.action),
            Some(CommandPaletteAction::OpenConnectionManager)
        );
        let assistant = default_items()
            .iter()
            .copied()
            .find(|item| item.action == CommandPaletteAction::ToggleAssistant)
            .unwrap_or_else(|| panic!("default palette must register the assistant action"));
        assert_eq!(
            assistant.localized_title(CommandPaletteLanguage::ZhCn, context),
            "关闭 AI 助手"
        );
        assert_eq!(
            assistant.localized_title(CommandPaletteLanguage::En, context),
            "Close AI assistant"
        );
        assert!(assistant.is_active(context));
        assert_eq!(
            assistant.localized_description(CommandPaletteLanguage::En),
            "Open or close the AI assistant side panel"
        );
    }

    #[test]
    fn directional_session_actions_follow_the_actual_tab_boundaries() {
        let first = search(
            "",
            CommandPaletteContext {
                has_session: true,
                assistant_open: false,
                can_previous_session: false,
                can_next_session: true,
            },
        );
        assert!(!actions(&first).contains(&CommandPaletteAction::PreviousSession));
        assert!(actions(&first).contains(&CommandPaletteAction::NextSession));

        let last = search(
            "",
            CommandPaletteContext {
                has_session: true,
                assistant_open: false,
                can_previous_session: true,
                can_next_session: false,
            },
        );
        assert!(actions(&last).contains(&CommandPaletteAction::PreviousSession));
        assert!(!actions(&last).contains(&CommandPaletteAction::NextSession));

        let only = search(
            "",
            CommandPaletteContext {
                has_session: true,
                assistant_open: false,
                can_previous_session: false,
                can_next_session: false,
            },
        );
        assert!(!actions(&only).contains(&CommandPaletteAction::PreviousSession));
        assert!(!actions(&only).contains(&CommandPaletteAction::NextSession));
    }

    #[test]
    fn shortcut_hints_remain_static_and_bilingual_fields_select_without_allocating() {
        let hint = ShortcutHint::new("⌘K", "Ctrl+K");
        assert_eq!(hint.macos, "⌘K");
        assert_eq!(hint.other, "Ctrl+K");
        let text = BilingualText::new("连接", "Connect");
        assert_eq!(text.get(CommandPaletteLanguage::ZhCn), "连接");
        assert_eq!(text.get(CommandPaletteLanguage::En), "Connect");
    }
}
