//! The context commands run against: the graph, the command registry, and the scene-level state
//! (pickWalk's ghosts, pending compounds, the naming prefs).

use std::collections::BTreeMap;

use aztek_ops::{CallSource, CommandDef, CommandError, CommandHost, Commands};
use serde_json::{json, Value};

use crate::error::GraphError;
use crate::graph::Graph;
use crate::naming::NamingPolicy;
use crate::node::types;
use crate::plug::NodeId;

impl From<GraphError> for CommandError {
    fn from(e: GraphError) -> Self {
        CommandError::new(e.to_string())
    }
}

/// A deleted object's place in the hierarchy, so a pickWalk right after the delete can walk on from it.
#[derive(Debug, Clone, Default)]
pub(crate) struct WalkGhost {
    pub id: NodeId,
    pub parent: NodeId,
    /// Walkable, itself included, in order.
    pub siblings: Vec<NodeId>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WalkGhosts {
    pub selection_serial: u64,
    pub ghosts: Vec<WalkGhost>,
}

pub struct Session {
    pub graph: Graph,
    pub commands: Commands<Session>,
    pub(crate) walk_ghosts: WalkGhosts,
    /// Compounds declared with addAttr(at='compound') that have no children yet: multi or not.
    pub(crate) pending_compounds: BTreeMap<(NodeId, String), bool>,
    /// The "naming" prefs file's contents. The prefs store is not implemented; this holds what it would.
    pub naming_prefs: Value,
    /// The last adjustable op run from the UI.
    pub(crate) last_op: crate::last_op::State,
    /// The paint skin weights tool's settings (artAttrSkinPaintCtx).
    pub paint_ctx: crate::skin::PaintCtx,
    /// The paint attribute tool's settings (artAttrCtx).
    pub attr_ctx: crate::deform_paint::AttrCtx,
    /// What copyKey and cutKey took.
    pub key_clipboard: crate::anim_edit::KeyClipboard,
    /// selectKey's keys: curve and frame.
    pub key_selection: Vec<(NodeId, f64)>,
    /// The curves' states the Graph Editor's buffer holds (bufferCurve).
    pub buffer_curves: std::collections::HashMap<NodeId, crate::anim::Keys>,
    /// The characters keys go to when nothing is selected (currentCharacters).
    pub current_characters: Vec<NodeId>,
    /// The live surface: what Quad Draw and snapping draw on.
    pub live_surface: Vec<NodeId>,
    /// optionVar's values.
    pub option_vars: BTreeMap<String, Value>,
    /// snapMode's switches.
    pub snap_modes: BTreeMap<String, bool>,
    /// polyOptions' display settings, polySelectConstraint's state and polyClipboard's contents.
    pub poly_state: crate::mesh_edit::ref_poly::PolyState,
    /// The sculpting tools' settings.
    pub sculpt_ctx: crate::sculpt::SculptCtx,
    /// The scene's file, what `file -q -sn` returns. None while untitled.
    pub scene_path: Option<String>,
    /// The undo stack's state when the scene was last saved, opened or made; None until one of those.
    pub saved_state: Option<u64>,
    /// The file type asked for with `file -type`, for the next save.
    pub scene_type: Option<crate::ma_file::Format>,
    /// Scene settings and references: see system_cmds.
    pub extras: crate::system_cmds::Extras,
    /// The first node id the running top-level command could have made, while new meshes are to join the default shading group.
    pub(crate) new_nodes_from: Option<NodeId>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session with the built-in node types and every command registered.
    pub fn new() -> Self {
        types::register_builtins();
        crate::draw_style::reset_colors();
        let mut s = Session {
            graph: Graph::new(),
            commands: Commands::default(),
            walk_ghosts: WalkGhosts::default(),
            pending_compounds: BTreeMap::new(),
            naming_prefs: json!({}),
            last_op: Default::default(),
            paint_ctx: Default::default(),
            attr_ctx: Default::default(),
            key_clipboard: Default::default(),
            key_selection: Vec::new(),
            buffer_curves: Default::default(),
            current_characters: Vec::new(),
            live_surface: Vec::new(),
            option_vars: BTreeMap::new(),
            snap_modes: BTreeMap::new(),
            poly_state: Default::default(),
            sculpt_ctx: Default::default(),
            scene_path: None,
            saved_state: None,
            scene_type: None,
            extras: Default::default(),
            new_nodes_from: None,
        };
        crate::commands::register(&mut s.commands);
        crate::components::register(&mut s.commands);
        crate::modifiers::register(&mut s.commands);
        crate::modifiers::register_stack_commands(&mut s.commands);
        crate::instancing::register(&mut s.commands);
        crate::mesh_nodes::register(&mut s.commands);
        crate::component_query::register(&mut s.commands);
        crate::mesh_edit::register(&mut s.commands);
        crate::mesh_files::register(&mut s.commands);
        crate::object_sets::register(&mut s.commands);
        crate::split_rings_modifier::register(&mut s.commands);
        crate::auto_bevel::register(&mut s.commands);
        crate::conform_commands::register(&mut s.commands);
        crate::object_ops::register(&mut s.commands);
        crate::display_layers::register(&mut s.commands);
        crate::draw_style::register(&mut s.commands);
        crate::duplicate::register(&mut s.commands);
        crate::booleans::register(&mut s.commands);
        crate::anim::register(&mut s.commands);
        crate::uv_ops::register(&mut s.commands);
        crate::rig::register(&mut s.commands);
        crate::constraint::register(&mut s.commands);
        crate::ik::register(&mut s.commands);
        crate::skin::register(&mut s.commands);
        crate::deform::register(&mut s.commands);
        crate::curves::register(&mut s.commands);
        crate::hik::register(&mut s.commands);
        crate::surfaces::register(&mut s.commands);
        crate::sculpt::register(&mut s.commands);
        crate::cameras::register(&mut s.commands);
        crate::deform_lattice::register(&mut s.commands);
        crate::deform_soft::register(&mut s.commands);
        crate::deform_mush::register(&mut s.commands);
        crate::deform_wrap::register(&mut s.commands);
        crate::deform_wire::register(&mut s.commands);
        crate::deform_blend::register(&mut s.commands);
        crate::deform_extra::register(&mut s.commands);
        crate::deform_sets::register(&mut s.commands);
        crate::deform_paint::register(&mut s.commands);
        crate::skin_extra::register(&mut s.commands);
        crate::deform_cmds::register(&mut s.commands);
        crate::ref_cmds::register(&mut s.commands);
        crate::ref_core::register(&mut s.commands);
        // After the modules above: these replace or extend commands they registered.
        crate::mesh_edit::ref_poly::register(&mut s.commands);
        crate::ma_file::register(&mut s.commands);
        crate::expression::register(&mut s.commands);
        crate::script_node::register_types();
        crate::nurbs_nodes::register_types();
        crate::ref_attrs::register(&mut s.commands);
        crate::ref_xforms::register(&mut s.commands);
        crate::ref_scene::register(&mut s.commands);
        crate::harold::commands::register(&mut s.commands);
        crate::system_cmds::register(&mut s.commands);
        crate::references::register(&mut s.commands);
        crate::rig_misc::register(&mut s.commands);
        crate::anim_misc::register(&mut s.commands);
        crate::uv_misc::register(&mut s.commands);
        crate::nurbs_ops::register(&mut s.commands);
        crate::rig_extra::register(&mut s.commands);
        crate::general_cmds::register(&mut s.commands);
        crate::anim_extra::register(&mut s.commands);
        crate::model_extra::register(&mut s.commands);
        crate::shading::register(&mut s.commands);
        crate::light_cmds::register(&mut s.commands);
        crate::dyn_cmds::register(&mut s.commands);
        crate::dyn_particles::register(&mut s.commands);
        // Last: history node types take their attributes from the commands registered above.
        crate::history::register(&mut s.commands);
        crate::unit_tables::tag_commands(&mut s.commands);
        crate::instancing::install(&mut s.graph);
        crate::duplicate::install(&mut s.graph);
        crate::anim::install(&mut s.graph);
        crate::deform_wire::install(&mut s.graph);
        crate::rig::install(&mut s.graph);
        crate::dyn_solve::install(&mut s.graph);
        s.load_naming_policy();
        s
    }

    /// An empty session to hold the place of a real one moved elsewhere for a moment: no scene, no commands.
    pub fn hollow() -> Self {
        Session {
            graph: Graph::new(),
            commands: Commands::default(),
            walk_ghosts: WalkGhosts::default(),
            pending_compounds: BTreeMap::new(),
            naming_prefs: json!({}),
            last_op: Default::default(),
            paint_ctx: Default::default(),
            attr_ctx: Default::default(),
            key_clipboard: Default::default(),
            key_selection: Vec::new(),
            live_surface: Vec::new(),
            poly_state: Default::default(),
            sculpt_ctx: Default::default(),
            scene_path: Default::default(),
            saved_state: None,
            scene_type: Default::default(),
            extras: Default::default(),
            buffer_curves: Default::default(),
            current_characters: Default::default(),
            option_vars: BTreeMap::new(),
            snap_modes: BTreeMap::new(),
            new_nodes_from: None,
        }
    }

    pub fn register(&mut self, def: CommandDef<Session>) {
        self.commands.register(def);
    }

    /// Runs a command from a script.
    pub fn run(&mut self, name: &str, call: Value) -> Result<Value, CommandError> {
        aztek_ops::run(self, name, call)
    }

    /// Undoes a step, keeping the last op in step with the stack.
    pub fn undo(&mut self) -> bool {
        let r = self.graph.undo();
        crate::last_op::sync(self);
        r
    }

    pub fn redo(&mut self) -> bool {
        let r = self.graph.redo();
        crate::last_op::sync(self);
        r
    }

    pub fn load_naming_policy(&mut self) {
        self.graph
            .set_naming_policy(NamingPolicy::from_json(&self.naming_prefs));
    }
}

impl CommandHost for Session {
    fn commands(&self) -> &Commands<Self> {
        &self.commands
    }
    fn commands_mut(&mut self) -> &mut Commands<Self> {
        &mut self.commands
    }
    fn selection_paths(&self) -> Vec<String> {
        self.graph
            .selection()
            .iter()
            .map(|&id| self.graph.path(id))
            .collect()
    }
    fn open_undo_chunk(&mut self, label: &str) {
        if self.commands.depth() == 1 {
            self.new_nodes_from = Some(self.graph.next_node_id());
        }
        self.graph.open_chunk(label);
    }
    fn close_undo_chunk(&mut self) {
        // Still inside the command's undo step: undoing it takes the membership with the mesh.
        if self.commands.depth() == 1 {
            crate::shading::adopt_new_meshes(self);
        }
        self.graph.close_chunk();
    }
    fn running_args_changed(&mut self, args: Option<&aztek_ops::Args>) {
        // A command's own default name ("Group") without a name flag is made unique across the scene.
        self.graph
            .set_command_default_names(args.is_some_and(|a| !a.has("name")));
    }
    fn top_level_started(&mut self, name: &str, call: &Value, _source: CallSource) {
        crate::components::command_started(&mut self.graph);
        crate::last_op::command_started(self, name, call);
    }
    fn top_level_finished(&mut self, name: &str, call: &Value, source: CallSource, ok: bool) {
        crate::components::command_finished(&mut self.graph, name, source, ok);
        crate::last_op::command_finished(self, name, call, source, ok);
        crate::motion_path::after_command(self, name, ok);
    }
}

impl Session {
    /// Whether the scene has changes that haven't been saved. Undoing back to what was saved counts as no change.
    pub fn is_modified(&self) -> bool {
        match self.saved_state {
            Some(id) => self.graph.undo_stack().newest_id() != id,
            None => self.graph.undo_stack().can_undo(),
        }
    }

    /// What's in the scene now is what's on disk, or a new scene.
    pub fn mark_saved(&mut self) {
        self.saved_state = Some(self.graph.undo_stack().newest_id());
    }
}
