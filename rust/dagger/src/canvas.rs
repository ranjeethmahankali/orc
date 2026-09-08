//! Pan/zoom canvas: the affine transform between canvas space and screen space,
//! and the input handling that drives it.

use eframe::egui::{self, Pos2, Rect, Vec2};

/// Zoom limits.
pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 5.0;
/// Scroll wheel points to e-folds of zoom.
const ZOOM_SENSITIVITY: f32 = 0.002;

/// Affine map from canvas space to screen space: `screen = canvas * zoom + pan`.
///
/// Node positions, sizes and pin offsets are all in canvas space. They are mapped
/// through this transform at draw time, and cursor positions are mapped back
/// through it for hit testing.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Transform {
    pub fn to_screen(&self, p: Pos2) -> Pos2 {
        (p.to_vec2() * self.zoom + self.pan).to_pos2()
    }

    pub fn to_canvas(&self, p: Pos2) -> Pos2 {
        ((p.to_vec2() - self.pan) / self.zoom).to_pos2()
    }

    pub fn rect_to_screen(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(r.min), self.to_screen(r.max))
    }

    /// Canvas length to screen length.
    pub fn scale(&self, length: f32) -> f32 {
        length * self.zoom
    }

    /// Multiply the zoom by `factor`, keeping the canvas point currently under
    /// `anchor` (screen space) pinned to `anchor`.
    pub fn zoom_about(&mut self, anchor: Pos2, factor: f32) {
        let pivot = self.to_canvas(anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = anchor.to_vec2() - pivot.to_vec2() * self.zoom;
    }
}

/// Claim the whole available area as the canvas and apply pan/zoom input to `view`.
///
/// Middle-drag pans, the scroll wheel zooms about the cursor, and ctrl+scroll or a
/// trackpad pinch zooms too. Returns the canvas response (later phases hit test
/// against it) and whether the user moved the view this frame.
pub fn interact(ui: &mut egui::Ui, view: &mut Transform) -> (egui::Response, bool) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    let mut moved = false;

    if response.dragged_by(egui::PointerButton::Middle) {
        let delta = response.drag_delta();
        if delta != Vec2::ZERO {
            view.pan += delta;
            moved = true;
        }
    }

    if response.contains_pointer() {
        let (scroll, pinch, cursor) = ui.input(|i| {
            (
                i.smooth_scroll_delta.y,
                i.zoom_delta(),
                i.pointer.hover_pos(),
            )
        });
        let factor = (scroll * ZOOM_SENSITIVITY).exp() * pinch;
        if factor != 1.0 {
            view.zoom_about(cursor.unwrap_or_else(|| rect.center()), factor);
            moved = true;
        }
    }

    (response, moved)
}

#[cfg(test)]
mod test {
    use super::*;

    fn assert_close(a: Pos2, b: Pos2) {
        assert!(
            (a - b).length() < 1e-3,
            "expected {a:?} to be close to {b:?}"
        );
    }

    #[test]
    fn canvas_screen_round_trip() {
        let view = Transform {
            pan: Vec2::new(37.0, -12.0),
            zoom: 2.5,
        };
        let p = Pos2::new(123.0, 456.0);
        assert_close(view.to_canvas(view.to_screen(p)), p);
        assert_close(view.to_screen(view.to_canvas(p)), p);
    }

    #[test]
    fn zoom_pins_the_point_under_the_anchor() {
        let mut view = Transform::default();
        let anchor = Pos2::new(640.0, 400.0);
        let pivot = view.to_canvas(anchor);
        for factor in [1.1, 1.1, 0.8, 3.0, 0.5] {
            view.zoom_about(anchor, factor);
            assert_close(view.to_screen(pivot), anchor);
        }
    }

    #[test]
    fn zoom_is_clamped() {
        let anchor = Pos2::new(100.0, 100.0);

        let mut view = Transform::default();
        for _ in 0..100 {
            view.zoom_about(anchor, 2.0);
        }
        assert_eq!(view.zoom, MAX_ZOOM);
        // The anchor stays pinned even when the zoom saturates.
        assert_close(view.to_screen(view.to_canvas(anchor)), anchor);

        let mut view = Transform::default();
        for _ in 0..100 {
            view.zoom_about(anchor, 0.5);
        }
        assert_eq!(view.zoom, MIN_ZOOM);
    }

    #[test]
    fn pan_translates_without_scaling() {
        let mut view = Transform {
            pan: Vec2::ZERO,
            zoom: 0.5,
        };
        let before = view.to_screen(Pos2::new(10.0, 20.0));
        view.pan += Vec2::new(15.0, -5.0);
        let after = view.to_screen(Pos2::new(10.0, 20.0));
        assert_close(after, before + Vec2::new(15.0, -5.0));
        assert_eq!(view.zoom, 0.5);
    }
}
