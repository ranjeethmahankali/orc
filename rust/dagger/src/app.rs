use crate::canvas;
use crate::context_menu;
use crate::file_menu;
use crate::interaction;
use crate::layout;
use crate::render;
use crate::state::EditorState;
use eframe::egui;
use orc_sdk::Workflow;
use std::path::PathBuf;

pub struct DaggerApp {
    state: EditorState,
}

impl DaggerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        workflow: Workflow,
        path: Option<PathBuf>,
    ) -> Self {
        let mut state = EditorState::from_workflow(workflow);
        state.current_path = path;
        Self { state }
    }
}

impl eframe::App for DaggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("dagger-menu-bar").show(ui, |ui| {
            file_menu::menu_bar(ui, &mut self.state);
        });
        file_menu::error_window(ui.ctx(), &mut self.state);
        file_menu::update_window_title(ui.ctx(), &self.state);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if self.state.needs_measure {
                    self.state.measure(ui.ctx());
                }
                let (canvas_response, view_moved) = canvas::interact(ui, &mut self.state.view);
                let deleted = interaction::delete_selected(ui, &mut self.state);

                // Runs before the layout step, using hit rects from the end of the previous
                // frame, so a drag this frame can tell the layout step which node to leave
                // alone.
                let events = interaction::update(ui, &mut self.state, &canvas_response);
                if events.changed || deleted {
                    ui.ctx().request_repaint();
                }
                if let Some(request) = events.context_menu {
                    context_menu::open(&mut self.state, request);
                }

                if !self.state.layout_converged {
                    for _ in 0..2 {
                        if layout::step(&mut self.state, events.dragged_node) {
                            self.state.layout_converged = true;
                            break;
                        }
                    }
                    ui.ctx().request_repaint();
                } else if view_moved {
                    // Scroll deltas are smoothed over several frames, so keep painting
                    // until the zoom has caught up with the wheel.
                    ui.ctx().request_repaint();
                }

                render::draw(ui, &self.state);

                context_menu::update(ui, &mut self.state);
                if self.state.session_info_open {
                    let mut open = self.state.session_info_open;
                    context_menu::session_info_window(ui.ctx(), &mut open);
                    self.state.session_info_open = open;
                }
            });
    }
}
