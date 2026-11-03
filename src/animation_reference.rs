//! Frozen pre-workspace reference for exact numerical/transactional regression tests.
//! Original gltf_animation.rs SHA256 911cd20f3048bba97a0e4768c2e812fb3ca211f3055cc14f5eba91126290fe88
//! Intentional duplication: reference must not call the optimized sampler/traversal.
use crate::gltf_animation::validate_pose_budget;
use crate::gltf_model::*;
use glam::{Mat4, Quat, Vec3};
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
pub fn update(model: &mut ModelInstance, delta: f32) -> Result<bool, String> {
    #[cfg(target_arch = "wasm32")]
    let _scope = crate::render::profiler::cpu_scope("gltf.animation");
    if !delta.is_finite() || delta < 0. {
        return Err("Invalid animation delta".into());
    }
    let Some(mut playback) = model.playback.clone() else {
        return Ok(false);
    };
    if !playback.playing || delta == 0. || playback.speed == 0. {
        return Ok(false);
    }
    let duration = model.asset.animations[playback.clip].duration;
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
    apply_clip(&model.asset, &mut model.pose, playback.clip, playback.time)?;
    apply_morph_overrides(model, playback.clip);
    model.playback = Some(playback);
    model.revision = model.revision.wrapping_add(1);
    Ok(true)
}
fn apply_morph_overrides(model: &mut ModelInstance, clip: usize) {
    let animated: std::collections::HashSet<_> = model.asset.animations[clip]
        .channels
        .iter()
        .filter(|channel| channel.property == AnimationProperty::Weights)
        .map(|channel| channel.node)
        .collect();
    for (&node, weights) in &model.morph_overrides {
        if !animated.contains(&node) {
            model.pose.node_weights[node] = weights.clone();
        }
    }
}
