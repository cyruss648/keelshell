//! Keyboard browsing never activates a connection until the key is released.
use super::*;

impl Workspace {
    fn navigation_accepts_keys(&self, window: &Window) -> bool {
        self.navigation_tree.focus.is_focused(window)
            && self.active_modal().is_none()
            && self.navigation_visible(window)
            && self.navigation_panel == NavigationPanel::Connections
    }

    pub(in crate::workspace) fn intercept_navigation_keys(
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        let owner = cx.entity().downgrade();
        let owner_window = window.window_handle().window_id();
        // Bound actions precede raw key handlers in GPUI. Keep the guard scoped
        // to this workspace's window, including replacement modal surfaces.
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() != owner_window {
                return;
            }
            let blocked = owner.update(cx, |view, cx| {
                let modifiers = event.keystroke.modifiers;
                let session_modifier = if cfg!(target_os = "macos") {
                    modifiers.platform && modifiers.alt && !modifiers.control && !modifiers.shift
                } else {
                    modifiers.control && modifiers.alt && !modifiers.platform && !modifiers.shift
                };
                if !modifiers.function
                    && session_modifier
                    && matches!(event.keystroke.key.as_str(), "left" | "right")
                {
                    if event.keystroke.key == "left" {
                        view.previous_session(&PreviousSession, window, cx);
                    } else {
                        view.next_session(&NextSession, window, cx);
                    }
                    return true;
                }
                let blocked = view.navigation_tree.held.contains(&event.keystroke.key)
                    && !view.navigation_accepts_keys(window);
                if blocked {
                    view.navigation_tree.armed = None;
                }
                blocked
            });
            if matches!(blocked, Ok(true)) {
                window.prevent_default();
                cx.stop_propagation();
            }
        })
    }

    pub(in crate::workspace) fn capture_navigation_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke.key;
        // A fresh down can recover a missed release after OS focus loss. The
        // interceptor still discards that first cross-focus event before actions.
        if !event.is_held && self.navigation_tree.held.remove(key) {
            self.navigation_tree.armed = None;
        }
        if self.navigation_tree.held.contains(key) && !self.navigation_accepts_keys(window) {
            self.navigation_tree.armed = None;
            window.prevent_default();
            cx.stop_propagation();
        }
        self.capture_surface_input(window, cx);
    }

    pub(in crate::workspace) fn capture_navigation_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.navigation_tree.held.remove(&event.keystroke.key)
            && !self.navigation_accepts_keys(window)
        {
            self.navigation_tree.armed = None;
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    pub(super) fn activate_navigation_node(
        &mut self,
        id: NodeId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_modal().is_some()
            || !self.navigation_visible(window)
            || self.navigation_panel != NavigationPanel::Connections
        {
            return;
        }
        self.navigation_tree
            .synchronize(&self.state, &self.search.read(cx).value());
        if !self.navigation_tree.visible.iter().any(|row| row.id == id) {
            if let NodeId::Connection(profile) = id
                && !self
                    .state
                    .connections
                    .iter()
                    .any(|connection| connection.id == profile)
            {
                self.status = Message::new(
                    "连接已移除，请重新选择。",
                    "Connection removed. Select another saved connection.",
                );
                cx.notify();
            }
            return;
        }
        self.navigation_tree.select(id);
        self.navigation_tree.focus.focus(window, cx);
        match id {
            NodeId::Connection(profile) => self.activate_navigation_connection(profile, window, cx),
            _ => self.navigation_tree.toggle(id),
        }
        cx.notify();
    }

    pub(super) fn navigation_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.navigation_accepts_keys(window) {
            self.navigation_tree.armed = None;
            return;
        }
        self.navigation_tree
            .synchronize(&self.state, &self.search.read(cx).value());
        if event.keystroke.modifiers != Modifiers::default() {
            self.navigation_tree.armed = None;
            return;
        }
        let key = event.keystroke.key.as_str();
        if !matches!(
            key,
            "up" | "down" | "left" | "right" | "home" | "end" | "enter" | "space"
        ) {
            self.navigation_tree.armed = None;
            return;
        }
        self.navigation_tree.held.insert(key.into());
        let Some(index) = self
            .navigation_tree
            .visible
            .iter()
            .position(|row| Some(row.id) == self.navigation_tree.cursor)
        else {
            window.prevent_default();
            cx.stop_propagation();
            return;
        };
        let row = self.navigation_tree.visible[index].clone();
        let mut next = None;
        if !matches!(key, "enter" | "space") {
            self.navigation_tree.armed = None;
        }
        match key {
            "up" => next = Some(index.saturating_sub(1)),
            "down" => next = Some((index + 1).min(self.navigation_tree.visible.len() - 1)),
            "home" => next = Some(0),
            "end" => next = Some(self.navigation_tree.visible.len() - 1),
            "left" if row.expanded => self.navigation_tree.toggle(row.id),
            "left" => {
                next = row.parent.and_then(|parent| {
                    self.navigation_tree
                        .visible
                        .iter()
                        .position(|row| row.id == parent)
                })
            }
            "right" if row.has_children && !row.expanded => self.navigation_tree.toggle(row.id),
            "right" if row.expanded => {
                next = self
                    .navigation_tree
                    .visible
                    .get(index + 1)
                    .filter(|child| child.parent == Some(row.id))
                    .map(|_| index + 1)
            }
            "right" => {}
            "enter" | "space" => {
                if self.navigation_tree.held.len() != 1 {
                    self.navigation_tree.armed = None;
                } else if !event.is_held {
                    self.navigation_tree.armed = Some((key.into(), row.id));
                }
            }
            _ => {
                self.navigation_tree.armed = None;
                return;
            }
        }
        if let Some(index) = next {
            self.navigation_tree
                .select(self.navigation_tree.visible[index].id);
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn navigation_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigation_tree
            .synchronize(&self.state, &self.search.read(cx).value());
        let armed = self.navigation_tree.armed.take();
        if let Some((key, id)) = armed
            && event.keystroke.key == key
            && event.keystroke.modifiers == Modifiers::default()
            && self.navigation_tree.focus.is_focused(window)
            && self.navigation_tree.cursor == Some(id)
            && self.navigation_tree.held.is_empty()
        {
            self.activate_navigation_node(id, window, cx);
            window.prevent_default();
            cx.stop_propagation();
        }
    }
}
