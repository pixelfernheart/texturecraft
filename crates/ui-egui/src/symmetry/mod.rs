use egui::{Rect, Ui};
use texturecraft_engine::geom::glam::Vec3;
use texturecraft_render::OverlayQuad;

use crate::TextureCraftApp;

const COLOR: [f32; 3] = [0.95, 0.48, 0.16];
const PLANE_OPACITY: f32 = 0.12;
const AXIS_OPACITY: f32 = 0.85;

fn shown(app: &TextureCraftApp) -> bool {
    app.ui.mode == crate::state::Mode::Painting
        && matches!(
            app.session.tool.tool,
            texturecraft_engine::Tool::Paint
                | texturecraft_engine::Tool::Eraser
                | texturecraft_engine::Tool::Projection
                | texturecraft_engine::Tool::Clone
                | texturecraft_engine::Tool::Path
        )
        && app.session.tool.brush.symmetry.any()
}

pub(crate) fn scene_overlay(app: &TextureCraftApp, _ui: &Ui, _rect: Rect) -> Vec<OverlayQuad> {
    if !shown(app) {
        return Vec::new();
    }

    let symmetry = app.session.tool.brush.symmetry;
    let (mesh_lo, mesh_hi) = app.session.mesh.bounds();
    let mesh_center = (mesh_lo + mesh_hi) * 0.5;
    let center = symmetry.origin(mesh_center);
    let margin = (mesh_hi - mesh_lo) * 0.05;
    let (lo, hi) = (mesh_lo - mesh_center - margin, mesh_hi - mesh_center + margin);
    let mut quads: Vec<_> = [symmetry.x, symmetry.y, symmetry.z]
        .into_iter()
        .enumerate()
        .filter_map(|(axis, enabled)| {
            if !enabled {
                return None;
            }

            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let corner = |at_u: f32, at_v: f32| {
                let mut point = center;
                point[u] += at_u;
                point[v] += at_v;
                point
            };
            Some(OverlayQuad {
                corners: [corner(lo[u], lo[v]), corner(hi[u], lo[v]), corner(hi[u], hi[v]), corner(lo[u], hi[v])],
                color: COLOR,
                opacity: PLANE_OPACITY,
            })
        })
        .collect();

    if symmetry.radial_mode() {
        let axis = symmetry.radial_axis();
        let camera_right = app.session.camera.view().inverse().transform_vector3(Vec3::X).normalize_or_zero();
        let diagonal = (mesh_hi - mesh_lo).length();
        let side = axis.cross(camera_right).try_normalize().unwrap_or_else(|| axis.any_orthonormal_vector()) * diagonal * 0.001;
        let reach = diagonal * 0.5;
        quads.push(OverlayQuad {
            corners: [center - axis * reach - side, center + axis * reach - side, center + axis * reach + side, center - axis * reach + side],
            color: COLOR,
            opacity: AXIS_OPACITY,
        });
    }

    quads
}

fn visible(app: &TextureCraftApp, rect: Rect, point: Vec3) -> bool {
    let aspect = rect.width() / rect.height().max(1.0);
    let Some(ndc) = app.session.camera.project(point, aspect) else {
        return false;
    };
    let ray = app.session.camera.ray(ndc.truncate(), aspect);
    let distance = (point - ray.origin).dot(ray.dir);
    let tolerance = (app.session.mesh.bounds().1 - app.session.mesh.bounds().0).length() * 2e-3;
    app.session.bvh.closest(ray.origin, ray.dir, f32::MAX).is_some_and(|hit| (hit.t - distance).abs() <= tolerance)
}

pub(crate) fn overlay(app: &TextureCraftApp, ui: &Ui, rect: Rect, cursor: Option<(Vec3, Vec3)>) {
    if !shown(app) {
        return;
    }

    if let Some((position, normal)) = cursor {
        let (lo, hi) = app.session.mesh.bounds();
        let symmetry = app.session.tool.brush.symmetry;
        let origin = symmetry.origin((lo + hi) * 0.5);
        for transform in symmetry.transforms().iter().skip(1) {
            let copy = origin + *transform * (position - origin);
            if visible(app, rect, copy) {
                crate::viewport::brush_cursor(app, ui, rect, copy, (*transform * normal).normalize_or_zero());
            }
        }
    }
}
