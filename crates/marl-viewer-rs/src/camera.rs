//! Deterministic orthographic camera basis helpers for the viewer.
//!
//! A `CameraBasis` is a set of three orthonormal vectors (`right`, `up`,
//! `dir`) plus a `zoom` factor. The fragment shader uses these to construct
//! orthographic rays through the normalized simulation box.

use crate::args::ViewMode;

// ---------------------------------------------------------------------------
// CameraBasis
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct CameraBasis {
    pub(crate) right: [f32; 3],
    pub(crate) up: [f32; 3],
    pub(crate) dir: [f32; 3],
    pub(crate) zoom: f32,
}

// ---------------------------------------------------------------------------
// Public constructor
// ---------------------------------------------------------------------------

pub(crate) fn camera_basis(mode: ViewMode) -> CameraBasis {
    match mode {
        ViewMode::Iso => iso_basis(),
        ViewMode::Top => top_basis(),
    }
}

// ---------------------------------------------------------------------------
// Basis implementations
// ---------------------------------------------------------------------------

/// Isometric-style oblique view: looking from above/front/right.
///
/// The camera direction has positive x, negative y (so increasing x is to
/// the right on screen), and negative z so we look down into the volume.
fn iso_basis() -> CameraBasis {
    let dir = normalize([1.0, -1.0, -0.8]);
    // right is perpendicular to dir, in the XY plane
    let right = normalize([dir[1], -dir[0], 0.0]);
    // up = right × dir (maintains right-handedness)
    let up = cross(&right, &dir);
    CameraBasis {
        right,
        up,
        dir,
        zoom: 1.55,
    }
}

/// Top-down orthographic view that mimics the old z-stepping projection.
fn top_basis() -> CameraBasis {
    CameraBasis {
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        dir: [0.0, 0.0, -1.0],
        zoom: 1.05,
    }
}

// ---------------------------------------------------------------------------
// 3-vector helpers (no dependency)
// ---------------------------------------------------------------------------

fn dot(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(v: &[f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = length(&v);
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn cross(a: &[f32; 3], b: &[f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn is_unit(v: &[f32; 3]) -> bool {
        let len = length(v);
        (len - 1.0).abs() < 1e-5
    }

    fn is_finite(v: &[f32; 3]) -> bool {
        v.iter().all(|x| x.is_finite())
    }

    #[test]
    fn iso_basis_vectors_unit_and_orthonormal() {
        let cam = iso_basis();
        assert!(is_unit(&cam.right), "right not unit");
        assert!(is_unit(&cam.up), "up not unit");
        assert!(is_unit(&cam.dir), "dir not unit");
        assert!(is_finite(&cam.right));
        assert!(is_finite(&cam.up));
        assert!(is_finite(&cam.dir));
        assert!(cam.zoom.is_finite() && cam.zoom > 0.0, "bad zoom");

        assert!((dot(&cam.right, &cam.up)).abs() < 1e-5);
        assert!((dot(&cam.right, &cam.dir)).abs() < 1e-5);
        assert!((dot(&cam.up, &cam.dir)).abs() < 1e-5);
    }

    #[test]
    fn top_basis_vectors_unit_and_orthonormal() {
        let cam = top_basis();
        assert!(is_unit(&cam.right));
        assert!(is_unit(&cam.up));
        assert!(is_unit(&cam.dir));
        assert!(is_finite(&cam.right));
        assert!(is_finite(&cam.up));
        assert!(is_finite(&cam.dir));
        assert!(cam.zoom.is_finite() && cam.zoom > 0.0);

        assert!((dot(&cam.right, &cam.up)).abs() < 1e-5);
        assert!((dot(&cam.right, &cam.dir)).abs() < 1e-5);
        assert!((dot(&cam.up, &cam.dir)).abs() < 1e-5);
    }

    #[test]
    fn iso_dir_has_positive_x_negative_y_negative_z() {
        let cam = iso_basis();
        // Looking from "front-right-top" means opposite sign convention
        // Our dir is [1, -1, -0.8] normalized, so x>0, y<0, z<0
        assert!(cam.dir[0] > 0.0, "dir x should be positive");
        assert!(cam.dir[1] < 0.0, "dir y should be negative");
        assert!(cam.dir[2] < 0.0, "dir z should be negative");
    }

    #[test]
    fn top_dir_straight_down() {
        let cam = top_basis();
        assert!((cam.dir[0] - 0.0).abs() < 1e-6);
        assert!((cam.dir[1] - 0.0).abs() < 1e-6);
        assert!((cam.dir[2] + 1.0).abs() < 1e-6);
    }
}
