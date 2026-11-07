//! The camera: Maya's camera model of focal length over a film back in inches, film fit, orthographic width, and a thin
//! lens for depth of field from the f-stop and focus distance.

use glam::{Mat4, Vec2, Vec3};

use crate::geom::lerp_mat;
use crate::math::{concentric_disk, Ray};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FilmFit {
    #[default]
    Fill,
    Horizontal,
    Vertical,
    Overscan,
}

impl FilmFit {
    pub fn from_index(i: i64) -> FilmFit {
        match i {
            1 => FilmFit::Horizontal,
            2 => FilmFit::Vertical,
            3 => FilmFit::Overscan,
            _ => FilmFit::Fill,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub name: String,
    /// Camera to world, per motion key. The camera looks down its -z.
    pub xforms: Vec<Mat4>,
    pub orthographic: bool,
    pub ortho_width: f32,
    pub focal_mm: f32,
    /// Film back, inches.
    pub h_aperture: f32,
    pub v_aperture: f32,
    pub film_offset: Vec2,
    pub film_fit: FilmFit,
    pub lens_squeeze: f32,
    pub dof: bool,
    pub f_stop: f32,
    pub focus_distance: f32,
    /// Scene units per centimetre: the lens radius is in millimetres.
    pub units_per_cm: f32,
    pub near: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            name: "persp".into(),
            xforms: vec![Mat4::IDENTITY],
            orthographic: false,
            ortho_width: 30.0,
            focal_mm: 35.0,
            h_aperture: 1.41732,
            v_aperture: 0.94488,
            film_offset: Vec2::ZERO,
            film_fit: FilmFit::Fill,
            lens_squeeze: 1.0,
            dof: false,
            f_stop: 5.6,
            focus_distance: 5.0,
            units_per_cm: 1.0,
            near: 0.0,
        }
    }
}

impl Camera {
    pub fn looking(from: Vec3, at: Vec3, focal_mm: f32) -> Camera {
        let view = Mat4::look_at_rh(from, at, Vec3::Y);
        Camera { xforms: vec![view.inverse()], focal_mm, ..Default::default() }
    }

    /// Half the film's extent at unit distance, as (x, y), for an image of the given aspect (width over height).
    pub fn window(&self, aspect: f32) -> Vec2 {
        let film = Vec2::new(self.h_aperture * self.lens_squeeze, self.v_aperture);
        let film_aspect = film.x / film.y.max(1e-9);
        let fit_h = match self.film_fit {
            FilmFit::Horizontal => true,
            FilmFit::Vertical => false,
            FilmFit::Fill => film_aspect < aspect,
            FilmFit::Overscan => film_aspect >= aspect,
        };
        // Inches to millimetres: 25.4.
        let (w, h) = if fit_h {
            let w = film.x * 25.4 * 0.5 / self.focal_mm;
            (w, w / aspect)
        } else {
            let h = film.y * 25.4 * 0.5 / self.focal_mm;
            (h * aspect, h)
        };
        Vec2::new(w, h)
    }

    pub fn xform_at(&self, time: f32) -> Mat4 {
        if self.xforms.len() == 1 {
            return self.xforms[0];
        }
        let (a, b, f) = crate::geom::key_span(self.xforms.len(), time);
        lerp_mat(&self.xforms[a], &self.xforms[b], f)
    }

    /// A ray through a point of the image in [0,1]^2 from the top left, with a lens sample and a shutter time.
    pub fn ray(&self, ndc: Vec2, aspect: f32, lens: Vec2, time: f32) -> Ray {
        let m = self.xform_at(time);
        let sx = ndc.x * 2.0 - 1.0;
        let sy = 1.0 - ndc.y * 2.0;
        if self.orthographic {
            let half = Vec2::new(self.ortho_width * 0.5, self.ortho_width * 0.5 / aspect);
            let o = Vec3::new(sx * half.x, sy * half.y, 0.0);
            return Ray::new(m.transform_point3(o), m.transform_vector3(-Vec3::Z).normalize(), time);
        }
        let win = self.window(aspect);
        // Film offset in inches, relative to the film's half width.
        let off = self.film_offset * 25.4 / self.focal_mm;
        let dir = Vec3::new(sx * win.x + off.x, sy * win.y + off.y, -1.0);
        if self.dof && self.f_stop > 0.0 && self.focus_distance > 0.0 {
            // Aperture diameter = focal length / f-stop, in mm, into scene units.
            let radius = 0.5 * self.focal_mm / self.f_stop * 0.1 * self.units_per_cm;
            let d = concentric_disk(lens) * radius;
            let focus = dir * self.focus_distance;
            let o = Vec3::new(d.x, d.y, 0.0);
            let nd = (focus - o).normalize();
            return Ray::new(m.transform_point3(o), m.transform_vector3(nd).normalize(), time);
        }
        let mut r = Ray::new(m.transform_point3(Vec3::ZERO), m.transform_vector3(dir.normalize()).normalize(), time);
        if self.near > 0.0 {
            r.o += r.d * self.near;
        }
        r
    }

    /// Where a world point lands on the image, in [0,1]^2 from the top left; None behind the camera.
    pub fn project(&self, p: Vec3, aspect: f32) -> Option<Vec2> {
        let inv = self.xform_at(0.5).inverse();
        let q = inv.transform_point3(p);
        if self.orthographic {
            let half = Vec2::new(self.ortho_width * 0.5, self.ortho_width * 0.5 / aspect);
            return Some(Vec2::new((q.x / half.x + 1.0) * 0.5, (1.0 - q.y / half.y) * 0.5));
        }
        if q.z >= 0.0 {
            return None;
        }
        let win = self.window(aspect);
        let off = self.film_offset * 25.4 / self.focal_mm;
        let x = (q.x / -q.z - off.x) / win.x;
        let y = (q.y / -q.z - off.y) / win.y;
        Some(Vec2::new((x + 1.0) * 0.5, (1.0 - y) * 0.5))
    }

    /// The angle one pixel spans at the centre: for texture filtering.
    pub fn pixel_angle(&self, width: u32, aspect: f32) -> f32 {
        if self.orthographic {
            return 0.0;
        }
        2.0 * self.window(aspect).x / width.max(1) as f32
    }

    /// The width of a pixel at distance zero for an orthographic camera.
    pub fn pixel_width(&self, width: u32) -> f32 {
        if self.orthographic {
            self.ortho_width / width.max(1) as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_ray_looks_down_minus_z_and_projects_back() {
        let c = Camera::default();
        let r = c.ray(Vec2::splat(0.5), 1.5, Vec2::splat(0.5), 0.0);
        assert!((r.d - -Vec3::Z).length() < 1e-5);
        let r = c.ray(Vec2::new(0.2, 0.7), 1.5, Vec2::splat(0.5), 0.0);
        let p = r.at(10.0);
        let back = c.project(p, 1.5).unwrap();
        assert!((back - Vec2::new(0.2, 0.7)).length() < 1e-4, "{back}");
        // 35 mm on a 1.417 in film back: about 54.4 degrees across.
        let fov = 2.0 * c.window(1.5).x.atan();
        assert!((fov.to_degrees() - 54.43).abs() < 0.1, "{}", fov.to_degrees());
    }
}
