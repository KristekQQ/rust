//! CPU representation of glTF 2.0 core. All indices address the containing asset.
use glam::{Mat4, Quat, Vec3};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub color: [f32; 4],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
    /// All JOINTS_n/WEIGHTS_n sets, including sets beyond zero.
    pub influences: Vec<(u16, f32)>,
    pub tex_coords: Vec<[f32; 2]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveMode {
    Points,
    Lines,
    LineLoop,
    LineStrip,
    Triangles,
    TriangleStrip,
    TriangleFan,
}
#[derive(Clone, Debug)]
pub struct MorphTarget {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 3]>,
}
#[derive(Clone, Debug)]
pub struct ModelPrimitive {
    /// glTF requires geometric face normals when NORMAL is absent.
    pub flat_normals: bool,
    /// Slice in ModelAsset.vertices. Indices are LOCAL to this slice.
    pub vertices: Range<usize>,
    pub indices: Vec<u32>,
    pub mode: PrimitiveMode,
    pub material: Option<usize>,
    pub morph_targets: Vec<MorphTarget>,
}
#[derive(Clone, Debug)]
pub struct ModelMesh {
    pub name: Option<String>,
    pub primitives: Vec<usize>,
    pub weights: Vec<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}
#[derive(Clone, Copy, Debug)]
pub struct TextureSlot {
    pub texture: usize,
    pub tex_coord: u32,
}
#[derive(Clone, Debug)]
pub struct ModelMaterial {
    pub name: Option<String>,
    pub base_color_factor: [f32; 4],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: [f32; 3],
    pub base_color_texture: Option<TextureSlot>,
    pub metallic_roughness_texture: Option<TextureSlot>,
    pub normal_texture: Option<TextureSlot>,
    pub normal_scale: f32,
    pub occlusion_texture: Option<TextureSlot>,
    pub occlusion_strength: f32,
    pub emissive_texture: Option<TextureSlot>,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}
#[derive(Clone, Debug)]
pub struct ModelImage {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct ModelTexture {
    pub image: usize,
    pub sampler: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrapMode {
    ClampToEdge,
    MirroredRepeat,
    Repeat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMode {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    LinearMipmapLinear,
}
#[derive(Clone, Debug)]
pub struct ModelSampler {
    pub wrap_s: WrapMode,
    pub wrap_t: WrapMode,
    pub mag_filter: Option<FilterMode>,
    pub min_filter: Option<FilterMode>,
}
#[derive(Clone, Copy, Debug)]
pub struct NodeTrs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl NodeTrs {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
#[derive(Clone, Debug)]
pub struct ModelNode {
    pub name: Option<String>,
    pub children: Vec<usize>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub camera: Option<usize>,
    pub trs: NodeTrs,
    /// Matrix-authored nodes cannot be targets of TRS animation channels.
    pub matrix: Option<Mat4>,
    pub weights: Option<Vec<f32>>,
}
#[derive(Clone, Debug)]
pub struct ModelSkin {
    pub name: Option<String>,
    pub joints: Vec<usize>,
    pub inverse_bind_matrices: Vec<Mat4>,
    pub skeleton: Option<usize>,
}
#[derive(Clone, Debug)]
pub enum ModelCamera {
    Perspective {
        name: Option<String>,
        yfov: f32,
        aspect_ratio: Option<f32>,
        znear: f32,
        zfar: Option<f32>,
    },
    Orthographic {
        name: Option<String>,
        xmag: f32,
        ymag: f32,
        znear: f32,
        zfar: f32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationProperty {
    Translation,
    Rotation,
    Scale,
    Weights,
}
#[derive(Clone, Debug)]
pub struct AnimationChannel {
    pub node: usize,
    pub property: AnimationProperty,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    /// Flat sample scalars. For CUBICSPLINE each key is [in tangent, value, out tangent].
    pub values: Vec<f32>,
    pub components: usize,
}
#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub name: Option<String>,
    pub channels: Vec<AnimationChannel>,
    pub duration: f32,
}
#[derive(Clone, Debug)]
pub struct ModelScene {
    pub name: Option<String>,
    pub nodes: Vec<usize>,
}
#[derive(Clone, Debug)]
pub struct ModelAsset {
    pub vertices: Vec<ModelVertex>,
    pub primitives: Vec<ModelPrimitive>,
    pub meshes: Vec<ModelMesh>,
    pub materials: Vec<ModelMaterial>,
    pub images: Vec<ModelImage>,
    pub textures: Vec<ModelTexture>,
    pub samplers: Vec<ModelSampler>,
    pub nodes: Vec<ModelNode>,
    pub skins: Vec<ModelSkin>,
    pub animations: Vec<AnimationClip>,
    pub scenes: Vec<ModelScene>,
    pub default_scene: Option<usize>,
    pub cameras: Vec<ModelCamera>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct ModelPose {
    pub local_trs: Vec<NodeTrs>,
    pub world_matrices: Vec<Mat4>,
    pub node_weights: Vec<Vec<f32>>,
    /// Per mesh NODE, already in that node's local space: inverse(meshWorld)*jointWorld*IBM.
    pub skin_matrices: Vec<Vec<Mat4>>,
    /// Singular mesh transforms use world-space skin deformation instead of an inverse.
    pub skin_world_space: Vec<bool>,
    /// Nodes reachable from the selected scene; joint matrices may use other nodes.
    pub active_nodes: Vec<usize>,
}
#[derive(Clone, Debug)]
pub struct AnimationPlayback {
    pub clip: usize,
    pub time: f32,
    pub speed: f32,
    pub looping: bool,
    pub playing: bool,
}
#[derive(Clone, Debug)]
pub struct ModelInstance {
    pub(crate) render_bounds_cache: std::sync::Arc<crate::gltf_bounds::RenderBoundsCache>,
    pub asset: std::sync::Arc<ModelAsset>,
    pub pose: ModelPose,
    pub playback: Option<AnimationPlayback>,
    /// Identity of an explicit playback command; normal animation frames keep it.
    pub playback_revision: u64,
    pub scene: usize,
    pub revision: u64,
    pub last_error: Option<String>,
    /// Manual weights persist for properties not targeted by the active animation.
    pub morph_overrides: std::collections::BTreeMap<usize, Vec<f32>>,
}
