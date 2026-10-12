//! Bounded, CPU-only import of static, opaque glTF 2.0 triangle geometry.
//! Buffer fetching belongs to the caller; this module performs no file or network I/O.
use glam::{Mat3, Mat4, Vec3};

const MAX_VERTICES: usize = 1_000_000;
const MAX_INDICES: usize = 3_000_000;
const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_NODES: usize = 100_000;

#[derive(Clone, Debug)]
pub struct ImportedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}
#[derive(Clone, Debug)]
pub struct ImportedMesh {
    pub vertices: Vec<ImportedVertex>,
    pub indices: Vec<u32>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub warnings: Vec<String>,
}

fn document(bytes: &[u8]) -> Result<gltf::Gltf, String> {
    if bytes.len() > MAX_BYTES {
        return Err("glTF exceeds the 128 MiB input limit".into());
    }
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| format!("Invalid glTF: {e}"))?;
    if let Some(extension) = gltf.extensions_required().next() {
        return Err(format!(
            "Required glTF extension is unsupported: {extension}"
        ));
    }
    if let Some(extension) = gltf.extensions_used().find(|name| {
        matches!(
            *name,
            "KHR_draco_mesh_compression" | "EXT_meshopt_compression" | "KHR_mesh_quantization"
        )
    }) {
        return Err(format!(
            "Compressed/quantized glTF geometry is unsupported: {extension}"
        ));
    }
    Ok(gltf)
}

/// One URI per buffer index; an empty entry denotes the embedded GLB BIN chunk.
pub fn buffer_uris(bytes: &[u8]) -> Result<Vec<String>, String> {
    Ok(document(bytes)?
        .buffers()
        .map(|buffer| match buffer.source() {
            gltf::buffer::Source::Bin => String::new(),
            gltf::buffer::Source::Uri(uri) => uri.to_owned(),
        })
        .collect())
}

pub fn parse(bytes: &[u8], external_buffers: &[Vec<u8>]) -> Result<ImportedMesh, String> {
    let gltf = document(bytes)?;
    if gltf.skins().next().is_some() {
        return Err("Skinned glTF geometry is unsupported".into());
    }
    if gltf.textures().next().is_some() || gltf.images().next().is_some() {
        return Err("Textured glTF materials are unsupported; provide untextured vertex colors/baseColorFactor".into());
    }
    if gltf.nodes().count() > MAX_NODES {
        return Err("glTF exceeds the node limit".into());
    }
    let mut buffers: Vec<&[u8]> = Vec::new();
    let mut total_bytes = 0usize;
    for buffer in gltf.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => gltf.blob.as_deref().ok_or("Missing GLB BIN chunk")?,
            gltf::buffer::Source::Uri(_) => external_buffers
                .get(buffer.index())
                .map(Vec::as_slice)
                .ok_or_else(|| format!("Missing external buffer {}", buffer.index()))?,
        };
        total_bytes = total_bytes
            .checked_add(data.len())
            .ok_or("Buffer size overflow")?;
        if total_bytes > MAX_BYTES {
            return Err("glTF buffers exceed the 128 MiB limit".into());
        }
        if data.len() < buffer.length() {
            return Err(format!("Truncated buffer {}", buffer.index()));
        }
        // Readers may not consume bytes beyond declared byteLength (GLB padding is allowed).
        buffers.push(&data[..buffer.length()]);
    }
    // Validate ranges before invoking gltf's typed readers, which assume valid slices.
    for view in gltf.views() {
        let end = view
            .offset()
            .checked_add(view.length())
            .ok_or("Buffer view overflow")?;
        if end > buffers[view.buffer().index()].len() {
            return Err("Buffer view exceeds its buffer".into());
        }
    }
    for accessor in gltf.accessors() {
        if accessor.sparse().is_some() {
            return Err("Sparse glTF accessors are unsupported".into());
        }
        if accessor.count() > MAX_INDICES {
            return Err("Accessor exceeds the element limit".into());
        }
        let view = accessor.view().ok_or("Accessor has no buffer view")?;
        let stride = view.stride().unwrap_or(accessor.size());
        if stride < accessor.size() {
            return Err("Accessor stride is smaller than its element".into());
        }
        let span = accessor
            .count()
            .saturating_sub(1)
            .checked_mul(stride)
            .and_then(|n| {
                n.checked_add(if accessor.count() == 0 {
                    0
                } else {
                    accessor.size()
                })
            })
            .and_then(|n| n.checked_add(accessor.offset()))
            .ok_or("Accessor range overflow")?;
        if span > view.length() {
            return Err("Accessor exceeds its buffer view".into());
        }
    }
    let mut out = ImportedMesh {
        vertices: vec![],
        indices: vec![],
        bounds_min: [f32::INFINITY; 3],
        bounds_max: [f32::NEG_INFINITY; 3],
        warnings: vec![],
    };
    if gltf.animations().next().is_some() {
        out.warnings
            .push("Animations are not played; imported the default static node pose".into());
    }
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or("glTF has no scene")?;
    // Iterative traversal bounds stack usage even for hostile, deeply nested graphs.
    let mut stack: Vec<_> = scene.nodes().map(|node| (node, Mat4::IDENTITY)).collect();
    let mut seen = vec![false; gltf.nodes().count()];
    while let Some((node, parent)) = stack.pop() {
        if seen[node.index()] {
            return Err("glTF scene contains a cycle or repeated node".into());
        }
        seen[node.index()] = true;
        let model = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if model.x_axis.w != 0.0
            || model.y_axis.w != 0.0
            || model.z_axis.w != 0.0
            || model.w_axis.w != 1.0
        {
            return Err("glTF node transform must be affine".into());
        }
        let linear = Mat3::from_mat4(model);
        let determinant = linear.determinant();
        if !model.is_finite() || !determinant.is_finite() || determinant == 0.0 {
            return Err("Non-finite or singular glTF node transform".into());
        }
        let normal_matrix = linear.inverse().transpose();
        if !normal_matrix.is_finite() {
            return Err("Invalid glTF normal transform".into());
        }
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                append_primitive(
                    &primitive,
                    &buffers,
                    model,
                    normal_matrix,
                    determinant < 0.0,
                    &mut out,
                )?;
            }
        }
        stack.extend(node.children().map(|child| (child, model)));
        if stack.len() > MAX_NODES {
            return Err("glTF node traversal exceeds the node limit".into());
        }
    }
    if out.vertices.is_empty() || out.indices.is_empty() {
        return Err("glTF scene contains no triangle geometry".into());
    }
    Ok(out)
}

fn append_primitive(
    primitive: &gltf::Primitive<'_>,
    buffers: &[&[u8]],
    model: Mat4,
    normal_matrix: Mat3,
    mirrored: bool,
    out: &mut ImportedMesh,
) -> Result<(), String> {
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        return Err("Only glTF TRIANGLES are supported".into());
    }
    if primitive.morph_targets().next().is_some() {
        return Err("Morph targets are unsupported".into());
    }
    if primitive.attributes().any(|(semantic, _)| {
        matches!(
            semantic,
            gltf::Semantic::Joints(_) | gltf::Semantic::Weights(_)
        )
    }) {
        return Err("Skinned vertex attributes are unsupported".into());
    }
    let material = primitive.material();
    if material.alpha_mode() != gltf::material::AlphaMode::Opaque {
        return Err("glTF BLEND/MASK materials are unsupported".into());
    }
    if material.double_sided() {
        return Err("glTF doubleSided materials are unsupported by the renderer".into());
    }
    if !out
        .warnings
        .iter()
        .any(|w| w.contains("metallic/roughness"))
    {
        out.warnings.push("Imported base colors use the engine lighting model; glTF metallic/roughness PBR shading is not reproduced".into());
    }
    let position_accessor = primitive
        .get(&gltf::Semantic::Positions)
        .ok_or("Primitive is missing POSITION")?;
    if position_accessor.data_type() != gltf::accessor::DataType::F32
        || position_accessor.dimensions() != gltf::accessor::Dimensions::Vec3
    {
        return Err("POSITION must be float VEC3".into());
    }
    let count = position_accessor.count();
    if count == 0
        || out
            .vertices
            .len()
            .checked_add(count)
            .filter(|&n| n <= MAX_VERTICES)
            .is_none()
    {
        return Err("glTF exceeds the vertex limit or has empty geometry".into());
    }
    if let Some(accessor) = primitive.get(&gltf::Semantic::Normals) {
        if accessor.data_type() != gltf::accessor::DataType::F32
            || accessor.dimensions() != gltf::accessor::Dimensions::Vec3
            || accessor.count() != count
        {
            return Err("NORMAL must be float VEC3 with matching vertex count".into());
        }
    }
    if let Some(accessor) = primitive.get(&gltf::Semantic::Colors(0)) {
        if accessor.count() != count
            || !matches!(
                accessor.dimensions(),
                gltf::accessor::Dimensions::Vec3 | gltf::accessor::Dimensions::Vec4
            )
            || !matches!(
                accessor.data_type(),
                gltf::accessor::DataType::U8
                    | gltf::accessor::DataType::U16
                    | gltf::accessor::DataType::F32
            )
            || (accessor.data_type() != gltf::accessor::DataType::F32 && !accessor.normalized())
        {
            return Err(
                "COLOR_0 must be float or normalized unsigned VEC3/VEC4 with matching vertex count"
                    .into(),
            );
        }
    }
    if let Some(accessor) = primitive.indices() {
        if accessor.dimensions() != gltf::accessor::Dimensions::Scalar
            || !matches!(
                accessor.data_type(),
                gltf::accessor::DataType::U8
                    | gltf::accessor::DataType::U16
                    | gltf::accessor::DataType::U32
            )
            || accessor.normalized()
        {
            return Err("Indices must be unsigned scalar values".into());
        }
    }
    let index_count = primitive.indices().map_or(count, |a| a.count());
    if index_count == 0
        || index_count % 3 != 0
        || out
            .indices
            .len()
            .checked_add(index_count)
            .filter(|&n| n <= MAX_INDICES)
            .is_none()
    {
        return Err("Invalid triangle index count or glTF index limit exceeded".into());
    }
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).copied());
    let positions: Vec<_> = reader
        .read_positions()
        .ok_or("Cannot read POSITION")?
        .map(|p| model.transform_point3(Vec3::from_array(p)))
        .collect();
    if positions.len() != count || positions.iter().any(|p| !p.is_finite()) {
        return Err("Non-finite or incomplete POSITION data".into());
    }
    let mut indices: Vec<u32> = match reader.read_indices() {
        Some(values) => values.into_u32().collect(),
        None if primitive.indices().is_none() => (0..count as u32).collect(),
        None => return Err("Cannot read indices".into()),
    };
    if indices.len() != index_count || indices.iter().any(|&i| i as usize >= count) {
        return Err("Triangle index is outside vertex range".into());
    }
    if mirrored {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    let normals: Vec<Vec3> = if let Some(values) = reader.read_normals() {
        values
            .map(|n| normal_matrix * Vec3::from_array(n))
            .collect()
    } else {
        let mut normals = vec![Vec3::ZERO; count];
        for tri in indices.chunks_exact(3) {
            let a = positions[tri[0] as usize];
            let face = (positions[tri[1] as usize] - a).cross(positions[tri[2] as usize] - a);
            if !face.is_finite() {
                return Err("Triangle normal overflow".into());
            }
            for &i in tri {
                normals[i as usize] += face;
            }
        }
        normals
    };
    if normals.len() != count || normals.iter().any(|n| !n.is_finite()) {
        return Err("Non-finite or incomplete NORMAL data".into());
    }
    let colors: Vec<[f32; 4]> = reader
        .read_colors(0)
        .map(|c| c.into_rgba_f32().collect())
        .unwrap_or_else(|| vec![[1.0; 4]; count]);
    if colors.len() != count
        || colors
            .iter()
            .flatten()
            .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    {
        return Err("Invalid COLOR_0 data".into());
    }
    let factor = material.pbr_metallic_roughness().base_color_factor();
    if factor
        .iter()
        .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    {
        return Err("Invalid baseColorFactor".into());
    }
    let base = out.vertices.len() as u32;
    for ((position, normal), color) in positions.into_iter().zip(normals).zip(colors) {
        // Degenerate triangles/unreferenced vertices have no surface normal.
        let normal = normal.try_normalize().unwrap_or(Vec3::Y);
        for axis in 0..3 {
            out.bounds_min[axis] = out.bounds_min[axis].min(position[axis]);
            out.bounds_max[axis] = out.bounds_max[axis].max(position[axis]);
        }
        out.vertices.push(ImportedVertex {
            position: position.to_array(),
            normal: normal.to_array(),
            color: [
                color[0] * factor[0],
                color[1] * factor[1],
                color[2] * factor[2],
            ],
        });
    }
    out.indices.extend(indices.into_iter().map(|i| i + base));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_bytes() -> Vec<u8> {
        [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect()
    }
    fn json(
        buffer: &str,
        extra_views: &str,
        extra_accessors: &str,
        primitive: &str,
        extra: &str,
        nodes: &str,
    ) -> Vec<u8> {
        format!(r#"{{"asset":{{"version":"2.0"}},"buffers":[{buffer}],"bufferViews":[{{"buffer":0,"byteLength":36}}{extra_views}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}}{extra_accessors}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}, {primitive}}}]}}],"nodes":[{nodes}],"scenes":[{{"nodes":[0]}}],"scene":0{extra}}}"#).into_bytes()
    }
    fn basic() -> Vec<u8> {
        json(
            r#"{"uri":"triangle.bin","byteLength":36}"#,
            "",
            "",
            r#""mode":4"#,
            "",
            r#"{"mesh":0}"#,
        )
    }
    fn glb(json: &[u8], bin: &[u8]) -> Vec<u8> {
        let mut json = json.to_vec();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut bin = bin.to_vec();
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        bytes.extend_from_slice(&json);
        bytes.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x004e4942u32.to_le_bytes());
        bytes.extend_from_slice(&bin);
        bytes
    }
    #[test]
    fn external_nonindexed_triangle_generates_normals() {
        assert_eq!(buffer_uris(&basic()).unwrap(), ["triangle.bin"]);
        let imported = parse(&basic(), &[triangle_bytes()]).unwrap();
        assert_eq!(imported.indices, [0, 1, 2]);
        assert_eq!(imported.bounds_min, [0., 0., 0.]);
        assert_eq!(imported.bounds_max, [1., 1., 0.]);
        assert_eq!(imported.vertices[0].normal, [0., 0., 1.]);
    }
    #[test]
    fn binary_glb_and_uri_index_order() {
        let doc = json(
            r#"{"byteLength":36}"#,
            "",
            "",
            r#""mode":4"#,
            "",
            r#"{"mesh":0}"#,
        );
        let bytes = glb(&doc, &triangle_bytes());
        assert_eq!(buffer_uris(&bytes).unwrap(), [""]);
        assert_eq!(parse(&bytes, &[]).unwrap().vertices.len(), 3);
        let uri_doc = String::from_utf8(basic())
            .unwrap()
            .replace("triangle.bin", "data:application/octet-stream;base64,AA==");
        assert_eq!(
            buffer_uris(uri_doc.as_bytes()).unwrap(),
            ["data:application/octet-stream;base64,AA=="]
        );
        let two_buffers = String::from_utf8(doc).unwrap().replace(
            "\"buffers\":[{\"byteLength\":36}]",
            "\"buffers\":[{\"byteLength\":36},{\"uri\":\"extra.bin\",\"byteLength\":1}]",
        );
        let bytes = glb(two_buffers.as_bytes(), &triangle_bytes());
        assert_eq!(buffer_uris(&bytes).unwrap(), ["", "extra.bin"]);
        assert!(parse(&bytes, &[vec![0]]).is_err());
        assert!(parse(&bytes, &[vec![], vec![0]]).is_ok());
        assert!(parse(&bytes[..bytes.len() - 1], &[]).is_err());
    }
    #[test]
    fn nested_mirrored_transform_corrects_winding_and_normals() {
        let doc = json(
            r#"{"uri":"a.bin","byteLength":36}"#,
            "",
            "",
            r#""mode":4"#,
            "",
            r#"{"translation":[2,3,4],"children":[1]},{"mesh":0,"scale":[-2,3,1]}"#,
        );
        let mesh = parse(&doc, &[triangle_bytes()]).unwrap();
        assert_eq!(mesh.indices, [0, 2, 1]);
        assert_eq!(mesh.bounds_min, [0., 3., 4.]);
        assert_eq!(mesh.bounds_max, [2., 6., 4.]);
        assert_eq!(mesh.vertices[0].normal, [0., 0., 1.]);
    }
    #[test]
    fn interleaved_normalized_colors_and_all_index_widths() {
        for (component_type, width) in [(5121, 1), (5123, 2), (5125, 4)] {
            let mut bytes = vec![];
            for (pos, color) in [
                ([0f32, 0., 0.], [255u8, 0, 0, 255]),
                ([1., 0., 0.], [0, 255, 0, 255]),
                ([0., 1., 0.], [0, 0, 255, 255]),
            ] {
                bytes.extend(pos.into_iter().flat_map(f32::to_le_bytes));
                bytes.extend(color);
            }
            for i in 0u32..3 {
                bytes.extend_from_slice(&i.to_le_bytes()[..width]);
            }
            let doc = format!(
                r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"uri":"a.bin","byteLength":{len}}}],"bufferViews":[{{"buffer":0,"byteLength":48,"byteStride":16}},{{"buffer":0,"byteOffset":48,"byteLength":{idx_len}}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},{{"bufferView":0,"byteOffset":12,"componentType":5121,"normalized":true,"count":3,"type":"VEC4"}},{{"bufferView":1,"componentType":{component_type},"count":3,"type":"SCALAR"}}],"materials":[{{"pbrMetallicRoughness":{{"baseColorFactor":[0.5,0.25,1,1]}}}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0,"COLOR_0":1}},"indices":2,"material":0}}]}}],"nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}]}}"#,
                len = bytes.len(),
                idx_len = 3 * width
            );
            let mesh = parse(doc.as_bytes(), &[bytes]).unwrap();
            assert_eq!(mesh.indices, [0, 1, 2]);
            assert_eq!(mesh.vertices[0].color, [0.5, 0., 0.]);
            assert_eq!(mesh.vertices[1].color, [0., 0.25, 0.]);
            assert_eq!(mesh.vertices[2].color, [0., 0., 1.]);
        }
    }
    #[test]
    fn provided_normals_use_inverse_transpose() {
        let mut bytes = triangle_bytes();
        for _ in 0..3 {
            bytes.extend([1f32, 1., 0.].into_iter().flat_map(f32::to_le_bytes));
        }
        let doc = String::from_utf8(json(
            r#"{"uri":"a.bin","byteLength":72}"#,
            r#",{"buffer":0,"byteOffset":36,"byteLength":36}"#,
            r#",{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"}"#,
            r#""mode":4"#,
            "",
            r#"{"mesh":0,"scale":[2,1,1]}"#,
        ))
        .unwrap()
        .replace(r#""POSITION":0"#, r#""POSITION":0,"NORMAL":1"#);
        let mesh = parse(doc.as_bytes(), &[bytes]).unwrap();
        assert!((mesh.vertices[0].normal[0] - 0.4472136).abs() < 1e-6);
        assert!((mesh.vertices[0].normal[1] - 0.8944272).abs() < 1e-6);
    }
    #[test]
    fn rejects_missing_truncated_and_invalid_ranges() {
        assert!(parse(&basic(), &[])
            .unwrap_err()
            .contains("Missing external"));
        assert!(parse(&basic(), &[vec![0; 35]])
            .unwrap_err()
            .contains("Truncated"));
        let bad = String::from_utf8(basic()).unwrap().replace(
            "\"buffer\":0,\"byteLength\":36",
            "\"buffer\":0,\"byteLength\":32",
        );
        assert!(parse(bad.as_bytes(), &[vec![0; 36]])
            .unwrap_err()
            .contains("Accessor exceeds"));
        let mut bytes = triangle_bytes();
        bytes.extend_from_slice(&[0, 1, 3]);
        let doc = json(
            r#"{"uri":"a.bin","byteLength":39}"#,
            r#",{"buffer":0,"byteOffset":36,"byteLength":3}"#,
            r#",{"bufferView":1,"componentType":5121,"count":3,"type":"SCALAR"}"#,
            r#""indices":1"#,
            "",
            r#"{"mesh":0}"#,
        );
        assert!(parse(&doc, &[bytes])
            .unwrap_err()
            .contains("outside vertex"));
        let mut nonfinite = triangle_bytes();
        nonfinite[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(parse(&basic(), &[nonfinite])
            .unwrap_err()
            .contains("Non-finite"));
    }
    #[test]
    fn explicitly_rejects_unsupported_features_and_singular_nodes() {
        let bytes = triangle_bytes();
        for (extra, primitive, nodes, message) in [
            (
                r#", "materials":[{"alphaMode":"BLEND"}]"#,
                r#""material":0"#,
                r#"{"mesh":0}"#,
                "BLEND/MASK",
            ),
            (
                r#", "materials":[{"doubleSided":true}]"#,
                r#""material":0"#,
                r#"{"mesh":0}"#,
                "doubleSided",
            ),
            (
                r#", "images":[{"uri":"a.png"}],"textures":[{"source":0}]"#,
                r#""mode":4"#,
                r#"{"mesh":0}"#,
                "Textured",
            ),
            ("", r#""mode":1"#, r#"{"mesh":0}"#, "TRIANGLES"),
            (
                "",
                r#""mode":4"#,
                r#"{"mesh":0,"scale":[0,1,1]}"#,
                "singular",
            ),
            ("", r#""mode":4"#, r#"{"mesh":0,"children":[0]}"#, "cycle"),
        ] {
            let doc = json(
                r#"{"uri":"a.bin","byteLength":36}"#,
                "",
                "",
                primitive,
                extra,
                nodes,
            );
            assert!(parse(&doc, std::slice::from_ref(&bytes))
                .unwrap_err()
                .contains(message));
        }
    }
    #[test]
    fn real_sibenik_architecture_has_complete_large_triangle_mesh() {
        let bytes = include_bytes!("../assets/models/sibenik-cathedral/sibenik-geometry.glb");
        let model = parse(bytes, &[]).unwrap();
        assert_eq!(model.vertices.len(), 145_857);
        assert_eq!(model.indices.len(), 72_513 * 3);
        assert!(model.indices.iter().any(|&i| i > u16::MAX as u32));
        assert!(model.vertices.iter().all(|v| {
            Vec3::from_array(v.position).is_finite()
                && (Vec3::from_array(v.normal).length() - 1.0).abs() < 1e-5
        }));
        assert!(model.bounds_min[0] < -20.0 && model.bounds_max[1] >= 15.3);
    }
    #[test]
    fn cathedral_window_transmits_light_while_nearby_stone_blocks_it() {
        let model = parse(
            include_bytes!("../assets/models/sibenik-cathedral/sibenik-geometry.glb"),
            &[],
        )
        .unwrap();
        let to_local = |p: Vec3| (p - Vec3::new(0.0, 11.2498456, 0.0)) / 0.8;
        let blocked = |light: Vec3, end: Vec3| {
            let start = to_local(light);
            let direction = to_local(end) - start;
            model.indices.chunks_exact(3).any(|ids| {
                let a = Vec3::from_array(model.vertices[ids[0] as usize].position);
                let edge1 = Vec3::from_array(model.vertices[ids[1] as usize].position) - a;
                let edge2 = Vec3::from_array(model.vertices[ids[2] as usize].position) - a;
                let p = direction.cross(edge2);
                let determinant = edge1.dot(p);
                if determinant.abs() < 1e-8 {
                    return false;
                }
                let delta = start - a;
                let u = delta.dot(p) / determinant;
                if !(0.0..=1.0).contains(&u) {
                    return false;
                }
                let q = delta.cross(edge1);
                let v = direction.dot(q) / determinant;
                let t = edge2.dot(q) / determinant;
                v >= 0.0 && u + v <= 1.0 && t > 1e-5 && t < 0.9999
            })
        };
        assert!(
            !blocked(Vec3::new(-5.3, 15.6, -5.0), Vec3::new(-5.3, 0.0, 0.0)),
            "The demonstration light must enter through a real opening"
        );
        assert!(
            blocked(Vec3::new(-5.3, 15.6, -5.0), Vec3::new(-4.3, 0.0, 0.0)),
            "Window frames and surrounding stone must remain occluders"
        );
        assert!(
            !blocked(Vec3::new(-19.0, 12.0, 0.2), Vec3::new(0.0, -0.5, 0.2)),
            "Rose window apertures must transmit the scene light"
        );
        assert!(
            blocked(Vec3::new(-19.0, 11.1, 0.0), Vec3::new(0.0, -0.5, 0.0)),
            "Rose window stone center must cast a shadow"
        );
    }

    #[test]
    fn real_kenney_bridge_glb_has_finite_geometry() {
        let bytes = include_bytes!(
            "../assets/models/kenney-nature-bridge-stone-round/bridge_stone_round.glb"
        );
        let mesh = parse(bytes, &[]).unwrap();
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert!(mesh
            .bounds_min
            .iter()
            .chain(&mesh.bounds_max)
            .all(|v| v.is_finite()));
        assert!(mesh.bounds_max[0] > mesh.bounds_min[0]);
    }

    #[test]
    fn rejects_sparse_morph_required_extensions_and_oversize_accessors() {
        let source = String::from_utf8(basic()).unwrap();
        let sparse = source.replace("\"bufferView\":0,", "\"bufferView\":0,\"sparse\":{\"count\":1,\"indices\":{\"bufferView\":0,\"componentType\":5121},\"values\":{\"bufferView\":0}},");
        assert!(parse(sparse.as_bytes(), &[triangle_bytes()])
            .unwrap_err()
            .contains("Sparse"));
        let morph = source.replace("\"mode\":4", "\"mode\":4,\"targets\":[{\"POSITION\":0}]");
        assert!(parse(morph.as_bytes(), &[triangle_bytes()])
            .unwrap_err()
            .contains("Morph"));
        let huge = source.replace("\"count\":3", "\"count\":3000001");
        assert!(parse(huge.as_bytes(), &[triangle_bytes()])
            .unwrap_err()
            .contains("element limit"));
        let extension = source.replace("\"asset\":", "\"extensionsRequired\":[\"KHR_draco_mesh_compression\"],\"extensionsUsed\":[\"KHR_draco_mesh_compression\"],\"asset\":");
        assert!(parse(extension.as_bytes(), &[triangle_bytes()]).is_err());
        let compressed = source.replace(
            "\"asset\":",
            "\"extensionsUsed\":[\"EXT_meshopt_compression\"],\"asset\":",
        );
        assert!(parse(compressed.as_bytes(), &[triangle_bytes()])
            .unwrap_err()
            .contains("Compressed"));
    }

    #[test]
    fn static_animation_warning_and_skin_rejection() {
        let source = String::from_utf8(basic()).unwrap();
        let animated = source.replace(
            "\"asset\":",
            "\"animations\":[{\"channels\":[],\"samplers\":[]}],\"asset\":",
        );
        let mesh = parse(animated.as_bytes(), &[triangle_bytes()]).unwrap();
        assert!(mesh.warnings[0].contains("static node pose"));
        let skinned = source.replace("\"asset\":", "\"skins\":[{\"joints\":[0]}],\"asset\":");
        assert!(parse(skinned.as_bytes(), &[triangle_bytes()])
            .unwrap_err()
            .contains("Skinned"));
    }
}
