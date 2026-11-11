//! View cameras: the perspective camera tumbling about its centre of interest, and the orthographic top, front and
//! side cameras. Yaw and pitch are in the Y up frame (`aztek_scene::world`): about world up, above the ground, whichever
//! axis is up.

use aztek_scene::world::{from_world, to_world};
use glam::{DMat4, DVec2, DVec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamKind {
    Persp,
    Top,
    Front,
    Side,
}

impl CamKind {
    pub fn name(self) -> &'static str {
        match self {
            CamKind::Persp => "persp",
            CamKind::Top => "top",
            CamKind::Front => "front",
            CamKind::Side => "side",
        }
    }

    pub fn from_name(s: &str) -> Option<CamKind> {
        Some(match s {
            "persp" => CamKind::Persp,
            "top" => CamKind::Top,
            "front" => CamKind::Front,
            "side" => CamKind::Side,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct Camera {
    pub kind: CamKind,
    pub coi: DVec3,
    /// Degrees about world Y.
    pub yaw: f64,
    /// Degrees above the horizon.
    pub pitch: f64,
    pub distance: f64,
    /// Horizontal field of view, degrees (35 mm lens on a 1.417 in aperture).
    pub hfov: f64,
    /// Orthographic view width in world units.
    pub ortho_width: f64,
    /// Looking straight on, no perspective. Default views are; so may any camera be.
    pub orthographic: bool,
    /// Degrees about the view axis, from the Roll tool. The view keeps it; the camera node doesn't hold it.
    pub roll: f64,
}

impl Camera {
    pub fn new(kind: CamKind) -> Camera {
        let mut c = Camera { kind, coi: DVec3::ZERO, yaw: 0.0, pitch: 0.0, distance: 1000.0, hfov: 54.43, ortho_width: 30.0, orthographic: kind != CamKind::Persp, roll: 0.0 };
        match kind {
            CamKind::Persp => c.look_from(to_world(DVec3::new(28.0, 21.0, 28.0))),
            CamKind::Top => {
                c.pitch = 90.0;
            }
            CamKind::Front => {}
            CamKind::Side => c.yaw = 90.0,
        }
        c
    }

    pub fn ortho(&self) -> bool {
        self.orthographic
    }

    pub fn look_from(&mut self, eye: DVec3) {
        let d = from_world(eye - self.coi);
        self.distance = d.length().max(1e-6);
        self.yaw = d.x.atan2(d.z).to_degrees();
        self.pitch = (d.y / self.distance).clamp(-1.0, 1.0).asin().to_degrees();
    }

    pub fn dir_to_eye(&self) -> DVec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        to_world(DVec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos()))
    }

    /// Yaw and pitch, degrees, looking from the centre of interest along a world direction.
    pub fn angles_of(dir: DVec3) -> (f64, f64) {
        let d = from_world(dir).normalize_or_zero();
        (d.x.atan2(d.z).to_degrees(), d.y.clamp(-1.0, 1.0).asin().to_degrees())
    }

    pub fn eye(&self) -> DVec3 {
        self.coi + self.dir_to_eye() * self.distance
    }

    pub fn view(&self) -> DMat4 {
        // Straight down: up is -Z (+Y when Z is up) so X runs right, as the reference's top view.
        let up = to_world(if self.pitch.abs() > 89.99 { DVec3::new(0.0, 0.0, -self.pitch.signum()) } else { DVec3::Y });
        let m = DMat4::look_at_rh(self.eye(), self.coi, up);
        if self.roll == 0.0 {
            m
        } else {
            DMat4::from_rotation_z(self.roll.to_radians()) * m
        }
    }

    pub fn vfov(&self, aspect: f64) -> f64 {
        2.0 * ((self.hfov.to_radians() * 0.5).tan() / aspect.max(1e-6)).atan()
    }

    /// Reversed depth, y down to match Vulkan and screen pixels.
    pub fn proj(&self, aspect: f64) -> DMat4 {
        let mut p = if self.ortho() {
            let hw = self.ortho_width * 0.5;
            let hh = hw / aspect.max(1e-6);
            // far then near: depth runs 1 at the eye to 0 far away.
            DMat4::orthographic_rh(-hw, hw, -hh, hh, self.distance * 4.0 + 1e5, 0.0)
        } else {
            let near = (self.distance * 0.01).clamp(0.01, 10.0);
            DMat4::perspective_infinite_reverse_rh(self.vfov(aspect), aspect, near)
        };
        p.y_axis.y = -p.y_axis.y;
        p
    }

    pub fn tumble(&mut self, delta: DVec2) {
        if self.ortho() {
            return;
        }
        self.yaw -= delta.x * 0.3;
        self.pitch = (self.pitch + delta.y * 0.3).clamp(-89.9, 89.9);
    }

    /// World units per pixel at the centre of interest.
    pub fn units_per_px_at(&self, at: DVec3, height: f64, aspect: f64) -> f64 {
        if self.ortho() {
            return self.ortho_width / aspect.max(1e-6) / height.max(1.0);
        }
        let d = (at - self.eye()).length().max(1e-6);
        2.0 * d * (self.vfov(aspect) * 0.5).tan() / height.max(1.0)
    }

    /// delta in pixels, height the viewport's.
    pub fn track(&mut self, delta: DVec2, height: f64, aspect: f64) {
        let view = self.view();
        let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = DVec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let per_px = self.units_per_px_at(self.coi, height, aspect);
        self.coi += (-right * delta.x + up * delta.y) * per_px;
    }

    pub fn dolly(&mut self, amount: f64) {
        let f = (-amount * 0.005).exp();
        if self.ortho() {
            self.ortho_width = (self.ortho_width * f).max(1e-4);
        } else {
            self.distance = (self.distance * f).max(1e-3);
        }
    }

    /// Fits a sphere in view.
    pub fn frame(&mut self, center: DVec3, radius: f64, aspect: f64) {
        self.coi = center;
        if self.ortho() {
            self.ortho_width = radius.max(1e-3) * 2.2 * aspect.max(1.0);
            self.distance = self.distance.max(radius * 4.0);
            return;
        }
        let half = (self.vfov(aspect).min(self.hfov.to_radians()) * 0.5).max(1e-3);
        self.distance = (radius.max(1e-3) / half.sin()) * 1.05;
    }

    /// World ray through a viewport pixel.
    pub fn ray(&self, px: DVec2, size: DVec2) -> (DVec3, DVec3) {
        let aspect = size.x / size.y.max(1.0);
        let inv = (self.proj(aspect) * self.view()).inverse();
        let ndc = DVec2::new(px.x / size.x * 2.0 - 1.0, px.y / size.y * 2.0 - 1.0);
        let near = inv.project_point3(DVec3::new(ndc.x, ndc.y, 1.0));
        let far = inv.project_point3(DVec3::new(ndc.x, ndc.y, if self.ortho() { 0.0 } else { 1e-6 }));
        (near, (far - near).normalize())
    }
}
