//! Scene-owned opt-in numeric font. Immutable glyph assets, ordinary shared instances.
use super::*;
use std::collections::BTreeSet;
#[derive(Default)]
pub(super) struct NumericFont {
    assets: Option<[u32; 10]>,
    panels: BTreeMap<u32, Panel>,
    children: BTreeSet<u32>,
}
#[derive(PartialEq)]
struct Key {
    revision: u64,
    matrix: [f32; 16],
    options: RenderOptions,
    material: [f32; 7],
}
struct Panel {
    key: Key,
    children: Vec<(u8, u32)>,
}
impl SceneManager {
    pub(crate) fn is_numeric_panel(&self, id: u32) -> bool {
        self.font3d.texts.contains_key(&id)
            || self.info_panels.get(&id).is_some_and(|p| p.numeric())
    }
    pub(crate) fn is_numeric_glyph(&self, id: u32) -> bool {
        self.numeric_font.children.contains(&id)
    }
    pub(crate) fn is_text_object(&self, id: u32) -> bool {
        self.info_panels.contains_key(&id)
            || self.numeric_font.children.contains(&id)
            || self.font3d.children.contains(&id)
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn numeric_font_digit(&self, handle: u32) -> Option<u8> {
        self.numeric_font
            .assets
            .as_ref()?
            .iter()
            .position(|&id| id == handle)
            .map(|digit| digit as u8)
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn numeric_font_assets(&self) -> impl Iterator<Item = &u32> {
        self.numeric_font.assets.iter().flatten()
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn numeric_font_children(&self) -> impl Iterator<Item = &u32> {
        self.numeric_font.children.iter()
    }
    pub fn numeric_font_stats(&self) -> (usize, usize, usize) {
        (
            self.numeric_font.assets.as_ref().map_or(0, |_| 10),
            self.numeric_font.children.len(),
            self.numeric_font.panels.len(),
        )
    }
    pub(super) fn remove_numeric_panel(&mut self, id: u32) {
        let Some(panel) = self.numeric_font.panels.remove(&id) else {
            return;
        };
        for (_, child) in panel.children {
            self.numeric_font.children.remove(&child);
            self.remove_object(child);
        }
        if self.numeric_font.panels.is_empty() {
            if let Some(assets) = self.numeric_font.assets.take() {
                for asset in assets {
                    self.remove_mesh_variants(asset);
                }
            }
        }
    }
    /// Glyph instances share immutable render-only topology. They do not consume
    /// the scene's per-object cutting budget or enter the public cuttable API.
    pub(super) fn add_numeric_glyph_instance(&mut self, handle: u32) -> Result<u32, String> {
        let mesh = self
            .mesh_variants
            .get(&handle)
            .filter(|asset| asset.registered)
            .and_then(|asset| asset.variants.first())
            .ok_or("Invalid numeric glyph asset")?
            .clone();
        let (min, max) = mesh.bounds();
        let id = self.add_imported(min, max);
        if id == INVALID_ID {
            return Err("Object IDs exhausted".into());
        }
        self.triangle_meshes.insert(
            id,
            TriangleObject {
                mesh,
                cuttable: false,
                cap_color: Vec3::ONE,
            },
        );
        self.variant_instances.insert(id, (handle, 0));
        let object = self.objects.get_mut(&id).unwrap();
        object.mesh = MeshKind::Imported(handle);
        object.refresh_render_data();
        Ok(id)
    }
    /// Call before uploading/extracting a frame. Unchanged panels allocate nothing.
    pub fn sync_numeric_font(&mut self) {
        let mut font = std::mem::take(&mut self.numeric_font);
        // Use the panel map directly on the unchanged path: no per-frame ID collection.
        let mut cursor = None;
        loop {
            let next = self
                .info_panels
                .iter()
                .find(|(id, p)| {
                    p.numeric() && !p.font3d_bound && cursor.is_none_or(|previous| **id > previous)
                })
                .map(|(&id, _)| id);
            let Some(id) = next else {
                break;
            };
            cursor = Some(id);
            let parent = &self.objects[&id];
            let key = Key {
                revision: self.info_panels[&id].revision,
                matrix: parent.transform.model().to_cols_array(),
                options: parent.render_options,
                material: [
                    parent.emission,
                    parent.color_cutout,
                    parent.opacity,
                    parent.opaque_above,
                    parent.opacity_variation,
                    parent.opacity_coverage,
                    parent.cut_assist,
                ],
            };
            if font.panels.get(&id).is_some_and(|p| p.key == key) {
                continue;
            }
            if font.assets.is_none() {
                let mut handles = [0; 10];
                for (digit, handle) in handles.iter_mut().enumerate() {
                    let mesh = crate::info_panel::numeric_glyph(digit as u8);
                    let source = crate::cutting_mesh::MeshData::from(
                        crate::cutting::CutMesh::from_render_mesh(&mesh),
                    );
                    let id = self.allocate_id().expect("numeric glyph asset ID");
                    self.mesh_variants.insert(
                        id,
                        MeshVariants {
                            colors: mesh
                                .indices
                                .as_chunks::<3>()
                                .0
                                .iter()
                                .map(|tri| {
                                    let c = mesh.vertices[tri[0] as usize].color;
                                    [c[0], c[1], c[2], 1.]
                                })
                                .collect(),
                            variants: vec![source],
                            registered: true,
                            varying_triangles: vec![],
                        },
                    );
                    *handle = id;
                }
                font.assets = Some(handles);
            }
            let layout = self.info_panels[&id].numeric_layout();
            let transform = self.objects[&id].transform;
            let mut children = font
                .panels
                .remove(&id)
                .map_or_else(Vec::new, |p| p.children);
            while children.len() > layout.len() {
                let (_, child) = children.pop().unwrap();
                font.children.remove(&child);
                self.remove_object(child);
            }
            for (index, &(digit, x, unit, depth)) in layout.iter().enumerate() {
                if index < children.len() && children[index].0 != digit {
                    let child = children[index].1;
                    let handle = font.assets.unwrap()[digit as usize];
                    let mesh = self.mesh_variants[&handle].variants[0].clone();
                    let (min, max) = mesh.bounds();
                    self.triangle_meshes.get_mut(&child).unwrap().mesh = mesh;
                    self.variant_instances.insert(child, (handle, 0));
                    let object = self.objects.get_mut(&child).unwrap();
                    object.mesh = MeshKind::Imported(handle);
                    object.local_bounds = Some((min, max));
                    children[index].0 = digit;
                } else if index == children.len() {
                    let child = self
                        .add_numeric_glyph_instance(font.assets.unwrap()[digit as usize])
                        .expect("numeric glyph instance");
                    font.children.insert(child);
                    children.push((digit, child));
                }
                let child = self.objects.get_mut(&children[index].1).unwrap();
                child.transform = Transform::new(
                    transform.model().transform_point3(Vec3::new(x, 0., 0.003)),
                    transform.rotation,
                    transform.scale * Vec3::new(unit, unit, depth),
                );
                self.shadow_revisions.changed(child.render_options);
                self.shadow_revisions.changed(key.options);
                child.render_options = key.options;
                [
                    child.emission,
                    child.color_cutout,
                    child.opacity,
                    child.opaque_above,
                    child.opacity_variation,
                    child.opacity_coverage,
                    child.cut_assist,
                ] = key.material;
                child.refresh_render_data();
            }
            self.render_revision = self.render_revision.wrapping_add(1);
            // A static numeric parent may change content or pose too.
            if key.options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
            font.panels.insert(id, Panel { key, children });
        }
        self.numeric_font = font;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_font_shares_digits_and_updates_without_asset_growth() {
        let mut scene = SceneManager::default();
        let timer = scene.add_info_panel("countdown", 1., 0.5).unwrap();
        let score = scene.add_info_panel("score", 1., 0.5).unwrap();
        scene.sync_numeric_font();
        assert_eq!(scene.numeric_font_stats(), (10, 3, 2));
        let timer_zero = scene.numeric_font.panels[&timer].children[1].1;
        let score_zero = scene.numeric_font.panels[&score].children[0].1;
        assert_eq!(
            scene.variant_instances[&timer_zero],
            scene.variant_instances[&score_zero]
        );
        let mut initial_queue = crate::visibility::RenderQueue::default();
        initial_queue.extract(&scene, Mat4::IDENTITY, false);
        assert_eq!(
            initial_queue.imported[&scene.variant_instances[&timer_zero].0].len(),
            2
        );
        assert!(!initial_queue.imported.contains_key(&timer));
        assert!(!initial_queue.imported.contains_key(&score));
        let assets = scene.numeric_font.assets;
        for value in ["29", "28"] {
            scene.update_info_panels(
                crate::info_panel::PanelKind::Countdown,
                &[("", value.into())],
            );
            scene.sync_numeric_font();
            assert_eq!(scene.numeric_font.assets, assets);
            assert_eq!(scene.mesh_variants.len(), 10);
            assert_eq!(scene.numeric_font.panels[&timer].children[1].1, timer_zero);
        }
        let revision = scene.render_revision();
        scene.sync_numeric_font();
        assert_eq!(scene.render_revision(), revision);
        let mut queue = crate::visibility::RenderQueue::default();
        queue.extract(&scene, Mat4::IDENTITY, false);
        assert_eq!(queue.visible, 3);
        assert_eq!(queue.imported.values().map(Vec::len).sum::<usize>(), 3);
    }
    #[test]
    fn numeric_font_inherits_mirrored_pose_material_and_releases_lifetime() {
        let mut scene = SceneManager::default();
        let id = scene.add_info_panel("countdown", 1., 0.5).unwrap();
        let transform = Transform::new(
            Vec3::new(2., 3., -4.),
            Vec3::new(0.1, 0.7, 0.2),
            Vec3::new(-2., 3., 4.),
        );
        scene.objects.get_mut(&id).unwrap().transform = transform;
        scene.objects.get_mut(&id).unwrap().opacity = 0.3;
        scene.objects.get_mut(&id).unwrap().emission = 2.;
        scene.sync_numeric_font();
        let layout = scene.info_panels[&id].numeric_layout();
        for ((_, child), (_, x, unit, depth)) in
            scene.numeric_font.panels[&id].children.iter().zip(layout)
        {
            let child = &scene.objects[child];
            let expected = transform.model()
                * Mat4::from_translation(Vec3::new(x, 0., 0.003))
                * Mat4::from_scale(Vec3::new(unit, unit, depth));
            assert!(child.transform.model().abs_diff_eq(expected, 1e-5));
            assert_eq!(child.opacity, 0.3);
            assert_eq!(child.emission, 2.);
            assert!(scene.is_text_object(scene.numeric_font.panels[&id].children[0].1));
        }
        scene.objects.get_mut(&id).unwrap().opacity = 0.6;
        scene.objects.get_mut(&id).unwrap().transform.position.x += 1.;
        scene.sync_numeric_font();
        for (_, child) in &scene.numeric_font.panels[&id].children {
            assert_eq!(scene.objects[child].opacity, 0.6);
        }
        let revision = scene.render_revision();
        scene.sync_numeric_font();
        assert_eq!(scene.render_revision(), revision);
        assert!(scene.remove_object(id));
        assert_eq!(scene.numeric_font_stats(), (0, 0, 0));
        assert!(scene.objects.is_empty());
        assert!(scene.mesh_variants.is_empty());
        let id = scene.add_info_panel("score", 1., 1.).unwrap();
        scene.sync_numeric_font();
        assert!(scene.object_exists(id));
        scene.clear_scene();
        assert_eq!(scene.numeric_font_stats(), (0, 0, 0));
        assert!(scene.variant_instances.is_empty());
    }
    #[test]
    fn numeric_font_layout_matches_cpu_mesh_for_mixed_narrow_digits() {
        let mut scene = SceneManager::default();
        let id = scene.add_info_panel("score", 1.2, 0.5).unwrap();
        scene.update_info_panels(crate::info_panel::PanelKind::Score, &[("", "101".into())]);
        scene.sync_numeric_font();
        let panel = &scene.info_panels[&id];
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (_, child) in &scene.numeric_font.panels[&id].children {
            let object = &scene.objects[child];
            for vertex in &scene.mesh_variants[&scene.variant_instances[child].0]
                .gpu_mesh()
                .vertices
            {
                let p = object
                    .transform
                    .model()
                    .transform_point3(Vec3::from_array(vertex.position));
                min = min.min(p);
                max = max.max(p);
            }
        }
        let cpu_min = panel
            .mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let cpu_max = panel
            .mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        assert!(min.abs_diff_eq(cpu_min, 1e-5), "{min:?} {cpu_min:?}");
        assert!(max.abs_diff_eq(cpu_max, 1e-5));
    }
}
