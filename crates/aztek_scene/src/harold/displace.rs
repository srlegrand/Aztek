//! Height displacement at render: each vertex of the render mesh, after subdivision, moved along its normal by the
//! height its shading group's displacement gives at its UV, then normals made again from the moved surface. Vector
//! displacement and autobump are not done.

use aztek_render::geom::MeshData;
use aztek_render::shader::{Input, Network, ShadeCtx};
use aztek_render::texture::TextureCache;
use glam::Vec3;

use super::network::{attr, get_f, NetBuilder};
use crate::plug::NodeId;

/// One shading group's displacement: a height input, scale and the height that means no movement.
#[derive(Clone, Copy, Debug)]
pub struct Disp {
    pub height: Input,
    pub scale: f32,
    pub zero: f32,
}

/// What a shading group's displacementShader plug is fed by, as a height.
pub fn of_group(nb: &mut NetBuilder, sg: NodeId) -> Option<Disp> {
    let p = attr(nb.g, sg, "displacementShader")?;
    let src = nb.g.source(&p)?;
    let ty = nb.g.node(src.node)?.ty().name.clone();
    if ty == "displacementShader" {
        let d = src.node;
        let height = nb.input(d, "displacement");
        return Some(Disp { height, scale: get_f(nb.g, d, "scale", 1.0) as f32, zero: get_f(nb.g, d, "aiDisplacementZeroValue", 0.0) as f32 });
    }
    // A texture straight into the group.
    let n = nb.node(src.node)?;
    let out = if nb.g.attr(&src).name.contains("Alpha") { aztek_render::shader::Out::A } else { aztek_render::shader::Out::Mean };
    Some(Disp { height: Input::Link(n, out), scale: 1.0, zero: 0.0 })
}

/// Moves the mesh's points; per triangle the shader index picks which displacement. `height` is the shape's
/// aiDispHeight.
pub fn displace(mesh: &mut MeshData, disp: &[Option<Disp>], height: f32, net: &Network, tex: &TextureCache) {
    if mesh.uvs.is_empty() || mesh.normals.is_empty() {
        return;
    }
    let n = mesh.keys[0].len();
    // Each vertex's displacement: that of the first triangle that uses it.
    let mut which: Vec<Option<Disp>> = vec![None; n];
    for (t, tri) in mesh.tris.iter().enumerate() {
        let d = disp.get(mesh.tri_shader[t] as usize).copied().flatten();
        for &v in tri {
            if which[v as usize].is_none() {
                which[v as usize] = d;
            }
        }
    }
    let normals = mesh.normals[0].clone();
    for key in &mut mesh.keys {
        for v in 0..n {
            let Some(d) = which[v] else { continue };
            let ctx = ShadeCtx { p: key[v], p_obj: key[v], n: normals[v], ng: normals[v], uv: mesh.uvs[v], footprint: 0.0, ..Default::default() };
            let h = (net.eval_f(d.height, &ctx, tex) - d.zero) * d.scale * height;
            key[v] += normals[v] * h;
        }
    }
    // Normals of the moved surface, area weighted; vertices split at seams are joined by position.
    let pts = &mesh.keys[0];
    let mut acc = vec![Vec3::ZERO; n];
    for tri in &mesh.tris {
        let (a, b, c) = (pts[tri[0] as usize], pts[tri[1] as usize], pts[tri[2] as usize]);
        let nn = (b - a).cross(c - a);
        for &v in tri {
            acc[v as usize] += nn;
        }
    }
    let mut by_pos: std::collections::HashMap<[u32; 3], Vec3> = std::collections::HashMap::new();
    for v in 0..n {
        *by_pos.entry(pts[v].to_array().map(f32::to_bits)).or_default() += acc[v];
    }
    let fresh: Vec<Vec3> = (0..n)
        .map(|v| {
            let s = by_pos[&pts[v].to_array().map(f32::to_bits)];
            // Keep hard edges hard: only join where the old normals agreed.
            let joined = s.normalize_or_zero();
            let own = acc[v].normalize_or_zero();
            if joined.dot(normals[v]) > 0.7 { joined } else if own.length_squared() > 0.5 { own } else { normals[v] }
        })
        .collect();
    mesh.normals = vec![fresh];
    mesh.build();
}
