// Included by hik_rig.rs: the command table.

fn no_op(c: &mut Commands<Session>, names: &[&str]) {
    for name in names {
        c.register(Cmd::new(name, "Refreshes the HumanIK window; here it follows the scene by itself.", |_s, _a| Ok(Value::Null)).undoable(false).positional("ignored"));
    }
}

/// What the HumanIK window draws.
fn window_info(s: &mut Session) -> Value {
    let cur = current(s);
    let g = &mut s.graph;
    let list: Vec<String> = characters(g).iter().map(|&c| g.path(c)).collect();
    let mut me = json!({});
    if let Some(name) = &cur {
        if let Some(c) = g.find(name).ok().flatten() {
            let status = definition_status(g, c);
            let slots = slots_of(g, c);
            let props = g.source(&PlugRef::new(c, C_PROPERTY as i32)).map(|p| p.node);
            let mut pv = serde_json::Map::new();
            if let Some(p) = props {
                for (k, (n, _)) in property_list().iter().enumerate().take(14) {
                    pv.insert(n.clone(), json!(g.stored_value(&PlugRef::new(p, (P_FIRST + k) as i32)).scalar().unwrap_or(0.0)));
                }
            }
            me = json!({"name": name, "slots": slots, "status": status, "locked": locked(g, c), "source": current_source(g, c), "controlRig": has_control_rig(g, c), "properties": pv, "propertyNode": props.map(|p| g.path(p))});
        }
    }
    json!({"characters": list, "current": cur, "character": me, "slotNames": slot_names()})
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        Cmd::new("hikCreateSkeleton", "Make a standard humanoid skeleton in T-pose, joints named Character1_Hips and so on: -n the name, -h height, -sc spine joints, -fg fingers, -ch a character definition with it. Returns the joints.", make_skeleton)
            .flag(f("name", "n", ArgType::String, "Name, and the joints' prefix."))
            .flag(f("height", "h", ArgType::Float, "Height (170)."))
            .flag(f("spineCount", "sc", ArgType::Int, "Spine joints (3)."))
            .flag(f("fingers", "fg", ArgType::Bool, "Fingers too."))
            .flag(f("character", "ch", ArgType::Bool, "Make the character definition too.")),
    );
    let create = |s: &mut Session, a: &Args| -> CmdResult {
        let names = aztek_ops::objects(s, a);
        let (cname, joints) = match names.first() {
            Some(n) if s.graph.find(n)?.is_none() => (n.clone(), names[1..].to_vec()),
            _ => (a.string("name", "Character1"), names.clone()),
        };
        s.graph.open_chunk("Create Character");
        let r = (|| -> CmdResult<Value> {
            let ch = new_character(s, &cname)?;
            let g = &mut s.graph;
            let roots: Vec<NodeId> = joints.iter().filter_map(|n| g.find(n).ok().flatten()).collect();
            if !roots.is_empty() {
                name_match(g, ch, &roots, "")?;
            }
            Ok(json!(g.path(ch)))
        })();
        s.graph.close_chunk();
        let out = r?;
        if let Some(n) = out.as_str() {
            set_current(s, n);
        }
        Ok(out)
    };
    for name in ["hikCreateCharacter", "newCharacterWithName"] {
        c.register(Cmd::new(name, "Define a character. Joints named (or selected, with everything below them) go to slots by their names; setCharacterObject sets others. Returns the character.", create).positional("the new character's name, then joints").flag(f("name", "n", ArgType::String, "The character's name.")));
    }
    c.register(Cmd::new("hikCreateDefinition", "A new empty definition, Character1.", |s, _a| {
        s.graph.open_chunk("Create Character");
        let r = new_character(s, "Character1");
        s.graph.close_chunk();
        let id = r?;
        let n = s.graph.path(id);
        set_current(s, &n);
        Ok(json!(n))
    }));
    let set_object = |s: &mut Session, a: &Args| -> CmdResult {
        let (Some(j), Some(cn), Some(slot)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string), slot_arg(a, 2)) else { fail!("setCharacterObject: joint, character, slot, 0") };
        let g = &mut s.graph;
        let c = character(g, &cn, "setCharacterObject")?;
        let Some(id) = g.find(&j)? else { fail!("setCharacterObject: no object named {j}") };
        let deleting = a.positional.get(3).and_then(Value::as_i64).unwrap_or(0) != 0;
        set_slot(g, c, slot, (!deleting).then_some(id))?;
        refresh_rig(g, c)?;
        Ok(Value::Null)
    };
    for name in ["setCharacterObject", "hikSetCharacterObject"] {
        c.register(Cmd::new(name, "Put a joint in a character's slot, as setCharacterObject(joint, character, slot, 0); the slot by id or name, the last 1 to take it out.", set_object).positional("joint, character, slot, 0"));
    }
    c.register(Cmd::new("hikRemoveCharacterObject", "Empty a character's slot: character, slot.", |s, a| {
        let (Some(cn), Some(slot)) = (a.pos_str(0).map(str::to_string), slot_arg(a, 1)) else { fail!("hikRemoveCharacterObject: character, slot") };
        let g = &mut s.graph;
        let c = character(g, &cn, "hikRemoveCharacterObject")?;
        set_slot(g, c, slot, None)?;
        refresh_rig(g, c)?;
        Ok(Value::Null)
    }).positional("character, slot"));
    c.register(Cmd::new("GetHIKNodeName", "The name of a slot by its id.", |_s, a| Ok(json!(slot_arg(a, 0).map(|i| slot_names()[i].clone()).unwrap_or_default()))).undoable(false).positional("id"));
    c.register(Cmd::new("hikGetNodeIdFromName", "The id of a slot by its name, -1 if there is none.", |_s, a| Ok(json!(a.pos_str(0).and_then(slot_index).map(|i| i as i64).unwrap_or(-1)))).undoable(false).positional("name"));
    c.register(Cmd::new("hikGetNodeCount", "How many slots a definition has.", |_s, _a| Ok(json!(SLOT_COUNT))).undoable(false));
    c.register(Cmd::new("hikGetSkNode", "The joint in a character's slot, by id or name; with only the character, every slot and joint in turn.", |s, a| {
        let Some(cn) = a.pos_str(0) else { fail!("hikGetSkNode: name a character") };
        let ch = character(&s.graph, cn, "hikGetSkNode")?;
        Ok(match slot_arg(a, 1) {
            Some(i) => json!(slot_joint(&s.graph, ch, i).map(|j| s.graph.path(j)).unwrap_or_default()),
            None => json!(slots_of(&s.graph, ch).into_iter().flat_map(|(k, v)| [k, v]).collect::<Vec<_>>()),
        })
    }).undoable(false).positional("character, slot"));
    for name in ["hikGetSkeletonNodes", "hikGetSkeletonNodesList"] {
        c.register(Cmd::new(name, "The joints of a character's definition.", |s, a| {
            let (_, ch) = the_character(s, a, "hikGetSkeletonNodes")?;
            Ok(json!(slots_of(&s.graph, ch).into_values().collect::<Vec<_>>()))
        }).undoable(false).positional("character"));
    }
    c.register(Cmd::new("hikGetCurrentCharacter", "The character the HumanIK window is on.", |s, _a| Ok(json!(current(s).unwrap_or_default()))).undoable(false));
    c.register(Cmd::new("hikSetCurrentCharacter", "Make a character the HumanIK window's. Returns it.", |s, a| {
        let Some(n) = a.pos_str(0).map(str::to_string) else { fail!("hikSetCurrentCharacter: name a character") };
        character(&s.graph, &n, "hikSetCurrentCharacter")?;
        set_current(s, &n);
        Ok(json!(n))
    }).undoable(false).positional("character"));
    c.register(Cmd::new("hikGetCurrentSource", "What drives the current character: None, Stance, Control Rig, or a character.", |s, _a| {
        let Some(n) = current(s) else { return Ok(json!("None")) };
        let ch = character(&s.graph, &n, "hikGetCurrentSource")?;
        Ok(json!(current_source(&s.graph, ch)))
    }).undoable(false));
    c.register(Cmd::new("hikSetCurrentSource", "Set what drives the current character: None, Stance, Control Rig, or a character.", |s, a| {
        let Some(n) = current(s) else { fail!("hikSetCurrentSource: no character") };
        let src = a.pos_str(0).unwrap_or("None").to_string();
        Ok(json!(set_source(s, &n, &src)?))
    }).positional("source"));
    c.register(Cmd::new("hikSetCurrentSourceFromCharacter", "hikSetCurrentSource by another name scripts use.", |s, a| {
        let Some(n) = current(s) else { fail!("hikSetCurrentSourceFromCharacter: no character") };
        let src = a.pos_str(0).unwrap_or("None").to_string();
        Ok(json!(set_source(s, &n, &src)?))
    }).positional("source"));
    c.register(Cmd::new("hikSetSource", "Drive one character from another: hikSetSource target source; with no source, stops. Returns how many slots.", |s, a| {
        let Some(t) = a.pos_str(0).map(str::to_string) else { fail!("hikSetSource: target, source") };
        let src = a.pos_str(1).unwrap_or("None").to_string();
        Ok(json!(set_source(s, &t, &src)?))
    }).positional("target, source"));
    c.register(Cmd::new("mayaHIKgetInputCharacter", "The character driving a character, or nothing.", |s, a| {
        let (_, ch) = the_character(s, a, "mayaHIKgetInputCharacter")?;
        let src = current_source(&s.graph, ch);
        Ok(json!(if matches!(src.as_str(), "None" | "Stance" | "Control Rig") { String::new() } else { src }))
    }).undoable(false).positional("character"));
    c.register(Cmd::new("hikCharacterLock", "Lock a character's definition, taking its T-pose from where the joints are: character, on, validate. A definition with a required slot empty won't lock when validating.", |s, a| {
        let (_, ch) = the_character(s, a, "hikCharacterLock")?;
        let on = a.positional.get(1).map(|v| v.as_i64().map(|x| x != 0).or(v.as_bool()).unwrap_or(true)).unwrap_or(true);
        let validate = a.positional.get(2).and_then(Value::as_i64).unwrap_or(0) != 0;
        let g = &mut s.graph;
        if on && validate {
            let st = definition_status(g, ch);
            if !st["valid"].as_bool().unwrap_or(false) {
                fail!("hikCharacterLock: {}", st["messages"][0].as_str().unwrap_or("the definition is not valid"));
            }
        }
        lock_character(g, ch, on)?;
        Ok(json!(on))
    }).positional("character, on, validate"));
    c.register(Cmd::new("hikToggleLockDefinition", "Lock or unlock the current character's definition.", |s, _a| {
        let Some(n) = current(s) else { fail!("hikToggleLockDefinition: no character") };
        let g = &mut s.graph;
        let ch = character(g, &n, "hikToggleLockDefinition")?;
        let on = !locked(g, ch);
        lock_character(g, ch, on)?;
        Ok(json!(on))
    }));
    for name in ["hikIsDefinitionLocked", "hikIsCharacterizationLocked"] {
        c.register(Cmd::new(name, "Whether a character's definition is locked.", |s, a| {
            let (_, ch) = the_character(s, a, "hikIsDefinitionLocked")?;
            Ok(json!(locked(&s.graph, ch)))
        }).undoable(false).positional("character"));
    }
    c.register(Cmd::new("hikEnableCharacter", "Turn a character's solve on or off: character, on. Off lets the joints go and on puts back the last source given.", |s, a| {
        let (n, ch) = the_character(s, a, "hikEnableCharacter")?;
        let on = a.positional.get(1).map(|v| v.as_i64().map(|x| x != 0).or(v.as_bool()).unwrap_or(true)).unwrap_or(true);
        let key = format!("disabled:{n}");
        if on {
            let last = s.extras.more.characterize.remove(&key).and_then(|v| v.as_str().map(str::to_string));
            if let Some(src) = last {
                set_source(s, &n, &src)?;
            }
        } else {
            let was = current_source(&s.graph, ch);
            s.extras.more.characterize.insert(key, json!(was));
            set_source(s, &n, "None")?;
        }
        Ok(json!(on))
    }).positional("character, on"));
    c.register(Cmd::new("hikDefinitionStatus", "A character's definition checked: per slot green, yellow or red, the messages, and the summary.", |s, a| {
        let (_, ch) = the_character(s, a, "hikDefinitionStatus")?;
        Ok(definition_status(&mut s.graph, ch))
    }).undoable(false).positional("character"));
    c.register(Cmd::new("hikNameMatch", "Map the joints under the named or selected roots to a character's slots by name: -c the character, -p a prefix to ignore such as mixamorig:. Returns how many.", |s, a| {
        let name = match a.get("character").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None => match current(s) {
                Some(n) => n,
                None => fail!("hikNameMatch: no character"),
            },
        };
        let roots = aztek_ops::objects(s, a);
        let g = &mut s.graph;
        let ch = character(g, &name, "hikNameMatch")?;
        let ids: Vec<NodeId> = roots.iter().filter_map(|n| g.find(n).ok().flatten()).collect();
        Ok(json!(name_match(g, ch, &ids, &a.string("prefix", ""))?))
    }).positional("root joints").flag(f("character", "c", ArgType::String, "The character.")).flag(f("prefix", "p", ArgType::String, "A prefix to ignore.")));
    c.register(Cmd::new("hikMirrorMatching", "Fill empty slots from their opposites: by name with the side changed, else by mirrored place. Returns how many.", |s, a| {
        let (_, ch) = the_character(s, a, "hikMirrorMatching")?;
        Ok(json!(mirror_match(&mut s.graph, ch)?))
    }).positional("character"));
    c.register(Cmd::new("hikSaveTemplate", "Write a character's mapping to an .xml template: file, character.", |s, a| {
        let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("hikSaveTemplate: name a file") };
        let name = a.pos_str(1).map(str::to_string).or_else(|| current(s)).unwrap_or_default();
        let ch = character(&s.graph, &name, "hikSaveTemplate")?;
        let mut text = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<hikTemplate>\n");
        for (slot, joint) in slots_of(&s.graph, ch) {
            let short = joint.rsplit(['|', ':']).next().unwrap_or(&joint).to_string();
            text.push_str(&format!("  <item key=\"{slot}\" value=\"{short}\"/>\n"));
        }
        text.push_str("</hikTemplate>\n");
        if let Err(e) = std::fs::write(&path, text) {
            fail!("hikSaveTemplate: {e}");
        }
        Ok(json!(path))
    }).undoable(false).positional("file, character"));
    c.register(Cmd::new("hikLoadTemplate", "Map a character's slots from an .xml template of slot and joint names: file, character. -p a namespace or prefix joints carry. Returns how many.", |s, a| {
        let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("hikLoadTemplate: name a file") };
        let name = a.pos_str(1).map(str::to_string).or_else(|| current(s)).unwrap_or_default();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => fail!("hikLoadTemplate: {path}: {e}"),
        };
        let prefix = a.string("prefix", "");
        let g = &mut s.graph;
        let ch = character(g, &name, "hikLoadTemplate")?;
        let mut n = 0;
        for (slot, joint) in read_template(&text) {
            let (Some(i), Some(id)) = (slot_index(&slot), g.find(&format!("{prefix}{joint}")).ok().flatten()) else { continue };
            set_slot(g, ch, i, Some(id))?;
            n += 1;
        }
        refresh_rig(g, ch)?;
        Ok(json!(n))
    }).positional("file, character").flag(f("prefix", "p", ArgType::String, "What joint names carry before the template's.")));
    c.register(Cmd::new("hikCreateControlRig", "Build a control rig on a character (the current one when none is named): FK hips, spine, neck and head, IK hands and feet with pole vectors. Returns the controls.", |s, a| {
        let (n, _) = the_character(s, a, "hikCreateControlRig")?;
        s.graph.open_chunk("Control Rig");
        let r = (|| -> CmdResult<Value> {
            set_source(s, &n, "None")?;
            let made = create_control_rig(s, &n)?;
            Ok(json!(made.into_iter().map(|x| x.0).collect::<Vec<_>>()))
        })();
        s.graph.close_chunk();
        r
    }).positional("the character"));
    c.register(Cmd::new("hikDeleteControlRig", "Remove a character's control rig; the joints stay where they are.", |s, a| {
        let (n, _) = the_character(s, a, "hikDeleteControlRig")?;
        delete_control_rig(s, &n)?;
        Ok(Value::Null)
    }).positional("the character"));
    let bake_flags = |x: Cmd| x.positional("the character").flag(f("time", "t", ArgType::Any, "The range: \"1:100\" (the playback range by default).")).flag(f("sampleBy", "sb", ArgType::Float, "Frames between keys (1).")).flag(f("eulerFilter", "ef", ArgType::Bool, "Run the Euler filter on the rotations (on by default)."));
    c.register(bake_flags(Cmd::new("hikBakeToSkeleton", "Bake what drives a character onto its joints, and leave them free of it.", bake_to_skeleton)));
    c.register(bake_flags(Cmd::new("hikBake", "As hikBakeToSkeleton.", bake_to_skeleton)));
    c.register(bake_flags(Cmd::new("hikBakeToControlRig", "Bake what drives a character onto a new control rig's keys, source None after.", bake_to_control_rig)));
    c.register(Cmd::new("hikStraightenArms", "For a rig bound with its arms down: writes a T-pose into the definition by turning the arms' stored stance out along X, the joints left where they are. Not a procedure of the reference's, where the rig is posed instead. Returns how many slots changed.", |s, a| {
        let (_, ch) = the_character(s, a, "hikStraightenArms")?;
        Ok(json!(straighten_arms(&mut s.graph, ch)?))
    }).positional("the character"));
    c.register(Cmd::new("hikInfo", "What the HumanIK window draws, as JSON.", |s, _a| Ok(window_info(s))).undoable(false));
    no_op(c, &["hikUpdateDefinitionUI", "hikUpdateCharacterList", "hikUpdateSourceList", "hikUpdateContextualUI", "hikUpdateCurrentCharacterFromUI", "hikUpdateCurrentSourceFromUI", "hikUpdateSkeletonUI", "hikControlRigSelectionChangedCallback", "hikSelectDefinitionTab", "hikUpdateTool", "hikUpdateCustomRigUI"]);
}
