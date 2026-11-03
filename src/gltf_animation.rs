//! Animation sampling and CPU morph/skin deformation; no browser or GPU state.
use crate::gltf_model::*;
use glam::{Mat3, Mat4, Quat, Vec3};
use std::sync::Arc;

pub fn sample_channel(channel: &AnimationChannel, time: f32) -> Result<Vec<f32>, String> {
    let mut sampled = Vec::new();
    sample_channel_into(channel, time, &mut sampled)?;
    Ok(sampled)
}
fn sample_channel_into(
    channel: &AnimationChannel,
    time: f32,
    sampled: &mut Vec<f32>,
) -> Result<(), String> {
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
    sampled.resize(n, 0.);
    let value = |key: usize| {
        &channel.values
            [key * width + if cubic { n } else { 0 }..key * width + if cubic { 2 * n } else { n }]
    };
    let upper = channel.times.partition_point(|&t| t <= time);
    let lower = upper.saturating_sub(1);
    if upper == 0 {
        sampled.copy_from_slice(value(0));
        return Ok(());
    }
    if upper >= channel.times.len() || channel.interpolation == Interpolation::Step {
        sampled.copy_from_slice(value(lower));
        return Ok(());
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
        sampled.copy_from_slice(&qa.slerp(qb, t).normalize().to_array());
        return Ok(());
    }
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
        sampled.copy_from_slice(&q.normalize().to_array());
    }
    Ok(())
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
pub(crate) fn validate_pose_budget(asset: &ModelAsset) -> Result<(), String> {
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
    recompute_world_reuse(asset, pose, &mut Vec::new(), &mut Vec::new())
}
fn recompute_world_reuse(
    asset: &ModelAsset,
    pose: &mut ModelPose,
    parents: &mut Vec<Option<usize>>,
    stack: &mut Vec<(usize, Mat4)>,
) -> Result<(), String> {
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
    parents.resize(asset.nodes.len(), None);
    parents.fill(None);
    for (i, node) in asset.nodes.iter().enumerate() {
        for &child in &node.children {
            if child >= parents.len() || parents[child].replace(i).is_some() {
                return Err("Model graph has multiple parents or invalid child".into());
            }
        }
    }
    stack.clear();
    stack.extend(
        parents
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_none())
            .map(|(i, _)| (i, Mat4::IDENTITY)),
    );
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
    let mut candidate = pose.clone();
    apply_clip_reuse(
        asset,
        &mut candidate,
        clip,
        time,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
    )?;
    *pose = candidate;
    Ok(())
}
fn apply_clip_reuse(
    asset: &ModelAsset,
    pose: &mut ModelPose,
    clip: usize,
    time: f32,
    sampled: &mut Vec<f32>,
    parents: &mut Vec<Option<usize>>,
    stack: &mut Vec<(usize, Mat4)>,
) -> Result<(), String> {
    let clip = asset
        .animations
        .get(clip)
        .ok_or("Animation clip index out of range")?;
    if pose.local_trs.len() != asset.nodes.len() || pose.node_weights.len() != asset.nodes.len() {
        return Err("Model pose node count mismatch".into());
    }
    for (i, node) in asset.nodes.iter().enumerate() {
        pose.local_trs[i] = node.trs;
        if let Some(weights) = node
            .weights
            .as_ref()
            .or_else(|| node.mesh.map(|m| &asset.meshes[m].weights))
        {
            pose.node_weights[i].clone_from(weights);
        } else {
            pose.node_weights[i].clear();
        }
    }
    for channel in &clip.channels {
        sample_channel_into(channel, time, sampled)?;
        let trs = pose
            .local_trs
            .get_mut(channel.node)
            .ok_or("Animation node index out of range")?;
        match channel.property {
            AnimationProperty::Translation => trs.translation = Vec3::from_slice(sampled),
            AnimationProperty::Scale => trs.scale = Vec3::from_slice(sampled),
            AnimationProperty::Rotation => {
                trs.rotation =
                    Quat::from_array([sampled[0], sampled[1], sampled[2], sampled[3]]).normalize()
            }
            AnimationProperty::Weights => pose.node_weights[channel.node].clone_from(sampled),
        }
    }
    recompute_world_reuse(asset, pose, parents, stack)
}

/// Never-animated instances pay only one pointer. Paused/stopped hot instances
/// retain bounded scratch for resume. Clones start empty so cold command
/// transactions never duplicate retained buffers.
#[derive(Debug, Default)]
pub(crate) struct AnimationWorkspace {
    scratch: Option<Box<AnimationScratch>>,
}
impl Clone for AnimationWorkspace {
    fn clone(&self) -> Self {
        Self::default()
    }
}
#[derive(Debug)]
struct AnimationScratch {
    pose: ModelPose,
    sampled: Vec<f32>,
    parents: Vec<Option<usize>>,
    stack: Vec<(usize, Mat4)>,
    next_playback: Option<AnimationPlayback>,
    animated_weights: Vec<bool>,
}
impl AnimationWorkspace {
    pub(crate) fn retained_bytes(&self) -> usize {
        let Some(s) = &self.scratch else {
            return 0;
        };
        let p = &s.pose;
        std::mem::size_of::<AnimationScratch>()
            + p.local_trs.capacity() * std::mem::size_of::<NodeTrs>()
            + p.world_matrices.capacity() * std::mem::size_of::<Mat4>()
            + p.node_weights.capacity() * std::mem::size_of::<Vec<f32>>()
            + p.node_weights
                .iter()
                .map(|v| v.capacity() * 4)
                .sum::<usize>()
            + p.skin_matrices.capacity() * std::mem::size_of::<Vec<Mat4>>()
            + p.skin_matrices
                .iter()
                .map(|v| v.capacity() * std::mem::size_of::<Mat4>())
                .sum::<usize>()
            + p.skin_world_space.capacity()
            + p.active_nodes.capacity() * std::mem::size_of::<usize>()
            + s.animated_weights.capacity()
            + s.sampled.capacity() * 4
            + s.parents.capacity() * std::mem::size_of::<Option<usize>>()
            + s.stack.capacity() * std::mem::size_of::<(usize, Mat4)>()
    }
    fn limit(&mut self) {
        // Large valid assets keep transient evaluation, never unbounded retained
        // scratch. This excludes shared assets and the already-published pose.
        if self.retained_bytes() > 2 * 1024 * 1024 {
            self.scratch = None;
        }
    }
}
fn copy_pose_into(source: &ModelPose, target: &mut ModelPose) {
    target.local_trs.clone_from(&source.local_trs);
    target.world_matrices.clone_from(&source.world_matrices);
    target.node_weights.clone_from(&source.node_weights);
    target.skin_matrices.clone_from(&source.skin_matrices);
    target.skin_world_space.clone_from(&source.skin_world_space);
    target.active_nodes.clone_from(&source.active_nodes);
}
fn apply_morph_overrides_into(
    asset: &ModelAsset,
    overrides: &std::collections::BTreeMap<usize, Vec<f32>>,
    clip: usize,
    pose: &mut ModelPose,
    animated: &mut Vec<bool>,
) {
    if overrides.is_empty() {
        return;
    }
    animated.resize(asset.nodes.len(), false);
    animated.fill(false);
    for channel in &asset.animations[clip].channels {
        if channel.property == AnimationProperty::Weights {
            if let Some(flag) = animated.get_mut(channel.node) {
                *flag = true;
            }
        }
    }
    for (&node, weights) in overrides {
        if !animated[node] {
            pose.node_weights[node].clone_from(weights);
        }
    }
}

impl ModelInstance {
    pub fn new(asset: Arc<ModelAsset>, scene: Option<usize>) -> Result<Self, String> {
        let scene = scene.or(asset.default_scene).unwrap_or(0);
        let pose = default_pose(&asset, scene)?;
        let render_bounds_cache = Arc::new(crate::gltf_bounds::RenderBoundsCache::new(&asset));
        Ok(Self {
            render_bounds_cache,
            asset,
            pose,
            playback: None,
            playback_revision: 0,
            scene,
            revision: 1,
            gpu_publication_queued: false,
            last_error: None,
            morph_overrides: Default::default(),
            animation_workspace: AnimationWorkspace::default(),
        })
    }
    pub(crate) fn render_bounds(&self) -> Result<(Vec3, Vec3), String> {
        self.render_bounds_cache.bounds(&self.asset, &self.pose)
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
        self.playback_revision = self.playback_revision.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn stop(&mut self) -> Result<(), String> {
        let pose = default_pose(&self.asset, self.scene)?;
        self.playback = None;
        self.playback_revision = self.playback_revision.wrapping_add(1);
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
        self.playback_revision = self.playback_revision.wrapping_add(1);
        self.apply_morph_overrides(clip);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    pub fn update(&mut self, delta: f32) -> Result<bool, String> {
        if !self.prepare_animation(delta)? {
            return Ok(false);
        }
        self.publish_animation();
        Ok(true)
    }
    pub(crate) fn prepare_animation(&mut self, delta: f32) -> Result<bool, String> {
        if let Some(scratch) = &mut self.animation_workspace.scratch {
            scratch.next_playback = None;
        }
        let result = self.prepare_animation_inner(delta);
        if result.is_err() {
            self.discard_animation();
        }
        result
    }
    pub(crate) fn prepared_render_bounds(&self) -> Result<(Vec3, Vec3), String> {
        let pose = &self
            .animation_workspace
            .scratch
            .as_ref()
            .ok_or("Animation not prepared")?
            .pose;
        self.render_bounds_cache.bounds(&self.asset, pose)
    }
    pub(crate) fn publish_animation(&mut self) {
        let scratch = self
            .animation_workspace
            .scratch
            .as_mut()
            .expect("animation prepared before publication");
        let playback = scratch
            .next_playback
            .take()
            .expect("animation prepared before publication");
        std::mem::swap(&mut self.pose, &mut scratch.pose);
        self.playback = Some(playback);
        self.revision = self.revision.wrapping_add(1);
        self.animation_workspace.limit();
    }
    pub(crate) fn discard_animation(&mut self) {
        if let Some(scratch) = &mut self.animation_workspace.scratch {
            scratch.next_playback = None;
        }
        self.animation_workspace.limit();
    }
    fn prepare_animation_inner(&mut self, delta: f32) -> Result<bool, String> {
        #[cfg(target_arch = "wasm32")]
        let _scope = crate::render::profiler::cpu_scope("gltf.animation");
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
        let scratch = self.animation_workspace.scratch.get_or_insert_with(|| {
            Box::new(AnimationScratch {
                pose: self.pose.clone(),
                sampled: Vec::new(),
                parents: Vec::new(),
                stack: Vec::new(),
                next_playback: None,
                animated_weights: Vec::new(),
            })
        });
        copy_pose_into(&self.pose, &mut scratch.pose);
        apply_clip_reuse(
            &self.asset,
            &mut scratch.pose,
            playback.clip,
            playback.time,
            &mut scratch.sampled,
            &mut scratch.parents,
            &mut scratch.stack,
        )?;
        apply_morph_overrides_into(
            &self.asset,
            &self.morph_overrides,
            playback.clip,
            &mut scratch.pose,
            &mut scratch.animated_weights,
        );
        scratch.next_playback = Some(playback);
        Ok(true)
    }
    pub fn select_scene(&mut self, scene: usize) -> Result<(), String> {
        self.pose = default_pose(&self.asset, scene)?;
        self.scene = scene;
        self.playback = None;
        self.playback_revision = self.playback_revision.wrapping_add(1);
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
        apply_morph_overrides_into(
            &self.asset,
            &self.morph_overrides,
            clip,
            &mut self.pose,
            &mut Vec::new(),
        );
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
    deformed_vertex_subset(asset, pose, node, primitive, 0..p.vertices.len() as u32)
}

/// Deform only the requested local indices, preserving order and duplicate indices.
/// Cutting uses this to bound preparation even for a single enormous primitive.
pub fn deformed_vertex_subset(
    asset: &ModelAsset,
    pose: &ModelPose,
    node: usize,
    primitive: usize,
    indices: impl IntoIterator<Item = u32>,
) -> Result<Vec<ModelVertex>, String> {
    deformed_subset(
        asset,
        pose,
        node,
        primitive,
        indices,
        |vertex, position, normal, tangent| {
            let mut out = vertex.clone();
            out.position = position.to_array();
            out.normal = normal.to_array();
            out.tangent = tangent;
            out
        },
        true,
    )
}

/// Compact cut payload: borrow skin/UV arrays and skip unused tangent work.
/// Morph positions, skinning and inverse-transpose normals share the full path.
pub(crate) fn deformed_cut_vertex_subset(
    asset: &ModelAsset,
    pose: &ModelPose,
    node: usize,
    primitive: usize,
    indices: impl IntoIterator<Item = u32>,
) -> Result<Vec<crate::gltf_loader::ImportedVertex>, String> {
    deformed_subset(
        asset,
        pose,
        node,
        primitive,
        indices,
        |vertex, position, normal, _| crate::gltf_loader::ImportedVertex {
            position: position.to_array(),
            normal: normal.to_array(),
            color: [vertex.color[0], vertex.color[1], vertex.color[2]],
        },
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn deformed_subset<T>(
    asset: &ModelAsset,
    pose: &ModelPose,
    node: usize,
    primitive: usize,
    indices: impl IntoIterator<Item = u32>,
    mut project: impl FnMut(&ModelVertex, Vec3, Vec3, [f32; 4]) -> T,
    tangents: bool,
) -> Result<Vec<T>, String> {
    let p = asset
        .primitives
        .get(primitive)
        .ok_or("Primitive index out of range")?;
    let model_node = asset.nodes.get(node).ok_or("Model node out of range")?;
    let weights = pose
        .node_weights
        .get(node)
        .ok_or("Pose node out of range")?;
    let source = asset
        .vertices
        .get(p.vertices.clone())
        .ok_or("Invalid primitive vertex range")?;
    let indices = indices.into_iter();
    let mut vertices = Vec::with_capacity(indices.size_hint().0);
    for index in indices {
        let i = index as usize;
        let vertex = source.get(i).ok_or("Primitive vertex index out of range")?;
        let mut position = Vec3::from_array(vertex.position);
        let mut normal = Vec3::from_array(vertex.normal);
        let mut tangent = if tangents {
            Vec3::from_slice(&vertex.tangent)
        } else {
            Vec3::ZERO
        };
        let mut handedness = vertex.tangent[3];
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
            if tangents {
                if let Some(delta) = target.tangents.get(i) {
                    tangent += Vec3::from_array(*delta) * weight;
                }
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
            if tangents {
                tangent = linear * tangent;
                if linear.determinant() < 0. {
                    handedness = -handedness;
                }
            }
        }
        if !position.is_finite() || !normal.is_finite() || !tangent.is_finite() {
            return Err("Model deformation overflow".into());
        }
        normal = normal.try_normalize().unwrap_or(Vec3::Y);
        if tangents {
            tangent = (tangent - normal * normal.dot(tangent))
                .try_normalize()
                .unwrap_or(Vec3::ZERO);
        }
        vertices.push(project(
            vertex,
            position,
            normal,
            [tangent.x, tangent.y, tangent.z, handedness],
        ));
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
    fn compact_cut_vertices_match_full_deformation_with_skin_morph_and_duplicate_indices() {
        for bytes in [
            include_bytes!("../assets/models/rim/drone-orbiter.glb").as_slice(),
            include_bytes!("../assets/models/rim/core-tank.glb").as_slice(),
            include_bytes!("../assets/models/rim/blade-wraith.glb").as_slice(),
            include_bytes!("../assets/models/fox/Fox.glb").as_slice(),
        ] {
            let asset = Arc::new(crate::gltf_loader::parse_model(bytes, &[], &[]).unwrap());
            let mut model = ModelInstance::new(asset, None).unwrap();
            if !model.asset.animations.is_empty() {
                model.play(0, true).unwrap();
                model.seek(0.37).unwrap();
            }
            for &node in &model.pose.active_nodes.clone() {
                let Some(mesh) = model.asset.nodes[node].mesh else {
                    continue;
                };
                for primitive in model.asset.meshes[mesh].primitives.clone() {
                    let mut asset = (*model.asset).clone();
                    let mut pose = model.pose.clone();
                    let p = &mut asset.primitives[primitive];
                    let indices: Vec<_> = p
                        .indices
                        .iter()
                        .take(21)
                        .copied()
                        .chain(p.indices.iter().take(3).copied())
                        .collect();
                    p.morph_targets = vec![MorphTarget {
                        positions: vec![[0.01, -0.02, 0.03]; p.vertices.len()],
                        normals: vec![[0.03, 0.01, -0.02]; p.vertices.len()],
                        tangents: vec![[0.02, 0., 0.01]; p.vertices.len()],
                    }];
                    pose.node_weights[node] = vec![0.6];
                    for matrix in &mut pose.skin_matrices[node] {
                        *matrix = Mat4::from_scale(Vec3::new(-1.1, 0.7, 1.4)) * *matrix;
                    }
                    let full = deformed_vertex_subset(
                        &asset,
                        &pose,
                        node,
                        primitive,
                        indices.iter().copied(),
                    )
                    .unwrap();
                    let compact = deformed_cut_vertex_subset(
                        &asset,
                        &pose,
                        node,
                        primitive,
                        indices.iter().copied(),
                    )
                    .unwrap();
                    assert_eq!(full.len(), indices.len());
                    assert_eq!(compact.len(), full.len());
                    for (a, b) in full.iter().zip(&compact) {
                        assert_eq!(a.position, b.position);
                        assert_eq!(a.normal, b.normal);
                        assert_eq!(&a.color[..3], b.color);
                    }
                }
            }
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

#[cfg(test)]
mod workspace_tests {
    use super::*;
    fn assets() -> Vec<Arc<ModelAsset>> {
        [
            include_bytes!("../assets/models/rim/core-tank.glb").as_slice(),
            include_bytes!("../assets/models/rim/drone-orbiter.glb").as_slice(),
            include_bytes!("../assets/models/fox/Fox.glb").as_slice(),
        ]
        .into_iter()
        .map(|b| Arc::new(crate::gltf_loader::parse_model(b, &[], &[]).unwrap()))
        .collect()
    }
    fn bits(p: &ModelPose) -> Vec<u32> {
        let mut out = Vec::new();
        for trs in &p.local_trs {
            out.extend(
                trs.translation
                    .to_array()
                    .into_iter()
                    .chain(trs.rotation.to_array())
                    .chain(trs.scale.to_array())
                    .map(f32::to_bits),
            );
        }
        out.extend(
            p.world_matrices
                .iter()
                .flat_map(|m| m.to_cols_array())
                .map(f32::to_bits),
        );
        out.extend(p.node_weights.iter().flatten().copied().map(f32::to_bits));
        out.extend(
            p.skin_matrices
                .iter()
                .flatten()
                .flat_map(|m| m.to_cols_array())
                .map(f32::to_bits),
        );
        out.extend(p.skin_world_space.iter().map(|b| *b as u32));
        out.extend(p.active_nodes.iter().map(|n| *n as u32));
        out
    }
    #[test]
    fn animation_workspace_matches_frozen_reference_for_all_authored_and_skin_clips() {
        for asset in assets() {
            for clip in 0..asset.animations.len() {
                let mut hot = ModelInstance::new(asset.clone(), None).unwrap();
                hot.play(clip, true).unwrap();
                let mut reference = hot.clone();
                for dt in [0., 1. / 90., 1. / 120., 0.1, 0.37, 2.33, 0.001, 1. / 60.] {
                    assert_eq!(
                        hot.update(dt).unwrap(),
                        crate::animation_reference::update(&mut reference, dt).unwrap()
                    );
                    assert_eq!(bits(&hot.pose), bits(&reference.pose));
                    assert_eq!(hot.revision, reference.revision);
                    assert_eq!(
                        hot.playback.as_ref().unwrap().time.to_bits(),
                        reference.playback.as_ref().unwrap().time.to_bits()
                    );
                }
                assert!(hot.animation_workspace.retained_bytes() > 0);
                let mut clone = hot.clone();
                assert_eq!(clone.animation_workspace.retained_bytes(), 0);
                let before = bits(&hot.pose);
                clone.update(0.13).unwrap();
                assert_eq!(before, bits(&hot.pose));
            }
        }
    }
    #[test]
    fn animation_workspace_preserves_pose_and_recovers_after_mutated_invalid_graph() {
        let asset = assets().remove(0);
        let mut hot = ModelInstance::new(asset.clone(), None).unwrap();
        hot.play(0, true).unwrap();
        hot.update(0.1).unwrap();
        let before = bits(&hot.pose);
        let revision = hot.revision;
        let time = hot.playback.as_ref().unwrap().time;
        Arc::make_mut(&mut hot.asset).nodes[0].children.push(0);
        assert!(hot.update(0.1).is_err());
        assert_eq!(bits(&hot.pose), before);
        assert_eq!(hot.revision, revision);
        assert_eq!(hot.playback.as_ref().unwrap().time, time);
        hot.asset = asset;
        assert!(hot.update(0.1).unwrap());
        assert_ne!(hot.revision, revision);
    }
    #[test]
    fn animation_workspace_retention_is_bounded_and_discard_keeps_published_pose() {
        let asset = assets().remove(0);
        let mut hot = ModelInstance::new(asset, None).unwrap();
        hot.play(0, true).unwrap();
        hot.update(0.1).unwrap();
        let before = bits(&hot.pose);
        hot.animation_workspace
            .scratch
            .as_mut()
            .unwrap()
            .parents
            .reserve(200_000);
        assert!(hot.animation_workspace.retained_bytes() > 2 * 1024 * 1024);
        hot.discard_animation();
        assert_eq!(hot.animation_workspace.retained_bytes(), 0);
        assert_eq!(bits(&hot.pose), before);
        assert!(hot.update(0.1).unwrap());
    }
    #[test]
    fn animation_workspace_sampler_matches_frozen_reference_endpoints_and_morph_cubic() {
        let mut output = Vec::new();
        for asset in assets() {
            for clip in &asset.animations {
                for channel in &clip.channels {
                    for time in [-0.1, 0., 0.001, 0.37, clip.duration, clip.duration + 0.1] {
                        sample_channel_into(channel, time, &mut output).unwrap();
                        assert_eq!(
                            output.iter().copied().map(f32::to_bits).collect::<Vec<_>>(),
                            crate::animation_reference::sample_channel(channel, time)
                                .unwrap()
                                .into_iter()
                                .map(f32::to_bits)
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        for interpolation in [
            Interpolation::Step,
            Interpolation::Linear,
            Interpolation::CubicSpline,
        ] {
            let width = if interpolation == Interpolation::CubicSpline {
                30
            } else {
                10
            };
            let channel = AnimationChannel {
                node: 0,
                property: AnimationProperty::Weights,
                interpolation,
                times: vec![0., 2.],
                values: (0..width).map(|i| i as f32 * 0.01).collect(),
                components: 5,
            };
            for time in [0., 0.2, 1., 2., 3.] {
                sample_channel_into(&channel, time, &mut output).unwrap();
                assert_eq!(
                    output,
                    crate::animation_reference::sample_channel(&channel, time).unwrap()
                );
            }
        }
    }
    #[test]
    fn animation_workspace_morph_flags_reset_after_clip_seek_and_preserve_untargeted_overrides() {
        let base =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gltf/full");
        let mut asset = crate::gltf_loader::parse_model(
            &std::fs::read(base.join("full.gltf")).unwrap(),
            &[std::fs::read(base.join("full.bin")).unwrap()],
            &[std::fs::read(base.join("checker.png")).unwrap()],
        )
        .unwrap();
        let mut second = asset.animations[0].clone();
        second.channels.retain(|c| c.node == 1);
        asset.animations[0].channels.retain(|c| c.node == 0);
        asset.animations.push(second);
        let mut hot = ModelInstance::new(Arc::new(asset), None).unwrap();
        hot.set_morph_weights(0, &[0.7]).unwrap();
        hot.set_morph_weights(1, &[0.4]).unwrap();
        hot.play(0, true).unwrap();
        let mut reference = hot.clone();
        for clip in [0, 1, 0] {
            hot.play(clip, true).unwrap();
            reference.play(clip, true).unwrap();
            hot.seek(0.2).unwrap();
            reference.seek(0.2).unwrap();
            for dt in [0.01, 0.1, 0.3] {
                hot.update(dt).unwrap();
                crate::animation_reference::update(&mut reference, dt).unwrap();
                assert_eq!(bits(&hot.pose), bits(&reference.pose));
                assert_eq!(
                    hot.pose.node_weights[if clip == 0 { 1 } else { 0 }],
                    if clip == 0 { vec![0.4] } else { vec![0.7] }
                );
            }
        }
    }
}
