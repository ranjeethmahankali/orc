use crate::canvas::Transform;
use crate::interaction::SelectBoxKind;
use crate::state::EditorState;
use eframe::egui::{
    self, Color32, FontFamily, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
};
use eframe::epaint::{CubicBezierShape, PathStroke};
use orc_sdk::{ArgInfo, IH, NodeInfo, NodePropBuf, Workflow};

/// Nodes are never narrower than this, however short their labels are.
pub const MIN_NODE_WIDTH: f32 = 120.0;
const TITLE_HEIGHT: f32 = 24.0;
const PIN_RADIUS: f32 = 5.0;
/// Half-size of a pin's hit rect, in canvas units. Wider than the drawn radius since
/// pins straddle the node edge and are otherwise fiddly to grab.
pub const PIN_GRAB_RADIUS: f32 = 10.0;
const PIN_STROKE_WIDTH: f32 = 1.5;
/// Stroke width for an unconnected input pin that a wire drag would land on if released now.
const PIN_HOVER_STROKE_WIDTH: f32 = 3.5;
const PIN_SPACING: f32 = 20.0;
const PIN_TOP_OFFSET: f32 = TITLE_HEIGHT + 12.0;
const NODE_ROUNDING: f32 = 6.0;
const FONT_SIZE: f32 = 13.0;
const LABEL_FONT_SIZE: f32 = 11.0;
/// Gap between the edge of a pin and the start of its label.
const LABEL_GAP: f32 = 4.0;
/// Horizontal padding around the title text.
const TITLE_PADDING: f32 = 8.0;
/// Minimum gap between the input and output label columns.
const LABEL_COLUMN_GAP: f32 = 12.0;
/// Below this zoom level text is too small to read, so it is not drawn at all.
const MIN_TEXT_ZOOM: f32 = 0.35;

/// A node caught in a cycle is drawn in these colours instead of its own. The graph is
/// allowed to hold cycles, so this flags the problem rather than preventing it.
const ERROR_BODY_COLOR: Color32 = Color32::from_rgb(120, 45, 45);
const ERROR_TITLE_COLOR: Color32 = Color32::from_rgb(165, 60, 60);
const ERROR_STROKE_COLOR: Color32 = Color32::from_rgb(235, 95, 95);
const NODE_STROKE_COLOR: Color32 = Color32::from_gray(40);
const SELECTION_COLOR: Color32 = Color32::from_rgb(230, 180, 60);
const SELECT_BOX_STROKE_COLOR: Color32 = Color32::from_rgb(120, 170, 230);
const SELECT_BOX_FILL_COLOR: Color32 = Color32::from_rgba_premultiplied(20, 35, 50, 30);

const LINK_COLOR: Color32 = Color32::from_rgb(180, 180, 180);
const LINK_WIDTH: f32 = 2.0;
const CONTROL_POINT_OFFSET: f32 = 80.0;

/// Indefinite-progress-bar style sweep drawn into an in-flight node's title bar. Purely
/// cosmetic; the node is genuinely running on a worker thread regardless of what this looks
/// like, since the app never blocks on execution.
const PULSE_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 65);
const PULSE_BAND_FRACTION: f32 = 0.35;
/// Full sweeps per second.
const PULSE_SPEED: f64 = 0.6;

/// Control points for a cubic bezier between `start` and `end`, both in the same space. The
/// caller maps them into screen space, since a link's endpoints are in canvas space but an
/// in-progress wire's cursor endpoint is already in screen space.
fn bezier_control_points(start: Pos2, end: Pos2, min_offset: f32) -> [Pos2; 4] {
    let dx = (end.x - start.x).abs().max(min_offset) * 0.5;
    [
        start,
        Pos2::new(start.x + dx, start.y),
        Pos2::new(end.x - dx, end.y),
        end,
    ]
}

fn link_shape(points: [Pos2; 4], stroke_width: f32) -> Shape {
    Shape::CubicBezier(CubicBezierShape::from_points_stroke(
        points,
        false,
        Color32::TRANSPARENT,
        PathStroke::new(stroke_width, LINK_COLOR),
    ))
}

fn node_color(info: &NodeInfo) -> Color32 {
    match info {
        NodeInfo::Constant(_) => Color32::from_rgb(80, 120, 80),
        NodeInfo::Function(_) => Color32::from_rgb(60, 90, 140),
        NodeInfo::NestedCall { .. } => Color32::from_rgb(120, 80, 120),
        NodeInfo::Inspect { .. } => Color32::from_rgb(140, 100, 50),
    }
}

fn title_color(info: &NodeInfo) -> Color32 {
    match info {
        NodeInfo::Constant(_) => Color32::from_rgb(100, 150, 100),
        NodeInfo::Function(_) => Color32::from_rgb(80, 115, 170),
        NodeInfo::NestedCall { .. } => Color32::from_rgb(150, 100, 150),
        NodeInfo::Inspect { .. } => Color32::from_rgb(170, 125, 65),
    }
}

pub fn node_height(n_inputs: usize, n_outputs: usize) -> f32 {
    let n_pins = n_inputs.max(n_outputs).max(1);
    PIN_TOP_OFFSET + n_pins as f32 * PIN_SPACING + 8.0
}

/// Default content area reserved below an Inspect node's pins for its text, until resizing (not
/// implemented yet) lets the user override it.
const INSPECT_CONTENT_WIDTH: f32 = 260.0;
const INSPECT_CONTENT_HEIGHT: f32 = 160.0;
const INSPECT_TEXT_FONT_SIZE: f32 = 11.0;
const INSPECT_TEXT_PADDING: f32 = 6.0;

pub fn node_rect(pos: [f32; 2], size: [f32; 2]) -> Rect {
    Rect::from_min_size(Pos2::new(pos[0], pos[1]), Vec2::new(size[0], size[1]))
}

pub fn input_pin_pos(rect: Rect, pin_index: usize) -> Pos2 {
    Pos2::new(
        rect.min.x,
        rect.min.y + PIN_TOP_OFFSET + pin_index as f32 * PIN_SPACING,
    )
}

pub fn output_pin_pos(rect: Rect, pin_index: usize) -> Pos2 {
    Pos2::new(
        rect.max.x,
        rect.min.y + PIN_TOP_OFFSET + pin_index as f32 * PIN_SPACING,
    )
}

/// The label to show beside a pin. An explicit label carried by the workflow wins; otherwise
/// we fall back to the argument name the plugin declared through the ABI.
///
/// A plugin may leave its argument arrays null, in which case there is no declared name and
/// the pin stays bare. `example_c_plugin` does exactly that.
fn pin_label<'a>(explicit: &'a str, declared: Option<&'a str>) -> &'a str {
    if explicit.is_empty() {
        declared.unwrap_or("")
    } else {
        explicit
    }
}

/// Declared argument names for a node, or `None` for node kinds with no plugin function
/// behind them.
fn declared_args(info: &NodeInfo) -> (Option<&[ArgInfo]>, Option<&[ArgInfo]>) {
    match info {
        NodeInfo::Function(func) => (
            Some(func.input_args.as_slice()),
            Some(func.output_args.as_slice()),
        ),
        _ => (None, None),
    }
}

fn declared_name(args: Option<&[ArgInfo]>, index: usize) -> Option<&str> {
    args?.get(index).map(|arg| arg.name.as_str())
}

fn text_width(ctx: &egui::Context, text: &str, size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                text.to_owned(),
                FontId::new(size, FontFamily::Monospace),
                Color32::WHITE,
            )
            .rect
            .width()
    })
}

/// Recompute every node's canvas-space size so that its title and both pin label columns fit.
///
/// This must run inside a frame, because it measures text through the font system — which is
/// why the layout can't be computed until the first frame, not at construction.
pub fn measure_nodes(ctx: &egui::Context, workflow: &Workflow, sizes: &mut NodePropBuf<[f32; 2]>) {
    let node_info_prop = workflow.node_info_prop();
    let input_labels_prop = workflow.input_labels_prop();
    let output_labels_prop = workflow.output_labels_prop();
    let (node_infos, input_labels, output_labels) = match (
        node_info_prop.try_borrow(),
        input_labels_prop.try_borrow(),
        output_labels_prop.try_borrow(),
    ) {
        (Ok(n), Ok(i), Ok(o)) => (n, i, o),
        _ => return,
    };

    for nh in workflow.node_iter() {
        let info = &node_infos[nh];
        let (in_args, out_args) = declared_args(info);

        let mut n_inputs = 0usize;
        let mut inputs_width = 0.0f32;
        for (i, ih) in workflow.node_inputs(nh).enumerate() {
            let label = pin_label(&input_labels[ih], declared_name(in_args, i));
            inputs_width = inputs_width.max(text_width(ctx, label, LABEL_FONT_SIZE));
            n_inputs += 1;
        }

        let mut n_outputs = 0usize;
        let mut outputs_width = 0.0f32;
        for (i, oh) in workflow.node_outputs(nh).enumerate() {
            let label = pin_label(&output_labels[oh], declared_name(out_args, i));
            outputs_width = outputs_width.max(text_width(ctx, label, LABEL_FONT_SIZE));
            n_outputs += 1;
        }

        // Each label column is inset from its pin, and the pin sits on the node edge.
        let label_inset = 2.0 * (PIN_RADIUS + LABEL_GAP);
        let mut labels_width = inputs_width + outputs_width + label_inset;
        if inputs_width > 0.0 && outputs_width > 0.0 {
            labels_width += LABEL_COLUMN_GAP;
        }
        let title_width = text_width(ctx, info.name(), FONT_SIZE) + 2.0 * TITLE_PADDING;
        let width = labels_width.max(title_width).max(MIN_NODE_WIDTH);
        let height = node_height(n_inputs, n_outputs);

        sizes[nh] = if matches!(info, NodeInfo::Inspect { .. }) {
            [
                width.max(INSPECT_CONTENT_WIDTH),
                height + INSPECT_CONTENT_HEIGHT,
            ]
        } else {
            [width, height]
        };
    }
}

pub fn draw(ui: &mut egui::Ui, state: &EditorState) {
    let view = state.view;
    let wire_target = pending_wire_target(ui, state, &view);
    let time = ui.input(|i| i.time);
    draw_links(ui, state, &view);
    draw_nodes(ui, state, &view, wire_target, time);
    draw_pending_wire(ui, state, &view);
    draw_select_box(ui, state);
}

/// Sweeps a soft highlight band left to right across `title_rect`, looping forever. The band
/// travels from fully off the left edge to fully off the right edge so it fades in/out at the
/// boundary instead of popping.
fn draw_in_flight_pulse(painter: &egui::Painter, title_rect: Rect, time: f64) {
    let phase = (time * PULSE_SPEED).rem_euclid(1.0) as f32;
    let band_width = title_rect.width() * PULSE_BAND_FRACTION;
    let travel = title_rect.width() + band_width;
    let center_x = title_rect.min.x - band_width / 2.0 + phase * travel;
    let band = Rect::from_center_size(
        Pos2::new(center_x, title_rect.center().y),
        Vec2::new(band_width, title_rect.height()),
    );
    let clipped = band.intersect(title_rect);
    if clipped.width() > 0.0 && clipped.height() > 0.0 {
        painter.rect_filled(clipped, 0.0, PULSE_COLOR);
    }
}

/// The input pin (if any) that releasing a wire drag right now would connect to, so it can be
/// highlighted as feedback before the drop actually happens. `None` when no wire is being
/// dragged, recomputed fresh every frame like everything else here.
fn pending_wire_target(ui: &egui::Ui, state: &EditorState, view: &Transform) -> Option<IH> {
    state.pending_wire?;
    let pos = ui.input(|i| i.pointer.interact_pos())?;
    crate::interaction::find_input_pin_at(state, pos, view)
}

/// The in-progress bezier while a wire is being dragged from an output pin (or an
/// already-connected input pin, which is disconnected as soon as the drag starts). The cursor
/// end is already in screen space, unlike a settled link's two canvas-space endpoints.
fn draw_pending_wire(ui: &mut egui::Ui, state: &EditorState, view: &Transform) {
    let Some(source) = state.pending_wire else {
        return;
    };
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let (positions, sizes) = match (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
    ) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return,
    };
    let src_nh = state.workflow.node_from_output(source);
    let src_rect = node_rect(positions[src_nh], sizes[src_nh]);
    let output_idx = state
        .workflow
        .node_outputs(src_nh)
        .position(|o| o == source)
        .unwrap_or(0);
    let start = view.canvas_to_screen(output_pin_pos(src_rect, output_idx));

    let points = bezier_control_points(start, cursor, CONTROL_POINT_OFFSET * view.zoom);
    ui.painter().add(link_shape(points, view.scale(LINK_WIDTH)));
}

/// The marquee rectangle while box-selecting on empty canvas. Stored in screen space already,
/// so it needs no further transform at paint time. A window-select (dragged left-to-right) gets
/// a solid outline; a crossing-select (dragged right-to-left) gets a dashed one, so the two
/// selection rules are visually distinguishable while dragging.
fn draw_select_box(ui: &mut egui::Ui, state: &EditorState) {
    let Some((rect, kind)) = state.select_box else {
        return;
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, SELECT_BOX_FILL_COLOR);
    let stroke = Stroke::new(1.0, SELECT_BOX_STROKE_COLOR);
    match kind {
        SelectBoxKind::Window => {
            painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
        }
        SelectBoxKind::Crossing => {
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
                rect.left_top(),
            ];
            painter.extend(Shape::dashed_line(&corners, stroke, 6.0, 4.0));
        }
    }
}

fn draw_links(ui: &mut egui::Ui, state: &EditorState, view: &Transform) {
    let painter = ui.painter();
    let (positions, sizes) = match (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
    ) {
        (Ok(p), Ok(s)) => (p, s),
        _ => return,
    };

    for nh in state.workflow.node_iter() {
        let dst_rect = node_rect(positions[nh], sizes[nh]);

        for (input_idx, ih) in state.workflow.node_inputs(nh).enumerate() {
            let src_oh = match state.workflow.input_source(ih) {
                Some(oh) => oh,
                None => continue,
            };

            let src_nh = state.workflow.node_from_output(src_oh);
            let src_rect = node_rect(positions[src_nh], sizes[src_nh]);

            let output_idx = state
                .workflow
                .node_outputs(src_nh)
                .position(|o| o == src_oh)
                .unwrap_or(0);

            // The curve is laid out in canvas space and then mapped to the screen.
            // The transform is affine, so mapping the four control points is exact.
            let start = output_pin_pos(src_rect, output_idx);
            let end = input_pin_pos(dst_rect, input_idx);

            let points = bezier_control_points(start, end, CONTROL_POINT_OFFSET)
                .map(|p| view.canvas_to_screen(p));
            painter.add(link_shape(points, view.scale(LINK_WIDTH)));
        }
    }
}

fn draw_nodes(
    ui: &mut egui::Ui,
    state: &EditorState,
    view: &Transform,
    wire_target: Option<IH>,
    time: f64,
) {
    let painter = ui.painter();
    let font = FontId::new(view.scale(FONT_SIZE), FontFamily::Monospace);
    let label_font = FontId::new(view.scale(LABEL_FONT_SIZE), FontFamily::Monospace);
    let draw_text = view.zoom >= MIN_TEXT_ZOOM;

    let node_info_prop = state.workflow.node_info_prop();
    let input_labels_prop = state.workflow.input_labels_prop();
    let output_labels_prop = state.workflow.output_labels_prop();

    let (node_infos, input_labels, output_labels) = match (
        node_info_prop.try_borrow(),
        input_labels_prop.try_borrow(),
        output_labels_prop.try_borrow(),
    ) {
        (Ok(n), Ok(i), Ok(o)) => (n, i, o),
        _ => return,
    };
    let (positions, sizes, in_cycle, selected) = match (
        state.node_positions.try_borrow(),
        state.node_sizes.try_borrow(),
        state.node_in_cycle.try_borrow(),
        state.selected.try_borrow(),
    ) {
        (Ok(p), Ok(s), Ok(c), Ok(sel)) => (p, s, c, sel),
        _ => return,
    };
    let inspect_cache = state.inspect_cache.try_borrow().ok();
    let inspect_font = FontId::new(view.scale(INSPECT_TEXT_FONT_SIZE), FontFamily::Monospace);

    for nh in state.workflow.node_iter() {
        let info = &node_infos[nh];
        let (in_args, out_args) = declared_args(info);
        let rect = node_rect(positions[nh], sizes[nh]);
        let (body_fill, title_fill, outline, outline_width) = if in_cycle[nh] {
            (ERROR_BODY_COLOR, ERROR_TITLE_COLOR, ERROR_STROKE_COLOR, 1.0)
        } else if selected[nh] {
            (node_color(info), title_color(info), SELECTION_COLOR, 2.0)
        } else {
            (node_color(info), title_color(info), NODE_STROKE_COLOR, 1.0)
        };

        let rounding = view.scale(NODE_ROUNDING);
        let body_rect = view.rect_to_screen(rect);
        let title_rect = view.rect_to_screen(Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width(), TITLE_HEIGHT),
        ));

        // Node body.
        painter.rect_filled(body_rect, rounding, body_fill);
        painter.rect_stroke(
            body_rect,
            rounding,
            Stroke::new(view.scale(outline_width), outline),
            StrokeKind::Outside,
        );

        // Title bar.
        painter.rect_filled(title_rect, rounding, title_fill);
        if rect.height() > TITLE_HEIGHT {
            let patch = view.rect_to_screen(Rect::from_min_size(
                Pos2::new(rect.min.x, rect.min.y + TITLE_HEIGHT - NODE_ROUNDING),
                Vec2::new(rect.width(), NODE_ROUNDING),
            ));
            painter.rect_filled(patch, 0.0, title_fill);
        }
        if crate::exec::is_node_in_flight(state, nh) {
            draw_in_flight_pulse(painter, title_rect, time);
        }

        // Title text.
        if draw_text {
            painter.text(
                view.canvas_to_screen(Pos2::new(rect.min.x + TITLE_PADDING, rect.min.y + 4.0)),
                egui::Align2::LEFT_TOP,
                info.name(),
                font.clone(),
                Color32::WHITE,
            );
        }

        let pin_radius = view.scale(PIN_RADIUS);
        let label_offset = pin_radius + view.scale(LABEL_GAP);

        // Input pins.
        for (i, ih) in state.workflow.node_inputs(nh).enumerate() {
            let pin_center = view.canvas_to_screen(input_pin_pos(rect, i));
            let connected = state.workflow.input_source(ih).is_some();
            if connected {
                painter.circle_filled(pin_center, pin_radius, Color32::from_rgb(200, 200, 200));
            } else {
                let stroke_width = if wire_target == Some(ih) {
                    PIN_HOVER_STROKE_WIDTH
                } else {
                    PIN_STROKE_WIDTH
                };
                painter.circle_stroke(
                    pin_center,
                    pin_radius,
                    Stroke::new(view.scale(stroke_width), Color32::from_rgb(160, 160, 160)),
                );
            }
            let label = pin_label(&input_labels[ih], declared_name(in_args, i));
            if draw_text && !label.is_empty() {
                painter.text(
                    pin_center + Vec2::new(label_offset, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    label_font.clone(),
                    Color32::from_gray(200),
                );
            }
        }

        // Output pins.
        for (i, oh) in state.workflow.node_outputs(nh).enumerate() {
            let pin_center = view.canvas_to_screen(output_pin_pos(rect, i));
            painter.circle_filled(pin_center, pin_radius, Color32::from_rgb(200, 200, 200));
            let label = pin_label(&output_labels[oh], declared_name(out_args, i));
            if draw_text && !label.is_empty() {
                painter.text(
                    pin_center - Vec2::new(label_offset, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    label,
                    label_font.clone(),
                    Color32::from_gray(200),
                );
            }
        }

        if draw_text
            && matches!(info, NodeInfo::Inspect { .. })
            && let Some(cache) = &inspect_cache
        {
            let n_pins = state.workflow.node_inputs(nh).count().max(1);
            let content_top = rect.min.y + PIN_TOP_OFFSET + n_pins as f32 * PIN_SPACING;
            let content_rect = view.rect_to_screen(Rect::from_min_max(
                Pos2::new(rect.min.x, content_top),
                rect.max,
            ));
            let clipped = painter.with_clip_rect(content_rect);
            clipped.text(
                content_rect.min + Vec2::splat(view.scale(INSPECT_TEXT_PADDING)),
                egui::Align2::LEFT_TOP,
                &cache[nh].text,
                inspect_font.clone(),
                Color32::from_gray(220),
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Node width has its own floor (`MIN_NODE_WIDTH`, driven by measured text), but height is
    /// purely a function of pin count — this is the contract `input_pin_pos`/`output_pin_pos`
    /// and every hit-test in `interaction.rs` built on top of `node_rect` rely on staying in
    /// sync with.
    #[test]
    fn t_node_height_grows_with_pin_count() {
        assert!(node_height(1, 1) < node_height(5, 1));
        assert!(node_height(1, 1) < node_height(1, 5));
        assert!(node_height(0, 0) > TITLE_HEIGHT, "even an empty node reserves room below the title");
    }

    /// Regression guard for the exact desync the review flagged as possible: if `node_height`'s
    /// formula and `input_pin_pos`'s per-pin offset ever drift apart, the last pin on a many-pin
    /// node would sit outside the node's own body with no test catching it.
    #[test]
    fn t_node_height_reserves_room_for_the_last_pin() {
        for n in [1usize, 4, 10] {
            let rect = node_rect([0.0, 0.0], [200.0, node_height(n, n)]);
            let last_input_y = input_pin_pos(rect, n - 1).y;
            let last_output_y = output_pin_pos(rect, n - 1).y;
            assert!(
                last_input_y < rect.max.y,
                "last input pin (n={n}) must fit inside the node body: {last_input_y} vs bottom {}",
                rect.max.y
            );
            assert!(
                last_output_y < rect.max.y,
                "last output pin (n={n}) must fit inside the node body: {last_output_y} vs bottom {}",
                rect.max.y
            );
        }
    }

    #[test]
    fn t_input_and_output_pins_sit_on_opposite_edges_at_the_same_height() {
        let rect = node_rect([10.0, 20.0], [200.0, 100.0]);
        let input = input_pin_pos(rect, 2);
        let output = output_pin_pos(rect, 2);
        assert_eq!(input.x, rect.min.x, "input pins sit on the left edge");
        assert_eq!(output.x, rect.max.x, "output pins sit on the right edge");
        assert_eq!(
            input.y, output.y,
            "the same pin index should line up horizontally across both columns"
        );
    }

    #[test]
    fn t_node_rect_uses_position_as_the_min_corner() {
        let rect = node_rect([5.0, 7.0], [40.0, 30.0]);
        assert_eq!(rect.min, Pos2::new(5.0, 7.0));
        assert_eq!(rect.max, Pos2::new(45.0, 37.0));
    }

    /// A reversed condition here (returning `declared` whenever it exists, regardless of
    /// `explicit`) would silently discard every user-set pin label; nothing else in the crate
    /// would catch it since labels are just cosmetic text.
    #[test]
    fn t_pin_label_prefers_explicit_over_declared() {
        assert_eq!(pin_label("custom", Some("declared")), "custom");
        assert_eq!(pin_label("", Some("declared")), "declared");
        assert_eq!(pin_label("", None), "");
    }

    #[test]
    fn t_bezier_control_points_respect_the_minimum_handle_offset() {
        // Start and end are much closer together than `min_offset`, so the handles must still
        // bow out by min_offset/2 rather than collapsing to a near-straight, near-zero-length
        // curve.
        let start = Pos2::new(0.0, 0.0);
        let end = Pos2::new(1.0, 0.0);
        let points = bezier_control_points(start, end, 80.0);
        assert_eq!(points[0], start);
        assert_eq!(points[3], end);
        assert!(points[1].x - start.x >= 40.0 - 1e-4);
        assert!(end.x - points[2].x >= 40.0 - 1e-4);
    }
}
