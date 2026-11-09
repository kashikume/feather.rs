//! Keyframe animation of skeletons (see `skeleton`).
//!
//! An `AnimationClip` gives, for each animated bone, keyframes of a rotation (degrees around X,
//! Y and Z, applied in that order, like scene file transforms) and of an offset (a translation
//! added to the bone's rest position). `AnimationClip::sample` interpolates them into a `Pose`
//! at any time; `Skeleton::apply` moves the bones to it.
//!
//! Clips are usually loaded from JSON:
//!
//! ```json
//! {
//!   "name": "Walk",
//!   "duration": 1.0,
//!   "bones": {
//!     "LeftHip":  { "rotation": [[0.0, [25, 0, 0]], [0.5, [-25, 0, 0]]] },
//!     "Hips":     { "offset":   [[0.0, [0, 0, 0]], [0.25, [0, 0, 0.03]]] }
//!   }
//! }
//! ```
//!
//! Each key is `[time, [x, y, z]]`, with times in seconds from 0 up to (not including)
//! `duration`, in increasing order. Clips loop: after the last key, values go smoothly back to
//! the first one, which comes again at `duration`. Interpolation is a looping Catmull-Rom spline,
//! so motion has no sudden changes of speed at the keys.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::assetsprovider::AssetsProvider;
use super::math::Vec3;

/// A value at a time, in a channel of an `AnimationClip`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// Seconds from the start of the clip.
    pub time: f32,
    pub value: Vec3,
}

/// How one bone moves: keys of its rotation and of its offset. A channel without keys keeps
/// the rest value (no rotation, no offset).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneChannel {
    /// Degrees around X, Y and Z, applied in that order.
    pub rotation: Vec<Key>,
    /// Translation added to the bone's rest position.
    pub offset: Vec<Key>,
}

/// Where a bone is at one moment, relative to its rest pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    /// Degrees around X, Y and Z, applied in that order.
    pub rotation: Vec3,
    pub offset: Vec3,
}

impl Default for BonePose {
    /// The rest pose: no rotation, no offset.
    fn default() -> Self {
        Self {
            rotation: Vec3::new(0.0, 0.0, 0.0),
            offset: Vec3::new(0.0, 0.0, 0.0),
        }
    }
}

/// Poses of bones, by bone name.
pub type Pose = BTreeMap<String, BonePose>;

/// A looping animation of named bones.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    /// Length of one loop, in seconds.
    pub duration: f32,
    pub bones: BTreeMap<String, BoneChannel>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipFile {
    name: String,
    duration: f32,
    bones: BTreeMap<String, ChannelFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelFile {
    #[serde(default)]
    rotation: Vec<(f32, [f32; 3])>,
    #[serde(default)]
    offset: Vec<(f32, [f32; 3])>,
}

impl AnimationClip {
    /// Creates a clip, checking that `duration` is positive and that every channel's keys are
    /// in increasing order within `0..duration`.
    pub fn new(
        name: impl Into<String>,
        duration: f32,
        bones: BTreeMap<String, BoneChannel>,
    ) -> Result<Self> {
        let name = name.into();
        if !(duration > 0.0) {
            bail!("animation `{name}`: duration must be positive, got {duration}");
        }
        for (bone, channel) in &bones {
            for (what, keys) in [("rotation", &channel.rotation), ("offset", &channel.offset)] {
                let mut previous = None;
                for key in keys {
                    if !(0.0..duration).contains(&key.time) {
                        bail!(
                            "animation `{name}`: bone `{bone}`: {what} key at {} is outside 0..{duration}",
                            key.time
                        );
                    }
                    if previous.is_some_and(|p| key.time <= p) {
                        bail!(
                            "animation `{name}`: bone `{bone}`: {what} keys are not in increasing order of time"
                        );
                    }
                    previous = Some(key.time);
                }
            }
        }
        Ok(Self {
            name,
            duration,
            bones,
        })
    }

    /// Reads a clip from JSON; see the module documentation for the format.
    pub fn from_json(json: &str) -> Result<Self> {
        let file: ClipFile = serde_json::from_str(json)?;
        let keys = |list: &[(f32, [f32; 3])]| {
            list.iter()
                .map(|&(time, value)| Key {
                    time,
                    value: Vec3::from(value),
                })
                .collect()
        };
        let bones = file
            .bones
            .iter()
            .map(|(bone, channel)| {
                let channel = BoneChannel {
                    rotation: keys(&channel.rotation),
                    offset: keys(&channel.offset),
                };
                (bone.clone(), channel)
            })
            .collect();
        Self::new(file.name, file.duration, bones)
    }

    /// Loads a clip from the JSON asset `asset`.
    pub fn load(assets: &dyn AssetsProvider, asset: &str) -> Result<Self> {
        let bytes = assets.get_asset(asset)?;
        let json = std::str::from_utf8(&bytes).with_context(|| format!("{asset} is not UTF-8"))?;
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        Self::from_json(json).with_context(|| format!("Cannot load animation {asset}"))
    }

    /// The pose of every animated bone at `time` seconds; times past `duration` (or before 0)
    /// wrap around, so the clip loops.
    pub fn sample(&self, time: f32) -> Pose {
        let time = time.rem_euclid(self.duration);
        self.bones
            .iter()
            .map(|(bone, channel)| {
                let pose = BonePose {
                    rotation: sample_keys(&channel.rotation, time, self.duration),
                    offset: sample_keys(&channel.offset, time, self.duration),
                };
                (bone.clone(), pose)
            })
            .collect()
    }
}

/// The value of looping `keys` at `time` (in `0..duration`), by Catmull-Rom interpolation.
fn sample_keys(keys: &[Key], time: f32, duration: f32) -> Vec3 {
    match keys.len() {
        0 => return Vec3::new(0.0, 0.0, 0.0),
        1 => return keys[0].value,
        _ => {}
    }
    let n = keys.len();
    // The key at or before `time`; before the first key, the last key of the previous loop.
    let i = keys.iter().rposition(|k| k.time <= time).unwrap_or(n - 1);
    let key = |k: isize| {
        // Keys of the loops before and after, with their times shifted by `duration`.
        let loops = k.div_euclid(n as isize);
        let key = keys[k.rem_euclid(n as isize) as usize];
        (key.time + loops as f32 * duration, key.value)
    };
    let i = i as isize + if time < keys[0].time { -(n as isize) } else { 0 };
    let (t0, p0) = key(i - 1);
    let (t1, p1) = key(i);
    let (t2, p2) = key(i + 1);
    let (t3, p3) = key(i + 2);
    let span = t2 - t1;
    let s = (time - t1) / span;
    // Tangents scaled to the span, so unevenly spaced keys stay smooth.
    let m1 = (p2 - p0) / (t2 - t0) * span;
    let m2 = (p3 - p1) / (t3 - t1) * span;
    let (s2, s3) = (s * s, s * s * s);
    p1 * (2.0 * s3 - 3.0 * s2 + 1.0)
        + m1 * (s3 - 2.0 * s2 + s)
        + p2 * (-2.0 * s3 + 3.0 * s2)
        + m2 * (s3 - s2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgmath::InnerSpace;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).magnitude() < 1e-4
    }

    fn keys(list: &[(f32, f32)]) -> Vec<Key> {
        list.iter()
            .map(|&(time, x)| Key {
                time,
                value: Vec3::new(x, 0.0, 0.0),
            })
            .collect()
    }

    fn clip(rotation: &[(f32, f32)], duration: f32) -> AnimationClip {
        let mut bones = BTreeMap::new();
        bones.insert(
            "Bone".to_string(),
            BoneChannel {
                rotation: keys(rotation),
                offset: Vec::new(),
            },
        );
        AnimationClip::new("Test", duration, bones).unwrap()
    }

    fn x_at(clip: &AnimationClip, time: f32) -> f32 {
        clip.sample(time)["Bone"].rotation.x
    }

    #[test]
    fn keys_are_hit_exactly() {
        let clip = clip(&[(0.0, 10.0), (0.3, -5.0), (0.6, 40.0)], 1.0);
        assert!((x_at(&clip, 0.0) - 10.0).abs() < 1e-4);
        assert!((x_at(&clip, 0.3) + 5.0).abs() < 1e-4);
        assert!((x_at(&clip, 0.6) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn values_between_keys_are_smooth_and_loop() {
        // A symmetric swing: the midpoints are 0 and the motion is smooth.
        let clip = clip(&[(0.0, 30.0), (0.5, -30.0)], 1.0);
        assert!(x_at(&clip, 0.25).abs() < 1e-4);
        assert!(x_at(&clip, 0.75).abs() < 1e-4);
        // Looping: the same values one loop later and earlier.
        for t in [0.1, 0.4, 0.8] {
            assert!((x_at(&clip, t) - x_at(&clip, t + 1.0)).abs() < 1e-3);
            assert!((x_at(&clip, t) - x_at(&clip, t - 3.0)).abs() < 1e-3);
        }
        // No jump where the loop closes.
        assert!((x_at(&clip, 0.9999) - x_at(&clip, 0.0)).abs() < 0.05);
    }

    #[test]
    fn time_before_the_first_key_comes_from_the_last() {
        let clip = clip(&[(0.2, 0.0), (0.6, 10.0)], 1.0);
        // Between 0.6 and 1.2 (= 0.2 of the next loop) the value goes from 10 back to 0.
        let early = x_at(&clip, 0.1);
        assert!(early > 0.0 && early < 10.0, "{early}");
        assert!((x_at(&clip, 0.1) - x_at(&clip, 1.1)).abs() < 1e-4);
    }

    #[test]
    fn empty_and_single_key_channels() {
        let mut bones = BTreeMap::new();
        bones.insert(
            "A".to_string(),
            BoneChannel {
                rotation: Vec::new(),
                offset: vec![Key {
                    time: 0.4,
                    value: Vec3::new(1.0, 2.0, 3.0),
                }],
            },
        );
        let clip = AnimationClip::new("Test", 2.0, bones).unwrap();
        let pose = clip.sample(1.3);
        assert!(close(pose["A"].rotation, Vec3::new(0.0, 0.0, 0.0)));
        assert!(close(pose["A"].offset, Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn from_json_reads_rotation_and_offset() {
        let clip = AnimationClip::from_json(
            r#"{
                "name": "Walk", "duration": 1.0,
                "bones": {
                    "Hip": { "rotation": [[0.0, [25, 0, 0]], [0.5, [-25, 0, 0]]] },
                    "Hips": { "offset": [[0.25, [0, 0, 0.03]]] }
                }
            }"#,
        )
        .unwrap();
        assert_eq!(clip.name, "Walk");
        assert_eq!(clip.bones.len(), 2);
        let pose = clip.sample(0.5);
        assert!(close(pose["Hip"].rotation, Vec3::new(-25.0, 0.0, 0.0)));
        assert!(close(pose["Hips"].offset, Vec3::new(0.0, 0.0, 0.03)));
    }

    #[test]
    fn invalid_clips_fail() {
        for (json, expected) in [
            (r#"{ "name": "A", "duration": 0, "bones": {} }"#, "duration must be positive"),
            (
                r#"{ "name": "A", "duration": 1, "bones": { "B": { "rotation": [[1.0, [0, 0, 0]]] } } }"#,
                "outside 0..1",
            ),
            (
                r#"{ "name": "A", "duration": 1, "bones": { "B": { "offset": [[0.5, [0, 0, 0]], [0.2, [0, 0, 0]]] } } }"#,
                "increasing order",
            ),
            (r#"{ "name": "A", "duration": 1, "bones": { "B": { "scale": [] } } }"#, "unknown field"),
        ] {
            let error = format!("{:#}", AnimationClip::from_json(json).unwrap_err());
            assert!(error.contains(expected), "expected `{expected}` in: {error}");
        }
    }
}
