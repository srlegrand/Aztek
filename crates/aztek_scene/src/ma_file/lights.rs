//! Lights in .ma, as the reference lays them out. A light is a transform and a shape written attribute by attribute with short
//! names; here is the rest: shape.ltd to :lightList1.l, transform.iog to :defaultLightSet.dsm, a volume light's ramps kept as
//! they were read, and the lightLinker1 relationship lines.

use super::*;
use crate::light_cmds::{self, LinkKind};
use crate::lights as li;

fn ramp_seg(name: &str) -> bool {
    matches!(name, "crg" | "colorRange" | "pen" | "penumbra")
}

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Num(v) => format!("{v}"),
        Tok::Word(s) => s.clone(),
        Tok::Str(s) => quoted(s),
    }
}

fn seg_text(s: &Seg) -> String {
    match s.range {
        Some((a, b)) if a == b => format!("{}[{a}]", s.name),
        Some((a, b)) => format!("{}[{a}:{b}]", s.name),
        None => s.name.clone(),
    }
}

impl Build<'_> {
    /// After the shapes are built: who is in defaultLightSet, links, and the ramps.
    pub(super) fn read_lights(&mut self) {
        let doc = self.doc;
        for name in &doc.light_set {
            let Some(id) = doc.find(name).and_then(|i| self.id(i)) else { continue };
            if let Err(e) = li::set_illuminates_by_default(&mut self.s.graph, id, true) {
                self.warnings.push(format!("{name}: {e}"));
            }
        }
        for (word, object, light) in &doc.relationships {
            let Some(kind) = LinkKind::from_word(word) else { continue };
            // The links every file has, between the default sets and each shading group, say nothing new.
            if light.trim_start_matches(':').starts_with(li::DEFAULT_SET) {
                continue;
            }
            let end = |p: &str| doc.find(p.rsplit_once('.').map_or(p, |x| x.0)).and_then(|i| self.id(i));
            let (Some(l), Some(o)) = (end(light), end(object)) else { continue };
            let g = &mut self.s.graph;
            let l = li::shape_of(g, l).map_or(l, |sh| crate::dag::transform_of(g, sh));
            if let Err(e) = light_cmds::add_pair(g, kind, l, o) {
                self.warnings.push(format!("{light}: {e}"));
            }
        }
        for (i, n) in doc.nodes.iter().enumerate() {
            if n.ty != "volumeLight" {
                continue;
            }
            let Some(id) = self.id(i) else { continue };
            let mut text = String::new();
            for s in n.sets.iter().filter(|s| s.path.first().is_some_and(|x| ramp_seg(&x.name))) {
                let path: Vec<String> = s.path.iter().map(seg_text).collect();
                let vals: Vec<String> = s.values.iter().map(tok_text).collect();
                let size = if s.values.is_empty() { "" } else { " " };
                let _ = writeln!(text, "\tsetAttr \".{}\"{size}{};", path.join("."), vals.join(" "));
            }
            let g = &mut self.s.graph;
            if let (false, Some(a)) = (text.is_empty(), g.node(id).and_then(|x| x.ty().attr_index(li::RAMP_LINES))) {
                let _ = g.set_value(PlugRef::new(id, a as i32), PlugValue::String(text));
            }
        }
    }
}

impl Writer<'_> {
    /// A volume light's ramps, straight after its attributes.
    pub(super) fn write_ramps(&mut self, id: NodeId) {
        let g = &self.s.graph;
        let Some(n) = g.node(id) else { return };
        let Some(a) = n.ty().attr_index(li::RAMP_LINES) else { return };
        if let PlugValue::String(x) = n.value(a) {
            self.out.push_str(x);
        }
    }

    /// The lists every light belongs to, and the links that aren't the defaults.
    pub(super) fn write_lights(&mut self) {
        let g = &self.s.graph;
        let shapes: Vec<NodeId> = li::all_shapes(g).into_iter().filter(|s| self.keep.contains(s) && self.dag_written(*s)).collect();
        let g = &self.s.graph;
        let mut set_lines = Vec::new();
        let mut list_lines = Vec::new();
        for &sh in &shapes {
            let tr = crate::dag::transform_of(g, sh);
            list_lines.push(format!("connectAttr {} \":lightList1.l\" -na;", quoted(&format!("{}.ltd", g.path(sh)))));
            if li::illuminates_by_default(g, tr) {
                set_lines.push(format!("connectAttr {} \":{}.dsm\" -na;", quoted(&format!("{}.iog", g.path(tr))), li::DEFAULT_SET));
            }
        }
        if !list_lines.is_empty() {
            let _ = writeln!(self.out, "select -ne :lightList1;\n\tsetAttr -s {} \".l\";", list_lines.len());
        }
        if !set_lines.is_empty() {
            let _ = writeln!(self.out, "select -ne :{};\n\tsetAttr -s {} \".dsm\";", li::DEFAULT_SET, set_lines.len());
        }
        self.shading.extend(list_lines);
        self.shading.extend(set_lines);
        let mut text = String::new();
        let g = &self.s.graph;
        for kind in LinkKind::ALL {
            for (l, o) in light_cmds::pairs(g, kind) {
                if self.keep.contains(&l) && self.keep.contains(&o) {
                    let _ = writeln!(text, "relationship \"{}\" \":lightLinker1\" {} {};", kind.word(), quoted(&format!("{}.message", g.path(o))), quoted(&format!("{}.message", g.path(l))));
                }
            }
        }
        self.links_text.push_str(&text);
    }
}
