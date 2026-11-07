//! Nodes that let curves and surfaces take part in a rig: `follicle`, `transformGeometry`, and the two adapters that carry
//! NURBS geometry through the deformers.
//!
//! The deformers read and write meshes. To deform a curve or surface, `nurbsToPoints` makes a mesh with one point per CV and
//! no faces, the deformers work on that as usual with their weights indexed by CV as in the reference, and `pointsToNurbs` puts the
//! points back into the geometry. A surface's CVs go U first: `u * numV + v`, which is the order the reference's weights use.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::Form;
use aztek_geo::Mesh;
use glam::{DMat3, DMat4, DVec2, DVec3};

use crate::curves;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};
use crate::surfaces;
use crate::transform::{matrix_to_euler, RotateOrder, Shape};

pub const TO_POINTS: &str = "nurbsToPoints";
pub const FROM_POINTS: &str = "pointsToNurbs";
pub const FOLLICLE: &str = "follicle";

const HIDDEN_IN: u32 = flags::INPUT | flags::HIDDEN;
const HIDDEN_OUT: u32 = flags::OUTPUT | flags::HIDDEN;

fn data_attr(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: fl, doc: doc.into(), ..Default::default() }
}

fn mesh_attr(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Mesh, default_value: PlugValue::Mesh(None), flags: fl, doc: doc.into(), ..Default::default() }
}

fn matrix_attr(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: fl, doc: doc.into(), ..Default::default() }
}

fn float_attr(name: &str, short: &str, default: f64, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(default), flags: flags::INPUT | flags::STORABLE | flags::KEYABLE, doc: doc.into(), ..Default::default() }
}

fn vec3_out(name: &str, short: &str, doc: &str) -> AttrDef {
    crate::attr_defs::vec3_attr(name, short, DVec3::ZERO, flags::OUTPUT, doc)
}

fn points_of(cvs: &[DVec3]) -> PlugValue {
    let mut m = Mesh::new();
    for &p in cvs {
        m.add_point(p);
    }
    PlugValue::Mesh(Some(Arc::new(m)))
}

struct ToPoints;

impl NodeBehavior for ToPoints {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let out = if let Some(c) = curves::curve_value(d.value(0)) {
            points_of(&c.cvs)
        } else if let Some(s) = surfaces::surface_value(d.value(0)) {
            points_of(&s.cvs)
        } else {
            PlugValue::Mesh(None)
        };
        d.set_output(1, out);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct FromPoints;

impl NodeBehavior for FromPoints {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(m) = d.mesh(1).cloned() else {
            d.set_output(2, d.value(0).clone());
            return;
        };
        let out = if let Some(c) = curves::curve_value(d.value(0)) {
            let mut c = c.clone();
            for (i, p) in c.cvs.iter_mut().enumerate() {
                if (i as u32) < m.point_capacity() {
                    *p = m.position(i as u32);
                }
            }
            // The wrapped CVs of a periodic curve follow their first lap, whose are the ones weighted.
            if c.form == Form::Periodic {
                let lap = c.spans();
                for i in 0..c.degree {
                    c.cvs[lap + i] = c.cvs[i % lap];
                }
            }
            curves::to_value(c)
        } else if let Some(s) = surfaces::surface_value(d.value(0)) {
            let mut s = s.clone();
            for (i, p) in s.cvs.iter_mut().enumerate() {
                if (i as u32) < m.point_capacity() {
                    *p = m.position(i as u32);
                }
            }
            s.rewrap();
            surfaces::to_value(s)
        } else {
            PlugValue::Data(None)
        };
        d.set_output(2, out);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TransformGeometry;

impl NodeBehavior for TransformGeometry {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let m = d.matrix(1);
        let m = if d.bool(2) { m.inverse() } else { m };
        let out = if let Some(c) = curves::curve_value(d.value(0)) {
            curves::to_value(c.transformed(&m))
        } else if let Some(s) = surfaces::surface_value(d.value(0)) {
            surfaces::to_value(s.transformed(&m))
        } else {
            PlugValue::Data(None)
        };
        d.set_output(3, out);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Point and tangents along U and V on a surface or mesh.
struct Frame {
    point: DVec3,
    du: DVec3,
    dv: DVec3,
}

/// A place on a mesh found by its UVs: kept between evaluations, since the topology rarely changes and points often do.
#[derive(Clone, Copy)]
struct UvHit {
    topology: u64,
    u: f64,
    v: f64,
    corners: [u32; 3],
    bary: [f64; 3],
}

#[derive(Default)]
struct Follicle {
    hit: Option<UvHit>,
}

impl Follicle {
    const IN_SURFACE: usize = Shape::COUNT;
    const IN_MESH: usize = Shape::COUNT + 1;
    const IN_WORLD: usize = Shape::COUNT + 2;
    const PARAM_U: usize = Shape::COUNT + 3;
    const PARAM_V: usize = Shape::COUNT + 4;
    const OUT_T: usize = Shape::COUNT + 5;
    const OUT_R: usize = Shape::COUNT + 6;

    /// Which triangle of the mesh holds a UV, nearest when it falls outside them all, as a follicle stays on the mesh.
    fn find_uv(m: &Mesh, u: f64, v: f64) -> Option<([u32; 3], [f64; 3])> {
        let uvs = m.corner_attrs().get::<[f32; 2]>("uv0");
        if uvs.is_empty() {
            return None;
        }
        let at = |c: u32| DVec2::new(uvs[c as usize][0] as f64, uvs[c as usize][1] as f64);
        let want = DVec2::new(u, v);
        let mut best: Option<(f64, [u32; 3], [f64; 3])> = None;
        for f in 0..m.face_capacity() {
            if !m.face_valid(f) {
                continue;
            }
            let corners = m.face_corners(f);
            for k in 1..corners.len().saturating_sub(1) {
                let tri = [corners[0], corners[k], corners[k + 1]];
                let (a, b, c) = (at(tri[0]), at(tri[1]), at(tri[2]));
                let (e0, e1, e2) = (b - a, c - a, want - a);
                let det = e0.perp_dot(e1);
                if det.abs() < 1e-14 {
                    continue;
                }
                let w1 = e2.perp_dot(e1) / det;
                let w2 = e0.perp_dot(e2) / det;
                let w0 = 1.0 - w1 - w2;
                // How far outside: zero means inside.
                let outside = (-w0).max(0.0) + (-w1).max(0.0) + (-w2).max(0.0);
                if best.as_ref().is_none_or(|b| outside < b.0) {
                    best = Some((outside, tri, [w0, w1, w2]));
                    if outside == 0.0 {
                        return Some((tri, [w0, w1, w2]));
                    }
                }
            }
        }
        best.map(|(_, t, w)| (t, w))
    }

    fn on_mesh(&mut self, m: &Mesh, u: f64, v: f64) -> Option<Frame> {
        let hit = match self.hit {
            Some(h) if h.topology == m.topology_id() && h.u == u && h.v == v => h,
            _ => {
                let (corners, bary) = Self::find_uv(m, u, v)?;
                let h = UvHit { topology: m.topology_id(), u, v, corners, bary };
                self.hit = Some(h);
                h
            }
        };
        let uvs = m.corner_attrs().get::<[f32; 2]>("uv0");
        let p = |k: usize| m.position(m.corner_point(hit.corners[k]));
        let uv = |k: usize| DVec2::new(uvs[hit.corners[k] as usize][0] as f64, uvs[hit.corners[k] as usize][1] as f64);
        let point = p(0) * hit.bary[0] + p(1) * hit.bary[1] + p(2) * hit.bary[2];
        let (e1, e2) = (p(1) - p(0), p(2) - p(0));
        let (t1, t2) = (uv(1) - uv(0), uv(2) - uv(0));
        let det = t1.perp_dot(t2);
        let (du, dv) = if det.abs() > 1e-14 { ((e1 * t2.y - e2 * t1.y) / det, (e2 * t1.x - e1 * t2.x) / det) } else { (e1, e2) };
        Some(Frame { point, du, dv })
    }
}

impl NodeBehavior for Follicle {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let (u, v) = (d.float(Self::PARAM_U), d.float(Self::PARAM_V));
        let frame = if let Some(s) = surfaces::surface_value(d.value(Self::IN_SURFACE)) {
            let (ulo, uhi) = s.domain_u();
            let (vlo, vhi) = s.domain_v();
            let (uu, vv) = (ulo + u.clamp(0.0, 1.0) * (uhi - ulo), vlo + v.clamp(0.0, 1.0) * (vhi - vlo));
            let (point, du, dv) = s.eval_ders(uu, vv);
            Some(Frame { point, du, dv })
        } else if let Some(m) = d.mesh(Self::IN_MESH).cloned() {
            self.on_mesh(&m, u, v)
        } else {
            None
        };
        let Some(f) = frame else { return };
        let w = d.matrix(Self::IN_WORLD);
        let t = w.transform_point3(f.point);
        // X along U, Y along V, Z the normal.
        let x = w.transform_vector3(f.du).normalize_or_zero();
        let along_v = w.transform_vector3(f.dv);
        let z = x.cross(along_v).normalize_or_zero();
        let y = z.cross(x);
        let rot = DMat3::from_cols(x, y, z);
        d.set_output(Self::OUT_T, PlugValue::Vec3(t));
        d.set_output(Self::OUT_R, PlugValue::Vec3(matrix_to_euler(&rot, RotateOrder::Xyz)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let reg = |t: NodeType| {
            let name = t.name.clone();
            types::register_type(t).expect(&name);
        };
        reg(NodeType {
            name: TO_POINTS.into(),
            doc: "A curve or surface as a mesh of its CVs, for the deformers.".into(),
            own_attrs: vec![data_attr("inputGeometry", "ig", HIDDEN_IN, "The curve or surface."), mesh_attr("outMesh", "o", HIDDEN_OUT, "One point per CV, no faces.")],
            create: Some(Arc::new(|| Box::new(ToPoints))),
            ..Default::default()
        });
        reg(NodeType {
            name: FROM_POINTS.into(),
            doc: "Deformed CVs put back into their curve or surface.".into(),
            own_attrs: vec![
                data_attr("template", "tp", HIDDEN_IN, "The geometry the CVs came from."),
                mesh_attr("inMesh", "i", HIDDEN_IN, "The CVs."),
                data_attr("outputGeometry", "og", HIDDEN_OUT, "The geometry with those CVs."),
            ],
            create: Some(Arc::new(|| Box::new(FromPoints))),
            ..Default::default()
        });
        reg(NodeType {
            name: "transformGeometry".into(),
            doc: "A curve or surface moved by a matrix, as Freeze Transformations leaves in history.".into(),
            own_attrs: vec![
                data_attr("inputGeometry", "ig", HIDDEN_IN, "The geometry."),
                matrix_attr("transform", "txf", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The matrix."),
                AttrDef { flags: flags::INPUT | flags::STORABLE, ..crate::attr_defs::bool_attr("invertTransform", false, "Use the matrix's inverse.") },
                data_attr("outputGeometry", "og", HIDDEN_OUT, "The geometry moved."),
            ],
            create: Some(Arc::new(|| Box::new(TransformGeometry))),
            ..Default::default()
        });
        let shape = types::find("shape").expect("shape");
        reg(NodeType {
            name: FOLLICLE.into(),
            base: Some(shape),
            dag: true,
            icon: "locator".into(),
            doc: "A point riding on a surface or mesh at a UV: outTranslate and outRotate drive a transform.".into(),
            own_attrs: vec![
                data_attr("inputSurface", "is", HIDDEN_IN, "The surface, in object space."),
                mesh_attr("inputMesh", "inm", HIDDEN_IN, "Or a mesh, in object space."),
                matrix_attr("inputWorldMatrix", "iwm", HIDDEN_IN, "The geometry's world matrix."),
                float_attr("parameterU", "pu", 0.5, "Where along U, 0 to 1."),
                float_attr("parameterV", "pv", 0.5, "Where along V, 0 to 1."),
                vec3_out("outTranslate", "ot", "The point, world space."),
                vec3_out("outRotate", "or", "The frame, world space."),
            ],
            create: Some(Arc::new(|| Box::<Follicle>::default())),
            free_of_parent: true,
            ..Default::default()
        });
    });
}
