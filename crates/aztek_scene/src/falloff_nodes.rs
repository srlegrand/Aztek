//! The falloff and component tag nodes that are not transforms: uniformFalloff, proximityFalloff, subsetFalloff,
//! componentFalloff, blendFalloff, transferFalloff, falloffEval, componentMatch and componentTagBase. Attributes are the
//! 2027 release's. Each falloff's outputWeightFunction is a `falloff::WeightFn`.

use super::*;
use crate::falloff::{value, WeightFn};

fn data_out(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Data, PlugValue::Data(None), OUT | flags::HIDDEN)
}
fn data_in(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Data, PlugValue::Data(None), flags::INPUT | flags::HIDDEN)
}
fn geometry(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Mesh, PlugValue::Mesh(None), STORE | flags::HIDDEN)
}
fn text(name: &str, short: &str, d: &str) -> AttrDef {
    base(name, short, PlugType::String, PlugValue::String(d.into()), STORE)
}
fn stored(mut a: AttrDef) -> AttrDef {
    a.flags = STORE;
    a
}
fn string_of(d: &NodeData, n: &str) -> String {
    match d.value(at(d, n)) {
        PlugValue::String(s) => s.clone(),
        _ => String::new(),
    }
}
fn ramp_of(d: &NodeData) -> crate::falloff::Ramp {
    ramp(d, "ramp.ramp_Position", "ramp.ramp_FloatValue", "ramp.ramp_Interp")
}

/// The falloff's ramp: ramp[i].ramp_Position, ramp_FloatValue, ramp_Interp.
pub(crate) fn ramp_attrs(name: &str, short: &str) -> Vec<AttrDef> {
    vec![
        kid(name, short, stored(fin(&format!("{name}_Position"), &format!("{short}p"), 0.0))),
        kid(name, short, stored(fin(&format!("{name}_FloatValue"), &format!("{short}fv"), 0.0))),
        kid(name, short, stored(en(&format!("{name}_Interp"), &format!("{short}i"), 0, &["None", "Linear", "Smooth", "Spline"]))),
    ]
}

fn uniform(d: &mut NodeData) {
    set(d, "outputWeightFunction", value(WeightFn::Uniform(f(d, "uniformWeight"))));
}

fn proximity(d: &mut NodeData) {
    let f = WeightFn::Proximity {
        geometry: d.mesh(at(d, "proximityGeometry")).cloned(),
        volume: i(d, "volume"),
        start: f(d, "start"),
        end: f(d, "end"),
        ramp: ramp_of(d),
        world: i(d, "vertexSpace") == 1,
        subset: string_of(d, "proximitySubset"),
    };
    set(d, "outputWeightFunction", value(f));
}

fn subset(d: &mut NodeData) {
    let f = WeightFn::Subset {
        tags: b(d, "useFalloffTags").then(|| string_of(d, "falloffTags")),
        volume: i(d, "mode") == 1,
        scale: f(d, "scale"),
        start: f(d, "start"),
        end: f(d, "end"),
        within: b(d, "withinBoundary"),
        ramp: ramp_of(d),
    };
    set(d, "outputWeightFunction", value(f));
}

/// One function per layer: its default weight (the painted tables are not read).
fn component(d: &mut NodeData) {
    let dw = at(d, "weightInfoLayers.defaultWeight");
    let mut layers: Vec<i32> = d.elements(dw).iter().map(|(k, _)| *k).collect();
    if layers.is_empty() {
        layers.push(0);
    }
    let out = layers.into_iter().map(|k| (k, value(WeightFn::Constant(d.element_or(dw, k).scalar().unwrap_or(1.0))))).collect();
    let a = at(d, "outputWeightFunction");
    d.set_output_elements(a, out);
}

fn blend(d: &mut NodeData) {
    let (ma, wa, fa) = (at(d, "target.mode"), at(d, "target.weight"), at(d, "target.weightFunction"));
    let mut idx: Vec<i32> = [ma, wa, fa].iter().flat_map(|&a| d.elements(a).iter().map(|(k, _)| *k)).collect();
    idx.sort_unstable();
    idx.dedup();
    let targets = idx
        .into_iter()
        .map(|k| {
            let func = crate::falloff::function_of(d.element_or(fa, k)).cloned().map(std::sync::Arc::new);
            (d.element_or(ma, k).scalar().unwrap_or(1.0) as i64, d.element_or(wa, k).scalar().unwrap_or(1.0), func)
        })
        .collect();
    set(d, "outputWeightFunction", value(WeightFn::Blend { base: f(d, "baseWeight"), targets }));
}

fn transfer(d: &mut NodeData) {
    let geometry = d.mesh(at(d, "weightedGeometry")).cloned();
    let ia = at(d, "weightFunction");
    let out = d
        .elements(ia)
        .iter()
        .filter_map(|(k, v)| {
            let source = crate::falloff::function_of(v)?.clone();
            Some((*k, match &geometry {
                Some(g) => value(WeightFn::Transfer { source: std::sync::Arc::new(source), geometry: g.clone() }),
                None => value(source),
            }))
        })
        .collect();
    let a = at(d, "outputWeightFunction");
    d.set_output_elements(a, out);
}

/// For each point of inputGeometry its tag expression names, the nearest point of targetGeometry.
fn component_match(d: &mut NodeData) {
    let (Some(input), Some(target)) = (d.mesh(at(d, "inputGeometry")).cloned(), d.mesh(at(d, "targetGeometry")).cloned()) else {
        let a = at(d, "componentLookup");
        d.set_output_elements(a, Vec::new());
        return;
    };
    let members = crate::component_tags::points(&input, &string_of(d, "componentTagExpression"));
    let tps: Vec<u32> = (0..target.point_capacity()).filter(|&p| target.point_valid(p)).collect();
    let mut out = Vec::new();
    for p in (0..input.point_capacity()).filter(|&p| input.point_valid(p)) {
        if members.as_ref().is_some_and(|m| !m.get(p as usize).copied().unwrap_or(false)) {
            continue;
        }
        let q = input.position(p);
        if let Some(&best) = tps.iter().min_by(|&&a, &&c| target.position(a).distance_squared(q).total_cmp(&target.position(c).distance_squared(q))) {
            out.push((p as i32, PlugValue::Int(best as i64)));
        }
    }
    let a = at(d, "componentLookup");
    d.set_output_elements(a, out);
}

/// Its own tags added to the geometry passing through.
fn tag_base(d: &mut NodeData) {
    let m = crate::component_tags::with_node_tags(d, d.mesh(at(d, "inputGeometry")).cloned());
    set(d, "outputGeometry", PlugValue::Mesh(m));
}

fn nothing(_d: &mut NodeData) {}

fn falloff_common(attrs: &mut Vec<AttrDef>) {
    attrs.extend(ramp_attrs("ramp", "rmp"));
    attrs.push(data_out("outputWeightFunction", "wft"));
}

pub(super) fn specs() -> Vec<Spec> {
    let keyable = |a: AttrDef| a;
    let mut prox = vec![
        keyable(fin("start", "st", 0.0)),
        keyable(fin("end", "ed", 1.0)),
        en("volume", "vol", 1, &["None", "Inside", "Outside"]),
        geometry("proximityGeometry", "pgm"),
        text("proximitySubset", "pss", "*"),
        with_flags(bin("useBindTags", "ubt", false), STORE),
        text("bindTagsFilter", "btf", "*Prox"),
        en("vertexSpace", "vspc", 0, &["Object Space", "World Space"]),
        with_flags(bin("useOriginalGeometry", "uo", true), STORE),
    ];
    falloff_common(&mut prox);
    let mut sub = vec![
        limited(fin("start", "st", 0.0), Some(0.0), None),
        limited(fin("end", "ed", 1.0), Some(0.0), None),
        text("falloffTags", "ftg", ""),
        en("mode", "md", 0, &["Connectivity", "Volume"]),
        limited(unit(fin("scale", "scl", 1.0), aztek_ops::units::Unit::Linear), Some(0.01), None),
        with_flags(bin("useFalloffTags", "uds", false), STORE),
        with_flags(bin("useOriginalGeometry", "uo", true), STORE),
        with_flags(bin("withinBoundary", "wb", true), STORE),
    ];
    falloff_common(&mut sub);
    let mut tag_base_attrs = crate::component_tags::shape_attrs();
    for a in &mut tag_base_attrs {
        a.flags &= !flags::HIDDEN;
    }
    tag_base_attrs.push(geometry("inputGeometry", "ig"));
    tag_base_attrs.push(base("outputGeometry", "og", PlugType::Mesh, PlugValue::Mesh(None), OUT | flags::HIDDEN));
    vec![
        Spec("uniformFalloff", "", "A falloff that weighs every point the same.",
            vec![stored(unit(fin("uniformWeight", "unw", 0.0), aztek_ops::units::Unit::Linear)), data_out("outputWeightFunction", "wft")], uniform),
        Spec("proximityFalloff", "", "A falloff by distance from another geometry.", prox, proximity),
        Spec("subsetFalloff", "", "A falloff from a subset of the points, by connectivity or distance.", sub, subset),
        Spec("componentFalloff", "", "Weights painted or cached on a geometry, per layer.",
            vec![
                kid("weightInfoLayers", "wil", text("layerName", "lnm", "")),
                kid("weightInfoLayers", "wil", stored(fin("defaultWeight", "dwt", 1.0))),
                geometry("weightedGeometry", "wgm"),
                multi(data_out("outputWeightFunction", "wft")),
            ],
            component),
        Spec("blendFalloff", "", "Falloffs combined in order: multiply, divide, add, subtract, override, max or alpha blend.",
            vec![
                stored(fin("baseWeight", "env", 1.0)),
                kid("target", "tgt", stored(en("mode", "mod", 1, &["No Operation", "Multiply", "Divide", "Add", "Subtract", "Override", "Max", "AlphaBlend"]))),
                kid("target", "tgt", stored(fin("weight", "wgt", 1.0))),
                kid("target", "tgt", data_in("weightFunction", "whf")),
                data_out("outputWeightFunction", "wft"),
            ],
            blend),
        Spec("transferFalloff", "", "A falloff from one geometry carried onto another by nearest point.",
            vec![
                text("bindTagsFilter", "btf", ""),
                multi(data_out("outputWeightFunction", "wft")),
                with_flags(bin("useBindTags", "ubt", false), STORE),
                multi(data_in("weightFunction", "iwf")),
                geometry("weightedGeometry", "wgm"),
            ],
            transfer),
        Spec("falloffEval", "", "Evaluates weight functions on a geometry.",
            vec![
                text("componentTagExpression", "gtg", ""),
                geometry("currentGeometry", "cgm"),
                geometry("originalGeometry", "ogm"),
                multi(data_in("weightFunction", "whf")),
            ],
            nothing),
        Spec("componentMatch", "", "Matches a geometry's points to another's: the nearest point of the target for each.",
            vec![
                text("componentTagExpression", "gtg", ""),
                multi(base("componentLookup", "clkp", PlugType::Int, PlugValue::Int(0), OUT)),
                geometry("inputGeometry", "ig"),
                en("matchMode", "mmde", 0, &["ClosestPoint", "Other"]),
                geometry("targetGeometry", "tg"),
                with_flags(bin("uniqueMatch", "unq", true), STORE),
            ],
            component_match),
        Spec("componentTagBase", "geometry", "Adds component tags to the geometry passing through.", tag_base_attrs, tag_base),
    ]
}

fn limited(mut a: AttrDef, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    a.min_value = lo;
    a.max_value = hi;
    a
}
fn unit(mut a: AttrDef, u: aztek_ops::units::Unit) -> AttrDef {
    a.unit = u;
    a
}
fn with_flags(mut a: AttrDef, fl: u32) -> AttrDef {
    a.flags = fl;
    a
}
