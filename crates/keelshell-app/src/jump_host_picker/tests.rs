use super::{JumpHostPicker, endpoint};
use crate::i18n;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, Focusable, SharedString, TestAppContext,
    WindowBounds, WindowOptions, point, px, size, test::TestWindowExt,
};
use keelshell_core::{AppState, Connection, Language};
use uuid::Uuid;

trait Checked<T> {
    fn checked(self, label: &str) -> T;
}
impl<T, E: std::fmt::Debug> Checked<T> for Result<T, E> {
    fn checked(self, label: &str) -> T {
        self.unwrap_or_else(|error| panic!("{label}: {error:?}"))
    }
}

fn mount(
    cx: &mut TestAppContext,
    state: &AppState,
    editing: Option<Uuid>,
    selected: Option<Uuid>,
) -> (AnyWindowHandle, Entity<JumpHostPicker>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        i18n::set_language(Language::ZhCn, cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(620.), px(720.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| JumpHostPicker::new(state, editing, selected, window, cx)),
        )
        .checked("mount jump picker")
    })
}

fn chain() -> Vec<Connection> {
    let mut connections = Vec::<Connection>::new();
    for index in 0..5 {
        let mut connection = Connection::new(
            format!("gateway-{index}"),
            format!("gateway-{index}.example.test"),
            "operator",
        );
        connection.jump_host = connections.last().map(|connection| connection.id);
        connections.push(connection);
    }
    connections
}

#[gpui_kit::test]
fn self_descendant_and_overlong_routes_cannot_be_selected(cx: &mut TestAppContext) {
    let connections = chain();
    let state = AppState {
        connections: connections.clone(),
        ..Default::default()
    };
    let (window, picker) = mount(cx, &state, Some(connections[1].id), None);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-jump-host", cx);
        for index in [1, 2, 3, 4] {
            picker.read(cx).search.clone().update(cx, |search, cx| {
                search.set_value(connections[index].name.clone(), window, cx)
            });
            window.render_frame(cx);
            let id = SharedString::from(format!("jump-host-select-{}", connections[index].id));
            assert!(window.find(id.clone()).visible());
            window.click(id, cx);
            assert_eq!(picker.read(cx).selected(), None);
            assert!(picker.read(cx).expanded);
            picker.update(cx, |picker, cx| {
                picker.choose(Some(connections[index].id), window, cx)
            });
            assert_eq!(picker.read(cx).selected(), None);
        }
        picker.read(cx).search.clone().update(cx, |search, cx| {
            search.set_value(connections[0].name.clone(), window, cx)
        });
        window.render_frame(cx);
        window.click(
            SharedString::from(format!("jump-host-select-{}", connections[0].id)),
            cx,
        );
        assert_eq!(picker.read(cx).selected(), Some(connections[0].id));
    })
    .checked("reject self and descendant routes at both UI and handler boundaries");
    let (window, picker) = mount(cx, &state, None, None);
    cx.update_window(window, |_, window, cx| {
        assert!(
            picker
                .update(cx, |picker, _| picker.route(connections[4].id))
                .error
                .is_some()
        );
        assert!(
            picker
                .update(cx, |picker, _| picker.route(connections[3].id))
                .error
                .is_none()
        );
        picker.update(cx, |picker, cx| {
            picker.choose(Some(connections[3].id), window, cx)
        });
        assert_eq!(picker.read(cx).selected(), Some(connections[3].id));
    })
    .checked("four jump hosts remain selectable for a new target");
}

#[gpui_kit::test]
fn search_and_pagination_reach_later_connections_without_changing_the_library(
    cx: &mut TestAppContext,
) {
    let state = AppState {
        connections: (0..120)
            .map(|index| {
                Connection::new(
                    format!("gateway-{index:03}"),
                    format!("host-{index}.example.test"),
                    format!("account-{index}"),
                )
            })
            .collect(),
        ..Default::default()
    };
    let before = state.clone();
    let (window, picker) = mount(cx, &state, None, None);
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-jump-host", cx);
        assert!(
            picker
                .read(cx)
                .search
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
        assert_eq!(picker.read(cx).routes.len(), 50);
        window.click("jump-host-next", cx);
        assert_eq!(picker.read(cx).page, 1);
        assert!(
            window
                .try_find(SharedString::from(format!(
                    "jump-host-select-{}",
                    state.connections[0].id
                )))
                .is_none()
        );
        window.click(
            SharedString::from(format!("jump-host-select-{}", state.connections[50].id)),
            cx,
        );
        assert_eq!(picker.read(cx).selected(), Some(state.connections[50].id));
        window.click("choose-jump-host", cx);
        window.click("jump-host-search", cx);
        window.input("account-119", cx);
    })
    .checked("choose the first item on page two then search beyond it");
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(picker.read(cx).page, 0);
        window.click(
            SharedString::from(format!("jump-host-select-{}", state.connections[119].id)),
            cx,
        );
        assert_eq!(picker.read(cx).selected(), Some(state.connections[119].id));
        assert_eq!(picker.read(cx).state, before);
    })
    .checked("search finds a saved account on the final page");
    assert_eq!(state, before);
}

#[gpui_kit::test]
fn language_changes_preserve_search_selection_and_an_unavailable_reference(
    cx: &mut TestAppContext,
) {
    let missing = Uuid::new_v4();
    let state = AppState::default();
    let (window, picker) = mount(cx, &state, None, Some(missing));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("choose-jump-host", cx);
        window.input("中文查询", cx);
        for language in [Language::En, Language::ZhCn] {
            i18n::set_language(language, cx);
            picker.update(cx, |picker, cx| picker.refresh_locale(window, cx));
            window.render_frame(cx);
            assert_eq!(picker.read(cx).selected(), Some(missing));
            assert_eq!(picker.read(cx).search.read(cx).value().as_str(), "中文查询");
            assert!(picker.read(cx).expanded);
            assert!(
                picker
                    .read(cx)
                    .search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            assert_eq!(
                window.find("jump-host-direct").label(),
                Some(if language == Language::En {
                    "No SSH jump host"
                } else {
                    "不使用 SSH 跳板"
                })
            );
        }
        window.click("jump-host-direct", cx);
        assert_eq!(picker.read(cx).selected(), None);
    })
    .checked("locale changes never silently turn an invalid saved route into direct SSH");
}

#[test]
fn endpoint_keeps_ipv6_account_and_port_unambiguous() {
    let mut connection = Connection::new("IPv6", "2001:db8::8", "operator");
    connection.port = 2202;
    assert_eq!(endpoint(&connection), "operator@[2001:db8::8]:2202");
}
