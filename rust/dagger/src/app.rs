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
        cc: &eframe::CreationContext<'_>,
        workflow: Workflow,
        path: Option<PathBuf>,
    ) -> Self {
        // By default egui snaps text to the nearest physical pixel so static text stays crisp.
        // Node labels are essentially never static here — they're carried along by node
        // dragging, the settling force layout and canvas pan/zoom — so that snap becomes visible
        // as the label hopping between pixels a frame at a time instead of sliding smoothly,
        // most noticeably right before the layout settles, when everything else has slowed to
        // sub-pixel motion and the snap is the only thing left to see. Shapes don't have this
        // problem: they're antialiased by feathering the edge, which blends smoothly across a
        // sub-pixel offset instead of rounding it away.
        cc.egui_ctx
            .tessellation_options_mut(|options| options.round_text_to_pixels = false);
        let mut state = EditorState::from_workflow(workflow);
        state.current_path = path;
        Self { state }
    }
}

impl eframe::App for DaggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe's default close behavior exits immediately once the OS close event arrives, so
        // unlike File > New/Open (which already gate on confirm_discard), closing the window
        // directly would otherwise silently discard unsaved work with no prompt at all.
        let close_requested = ui.ctx().input(|i| {
            i.viewport()
                .events
                .iter()
                .any(|e| matches!(e, egui::ViewportEvent::Close))
        });
        if close_requested && !file_menu::confirm_discard(&mut self.state) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        file_menu::handle_shortcuts(ui.ctx(), &mut self.state);
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
