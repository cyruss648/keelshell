//! Restore browsing context from the old visible ancestry after data changes.
use super::{keyboard::*, *};

#[gpui_kit::test]
fn removed_or_filtered_connection_selects_its_surviving_parent(cx: &mut TestAppContext) {
    for filter in [false, true] {
        let target = saved_profile("Target");
        let sibling = saved_profile("Match sibling");
        let other = saved_profile("Match other");
        let (fixture, panes) =
            navigation_fixture(cx, vec![target.clone(), sibling.clone(), other.clone()]);
        cx.update_window(fixture.window, |_, window, cx| {
            let parent = fixture.workspace.update(cx, |view, cx| {
                let unrelated = view
                    .state
                    .create_folder("A Other", None)
                    .checked("first root");
                let parent = view
                    .state
                    .create_folder("Z Production", None)
                    .checked("target root");
                for profile in [&target, &sibling] {
                    view.state
                        .move_connection(profile.id, Some(parent))
                        .checked("file target");
                }
                view.state
                    .move_connection(other.id, Some(unrelated))
                    .checked("file other");
                cx.notify();
                parent
            });
            window.render_frame(cx);
            window.click("navigation-search", cx);
            press(window, cx, "down");
            press(window, cx, "end");
            assert_eq!(window.find(row_id(target.id)).selected(), Some(true));
            fixture.workspace.update(cx, |view, cx| {
                if filter {
                    view.search
                        .update(cx, |input, cx| input.set_value("Match", window, cx));
                } else {
                    view.state
                        .soft_delete_connection(target.id, 1)
                        .checked("soft delete target");
                }
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find(row_id(target.id)).is_none());
            assert_eq!(window.find(folder_id(parent)).selected(), Some(true));
        })
        .checked("removal preserves the nearest visible parent");
        assert!(panes.iter().all(|pane| writes(pane).is_empty()));
    }
}

#[gpui_kit::test]
fn disappearing_parent_chain_selects_surviving_ancestor_then_first_root(cx: &mut TestAppContext) {
    let (fixture, panes) = navigation_fixture(cx, vec![]);
    cx.update_window(fixture.window, |_, window, cx| {
        let (first, root, parent, leaf) = fixture.workspace.update(cx, |view, cx| {
            let first = view.state.create_folder("A Other", None).checked("first");
            let root = view
                .state
                .create_folder("Z Production", None)
                .checked("root");
            let parent = view
                .state
                .create_folder("Branch", Some(root))
                .checked("parent");
            let leaf = view
                .state
                .create_folder("Leaf", Some(parent))
                .checked("leaf");
            cx.notify();
            (first, root, parent, leaf)
        });
        window.render_frame(cx);
        window.click("navigation-search", cx);
        press(window, cx, "tab");
        press(window, cx, "end");
        assert_eq!(window.find(folder_id(leaf)).selected(), Some(true));
        fixture.workspace.update(cx, |view, cx| {
            view.state.remove_folder(leaf).checked("remove leaf");
            view.state.remove_folder(parent).checked("remove parent");
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find(folder_id(root)).selected(), Some(true));
        fixture.workspace.update(cx, |view, cx| {
            view.state
                .remove_folder(root)
                .checked("remove last ancestor");
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find(folder_id(first)).selected(), Some(true));
    })
    .checked("walk old ancestors rather than jumping to unrelated root");
    assert!(panes.iter().all(|pane| writes(pane).is_empty()));
}
