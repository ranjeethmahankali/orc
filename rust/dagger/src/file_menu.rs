//! File menu: New, Open, Save, Save As. Native OS dialogs via `rfd`, `.orc` files on disk.

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

fn display_name(state: &EditorState) -> String {
    state.current_path.as_ref().map_or_else(
        || "Untitled".to_string(),
        |p| p.file_name().unwrap_or(p.as_os_str()).display().to_string(),
    )
}

/// Ask (via a native dialog) whether it's OK to discard the current workflow, saving it first
/// if the user wants to. Every path that would otherwise throw away unsaved work — New, Open —
/// must gate on this first.
fn confirm_discard(state: &mut EditorState) -> bool {
    if !state.dirty {
        return true;
    }
    let name = display_name(state);
    match rfd::MessageDialog::new()
        .set_title("Unsaved Changes")
        .set_description(format!("Save changes to \"{name}\" before continuing?"))
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show()
    {
        rfd::MessageDialogResult::Yes => {
            save(state);
            // If Save (or Save As) failed or was cancelled, `dirty` is still true — treat that
            // the same as the user cancelling the whole operation, rather than discarding.
            !state.dirty
        }
        rfd::MessageDialogResult::No => true,
        _ => false,
    }
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
fn save_workflow(workflow: &mut Workflow, path: &Path) -> Result<(), String> {
    workflow.garbage_collection().map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    workflow
        .write_to_msgpack(&crate::PLUGIN_SET, &crate::SERIAL_CONTEXT_ARENA, &mut writer)
        .map_err(|e| e.to_string())
}

fn save_as(state: &mut EditorState) {
    let Some(path) = dialog().save_file() else {
        return;
    };
    match save_workflow(&mut state.workflow, &path) {
        Ok(()) => {
            state.current_path = Some(path);
            state.dirty = false;
        }
        Err(e) => state.file_error = Some(format!("Failed to save {}: {e}", path.display())),
    }
}

fn save(state: &mut EditorState) {
    let Some(path) = state.current_path.clone() else {
        return save_as(state);
    };
    match save_workflow(&mut state.workflow, &path) {
        Ok(()) => state.dirty = false,
        Err(e) => state.file_error = Some(format!("Failed to save {}: {e}", path.display())),
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
        Err(e) => state.file_error = Some(format!("Failed to open {}: {e}", path.display())),
    }
}

fn new_workflow(state: &mut EditorState) {
    if !confirm_discard(state) {
        return;
    }
    *state = EditorState::from_workflow(Workflow::default());
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

/// Top menu bar: File > New / Open / Save / Save As, each showing its shortcut right-aligned via
/// egui's own `Button::shortcut_text`. Built entirely from egui's `MenuBar`/`menu_button`.
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
        });
    });
}

/// Reflect the current file and dirty state in the OS window title, e.g. `Dagger - foo.orc*`.
pub fn update_window_title(ctx: &egui::Context, state: &EditorState) {
    let dirty_mark = if state.dirty { "*" } else { "" };
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
        "Dagger - {}{dirty_mark}",
        display_name(state)
    )));
}

/// Popup reporting the last failed load/save, dismissed with its own close button or OK.
pub fn error_window(ctx: &egui::Context, state: &mut EditorState) {
    let Some(message) = state.file_error.clone() else {
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
        state.file_error = None;
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
    fn t_a_clean_workflow_does_not_need_confirmation_to_discard() {
        let mut state = EditorState::from_workflow(Workflow::default());
        assert!(!state.dirty);
        assert!(confirm_discard(&mut state), "nothing to lose, nothing to ask");
    }

    #[test]
    fn t_round_trips_a_workflow_through_save_and_open() {
        let mut workflow = Workflow::default();
        let mut inputs = vec![orc_sdk::IH::default()];
        workflow
            .add_inspect_node("inspect".to_string(), &mut inputs)
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "dagger_test_{}.orc",
            crate::HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        save_workflow(&mut workflow, &path).expect("save should succeed");
        let reloaded = open_workflow(&path).expect("open should succeed");
        std::fs::remove_file(&path).ok();

        assert_eq!(reloaded.node_iter().count(), workflow.node_iter().count());
    }
}
