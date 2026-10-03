//! In-form jump-host selection over an immutable connection-library snapshot.
//! Selection changes only this draft; the workspace revalidates the current graph
//! when saving, and no network or credential work happens in this component.

use std::collections::HashMap;

use gpui_kit::{
    component::input::{InputEvent, InputState},
    *,
};
use keelshell_core::{AppState, Connection, MAX_JUMP_HOSTS};
use uuid::Uuid;

use crate::i18n::{Message, t};

#[cfg(test)]
mod tests;
mod view;

const PAGE_SIZE: usize = 50;

#[derive(Clone)]
struct ChoiceRoute {
    hops: Vec<Connection>,
    error: Option<Message>,
}

/// A searchable, paginated draft field with its own transient presentation state.
pub struct JumpHostPicker {
    state: AppState,
    editing: Option<Uuid>,
    selected: Option<Uuid>,
    search: Entity<InputState>,
    page: usize,
    expanded: bool,
    focus: FocusHandle,
    routes: HashMap<Uuid, ChoiceRoute>,
    _subscription: Subscription,
}

impl JumpHostPicker {
    /// Keep an independent choice and metadata snapshot; do not resolve secrets.
    pub fn new(
        state: &AppState,
        editing: Option<Uuid>,
        selected: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(
                cx,
                "搜索名称、主机、账号或标签",
                "Search name, host, account or tags",
            ))
        });
        let subscription = cx.subscribe(&search, |picker, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                picker.page = 0;
                cx.notify();
            }
        });
        Self {
            state: state.clone(),
            editing,
            selected,
            search,
            page: 0,
            expanded: false,
            focus: cx.focus_handle(),
            routes: HashMap::new(),
            _subscription: subscription,
        }
    }

    /// Return the current draft reference, including an unavailable existing one.
    /// An invalid reference is never silently changed into a direct connection.
    pub fn selected(&self) -> Option<Uuid> {
        self.selected
    }

    /// Translate hints without changing the route, query, page or open state.
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = t(
            cx,
            "搜索名称、主机、账号或标签",
            "Search name, host, account or tags",
        );
        self.search.update(cx, |search, cx| {
            search.set_placeholder(placeholder, window, cx)
        });
        cx.notify();
    }

    fn route(&mut self, id: Uuid) -> ChoiceRoute {
        if let Some(cached) = self.routes.get(&id) {
            return cached.clone();
        }
        let route = match self.state.connection_route(id) {
            Ok(route) => {
                let hops = route.hops().to_vec();
                let error = if hops.iter().any(|hop| Some(hop.id) == self.editing) {
                    Some(Message::new(
                        "不能选择当前连接或经过当前连接的路线。",
                        "This choice would route back through the connection being edited.",
                    ))
                } else if hops.len() > MAX_JUMP_HOSTS {
                    Some(Message::new(
                        format!("加入目标后将超过 {MAX_JUMP_HOSTS} 个跳板的上限。"),
                        format!(
                            "Adding the target would exceed the limit of {MAX_JUMP_HOSTS} jump hosts."
                        ),
                    ))
                } else {
                    None
                };
                ChoiceRoute { hops, error }
            }
            Err(_) => ChoiceRoute {
                hops: Vec::new(),
                error: Some(Message::new(
                    "该配置的路线不可用，请先修复连接配置。",
                    "This profile's route is unavailable. Repair its configuration first.",
                )),
            },
        };
        // Only visible-page choices are resolved, once per immutable snapshot.
        self.routes.insert(id, route.clone());
        route
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.expanded = !self.expanded;
        if self.expanded {
            self.search.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn choose(&mut self, id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) {
        if id.is_some_and(|id| self.route(id).error.is_some()) {
            return;
        }
        self.selected = id;
        self.expanded = false;
        self.focus.focus(window, cx);
        cx.notify();
    }
}

/// Format a complete SSH endpoint with unambiguous IPv6 brackets.
pub(crate) fn endpoint(connection: &Connection) -> String {
    let host = if connection.host.contains(':') {
        format!("[{}]", connection.host)
    } else {
        connection.host.clone()
    };
    format!("{}@{host}:{}", connection.username, connection.port)
}
