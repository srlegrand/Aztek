//! Named, typed per-element layers.
//!
//! Vector values are plain arrays (`[f32; 2]`, `[f32; 3]`, `[f32; 4]`).

pub type Index = u32;
pub const INVALID: Index = Index::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Point,
    Edge,
    Face,
    Corner,
}

impl Domain {
    pub const ALL: [Domain; 4] = [Domain::Point, Domain::Edge, Domain::Face, Domain::Corner];
    pub fn name(self) -> &'static str {
        match self {
            Domain::Point => "point",
            Domain::Edge => "edge",
            Domain::Face => "face",
            Domain::Corner => "corner",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttrType {
    Float,
    Float2,
    Float3,
    Float4,
    Int,
    Bool,
}

impl AttrType {
    pub fn size(self) -> usize {
        match self {
            AttrType::Float | AttrType::Int => 4,
            AttrType::Float2 => 8,
            AttrType::Float3 => 12,
            AttrType::Float4 => 16,
            AttrType::Bool => 1,
        }
    }
    pub fn components(self) -> usize {
        match self {
            AttrType::Float2 => 2,
            AttrType::Float3 => 3,
            AttrType::Float4 => 4,
            _ => 1,
        }
    }
    pub fn is_float(self) -> bool {
        matches!(
            self,
            AttrType::Float | AttrType::Float2 | AttrType::Float3 | AttrType::Float4
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            AttrType::Float => "float",
            AttrType::Float2 => "float2",
            AttrType::Float3 => "float3",
            AttrType::Float4 => "float4",
            AttrType::Int => "int",
            AttrType::Bool => "bool",
        }
    }
}

/// A value type a layer can hold.
///
/// # Safety
/// `TYPE.size()` must equal `size_of::<Self>()`, alignment at most 4, and every bit pattern the layer can hold must be
/// a valid `Self`. Layers only ever hold values written as their own type (zero, defaults, copies and blends of
/// them), which keeps `bool` valid.
pub unsafe trait AttrValue: Copy + 'static {
    const TYPE: AttrType;
}
unsafe impl AttrValue for f32 {
    const TYPE: AttrType = AttrType::Float;
}
unsafe impl AttrValue for [f32; 2] {
    const TYPE: AttrType = AttrType::Float2;
}
unsafe impl AttrValue for [f32; 3] {
    const TYPE: AttrType = AttrType::Float3;
}
unsafe impl AttrValue for [f32; 4] {
    const TYPE: AttrType = AttrType::Float4;
}
unsafe impl AttrValue for i32 {
    const TYPE: AttrType = AttrType::Int;
}
unsafe impl AttrValue for bool {
    const TYPE: AttrType = AttrType::Bool;
}

/// Built-in attribute names.
pub mod names {
    /// Corner float3, custom split normals.
    pub const NORMAL: &str = "normal";
    /// Corner float4, w is the bitangent sign.
    pub const TANGENT: &str = "tangent";
    /// Point or corner float4.
    pub const COLOR: &str = "color";
    /// Edge bool.
    pub const HARD: &str = "hard";
    /// Edge bool.
    pub const SEAM: &str = "seam";
    /// Edge float, 0 smooth to 1+ sharp.
    pub const CREASE: &str = "crease";
    /// Edge float, 0 to 1, read by bevel modifiers.
    pub const BEVEL_WEIGHT: &str = "bevelWeight";
    /// Face int, index into `Mesh::material_names`.
    pub const MATERIAL: &str = "material";
    /// Face int, 0 means faceted.
    pub const SMOOTH_GROUP: &str = "smoothGroup";

    /// "uv0", "uv1", ... corner float2.
    pub fn uv(set: i32) -> String {
        format!("uv{set}")
    }
}

/// Bytes kept 4-aligned, so every value type can be viewed in place.
#[derive(Clone, Default, Debug)]
struct Bytes {
    words: Vec<u32>,
    len: usize,
}

impl Bytes {
    fn zeroed(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(4)],
            len,
        }
    }
    fn as_slice(&self) -> &[u8] {
        &bytemuck::cast_slice(&self.words)[..self.len]
    }
    fn as_mut(&mut self) -> &mut [u8] {
        &mut bytemuck::cast_slice_mut(&mut self.words)[..self.len]
    }
    /// New bytes are zero.
    fn resize(&mut self, len: usize) {
        if len < self.len {
            // Clear the tail so a later grow reads zeros.
            let end = self.len;
            bytemuck::cast_slice_mut::<u32, u8>(&mut self.words)[len..end].fill(0);
        }
        self.words.resize(len.div_ceil(4), 0);
        self.len = len;
    }
}

/// One named, typed value per element of a domain.
#[derive(Clone, Debug)]
pub struct Attribute {
    name: String,
    ty: AttrType,
    stride: usize,
    default: [u8; 16],
    data: Bytes,
}

impl Attribute {
    /// `count` elements, zeroed.
    pub fn new(name: impl Into<String>, ty: AttrType, count: usize) -> Self {
        let stride = ty.size();
        Self {
            name: name.into(),
            ty,
            stride,
            default: [0; 16],
            data: Bytes::zeroed(count * stride),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    pub fn ty(&self) -> AttrType {
        self.ty
    }
    pub fn len(&self) -> usize {
        if self.stride == 0 {
            0
        } else {
            self.data.len / self.stride
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Empty when `T` is not this layer's type.
    pub fn values<T: AttrValue>(&self) -> &[T] {
        if T::TYPE != self.ty {
            return &[];
        }
        let bytes = self.data.as_slice();
        // SAFETY: AttrValue's contract; the backing words are 4-aligned and the length is a whole number of values.
        unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const T, self.len()) }
    }

    pub fn values_mut<T: AttrValue>(&mut self) -> &mut [T] {
        if T::TYPE != self.ty {
            return &mut [];
        }
        let n = self.len();
        let bytes = self.data.as_mut();
        // SAFETY: as in `values`.
        unsafe { std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut T, n) }
    }

    pub fn raw(&self, i: usize) -> &[u8] {
        &self.data.as_slice()[i * self.stride..(i + 1) * self.stride]
    }
    fn raw_mut(&mut self, i: usize) -> &mut [u8] {
        let s = self.stride;
        &mut self.data.as_mut()[i * s..(i + 1) * s]
    }

    /// Value new elements get. Zero unless set.
    pub fn set_default<T: AttrValue>(&mut self, v: T) {
        if T::TYPE == self.ty {
            // SAFETY: T is plain data of `stride` bytes.
            let bytes =
                unsafe { std::slice::from_raw_parts(&v as *const T as *const u8, self.stride) };
            self.default[..self.stride].copy_from_slice(bytes);
        }
    }
    pub fn default_raw(&self) -> &[u8] {
        &self.default[..self.stride]
    }
    /// Takes another layer's default; it must be of this layer's type.
    pub fn set_default_from(&mut self, other: &Attribute) {
        if other.ty == self.ty {
            self.default = other.default;
        }
    }

    pub fn resize(&mut self, n: usize) {
        let old = self.len();
        self.data.resize(n * self.stride);
        if n > old && self.default.iter().any(|&b| b != 0) {
            for i in old..n {
                self.reset(i);
            }
        }
    }
    pub fn reset(&mut self, i: usize) {
        let d = self.default;
        let s = self.stride;
        self.raw_mut(i).copy_from_slice(&d[..s]);
    }
    pub fn copy(&mut self, dst: usize, src: usize) {
        let s = self.stride;
        self.data
            .as_mut()
            .copy_within(src * s..(src + 1) * s, dst * s);
    }
    pub fn copy_from(&mut self, other: &Attribute, dst: usize, src: usize) {
        if other.ty == self.ty {
            let v = other.raw(src).to_vec();
            self.raw_mut(dst).copy_from_slice(&v);
        }
    }

    fn blend(
        &self,
        srcs: &[Index],
        weights: &[f32],
        read: impl Fn(Index) -> Vec<u8>,
    ) -> Option<Vec<u8>> {
        if srcs.is_empty() {
            return None;
        }
        if !self.ty.is_float() {
            let mut best = 0;
            for i in 1..srcs.len() {
                if weights[i] > weights[best] {
                    best = i;
                }
            }
            return Some(read(srcs[best]));
        }
        let n = self.ty.components();
        let mut acc = [0f32; 4];
        for (i, &s) in srcs.iter().enumerate() {
            let bytes = read(s);
            for k in 0..n {
                let v = f32::from_ne_bytes(bytes[k * 4..k * 4 + 4].try_into().unwrap());
                acc[k] += v * weights[i];
            }
        }
        Some(bytemuck::cast_slice(&acc[..n]).to_vec())
    }

    /// Weighted blend of other's elements into dst. Ints and bools take the heaviest source.
    pub fn mix_from(&mut self, other: &Attribute, dst: usize, srcs: &[Index], weights: &[f32]) {
        if other.ty != self.ty {
            return;
        }
        if let Some(v) = self.blend(srcs, weights, |s| other.raw(s as usize).to_vec()) {
            self.raw_mut(dst).copy_from_slice(&v);
        }
    }

    /// Weighted blend of elements into dst. Ints and bools take the heaviest source.
    pub fn mix(&mut self, dst: usize, srcs: &[Index], weights: &[f32]) {
        if let Some(v) = self.blend(srcs, weights, |s| self.raw(s as usize).to_vec()) {
            self.raw_mut(dst).copy_from_slice(&v);
        }
    }

    /// Keeps element `new_to_old[i]` at position i.
    pub fn reorder(&mut self, new_to_old: &[Index]) {
        let s = self.stride;
        let mut out = Bytes::zeroed(new_to_old.len() * s);
        {
            let src = self.data.as_slice();
            let dst = out.as_mut();
            for (i, &o) in new_to_old.iter().enumerate() {
                dst[i * s..(i + 1) * s].copy_from_slice(&src[o as usize * s..(o as usize + 1) * s]);
            }
        }
        self.data = out;
    }
}

/// All attributes of one domain. Element count is kept in sync by the Mesh.
#[derive(Clone, Debug, Default)]
pub struct AttributeSet {
    layers: Vec<Attribute>,
    size: usize,
}

impl AttributeSet {
    pub fn find(&self, name: &str) -> Option<&Attribute> {
        self.layers.iter().find(|a| a.name == name)
    }
    pub fn find_mut(&mut self, name: &str) -> Option<&mut Attribute> {
        self.layers.iter_mut().find(|a| a.name == name)
    }

    /// Returns the existing layer when the type matches; a layer of another type is replaced.
    pub fn add(&mut self, name: &str, ty: AttrType) -> &mut Attribute {
        if let Some(i) = self.layers.iter().position(|a| a.name == name) {
            if self.layers[i].ty == ty {
                return &mut self.layers[i];
            }
            self.layers.remove(i);
        }
        self.layers.push(Attribute::new(name, ty, self.size));
        self.layers.last_mut().unwrap()
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.layers.iter().position(|a| a.name == name) {
            Some(i) => {
                self.layers.remove(i);
                true
            }
            None => false,
        }
    }

    /// Renames in place, keeping the layer's position. False when from is missing or to is taken.
    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        if self.find(from).is_none() || (from != to && self.find(to).is_some()) {
            return false;
        }
        self.find_mut(from).unwrap().set_name(to);
        true
    }

    /// Moves layer `from` to position `to`, shifting the layers between.
    pub fn move_layer(&mut self, from: usize, to: usize) {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return;
        }
        if from < to {
            self.layers[from..=to].rotate_left(1);
        } else {
            self.layers[to..=from].rotate_right(1);
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// Empty when missing or of another type.
    pub fn get<T: AttrValue>(&self, name: &str) -> &[T] {
        self.find(name).map(|a| a.values::<T>()).unwrap_or(&[])
    }
    pub fn get_mut<T: AttrValue>(&mut self, name: &str) -> &mut [T] {
        match self.find_mut(name) {
            Some(a) => a.values_mut::<T>(),
            None => &mut [],
        }
    }
    pub fn get_or_add<T: AttrValue>(&mut self, name: &str) -> &mut [T] {
        self.add(name, T::TYPE).values_mut::<T>()
    }

    pub fn len(&self) -> usize {
        self.size
    }
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
    pub fn layer(&self, i: usize) -> &Attribute {
        &self.layers[i]
    }
    pub fn layer_mut(&mut self, i: usize) -> &mut Attribute {
        &mut self.layers[i]
    }
    pub fn layers(&self) -> &[Attribute] {
        &self.layers
    }
    pub fn names(&self) -> Vec<String> {
        self.layers.iter().map(|a| a.name.clone()).collect()
    }

    pub fn resize(&mut self, n: usize) {
        self.size = n;
        for a in &mut self.layers {
            a.resize(n);
        }
    }
    pub fn reset(&mut self, i: usize) {
        for a in &mut self.layers {
            a.reset(i);
        }
    }
    pub fn copy(&mut self, dst: usize, src: usize) {
        for a in &mut self.layers {
            a.copy(dst, src);
        }
    }
    pub fn mix(&mut self, dst: usize, srcs: &[Index], weights: &[f32]) {
        for a in &mut self.layers {
            a.mix(dst, srcs, weights);
        }
    }
    pub fn reorder(&mut self, new_to_old: &[Index]) {
        self.size = new_to_old.len();
        for a in &mut self.layers {
            a.reorder(new_to_old);
        }
    }
    pub fn clear(&mut self) {
        self.layers.clear();
        self.size = 0;
    }

    /// The layer of `other` matching layer i of this set: same index first (sets built with merge_layout line up).
    fn matching<'a>(&self, other: &'a AttributeSet, i: usize) -> Option<&'a Attribute> {
        let name = &self.layers[i].name;
        match other.layers.get(i) {
            Some(o) if o.name == *name => Some(o),
            _ => other.find(name),
        }
    }

    /// Copies element src of other into dst for every layer the two share by name and type.
    pub fn copy_from(&mut self, other: &AttributeSet, dst: usize, src: usize) {
        for i in 0..self.layers.len() {
            if let Some(o) = self.matching(other, i) {
                self.layers[i].copy_from(o, dst, src);
            }
        }
    }

    /// Weighted blend of other's elements into dst, for layers shared by name and type.
    pub fn mix_from(&mut self, other: &AttributeSet, dst: usize, srcs: &[Index], weights: &[f32]) {
        for i in 0..self.layers.len() {
            if let Some(o) = self.matching(other, i) {
                self.layers[i].mix_from(o, dst, srcs, weights);
            }
        }
    }

    /// Adds any layer of other that this set lacks.
    pub fn merge_layout(&mut self, other: &AttributeSet) {
        for o in &other.layers {
            if self.find(&o.name).is_some_and(|a| a.ty == o.ty) {
                continue;
            }
            let size = self.size;
            let added = self.add(&o.name, o.ty);
            added.set_default_from(o);
            for i in 0..size {
                added.reset(i);
            }
        }
    }
}
