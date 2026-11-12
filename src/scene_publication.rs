//! Simulation-to-render publication. Snapshots are complete, so a latest-only
//! mailbox may discard intermediate frames. glTF assets/poses must remain static
//! and be loaded on both scenes with identical root IDs before publication.
use super::*;
use crate::gltf_loader::ImportedMesh;
use std::{collections::BTreeSet, sync::Arc};

#[derive(Default)]
pub(crate) struct MirrorState {
    pub(super) is_mirror: bool,
    // Rendering semantics only; gameplay Feedback stays on the simulation owner.
    pub(super) cut_feedback_rings: [Option<u32>; 2],
    // Authoritative font lifetime, including registered fonts with no glyphs.
    pub(super) font_owners: BTreeSet<u32>,
    pub(super) font_assets: BTreeSet<u32>,
    pub(super) font_children: BTreeSet<u32>,
    pub(super) font_texts: BTreeSet<u32>,
    pub(super) variants: BTreeMap<u32, Arc<PreparedVariant>>,
    #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
    pub(crate) panel_glow: BTreeMap<u32, Option<Arc<crate::render::neon_glow::PreparedGlowMesh>>>,
    pub(super) source_render: u64,
    pub(super) source_static: u64,
    pub(super) source_shadows: ShadowRevisions,
}
pub(super) struct PreparedVariant {
    pub(super) source: crate::cutting_mesh::MeshData,
    pub(super) mesh: ImportedMesh,
    pub(super) colors: Vec<[f32; 4]>,
}
/// Retains immutable prepared assets across captures; no frame-delta dependency.
#[derive(Default)]
pub struct RenderPublisher {
    variants: BTreeMap<u32, Arc<PreparedVariant>>,
}
/// Owned snapshot, Send on native; WASM keeps it on its owning thread.
/// Geometry payloads are shared, never copied per frame.
pub struct RenderPublication {
    pub(super) objects: BTreeMap<u32, SceneObject>,
    pub(super) cut_lineage: BTreeMap<u32, (u32, Mat4)>,
    pub(super) triangles: BTreeMap<u32, TriangleObject>,
    pub(super) variants: BTreeMap<u32, MeshVariants>,
    pub(super) instances: BTreeMap<u32, (u32, u32)>,
    pub(super) panels: BTreeMap<u32, crate::info_panel::InfoPanel>,
    pub(super) numeric: numeric_font::NumericFont,
    // Authoritative font lifetime, including registered fonts with no glyphs.
    pub(super) font_owners: BTreeSet<u32>,
    pub(super) font_assets: BTreeSet<u32>,
    pub(super) font_children: BTreeSet<u32>,
    pub(super) font_texts: BTreeSet<u32>,
    pub(super) prepared: BTreeMap<u32, Arc<PreparedVariant>>,
    pub(super) lights: BTreeMap<u32, Light>,
    pub(super) render_revision: u64,
    pub(super) static_revision: u64,
    pub(super) shadows: ShadowRevisions,
    pub(super) mirror: Option<u32>,
    pub(super) next_id: u32,
    pub(super) cut_feedback_rings: [Option<u32>; 2],
}
/// Previous render snapshot ownership. Return this to the simulation worker
/// and drop it there, keeping map/asset destruction off the XR frame thread.
pub struct RenderRetirement {
    _previous: RenderPublication,
}

impl RenderPublisher {
    /// Font layout and all CPU mesh conversion run on the simulation owner.
    pub fn capture(&mut self, scene: &mut SceneManager) -> RenderPublication {
        assert!(
            !scene.render_publication.is_mirror,
            "cannot publish a render mirror"
        );
        scene.sync_numeric_font();
        scene.sync_font3d();
        for (&id, asset) in &scene.mesh_variants {
            if self
                .variants
                .get(&id)
                .is_some_and(|v| v.source.same_source(&asset.variants[0]))
            {
                continue;
            }
            let digit = scene
                .numeric_font
                .assets
                .as_ref()
                .and_then(|a| a.iter().position(|&v| v == id));
            asset.variants[0].prepare_render();
            // Native publications cross threads; WASM retains the same Arc ownership
            // contract even though its MeshData uses local Rc/OnceCell storage.
            #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
            let prepared = Arc::new(PreparedVariant {
                source: asset.variants[0].clone(),
                mesh: digit.map_or_else(
                    || asset.gpu_mesh(),
                    |d| crate::info_panel::numeric_glyph(d as u8),
                ),
                colors: asset.gpu_colors(),
            });
            self.variants.insert(id, prepared);
        }
        self.variants
            .retain(|id, _| scene.mesh_variants.contains_key(id));
        for (&id, object) in &scene.triangle_meshes {
            if !scene.variant_instances.contains_key(&id) {
                object.mesh.prepare_render();
            }
        }
        RenderPublication {
            objects: scene.objects.clone(),
            cut_lineage: scene.cut_lineage.clone(),
            triangles: scene.triangle_meshes.clone(),
            variants: scene.mesh_variants.clone(),
            instances: scene.variant_instances.clone(),
            panels: scene.info_panels.clone(),
            numeric: scene.numeric_font.clone(),
            font_owners: scene.publication_font_owners(),
            font_assets: scene.publication_font_assets(),
            font_children: scene.font3d.children.clone(),
            font_texts: scene.font3d.texts.keys().copied().collect(),
            prepared: self.variants.clone(),
            lights: scene.lights.clone(),
            render_revision: scene.render_revision,
            static_revision: scene.static_revision,
            shadows: scene.shadow_revisions.clone(),
            mirror: scene.mirror_object,
            next_id: scene.next_id,
            cut_feedback_rings: scene.cut_feedback_ring_ids(),
        }
    }
}
impl SceneManager {
    pub(crate) fn is_render_mirror(&self) -> bool {
        self.render_publication.is_mirror
    }

    #[cfg(any(test, target_arch = "wasm32", feature = "native-render"))]
    pub(crate) fn prepared_variant_mesh(&self, id: u32) -> Option<&ImportedMesh> {
        self.render_publication.variants.get(&id).map(|a| &a.mesh)
    }
    #[cfg(any(test, target_arch = "wasm32", feature = "native-render"))]
    pub(crate) fn prepared_variant_colors(&self, id: u32) -> Option<&[[f32; 4]]> {
        self.render_publication
            .variants
            .get(&id)
            .map(|a| a.colors.as_slice())
    }
    /// Apply without simulation or layout. Local IDs retain their objects,
    /// geometry, materials and panel contents even if absent from publication.
    /// Send the returned token back to the worker for deferred destruction.
    pub fn apply_render_publication(
        &mut self,
        mut p: RenderPublication,
        local_ids: &[u32],
    ) -> RenderRetirement {
        // Exchange complete trees; touch only the handful of XR-owned IDs.
        // Displaced incoming local values join the retirement token as well.
        fn exchange<T>(
            target: &mut BTreeMap<u32, T>,
            incoming: &mut BTreeMap<u32, T>,
            local: &[u32],
        ) {
            std::mem::swap(target, incoming);
            for (index, id) in local.iter().enumerate() {
                if local[..index].contains(id) {
                    continue;
                }
                let old_local = incoming.remove(id);
                let displaced = target.remove(id);
                if let Some(value) = old_local {
                    target.insert(*id, value);
                }
                if let Some(value) = displaced {
                    incoming.insert(*id, value);
                }
            }
        }
        // Compare before exchanging, with no temporary tree allocation.
        for (&id, panel) in &mut p.panels {
            if local_ids.contains(&id) {
                continue;
            }
            panel.dirty = self
                .info_panels
                .get(&id)
                .is_none_or(|old| old.dirty || old.revision != panel.revision);
        }
        exchange(&mut self.objects, &mut p.objects, local_ids);
        exchange(&mut self.triangle_meshes, &mut p.triangles, local_ids);
        exchange(&mut self.mesh_variants, &mut p.variants, local_ids);
        exchange(&mut self.variant_instances, &mut p.instances, local_ids);
        exchange(&mut self.info_panels, &mut p.panels, local_ids);
        std::mem::swap(&mut self.numeric_font, &mut p.numeric);
        self.render_revision = self.render_revision.wrapping_add(
            p.render_revision
                .wrapping_sub(self.render_publication.source_render),
        );
        self.static_revision = self.static_revision.wrapping_add(
            p.static_revision
                .wrapping_sub(self.render_publication.source_static),
        );
        self.shadow_revisions.static_casters = self.shadow_revisions.static_casters.wrapping_add(
            p.shadows
                .static_casters
                .wrapping_sub(self.render_publication.source_shadows.static_casters),
        );
        self.shadow_revisions.dynamic_casters = self.shadow_revisions.dynamic_casters.wrapping_add(
            p.shadows
                .dynamic_casters
                .wrapping_sub(self.render_publication.source_shadows.dynamic_casters),
        );
        // Protected local objects must not inherit an incoming FX material tag.
        p.cut_feedback_rings = p
            .cut_feedback_rings
            .map(|id| id.filter(|id| !local_ids.contains(id)));
        std::mem::swap(
            &mut self.render_publication.cut_feedback_rings,
            &mut p.cut_feedback_rings,
        );
        self.render_publication.is_mirror = true;
        self.render_publication.source_render = p.render_revision;
        self.render_publication.source_static = p.static_revision;
        self.render_publication.source_shadows = p.shadows.clone();
        std::mem::swap(&mut self.render_publication.font_owners, &mut p.font_owners);
        std::mem::swap(&mut self.render_publication.font_assets, &mut p.font_assets);
        std::mem::swap(
            &mut self.render_publication.font_children,
            &mut p.font_children,
        );
        std::mem::swap(&mut self.render_publication.font_texts, &mut p.font_texts);
        std::mem::swap(&mut self.render_publication.variants, &mut p.prepared);
        std::mem::swap(&mut self.lights, &mut p.lights);
        self.mirror_object = p.mirror;
        self.next_id = self.next_id.max(p.next_id);
        RenderRetirement { _previous: p }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    fn assert_send<T: Send>() {}
    #[test]
    #[cfg(feature = "native-render")]
    fn publication_font_owners_drop_stale_startup_glyph_assets() {
        for clear in [false, true] {
            let mut worker = SceneManager::default();
            let font = worker.builtin_font3d("pixel").unwrap();
            let text = worker.add_font3d_text(font, "A", "{}").unwrap();
            worker.sync_font3d();
            let mut mirror = SceneManager::default();
            assert_eq!(mirror.builtin_font3d("pixel").unwrap(), font);
            assert_eq!(mirror.add_font3d_text(font, "A", "{}").unwrap(), text);
            mirror.sync_font3d();
            let old_asset = *mirror.font3d_assets().next().unwrap();
            let old_child = *mirror.font3d_children().next().unwrap();
            let mut publisher = RenderPublisher::default();
            drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
            assert!(mirror.font3d_assets().any(|id| *id == old_asset));
            if clear {
                worker.clear_scene();
            } else {
                assert!(worker.remove_object(text));
                assert!(worker.remove_font3d(font));
            }
            drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
            assert!(!mirror.mesh_variants.contains_key(&old_asset));
            assert!(!mirror.objects.contains_key(&old_child));
            assert!(!mirror.font3d_assets().any(|id| *id == old_asset));
            assert!(!mirror.font3d_children().any(|id| *id == old_child));
            assert!(mirror.font3d_glow_fonts().is_empty());
        }
    }

    #[test]
    fn publication_font_owners_follow_worker_lifetime() {
        let mut worker = SceneManager::default();
        let font = worker.builtin_font3d("pixel").unwrap();
        let mut mirror = SceneManager::default();
        assert_eq!(mirror.builtin_font3d("pixel").unwrap(), font);
        let mut publisher = RenderPublisher::default();
        drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
        assert!(mirror.render_publication.font_assets.is_empty());
        assert_eq!(
            mirror.render_publication.font_owners,
            BTreeSet::from([font])
        );
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        assert!(mirror.font3d_glow_fonts().contains(&font));
        assert!(worker.remove_font3d(font));
        drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
        assert!(mirror.render_publication.font_owners.is_empty());
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        assert!(mirror.font3d_glow_fonts().is_empty());
        assert!(
            !mirror.publication_font_owners().is_empty(),
            "startup registry remains stale"
        );
        let again = worker.builtin_font3d("pixel").unwrap();
        drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
        assert!(mirror.render_publication.font_owners.contains(&again));
        worker.clear_scene();
        drop(mirror.apply_render_publication(publisher.capture(&mut worker), &[]));
        assert!(mirror.render_publication.font_owners.is_empty());
    }

    #[test]
    fn publication_preserves_cut_material_tags_without_copying_simulation() {
        let mut worker = SceneManager::default();
        let unrelated = worker.add_cube(Vec3::ONE);
        let mut mirror = SceneManager::default();
        let mut publisher = RenderPublisher::default();
        let mut seen = BTreeSet::new();
        for _ in 0..5 {
            let pieces: [u32; 2] = std::array::from_fn(|i| {
                let mesh =
                    crate::cutting::CutMesh::from_render_mesh(&crate::info_panel::numeric_glyph(3));
                let id = worker.add_triangle_mesh(mesh).unwrap();
                worker.set_object_position(id, Vec3::new(i as f32 * 0.2, 1., -1.));
                id
            });
            let ids = worker.play_cut_feedback(&pieces).unwrap();
            seen.insert(ids[0]);
            // A dropped publication must not lose the semantic material tag.
            drop(publisher.capture(&mut worker));
            mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
            assert!(mirror.is_cut_feedback_ring(ids[0]));
            assert!(!mirror.is_cut_feedback_ring(ids[1]));
            assert!(!mirror.is_cut_feedback_ring(unrelated));
            assert_eq!(
                mirror.cut_feedback_ring_ids(),
                [None; 2],
                "simulation pool is not replicated"
            );
        }
        assert_eq!(seen.len(), 2, "the two pooled ring IDs are reused");
        worker.clear_cut_feedback();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        // Clear parks the reusable meshes; their material remains textured.
        assert!(seen.iter().all(|id| mirror.is_cut_feedback_ring(*id)
            && mirror.objects[id].transform.position.y < -1000.));
        for id in &seen {
            worker.remove_object(*id);
        }
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(seen.iter().all(|id| !mirror.is_cut_feedback_ring(*id)));
        assert!(mirror.objects.contains_key(&unrelated));
    }
    #[test]
    fn publication_and_simulation_owner_are_send() {
        assert_send::<SceneManager>();
        assert_send::<RenderPublication>();
        assert_send::<RenderPublisher>();
        assert_send::<RenderRetirement>();
    }
    #[test]
    fn publication_skipping_frames_retains_assets_and_source_identity() {
        let mut worker = SceneManager::default();
        let panel = worker.add_info_panel("score", 1., 0.5).unwrap();
        let mut publisher = RenderPublisher::default();
        let skipped = publisher.capture(&mut worker);
        let handle = *skipped.prepared.keys().next().unwrap();
        let prepared = skipped.prepared[&handle].clone();
        let colors = skipped.variants[&handle].colors.clone();
        drop(skipped);
        worker.update_info_panel(panel, &[("", "101".into())]);
        let latest = publisher.capture(&mut worker);
        assert!(Arc::ptr_eq(&prepared, &latest.prepared[&handle]));
        assert!(Arc::ptr_eq(&colors, &latest.variants[&handle].colors));
        let count = latest.objects.len();
        let mut mirror = SceneManager::default();
        mirror.apply_render_publication(latest, &[]);
        assert_eq!(mirror.objects.len(), count);
        assert_eq!(mirror.mesh_variants.len(), 10);
        assert!(mirror.prepared_variant_mesh(handle).is_some());
        assert!(mirror.numeric_font.assets.is_some());
        mirror.sync_numeric_font();
        mirror.sync_font3d();
        assert_eq!(
            mirror.objects.len(),
            count,
            "mirror must not lay out glyphs"
        );
        worker.remove_object(panel);
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(mirror.objects.is_empty());
        assert!(mirror.mesh_variants.is_empty());
        assert!(publisher.variants.is_empty());
    }
    #[test]
    fn publication_standalone_mesh_is_prepared_shared_and_removed() {
        let mut worker = SceneManager::default();
        let mesh = crate::cutting::CutMesh::from_render_mesh(&crate::info_panel::numeric_glyph(3));
        let id = worker.add_triangle_mesh(mesh).unwrap();
        let mut publisher = RenderPublisher::default();
        let first = publisher.capture(&mut worker);
        assert!(first.triangles[&id].mesh.prepared_render().is_some());
        let source = first.triangles[&id].mesh.clone();
        let mut mirror = SceneManager::default();
        mirror.apply_render_publication(first, &[]);
        assert!(source.same_source(&mirror.triangle_meshes[&id].mesh));
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(source.same_source(&mirror.triangle_meshes[&id].mesh));
        worker.remove_object(id);
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(!mirror.objects.contains_key(&id));
        assert!(!mirror.triangle_meshes.contains_key(&id));
    }
    #[test]
    fn publication_protects_local_transform_material_and_panel() {
        let mut worker = SceneManager::default();
        let id = worker.add_info_panel("diagnostics", 1., 0.5).unwrap();
        let mut mirror = SceneManager::default();
        assert_eq!(id, mirror.add_info_panel("diagnostics", 1., 0.5).unwrap());
        mirror.set_object_position(id, Vec3::splat(7.));
        mirror.set_emission(id, 3.);
        mirror.update_info_panel(id, &[("FPS", "120".into())]);
        let mesh = mirror.info_panels[&id].mesh.clone();
        let mut publisher = RenderPublisher::default();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[id]);
        assert_eq!(mirror.objects[&id].transform.position, Vec3::splat(7.));
        assert_eq!(mirror.objects[&id].emission, 3.);
        assert!(Arc::ptr_eq(&mesh, &mirror.info_panels[&id].mesh));
        worker.remove_object(id);
        mirror.apply_render_publication(publisher.capture(&mut worker), &[id]);
        assert!(mirror.objects.contains_key(&id));
        assert!(Arc::ptr_eq(&mesh, &mirror.info_panels[&id].mesh));
    }
    #[test]
    fn publication_preserves_static_revisions_and_local_invalidation() {
        let mut worker = SceneManager::default();
        let static_id = worker.add_cube(Vec3::ZERO);
        let mut options = worker.objects[&static_id].render_options;
        options.is_static = true;
        worker.set_render_options(static_id, options);
        let moving = worker.add_cube(Vec3::ONE);
        let mut mirror = SceneManager::default();
        let local = mirror.add_cube(Vec3::ZERO);
        let mut publisher = RenderPublisher::default();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[local]);
        let revision = mirror.static_revision();
        worker.set_object_position(moving, Vec3::splat(2.));
        mirror.apply_render_publication(publisher.capture(&mut worker), &[local]);
        assert_eq!(mirror.static_revision(), revision);
        worker.set_object_position(static_id, Vec3::splat(3.));
        mirror.apply_render_publication(publisher.capture(&mut worker), &[local]);
        assert_ne!(mirror.static_revision(), revision);
        mirror.set_render_options(local, options);
        mirror.set_object_position(local, Vec3::splat(4.));
        let local_revision = mirror.static_revision();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[local]);
        assert_eq!(mirror.static_revision(), local_revision);
    }
    #[test]
    fn publication_returns_old_map_ownership_for_worker_retirement() {
        let mut worker = SceneManager::default();
        let id = worker.add_cube(Vec3::ZERO);
        let mut publisher = RenderPublisher::default();
        let mut mirror = SceneManager::default();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        worker.remove_object(id);
        let retired = mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(mirror.objects.is_empty());
        assert!(retired._previous.objects.contains_key(&id));
        // Move and release the old tree on a different owner thread.
        std::thread::spawn(move || drop(retired)).join().unwrap();
    }
    #[test]
    fn publication_panel_upload_dirty_only_for_new_revisions() {
        let mut worker = SceneManager::default();
        let id = worker.add_info_panel("diagnostics", 1., 0.5).unwrap();
        let mut publisher = RenderPublisher::default();
        let mut mirror = SceneManager::default();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        mirror.info_panels.get_mut(&id).unwrap().dirty = false;
        let mesh = mirror.info_panels[&id].mesh.clone();
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(!mirror.info_panels[&id].dirty);
        assert!(Arc::ptr_eq(&mesh, &mirror.info_panels[&id].mesh));
        worker.update_info_panel(id, &[("FPS", "90".into())]);
        mirror.apply_render_publication(publisher.capture(&mut worker), &[]);
        assert!(mirror.info_panels[&id].dirty);
        assert!(!Arc::ptr_eq(&mesh, &mirror.info_panels[&id].mesh));
    }
}
