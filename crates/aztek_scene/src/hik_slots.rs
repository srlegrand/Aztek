//! The slots of a character definition, numbered as `GetHIKNodeName` and `setCharacterObject` number them. The order 0 to 171 is
//! the order a HIKCharacterNode's attributes are written in a file saved by the reference; 172 to 211, the leaf roll bones, follow a
//! published table's end (206 is LeafRightUpLegRoll5, 211 LeafRightForeArmRoll5).

use std::sync::OnceLock;

pub const SLOT_COUNT: usize = 212;

/// The fifteen slots a definition needs before it can be locked.
pub const REQUIRED: &[&str] = &["Hips", "LeftUpLeg", "LeftLeg", "LeftFoot", "RightUpLeg", "RightLeg", "RightFoot", "Spine", "LeftArm", "LeftForeArm", "LeftHand", "RightArm", "RightForeArm", "RightHand", "Head"];

pub fn slot_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut v: Vec<String> = [
            "Reference", "Hips", "LeftUpLeg", "LeftLeg", "LeftFoot", "RightUpLeg", "RightLeg", "RightFoot", "Spine", "LeftArm", "LeftForeArm", "LeftHand", "RightArm", "RightForeArm", "RightHand", "Head", "LeftToeBase", "RightToeBase",
            "LeftShoulder", "RightShoulder", "Neck", "LeftFingerBase", "RightFingerBase",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for i in 1..=9 {
            v.push(format!("Spine{i}"));
        }
        for i in 1..=9 {
            v.push(format!("Neck{i}"));
        }
        for s in ["LeftUpLegRoll", "LeftLegRoll", "RightUpLegRoll", "RightLegRoll", "LeftArmRoll", "LeftForeArmRoll", "RightArmRoll", "RightForeArmRoll", "HipsTranslation"] {
            v.push(s.to_string());
        }
        const DIGITS: [&str; 6] = ["Thumb", "Index", "Middle", "Ring", "Pinky", "ExtraFinger"];
        for limb in ["LeftHand", "RightHand", "LeftFoot", "RightFoot"] {
            for d in DIGITS {
                for k in 1..=4 {
                    v.push(format!("{limb}{d}{k}"));
                }
            }
        }
        for limb in ["LeftInHand", "RightInHand", "LeftInFoot", "RightInFoot"] {
            for d in DIGITS {
                v.push(format!("{limb}{d}"));
            }
        }
        v.push("LeftShoulderExtra".into());
        v.push("RightShoulderExtra".into());
        for k in 1..=5 {
            for s in ["LeftUpLeg", "LeftLeg", "RightUpLeg", "RightLeg", "LeftArm", "LeftForeArm", "RightArm", "RightForeArm"] {
                v.push(format!("Leaf{s}Roll{k}"));
            }
        }
        debug_assert_eq!(v.len(), SLOT_COUNT);
        v
    })
}

pub fn slot_index(name: &str) -> Option<usize> {
    slot_names().iter().position(|s| s == name)
}

/// The slot on the other side of the body: LeftArm and RightArm. The middle of the body answers itself.
pub fn mirror_slot(i: usize) -> usize {
    let n = &slot_names()[i];
    let other = if let Some(r) = n.strip_prefix("Left") {
        format!("Right{r}")
    } else if let Some(r) = n.strip_prefix("Right") {
        format!("Left{r}")
    } else if let Some(r) = n.strip_prefix("LeafLeft") {
        format!("LeafRight{r}")
    } else if let Some(r) = n.strip_prefix("LeafRight") {
        format!("LeafLeft{r}")
    } else {
        return i;
    };
    slot_index(&other).unwrap_or(i)
}

/// The slot a joint's name means, whatever rig it came from: Character1_LeftArm, mixamorig:LeftArm, L_UpperArm, l_arm...
pub fn slot_for_name(name: &str) -> Option<String> {
    let short = name.rsplit(['|', ':']).next().unwrap_or(name);
    let bare = short.rsplit('_').next().unwrap_or(short);
    // The exact slot names first, with any prefix off. The reference names finger bases Hand-less too: LeftHandIndex1.
    if let Some(s) = slot_names().iter().find(|s| *s == bare || s.eq_ignore_ascii_case(short)) {
        return Some(s.clone());
    }
    let low = short.to_lowercase();
    let starts = |p: &str| low.starts_with(p);
    let ends = |p: &str| low.ends_with(p);
    let side = if starts("l_") || ends("_l") || low.contains("_l_") || low.contains("left") {
        Some("Left")
    } else if starts("r_") || ends("_r") || low.contains("_r_") || low.contains("right") {
        Some("Right")
    } else {
        None
    };
    if let Some(sd) = side {
        // Fingers: thumb_01_l, l_index2.
        for (word, d) in [("thumb", "Thumb"), ("index", "Index"), ("middle", "Middle"), ("ring", "Ring"), ("pinky", "Pinky"), ("little", "Pinky")] {
            if low.contains(word) {
                let k: String = low.chars().filter(char::is_ascii_digit).collect();
                let k: u32 = k.parse().unwrap_or(1).clamp(1, 4);
                return Some(format!("{sd}Hand{d}{k}"));
            }
        }
        let part = [
            ("upperarm", "Arm"), ("uparm", "Arm"), ("forearm", "ForeArm"), ("lowerarm", "ForeArm"), ("elbow", "ForeArm"), ("clavicle", "Shoulder"), ("shoulder", "Shoulder"), ("hand", "Hand"), ("wrist", "Hand"), ("arm", "Arm"),
            ("thigh", "UpLeg"), ("upleg", "UpLeg"), ("upperleg", "UpLeg"), ("calf", "Leg"), ("shin", "Leg"), ("knee", "Leg"), ("lowerleg", "Leg"), ("ankle", "Foot"), ("foot", "Foot"), ("toe", "ToeBase"), ("ball", "ToeBase"), ("leg", "Leg"),
        ];
        return part.iter().find(|(k, _)| low.contains(*k)).map(|(_, p)| format!("{sd}{p}"));
    }
    if low.contains("hips") || low.contains("pelvis") {
        return Some("Hips".into());
    }
    if low.contains("head") {
        return Some("Head".into());
    }
    let numbered = |word: &str| -> Option<String> {
        let n: String = low.chars().filter(char::is_ascii_digit).collect();
        let n: u32 = n.parse().unwrap_or(0);
        Some(if n == 0 { word.to_string() } else { format!("{word}{n}") })
    };
    if low.contains("neck") {
        return numbered("Neck");
    }
    if low.contains("spine") || low.contains("chest") {
        return numbered("Spine");
    }
    None
}
