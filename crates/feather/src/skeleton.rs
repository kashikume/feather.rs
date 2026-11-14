//! A simple bone system: bones are scene nodes.
//!
//! A `Skeleton` is made from a node and its descendants: every named node in that subtree is a
//! bone, found by its name. The node's transform when the skeleton is made is the bone's rest
//! pose (where the joint is, relative to the parent bone). Applying a `Pose` (from
//! `AnimationClip::sample`) sets each posed bone's transform to its rest pose, moved by the
//! pose's offset and turned by its rotation, so all its child nodes (body parts and other
//! bones) follow.
//!
//! This is rigid animation: meshes attached to a bone move with it as a whole and don't bend at
//! the joints.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};
use cgmath::Deg;

use super::animationclip::{AnimationClip, BonePose, Pose};
use super::math::Mat4;
use super::scene::Scene;

/// A bone: a scene node and its rest transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bone {
    /// Handle of the node in `Scene::nodes`.
    pub node: usize,
    /// The node's transform relative to its parent when no pose is applied.
    pub rest: Mat4,
}

/// The bones of a node subtree, by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    bones: BTreeMap<String, Bone>,
}

impl Skeleton {
    /// A skeleton of the named nodes under `root` (including `root` itself if it is named),
    /// with their current transforms as rest poses. Fails if `root` doesn't exist or two nodes
    /// of the subtree have the same name.
    pub fn from_nodes(scene: &Scene, root: usize) -> Result<Self> {
        use super::object::Object;

        if scene.get_node(root).is_none() {
            bail!("Node {root} not found");
        }
        let mut bones = BTreeMap::new();
        let mut stack = vec![root];
        while let Some(handle) = stack.pop() {
            let node = scene.get_node(handle).unwrap();
            if let Some(name) = node.get_name() {
                let bone = Bone {
                    node: handle,
                    rest: node.get_transform(),
                };
                if bones.insert(name.clone(), bone).is_some() {
                    bail!("two bones are named `{name}`");
                }
            }
            stack.extend(node.get_children());
        }
        Ok(Self { bones })
    }

    pub fn get_bone(&self, name: &str) -> Option<&Bone> {
        self.bones.get(name)
    }

    /// Names of the bones, sorted.
    pub fn bone_names(&self) -> impl Iterator<Item = &str> {
        self.bones.keys().map(String::as_str)
    }

    /// Fails if `clip` animates a bone the skeleton doesn't have, naming it.
    pub fn check(&self, clip: &AnimationClip) -> Result<()> {
        for bone in clip.bones.keys() {
            if !self.bones.contains_key(bone) {
                bail!(
                    "animation `{}` moves bone `{bone}`, which the skeleton doesn't have",
                    clip.name
                );
            }
        }
        Ok(())
    }

    /// Moves the posed bones: each gets its rest transform × the offset × the rotation (around
    /// X, then Y, then Z). Bones not in `pose` keep their current transform. Fails on a bone
    /// the skeleton doesn't have, or one whose node was removed.
    pub fn apply(&self, scene: &mut Scene, pose: &Pose) -> Result<()> {
        for (name, bone_pose) in pose {
            let bone = self
                .bones
                .get(name)
                .ok_or_else(|| anyhow!("the skeleton has no bone `{name}`"))?;
            scene
                .get_node_mut(bone.node)
                .ok_or_else(|| anyhow!("the node of bone `{name}` no longer exists"))?
                .set_transform(bone.rest * pose_matrix(bone_pose));
        }
        Ok(())
    }

    /// Puts every bone back in its rest pose.
    pub fn reset(&self, scene: &mut Scene) {
        for bone in self.bones.values() {
            if let Some(node) = scene.get_node_mut(bone.node) {
                node.set_transform(bone.rest);
            }
        }
    }
}

/// The offset × rotation (Z × Y × X) of a bone pose.
fn pose_matrix(pose: &BonePose) -> Mat4 {
    let r = pose.rotation;
    Mat4::from_translation(pose.offset)
        * Mat4::from_angle_z(Deg(r.z))
        * Mat4::from_angle_y(Deg(r.y))
        * Mat4::from_angle_x(Deg(r.x))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Vec3, Vec4};
    use cgmath::InnerSpace;

    /// Root `Body` with a child `Arm` at (1, 0, 0), holding `Hand` at (0, 0, -1).
    fn arm_scene() -> (Scene, usize, usize, usize) {
        let mut scene = Scene::new();
        let body = scene.create_root_node(Some("Body".to_string()));
        let arm = scene.create_node(Some("Arm".to_string()), body).unwrap();
        let hand = scene.create_node(Some("Hand".to_string()), arm).unwrap();
        scene
            .get_node_mut(arm)
            .unwrap()
            .set_transform(Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)));
        scene
            .get_node_mut(hand)
            .unwrap()
            .set_transform(Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0)));
        (scene, body, arm, hand)
    }

    fn hand_position(scene: &mut Scene, hand: usize) -> Vec3 {
        scene.update_nodes();
        let m = scene
            .get_node(hand)
            .unwrap()
            .get_global_transform()
            .unwrap();
        let p = m * Vec4::new(0.0, 0.0, 0.0, 1.0);
        Vec3::new(p.x, p.y, p.z)
    }

    fn pose(bone: &str, rotation: Vec3, offset: Vec3) -> Pose {
        let mut pose = Pose::new();
        pose.insert(bone.to_string(), BonePose { rotation, offset });
        pose
    }

    #[test]
    fn named_nodes_become_bones_with_their_rest_transforms() {
        let (scene, body, arm, _) = arm_scene();
        let skeleton = Skeleton::from_nodes(&scene, body).unwrap();
        assert_eq!(
            skeleton.bone_names().collect::<Vec<_>>(),
            ["Arm", "Body", "Hand"]
        );
        let bone = skeleton.get_bone("Arm").unwrap();
        assert_eq!(bone.node, arm);
        assert_eq!(bone.rest, Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn rotating_a_bone_moves_its_children() {
        let (mut scene, body, _, hand) = arm_scene();
        let skeleton = Skeleton::from_nodes(&scene, body).unwrap();
        assert!((hand_position(&mut scene, hand) - Vec3::new(1.0, 0.0, -1.0)).magnitude() < 1e-5);

        // 90 degrees around X swings the hand from below the arm to the side (+Y).
        let swing = pose("Arm", Vec3::new(90.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        skeleton.apply(&mut scene, &swing).unwrap();
        let p = hand_position(&mut scene, hand);
        assert!((p - Vec3::new(1.0, 1.0, 0.0)).magnitude() < 1e-5, "{p:?}");

        // The offset moves the joint itself; the rest position stays the base.
        let lift = pose("Arm", Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.5));
        skeleton.apply(&mut scene, &lift).unwrap();
        let p = hand_position(&mut scene, hand);
        assert!((p - Vec3::new(1.0, 0.0, -0.5)).magnitude() < 1e-5, "{p:?}");

        skeleton.reset(&mut scene);
        let p = hand_position(&mut scene, hand);
        assert!((p - Vec3::new(1.0, 0.0, -1.0)).magnitude() < 1e-5, "{p:?}");
    }

    #[test]
    fn unknown_bones_are_errors() {
        let (mut scene, body, _, _) = arm_scene();
        let skeleton = Skeleton::from_nodes(&scene, body).unwrap();
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let error = skeleton
            .apply(&mut scene, &pose("Leg", zero, zero))
            .unwrap_err();
        assert!(error.to_string().contains("Leg"));

        let clip = AnimationClip::from_json(
            r#"{ "name": "Kick", "duration": 1, "bones": { "Leg": { "rotation": [[0, [10, 0, 0]]] } } }"#,
        )
        .unwrap();
        let error = skeleton.check(&clip).unwrap_err();
        assert!(error.to_string().contains("bone `Leg`"), "{error}");
    }

    #[test]
    fn duplicate_bone_names_fail() {
        let (mut scene, body, arm, _) = arm_scene();
        scene.create_node(Some("Hand".to_string()), arm).unwrap();
        assert!(Skeleton::from_nodes(&scene, body).is_err());
        assert!(Skeleton::from_nodes(&scene, 99).is_err());
    }
}
