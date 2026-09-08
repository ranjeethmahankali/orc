//! File menu: Save, Save As, Open. Native OS dialogs via `rfd`, `.orc` files on disk.

use crate::state::EditorState;
use eframe::egui;
use orc_sdk::Workflow;
use std::path::Path;
use std::sync::atomic::Ordering;

fn dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("orc workflow", &["orc"])
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

/// Top menu bar: File > Open / Save / Save As. Built entirely from egui's own `MenuBar` and
/// `menu_button`, matching the example in the egui docs.
pub fn menu_bar(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open...").clicked() {
                open(state);
                ui.close();
            }
            if ui.button("Save").clicked() {
                save(state);
                ui.close();
            }
            if ui.button("Save As...").clicked() {
                save_as(state);
                ui.close();
            }
        });
    });
}

/// Reflect the current file and dirty state in the OS window title, e.g. `Dagger - foo.orc*`.
pub fn update_window_title(ctx: &egui::Context, state: &EditorState) {
    let name = state.current_path.as_ref().map_or_else(
        || "Untitled".to_string(),
        |p| p.file_name().unwrap_or(p.as_os_str()).display().to_string(),
    );
    let dirty_mark = if state.dirty { "*" } else { "" };
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
        "Dagger - {name}{dirty_mark}"
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
