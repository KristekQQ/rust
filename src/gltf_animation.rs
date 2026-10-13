//! Animation sampling and CPU morph/skin deformation; no browser or GPU state.
use crate::gltf_model::*;
use glam::{Mat3, Mat4, Quat, Vec3};
use std::sync::Arc;

pub fn sample_channel(channel: &AnimationChannel, time: f32) -> Result<Vec<f32>, String> {
    if !time.is_finite() || channel.times.is_empty() || channel.components == 0 {
        return Err("Invalid animation sample".into());
    }
    let n = channel.components;
    let cubic = channel.interpolation == Interpolation::CubicSpline;
    let width = n
        .checked_mul(if cubic { 3 } else { 1 })
        .ok_or("Animation width overflow")?;
    if channel.values.len()
        != channel
            .times
            .len()
            .checked_mul(width)
            .ok_or("Animation size overflow")?
    {
        return Err("Invalid animation values length".into());
    }
    let value = |key: usize| {
        &channel.values
            [key * width + if cubic { n } else { 0 }..key * width + if cubic { 2 * n } else { n }]
    };
    let upper = channel.times.partition_point(|&t| t <= time);
    let lower = upper.saturating_sub(1);
    if upper == 0 {
        return Ok(value(0).to_vec());
    }
    if upper >= channel.times.len() || channel.interpolation == Interpolation::Step {
        return Ok(value(lower).to_vec());
    }
    let dt = channel.times[upper] - channel.times[lower];
    if !dt.is_finite() || dt <= 0.0 {
        return Err("Animation timestamps must increase".into());
    }
    let t = (time - channel.times[lower]) / dt;
    let a = value(lower);
    let b = value(upper);
    if channel.property == AnimationProperty::Rotation && !cubic {
        if n != 4 {
            return Err("Rotation channel must have four components".into());
        }
        let qa = Quat::from_array([a[0], a[1], a[2], a[3]]).normalize();
        let qb = Quat::from_array([b[0], b[1], b[2], b[3]]).normalize();
        return Ok(qa.slerp(qb, t).normalize().to_array().to_vec());
    }
    let mut sampled = vec![0.; n];
    for i in 0..n {
        sampled[i] = if cubic {
            let out = channel.values[lower * width + 2 * n + i];
            let input = channel.values[upper * width + i];
            let t2 = t * t;
            let t3 = t2 * t;
            (2. * t3 - 3. * t2 + 1.) * a[i]
                + (t3 - 2. * t2 + t) * dt * out
                + (-2. * t3 + 3. * t2) * b[i]
                + (t3 - t2) * dt * input
        } else {
            a[i] * (1. - t) + b[i] * t
        };
    }
    if sampled.iter().any(|v| !v.is_finite()) {
        return Err("Animation interpolation overflow".into());
    }
    if channel.property == AnimationProperty::Rotation {
        if n != 4 {
            return Err("Rotation channel must have four components".into());
        }
        let q = Quat::from_array([sampled[0], sampled[1], sampled[2], sampled[3]]);
        if q.length_squared() <= 0.0 {
            return Err("Animation produced a zero quaternion".into());
        }
        sampled = q.normalize().to_array().to_vec();
    }
    Ok(sampled)
}

pub fn default_pose(asset: &ModelAsset, scene: usize) -> Result<ModelPose, String> {
    if scene >= asset.scenes.len() {
        return Err("Model scene index is out of range".into());
    }
    let mut pose = rest_pose(asset)?;
    let mut pending = asset.scenes[scene].nodes.clone();
    let mut seen = vec![false; asset.nodes.len()];
    while let Some(node) = pending.pop() {
        if node >= asset.nodes.len() || seen[node] {
            return Err("Invalid/cyclic scene graph".into());
        }
        seen[node] = true;
        pose.active_nodes.push(node);
        pending.extend_from_slice(&asset.nodes[node].children);
    }
    Ok(pose)
}

/// Default transforms for every node, including assets with no scenes.
const MAX_POSE_BYTES: usize = 64 * 1024 * 1024;

// Count expanded per-node data, not just the shared mesh/skin source arrays.
fn validate_pose_budget(asset: &ModelAsset) -> Result<(), String> {
    let base = std::mem::size_of::<NodeTrs>()
        + std::mem::size_of::<Mat4>()
        + 2 * std::mem::size_of::<Vec<f32>>()
        + std::mem::size_of::<usize>()
        + 1;
    let mut bytes = asset
        .nodes
        .len()
        .checked_mul(base)
        .ok_or("Pose size overflow")?;
    for node in &asset.nodes {
        let mesh = node
            .mesh
            .map(|i| asset.meshes.get(i).ok_or("Invalid mesh index"))
            .transpose()?;
        let weights = node
            .weights
            .as_ref()
            .map_or_else(|| mesh.map_or(0, |m| m.weights.len()), Vec::len);
        bytes = bytes
            .checked_add(weights.checked_mul(4).ok_or("Pose size overflow")?)
            .ok_or("Pose size overflow")?;
        if let Some(index) = node.skin {
            let skin = asset.skins.get(index).ok_or("Invalid skin index")?;
            if skin.joints.len() != skin.inverse_bind_matrices.len() {
                return Err("Skin inverse bind matrix count mismatch".into());
            }
            bytes = bytes
                .checked_add(
                    skin.joints
                        .len()
                        .checked_mul(std::mem::size_of::<Mat4>())
                        .ok_or("Pose size overflow")?,
                )
                .ok_or("Pose size overflow")?;
        }
        if bytes > MAX_POSE_BYTES {
            return Err("Expanded model pose exceeds memory limit".into());
        }
    }
    if bytes > MAX_POSE_BYTES {
        return Err("Expanded model pose exceeds memory limit".into());
    }
    Ok(())
}

pub fn rest_pose(asset: &ModelAsset) -> Result<ModelPose, String> {
    validate_pose_budget(asset)?;
    let mut pose = ModelPose {
        local_trs: asset.nodes.iter().map(|n| n.trs).collect(),
        world_matrices: vec![Mat4::IDENTITY; asset.nodes.len()],
        node_weights: asset
            .nodes
            .iter()
            .map(|node| {
                node.weights.clone().unwrap_or_else(|| {
                    node.mesh
                        .map(|m| asset.meshes[m].weights.clone())
                        .unwrap_or_default()
                })
            })
            .collect(),
        skin_matrices: vec![vec![]; asset.nodes.len()],
        skin_world_space: vec![false; asset.nodes.len()],
        active_nodes: vec![],
    };
    recompute_world(asset, &mut pose)?;
    Ok(pose)
}

pub fn recompute_world(asset: &ModelAsset, pose: &mut ModelPose) -> Result<(), String> {
    validate_pose_budget(asset)?;
    let n = asset.nodes.len();
    if pose.local_trs.len() != n
        || pose.world_matrices.len() != n
        || pose.node_weights.len() != n
        || pose.skin_matrices.len() != n
        || pose.skin_world_space.len() != n
    {
        return Err("Model pose node count mismatch".into());
    }
    let mut parents = vec![None; asset.nodes.len()];
    for (i, node) in asset.nodes.iter().enumerate() {
        for &child in &node.children {
            if child >= parents.len() || parents[child].replace(i).is_some() {
                return Err("Model graph has multiple parents or invalid child".into());
            }
        }
    }
    let mut stack: Vec<_> = parents
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_none())
        .map(|(i, _)| (i, Mat4::IDENTITY))
        .collect();
    let mut visited = 0;
    while let Some((i, parent)) = stack.pop() {
        let node = &asset.nodes[i];
        let local = node.matrix.unwrap_or_else(|| pose.local_trs[i].matrix());
        let world = parent * local;
        if !world.is_finite() {
            return Err("Non-finite animated model matrix".into());
        }
        pose.world_matrices[i] = world;
        visited += 1;
        stack.extend(node.children.iter().map(|&j| (j, world)));
    }
    if visited != asset.nodes.len() {
        return Err("Model node graph contains a cycle".into());
    }
    for (i, node) in asset.nodes.iter().enumerate() {
        pose.skin_matrices[i].clear();
        if let Some(skin_index) = node.skin {
            let skin = asset.skins.get(skin_index).ok_or("Invalid skin index")?;
            let inverse = pose.world_matrices[i].inverse();
            pose.skin_world_space[i] = !inverse.is_finite();
            let inverse = if pose.skin_world_space[i] {
                Mat4::IDENTITY
            } else {
                inverse
            };
            for (&joint, ibm) in skin.joints.iter().zip(&skin.inverse_bind_matrices) {
                let world = pose.world_matrices.get(joint).ok_or("Invalid skin joint")?;
                let matrix = inverse * *world * *ibm;
                if !matrix.is_finite() {
                    return Err("Non-finite skin matrix".into());
                }
                pose.skin_matrices[i].push(matrix);
            }
        }
    }
    Ok(())
}

pub fn apply_clip(
    asset: &ModelAsset,
    pose: &mut ModelPose,
    clip: usize,
    time: f32,
) -> Result<(), String> {
    let clip = asset
        .animations
        .get(clip)
        .ok_or("Animation clip index out of range")?;
    let original = pose;
    let mut candidate = original.clone();
    let pose = &mut candidate;
    // Unanimated properties preserve the asset's default pose.
    for (i, node) in asset.nodes.iter().enumerate() {
        pose.local_trs[i] = node.trs;
        pose.node_weights[i] = node.weights.clone().unwrap_or_else(|| {
            node.mesh
                .map(|m| asset.meshes[m].weights.clone())
                .unwrap_or_default()
        });
    }
    for channel in &clip.channels {
        let sampled = sample_channel(channel, time)?;
        let trs = pose
            .local_trs
            .get_mut(channel.node)
            .ok_or("Animation node index out of range")?;
        match channel.property {
            AnimationProperty::Translation => trs.translation = Vec3::from_slice(&sampled),
            AnimationProperty::Scale => trs.scale = Vec3::from_slice(&sampled),
            AnimationProperty::Rotation => {
                trs.rotation =
                    Quat::from_array([sampled[0], sampled[1], sampled[2], sampled[3]]).normalize()
            }
            AnimationProperty::Weights => pose.node_weights[channel.node] = sampled,
        }
    }
    recompute_world(asset, pose)?;
    *original = candidate;
    Ok(())
}

impl ModelInstance {
    pub fn new(asset: Arc<ModelAsset>, scene: Option<usize>) -> Result<Self, String> {
        let scene = scene.or(asset.default_scene).unwrap_or(0);
        let pose = default_pose(&asset, scene)?;
        Ok(Self {
            asset,
            pose,
            playback: None,
            scene,
            revision: 1,
            last_error: None,
            morph_overrides: Default::default(),
        })
    }
    pub fn play(&mut self, clip: usize, looping: bool) -> Result<(), String> {
        if clip >= self.asset.animations.len() {
            return Err("Animation clip index out of range".into());
        }
        apply_clip(&self.asset, &mut self.pose, clip, 0.)?;
        self.apply_morph_overrides(clip);
        self.playback = Some(AnimationPlayback {
            clip,
            time: 0.,
            speed: 1.,
            looping,
            playing: true,
        });
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), String> {
        let pose = default_pose(&self.asset, self.scene)?;
        self.playback = None;
        self.pose = pose;
        self.morph_overrides.clear();
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn seek(&mut self, time: f32) -> Result<(), String> {
        if !time.is_finite() || time < 0. {
            return Err("Animation time must be finite and nonnegative".into());
        }
        let playback = self
            .playback
            .as_mut()
            .ok_or("Model has no active animation")?;
        let duration = self.asset.animations[playback.clip].duration;
        let target = time.min(duration);
        let clip = playback.clip;
        apply_clip(&self.asset, &mut self.pose, playback.clip, target)?;
        playback.time = target;
        self.apply_morph_overrides(clip);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn update(&mut self, delta: f32) -> Result<bool, String> {
        if !delta.is_finite() || delta < 0. {
            return Err("Invalid animation delta".into());
        }
        let Some(mut playback) = self.playback.clone() else {
            return Ok(false);
        };
        if !playback.playing || delta == 0. || playback.speed == 0. {
            return Ok(false);
        }
        let duration = self.asset.animations[playback.clip].duration;
        let advanced = playback.time + delta * playback.speed;
        if !advanced.is_finite() {
            return Err("Animation time overflow".into());
        }
        playback.time = if duration <= 0. {
            0.
        } else if playback.looping {
            advanced.rem_euclid(duration)
        } else {
            advanced.clamp(0., duration)
        };
        if !playback.looping && (advanced >= duration || advanced < 0.) {
            playback.playing = false;
        }
        apply_clip(&self.asset, &mut self.pose, playback.clip, playback.time)?;
        self.apply_morph_overrides(playback.clip);
        self.playback = Some(playback);
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub fn select_scene(&mut self, scene: usize) -> Result<(), String> {
        self.pose = default_pose(&self.asset, scene)?;
        self.scene = scene;
        self.playback = None;
        self.morph_overrides.clear();
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn set_morph_weights(&mut self, node: usize, weights: &[f32]) -> Result<(), String> {
        let model_node = self
            .asset
            .nodes
            .get(node)
            .ok_or("Morph node out of range")?;
        let mesh = model_node.mesh.ok_or("Morph node has no mesh")?;
        if weights.len() != self.asset.meshes[mesh].weights.len()
            || weights.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid morph weights".into());
        }
        self.pose.node_weights[node] = weights.to_vec();
        self.morph_overrides.insert(node, weights.to_vec());
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    fn apply_morph_overrides(&mut self, clip: usize) {
        let animated: std::collections::HashSet<_> = self.asset.animations[clip]
            .channels
            .iter()
            .filter(|channel| channel.property == AnimationProperty::Weights)
            .map(|channel| channel.node)
            .collect();
        for (&node, weights) in &self.morph_overrides {
            if !animated.contains(&node) {
                self.pose.node_weights[node] = weights.clone();
            }
        }
    }
}

/// CPU local-space deformation: morph targets first, then every joint influence.
pub fn deformed_vertices(
    asset: &ModelAsset,
    pose: &ModelPose,
    node: usize,
    primitive: usize,
) -> Result<Vec<ModelVertex>, String> {
    let p = asset
        .primitives
        .get(primitive)
        .ok_or("Primitive index out of range")?;
    let model_node = asset.nodes.get(node).ok_or("Model node out of range")?;
    let weights = pose
        .node_weights
        .get(node)
        .ok_or("Pose node out of range")?;
    let mut vertices = asset
        .vertices
        .get(p.vertices.clone())
        .ok_or("Invalid primitive vertex range")?
        .to_vec();
    for (i, vertex) in vertices.iter_mut().enumerate() {
        let mut position = Vec3::from_array(vertex.position);
        let mut normal = Vec3::from_array(vertex.normal);
        let mut tangent = Vec3::from_slice(&vertex.tangent);
        for (j, target) in p.morph_targets.iter().enumerate() {
            let weight = weights.get(j).copied().unwrap_or(0.);
            if weight == 0. {
                continue;
            }
            if let Some(delta) = target.positions.get(i) {
                position += Vec3::from_array(*delta) * weight;
            }
            if let Some(delta) = target.normals.get(i) {
                normal += Vec3::from_array(*delta) * weight;
            }
            if let Some(delta) = target.tangents.get(i) {
                tangent += Vec3::from_array(*delta) * weight;
            }
        }
        if model_node.skin.is_some() && !vertex.influences.is_empty() {
            let matrices = &pose.skin_matrices[node];
            let mut skin = Mat4::ZERO;
            for &(joint, weight) in &vertex.influences {
                if weight == 0. {
                    continue;
                }
                skin += *matrices
                    .get(joint as usize)
                    .ok_or("Vertex joint index out of skin range")?
                    * weight;
            }
            position = skin.transform_point3(position);
            let linear = Mat3::from_mat4(skin);
            let inverse = linear.inverse().transpose();
            normal = if inverse.is_finite() {
                inverse * normal
            } else {
                linear * normal
            };
            tangent = linear * tangent;
            if linear.determinant() < 0. {
                vertex.tangent[3] = -vertex.tangent[3];
            }
        }
        if !position.is_finite() || !normal.is_finite() || !tangent.is_finite() {
            return Err("Model deformation overflow".into());
        }
        normal = normal.try_normalize().unwrap_or(Vec3::Y);
        tangent = (tangent - normal * normal.dot(tangent))
            .try_normalize()
            .unwrap_or(Vec3::ZERO);
        vertex.position = position.to_array();
        vertex.normal = normal.to_array();
        vertex.tangent = [tangent.x, tangent.y, tangent.z, vertex.tangent[3]];
    }
    Ok(vertices)
}

pub fn model_bounds(asset: &ModelAsset, pose: &ModelPose) -> Result<(Vec3, Vec3), String> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &node in &pose.active_nodes {
        if let Some(mesh) = asset.nodes[node].mesh {
            for &primitive in &asset.meshes[mesh].primitives {
                for vertex in deformed_vertices(asset, pose, node, primitive)? {
                    let point = node_render_matrix(pose, node)
                        .transform_point3(Vec3::from_array(vertex.position));
                    if !point.is_finite() {
                        return Err("Model bounds overflow".into());
                    }
                    min = min.min(point);
                    max = max.max(point);
                }
            }
        }
    }
    if !min.is_finite() {
        return Ok((Vec3::ZERO, Vec3::ZERO));
    }
    Ok((min, max))
}

/// Matrix to apply AFTER CPU deformation (before the scene model root transform).
pub fn node_render_matrix(pose: &ModelPose, node: usize) -> Mat4 {
    if pose.skin_world_space[node] {
        Mat4::IDENTITY
    } else {
        pose.world_matrices[node]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(
        property: AnimationProperty,
        interpolation: Interpolation,
        values: Vec<f32>,
        components: usize,
    ) -> AnimationChannel {
        AnimationChannel {
            node: 0,
            property,
            interpolation,
            times: vec![0., 2.],
            values,
            components,
        }
    }
    #[test]
    fn step_linear_and_cubic_tangents_scale_by_key_duration() {
        let linear = channel(
            AnimationProperty::Weights,
            Interpolation::Linear,
            vec![0., 4.],
            1,
        );
        assert_eq!(sample_channel(&linear, 1.).unwrap(), [2.]);
        assert_eq!(sample_channel(&linear, -1.).unwrap(), [0.]);
        assert_eq!(sample_channel(&linear, 3.).unwrap(), [4.]);
        let step = AnimationChannel {
            interpolation: Interpolation::Step,
            ..linear
        };
        assert_eq!(sample_channel(&step, 1.9).unwrap(), [0.]);
        assert_eq!(sample_channel(&step, 2.).unwrap(), [4.]);
        let cubic = channel(
            AnimationProperty::Weights,
            Interpolation::CubicSpline,
            vec![0., 0., 2., 0., 0., 0.],
            1,
        );
        assert_eq!(sample_channel(&cubic, 1.).unwrap(), [0.5]);
    }
    #[test]
    fn rotations_take_short_path_and_cubic_rotation_normalizes() {
        let linear = channel(
            AnimationProperty::Rotation,
            Interpolation::Linear,
            vec![0., 0., 0., 1., 0., 0., 0., -1.],
            4,
        );
        let q = sample_channel(&linear, 1.).unwrap();
        assert!((q[3].abs() - 1.).abs() < 1e-6);
        let cubic = channel(
            AnimationProperty::Rotation,
            Interpolation::CubicSpline,
            vec![
                0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0.,
                0., 0., 0.,
            ],
            4,
        );
        let q = sample_channel(&cubic, 1.).unwrap();
        assert!((q.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 1e-6);
    }
}
