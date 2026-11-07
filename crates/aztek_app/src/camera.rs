//! The perspective camera and its Alt-drag navigation: tumble about the centre of interest, track, dolly.

use glam::{DMat4, DVec2, DVec3};

pub struct Camera {
    pub coi: DVec3,
    /// Degrees about world Y.
    pub yaw: f64,
    /// Degrees above the horizon.
    pub pitch: f64,
    pub distance: f64,
    /// Horizontal field of view, degrees (35 mm lens on a 1.417 in aperture).
    pub hfov: f64,
}

impl Default for Camera {
    fn default() -> Self {
        // Looking down at the origin from (28, 21, 28) scaled to centimetre scenes.
        let eye = DVec3::new(28.0, 21.0, 28.0) * 4.0;
        let mut c = Camera { coi: DVec3::ZERO, yaw: 0.0, pitch: 0.0, distance: 1.0, hfov: 54.43 };
        c.look_from(eye);
        c
    }
}

impl Camera {
    pub fn look_from(&mut self, eye: DVec3) {
        let d = eye - self.coi;
        self.distance = d.length().max(1e-6);
        self.yaw = d.x.atan2(d.z).to_degrees();
        self.pitch = (d.y / self.distance).clamp(-1.0, 1.0).asin().to_degrees();
    }

    pub fn eye(&self) -> DVec3 {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        self.coi + DVec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos()) * self.distance
    }

    pub fn view(&self) -> DMat4 {
        DMat4::look_at_rh(self.eye(), self.coi, DVec3::Y)
    }

    pub fn vfov(&self, aspect: f64) -> f64 {
        2.0 * ((self.hfov.to_radians() * 0.5).tan() / aspect.max(1e-6)).atan()
    }

    /// Reversed infinite depth, y down to match Vulkan and screen pixels.
    pub fn proj(&self, aspect: f64) -> DMat4 {
        let near = (self.distance * 1e-3).clamp(1e-3, 10.0);
        let mut p = DMat4::perspective_infinite_reverse_rh(self.vfov(aspect), aspect, near);
        p.y_axis.y = -p.y_axis.y;
        p
    }

    pub fn tumble(&mut self, delta: DVec2) {
        self.yaw -= delta.x * 0.3;
        self.pitch = (self.pitch + delta.y * 0.3).clamp(-89.9, 89.9);
    }

    /// delta in pixels, height the viewport's.
    pub fn track(&mut self, delta: DVec2, height: f64, aspect: f64) {
        let view = self.view();
        let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = DVec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        let per_px = 2.0 * self.distance * (self.vfov(aspect) * 0.5).tan() / height.max(1.0);
        self.coi += (-right * delta.x + up * delta.y) * per_px;
    }

    pub fn dolly(&mut self, amount: f64) {
        self.distance = (self.distance * (-amount * 0.005).exp()).max(1e-3);
    }

    /// Fits a sphere in view.
    pub fn frame(&mut self, center: DVec3, radius: f64, aspect: f64) {
        self.coi = center;
        let half = (self.vfov(aspect).min(self.hfov.to_radians()) * 0.5).max(1e-3);
        self.distance = (radius.max(1e-3) / half.sin()) * 1.05;
    }

    /// World ray through a viewport pixel.
    pub fn ray(&self, px: DVec2, size: DVec2) -> (DVec3, DVec3) {
        let aspect = size.x / size.y.max(1.0);
        let inv = (self.proj(aspect) * self.view()).inverse();
        let ndc = DVec2::new(px.x / size.x * 2.0 - 1.0, px.y / size.y * 2.0 - 1.0);
        let near = inv.project_point3(DVec3::new(ndc.x, ndc.y, 1.0));
        let far = inv.project_point3(DVec3::new(ndc.x, ndc.y, 1e-6));
        (near, (far - near).normalize())
    }
}
