//! File menu: New, Open, Save, Save As, Export Python. Native OS dialogs via `rfd`, `.orc` files
//! on disk (Python export writes a `.py` file alongside them, but is otherwise unrelated to the
//! save/open/dirty machinery).

use crate::state::EditorState;
use eframe::egui::{self, KeyboardShortcut, Modifiers};
use orc_sdk::Workflow;
use std::path::Path;
use std::sync::atomic::Ordering;

const NEW_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::N);
const OPEN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::O);
const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::S);
// `consume_shortcut` matches modifiers loosely (extra Shift/Alt are ignored), so this must be
// consumed before `SAVE_SHORTCUT` or a Save As press would also satisfy plain Save.
const SAVE_AS_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), egui::Key::S);

fn dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("orc workflow", &["orc"])
}

fn python_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Python script", &["py"])
}

/// Turns an arbitrary string (typically a file stem) into a valid Python identifier: any
/// character that isn't `[A-Za-z0-9_]` becomes `_`, and the result is prefixed with `_` if it
/// would otherwise start with a digit or be empty -- Python identifiers can't start with a
/// digit, and an empty function name isn't useful. Doesn't check against Python's reserved
/// keywords (e.g. a file literally named `class.orc`) -- a rare enough edge case not worth the
/// extra bookkeeping here.
fn sanitize_identifier(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// The name to hand to `Workflow::write_python_script`: the current file's own name, sanitized
/// into a valid Python identifier, or `None` (which defaults to `generated_workflow`) if the
/// workflow has never been saved, or its path is somehow not valid UTF-8.
fn export_python_name(state: &EditorState) -> Option<String> {
    let stem = state.current_path.as_ref()?.file_stem()?.to_str()?;
    Some(sanitize_identifier(stem))
}

fn display_name(state: &EditorState) -> String {
    state.current_path.as_ref().map_or_else(
        || "Untitled".to_string(),
        |p| p.file_name().unwrap_or(p.as_os_str()).display().to_string(),
    )
}

/// Given the user's answer to "save before continuing?" and whether a save was actually
/// attempted and succeeded, decide whether it's OK to discard the current workflow. Kept as a
/// pure function, independent of `rfd`, so this decision can be unit tested without driving a
/// real native dialog — `confirm_discard` below is otherwise untestable end to end.
fn resolve_discard(answer: rfd::MessageDialogResult, saved_successfully: bool) -> bool {
    match answer {
        // If Save (or Save As) failed or was cancelled, treat that the same as the user
        // cancelling the whole operation, rather than discarding.
        rfd::MessageDialogResult::Yes => saved_successfully,
        rfd::MessageDialogResult::No => true,
        _ => false,
    }
}

/// Ask (via a native dialog) whether it's OK to discard the current workflow, saving it first
/// if the user wants to. Every path that would otherwise throw away unsaved work — New, Open,
/// closing the app — must gate on this first.
pub(crate) fn confirm_discard(state: &mut EditorState) -> bool {
    if !state.dirty {
        return true;
    }
    let name = display_name(state);
    let answer = rfd::MessageDialog::new()
        .set_title("Unsaved Changes")
        .set_description(format!("Save changes to \"{name}\" before continuing?"))
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
    if answer == rfd::MessageDialogResult::Yes {
        save(state);
    }
    resolve_discard(answer, !state.dirty)
}

pub(crate) fn open_workflow(path: &Path) -> Result<Workflow, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    Workflow::read_from_msgpack(
        &mut reader,
        &crate::PLUGIN_SET,
        &crate::REGISTRY,
        0,
        &mut next_id,
    )
    .map_err(|e| e.to_string())
}

/// `write_to_msgpack` refuses to run while anything is flagged deleted, so every save first
/// collapses tombstoned nodes and links for good.
///
/// `garbage_collection()` compacts and renumbers every surviving output/input/link with no way
/// for the caller to remap a handle it's holding onto — so any `OH` captured in UI-side state
/// across a frame (an in-progress wire drag, or a context menu mid-"connect from output") would
/// be silently invalidated by it, either pointing at the wrong pin afterward or panicking. There
/// is no way to safely carry either across this call, so both are dropped first.
fn save_workflow(state: &mut EditorState, path: &Path) -> Result<(), String> {
    state.pending_wire = None;
    state.context_menu = None;
    state
        .workflow
        .garbage_collection()
        .map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    state
        .workflow
        .write_to_msgpack(
            &crate::PLUGIN_SET,
            &crate::SERIAL_CONTEXT_ARENA,
            &mut writer,
        )
        .map_err(|e| e.to_string())
}

fn save_as(state: &mut EditorState) {
    let Some(path) = dialog().save_file() else {
        return;
    };
    match save_workflow(state, &path) {
        Ok(()) => {
            state.current_path = Some(path);
            state.dirty = false;
        }
        Err(e) => state.last_error = Some(format!("Failed to save {}: {e}", path.display())),
    }
}

fn save(state: &mut EditorState) {
    let Some(path) = state.current_path.clone() else {
        return save_as(state);
    };
    match save_workflow(state, &path) {
        Ok(()) => state.dirty = false,
        Err(e) => state.last_error = Some(format!("Failed to save {}: {e}", path.display())),
    }
}

fn open(state: &mut EditorState) {
    if !confirm_discard(state) {
        return;
    }
    let Some(path) = dialog().pick_file() else {
        return;
    };
    match open_workflow(&path) {
        Ok(workflow) => {
            *state = EditorState::from_workflow(workflow);
            state.current_path = Some(path);
        }
        Err(e) => state.last_error = Some(format!("Failed to open {}: {e}", path.display())),
    }
}

fn new_workflow(state: &mut EditorState) {
    if !confirm_discard(state) {
        return;
    }
    *state = EditorState::from_workflow(Workflow::default());
}

/// Generates a Python script from `state`'s current workflow (named per `export_python_name`)
/// and writes it to `path`. Purely a side artifact -- unlike `save_workflow`, this never touches
/// `current_path` or `dirty`.
fn export_python_script(state: &EditorState, path: &Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    state
        .workflow
        .write_python_script(export_python_name(state).as_deref(), &mut writer)
        .map_err(|e| e.to_string())
}

fn export_python(state: &mut EditorState) {
    let Some(path) = python_dialog().save_file() else {
        return;
    };
    if let Err(e) = export_python_script(state, &path) {
        state.last_error = Some(format!("Failed to export {}: {e}", path.display()));
    }
}

/// Global keyboard shortcuts for the actions in the File menu. Independent of whether the menu
/// is open, matching how every other app on earth treats Ctrl+S etc.
pub fn handle_shortcuts(ctx: &egui::Context, state: &mut EditorState) {
    // Most specific first: see the comment on `SAVE_AS_SHORTCUT`.
    if ctx.input_mut(|i| i.consume_shortcut(&SAVE_AS_SHORTCUT)) {
        save_as(state);
    } else if ctx.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) {
        save(state);
    } else if ctx.input_mut(|i| i.consume_shortcut(&OPEN_SHORTCUT)) {
        open(state);
    } else if ctx.input_mut(|i| i.consume_shortcut(&NEW_SHORTCUT)) {
        new_workflow(state);
    }
}

/// Top menu bar: File > New / Open / Save / Save As / Export Python, each showing its shortcut
/// right-aligned via egui's own `Button::shortcut_text` (Export Python has none). Built entirely
/// from egui's `MenuBar`/`menu_button`.
pub fn menu_bar(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            let shortcut_text = |ui: &egui::Ui, s: &KeyboardShortcut| ui.ctx().format_shortcut(s);
            if ui
                .add(egui::Button::new("New").shortcut_text(shortcut_text(ui, &NEW_SHORTCUT)))
                .clicked()
            {
                new_workflow(state);
                ui.close();
            }
            if ui
                .add(egui::Button::new("Open...").shortcut_text(shortcut_text(ui, &OPEN_SHORTCUT)))
                .clicked()
            {
                open(state);
                ui.close();
            }
            if ui
                .add(egui::Button::new("Save").shortcut_text(shortcut_text(ui, &SAVE_SHORTCUT)))
                .clicked()
            {
                save(state);
                ui.close();
            }
            if ui
                .add(
                    egui::Button::new("Save As...")
                        .shortcut_text(shortcut_text(ui, &SAVE_AS_SHORTCUT)),
                )
                .clicked()
            {
                save_as(state);
                ui.close();
            }
            ui.separator();
            if ui.button("Export Python...").clicked() {
                export_python(state);
                ui.close();
            }
        });
    });
}

/// Reflect the current file and dirty state in the OS window title, e.g. `Dagger - foo.orc*`.
///
/// Called unconditionally every frame, so this must not issue `ViewportCommand::Title` unless the
/// title actually changed — winit's window-title setter is a real OS call (`SetWindowTextW` on
/// Windows), and sending it on every single frame of a drag is enough overhead on its own to make
/// dragging feel sluggish, independent of anything else this app does per frame.
pub fn update_window_title(ctx: &egui::Context, state: &mut EditorState) {
    let dirty_mark = if state.dirty { "*" } else { "" };
    let title = format!("Dagger - {}{dirty_mark}", display_name(state));
    if title != state.last_window_title {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        state.last_window_title = title;
    }
}

/// Popup reporting the last failed load/save, dismissed with its own close button or OK.
pub fn error_window(ctx: &egui::Context, state: &mut EditorState) {
    let Some(message) = state.last_error.clone() else {
        return;
    };
    let mut open = true;
    let mut dismissed = false;
    egui::Window::new("Error")
        .open(&mut open)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(&message);
            if ui.button("OK").clicked() {
                dismissed = true;
            }
        });
    if !open || dismissed {
        state.last_error = None;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn t_display_name_falls_back_to_untitled() {
        let state = EditorState::from_workflow(Workflow::default());
        assert_eq!(display_name(&state), "Untitled");
    }

    #[test]
    fn t_display_name_uses_the_file_name_not_the_full_path() {
        let mut state = EditorState::from_workflow(Workflow::default());
        state.current_path = Some(std::path::PathBuf::from("/some/dir/graph.orc"));
        assert_eq!(display_name(&state), "graph.orc");
    }

    #[test]
    fn t_sanitize_identifier_leaves_an_already_valid_name_alone() {
        assert_eq!(sanitize_identifier("my_workflow"), "my_workflow");
    }

    #[test]
    fn t_sanitize_identifier_replaces_non_identifier_characters() {
        assert_eq!(sanitize_identifier("my-workflow 2"), "my_workflow_2");
    }

    #[test]
    fn t_sanitize_identifier_prefixes_a_leading_digit() {
        assert_eq!(sanitize_identifier("2nd_workflow"), "_2nd_workflow");
    }

    #[test]
    fn t_sanitize_identifier_of_an_empty_string_is_not_empty() {
        assert_eq!(sanitize_identifier(""), "_");
    }

    #[test]
    fn t_export_python_name_is_none_for_an_unsaved_workflow() {
        let state = EditorState::from_workflow(Workflow::default());
        assert_eq!(export_python_name(&state), None);
    }

    #[test]
    fn t_export_python_name_sanitizes_the_saved_files_stem() {
        let mut state = EditorState::from_workflow(Workflow::default());
        state.current_path = Some(std::path::PathBuf::from("/some/dir/my-workflow.orc"));
        assert_eq!(export_python_name(&state), Some("my_workflow".to_string()));
    }

    #[test]
    fn t_export_python_script_uses_the_sanitized_saved_name() {
        let mut state = EditorState::from_workflow(Workflow::default());
        state.current_path = Some(std::path::PathBuf::from("/some/dir/my-workflow.orc"));

        let path = std::env::temp_dir().join(format!(
            "dagger_test_export_{}.py",
            crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        export_python_script(&state, &path).expect("export should succeed");
        let written = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(
            written.starts_with("def my_workflow():\n"),
            "got:\n{written}"
        );
    }

    #[test]
    fn t_export_python_script_defaults_the_name_for_an_unsaved_workflow() {
        let state = EditorState::from_workflow(Workflow::default());

        let path = std::env::temp_dir().join(format!(
            "dagger_test_export_{}.py",
            crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        export_python_script(&state, &path).expect("export should succeed");
        let written = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(
            written.starts_with("def generated_workflow():\n"),
            "got:\n{written}"
        );
    }

    #[test]
    fn t_a_clean_workflow_does_not_need_confirmation_to_discard() {
        let mut state = EditorState::from_workflow(Workflow::default());
        assert!(!state.dirty);
        assert!(
            confirm_discard(&mut state),
            "nothing to lose, nothing to ask"
        );
    }

    #[test]
    fn t_round_trips_a_workflow_through_save_and_open() {
        let mut workflow = Workflow::default();
        let mut inputs = vec![orc_sdk::IH::default()];
        workflow
            .add_inspect_node("inspect".to_string(), &mut inputs)
            .unwrap();
        let mut state = EditorState::from_workflow(workflow);

        let path = std::env::temp_dir().join(format!(
            "dagger_test_{}.orc",
            crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        save_workflow(&mut state, &path).expect("save should succeed");
        let reloaded = open_workflow(&path).expect("open should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(
            reloaded.node_iter().count(),
            state.workflow.node_iter().count()
        );
    }

    /// Regression test for a stale-handle bug found in review: `garbage_collection()` (which
    /// every save runs) compacts and renumbers surviving outputs with no remap table exposed to
    /// the caller, so any `OH` a UI gesture is still holding onto (an in-progress wire drag, or
    /// a context menu's "connect from output") must not survive it — otherwise finishing that
    /// gesture after the save could silently wire up the wrong pin, or panic.
    #[test]
    fn t_save_clears_pending_gestures_that_garbage_collection_would_invalidate() {
        let mut wf = Workflow::default();
        let mut ins: Vec<orc_sdk::IH> = vec![];
        let mut outs: Vec<orc_sdk::OH> = vec![orc_sdk::OH::default()];
        wf.add_function(orc_sdk::FuncInfo::default(), &mut ins, &mut outs)
            .unwrap();
        let oh = outs[0];
        let mut state = EditorState::from_workflow(wf);

        state.pending_wire = Some(oh);
        crate::context_menu::open(
            &mut state,
            crate::interaction::ContextMenuRequest::FromOutput(oh, egui::Pos2::ZERO),
        );
        assert!(state.context_menu.is_some());

        let path = std::env::temp_dir().join(format!(
            "dagger_test_gc_clear_{}.orc",
            crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        save_workflow(&mut state, &path).expect("save should succeed");
        std::fs::remove_file(&path).ok();

        assert!(
            state.pending_wire.is_none(),
            "a pending wire must not survive garbage_collection"
        );
        assert!(
            state.context_menu.is_none(),
            "an open context menu's connect_from must not survive garbage_collection"
        );
    }

    #[test]
    fn t_resolve_discard_yes_only_discards_if_the_save_actually_succeeded() {
        assert!(resolve_discard(rfd::MessageDialogResult::Yes, true));
        assert!(!resolve_discard(rfd::MessageDialogResult::Yes, false));
    }

    #[test]
    fn t_resolve_discard_no_always_discards() {
        assert!(resolve_discard(rfd::MessageDialogResult::No, false));
        assert!(resolve_discard(rfd::MessageDialogResult::No, true));
    }

    #[test]
    fn t_resolve_discard_cancel_never_discards() {
        assert!(!resolve_discard(rfd::MessageDialogResult::Cancel, true));
    }
}
