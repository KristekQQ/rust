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
        || !index_count.is_multiple_of(3)
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
        for triangle in indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
    }
    let normals: Vec<Vec3> = if let Some(values) = reader.read_normals() {
        values
            .map(|n| normal_matrix * Vec3::from_array(n))
            .collect()
    } else {
        let mut normals = vec![Vec3::ZERO; count];
        for tri in indices.as_chunks::<3>().0 {
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
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = bin.to_vec();
        while !bin.len().is_multiple_of(4) {
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
            model.indices.as_chunks::<3>().0.iter().any(|ids| {
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

/// Index-preserving URI lists. Empty entries refer to embedded buffer/image views.
#[derive(Clone, Debug)]
pub struct ModelResources {
    pub buffers: Vec<String>,
    pub images: Vec<String>,
}
pub fn resource_uris(bytes: &[u8]) -> Result<ModelResources, String> {
    let gltf = core_document(bytes)?;
    Ok(ModelResources {
        buffers: gltf
            .buffers()
            .map(|b| match b.source() {
                gltf::buffer::Source::Bin => String::new(),
                gltf::buffer::Source::Uri(s) => s.into(),
            })
            .collect(),
        images: gltf
            .images()
            .map(|i| match i.source() {
                gltf::image::Source::View { .. } => String::new(),
                gltf::image::Source::Uri { uri, .. } => uri.into(),
            })
            .collect(),
    })
}
fn core_document(bytes: &[u8]) -> Result<gltf::Gltf, String> {
    if bytes.len() > MAX_BYTES {
        return Err("glTF exceeds the 128 MiB input limit".into());
    }
    let doc = gltf::Gltf::from_slice(bytes).map_err(|e| format!("Invalid glTF: {e}"))?;
    if let Some(extension) = doc.extensions_required().next() {
        return Err(format!("Required glTF extension unsupported: {extension}"));
    }
    if doc.nodes().count() > MAX_NODES
        || doc.accessors().count() > MAX_NODES
        || doc.meshes().count() > MAX_NODES
        || doc.views().count() > MAX_NODES
        || doc.materials().count() > MAX_NODES
        || doc.images().count() > MAX_NODES
        || doc.textures().count() > MAX_NODES
        || doc.samplers().count() > MAX_NODES
        || doc.skins().count() > MAX_NODES
        || doc.animations().count() > MAX_NODES
        || doc.scenes().count() > MAX_NODES
        || doc.cameras().count() > MAX_NODES
    {
        return Err("glTF metadata exceeds the object limit".into());
    }
    Ok(doc)
}
fn core_buffers<'a>(
    gltf: &'a gltf::Gltf,
    external: &'a [Vec<u8>],
) -> Result<Vec<&'a [u8]>, String> {
    let mut buffers = vec![];
    let mut bytes = 0usize;
    let mut scalars = 0usize;
    for buffer in gltf.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => gltf.blob.as_deref().ok_or("Missing GLB BIN")?,
            gltf::buffer::Source::Uri(_) => external
                .get(buffer.index())
                .map(Vec::as_slice)
                .ok_or("Missing external buffer")?,
        };
        bytes = bytes
            .checked_add(data.len())
            .ok_or("Buffer length overflow")?;
        if bytes > MAX_BYTES || data.len() < buffer.length() {
            return Err("Oversize or truncated glTF buffers".into());
        }
        buffers.push(&data[..buffer.length()]);
    }
    for view in gltf.views() {
        if view
            .offset()
            .checked_add(view.length())
            .filter(|&end| end <= buffers[view.buffer().index()].len())
            .is_none()
        {
            return Err("Buffer view exceeds buffer".into());
        }
    }
    for accessor in gltf.accessors() {
        scalars = scalars
            .checked_add(
                accessor
                    .count()
                    .checked_mul(accessor.dimensions().multiplicity())
                    .ok_or("Accessor size overflow")?,
            )
            .ok_or("Accessor size overflow")?;
        if accessor.count() > MAX_INDICES || scalars > 32_000_000 {
            return Err("glTF accessor element limit exceeded".into());
        }
        let size = accessor_element_size(&accessor);
        if let Some(view) = accessor.view() {
            let stride = view.stride().unwrap_or(size);
            if stride < size {
                return Err("Invalid accessor stride".into());
            }
            let end = accessor
                .count()
                .saturating_sub(1)
                .checked_mul(stride)
                .and_then(|n| n.checked_add(size))
                .and_then(|n| n.checked_add(accessor.offset()))
                .ok_or("Accessor range overflow")?;
            if end > view.length() {
                return Err("Accessor exceeds buffer view".into());
            }
        } else if accessor.sparse().is_none() {
            return Err("Accessor needs a buffer view or sparse data".into());
        }
        if let Some(sparse) = accessor.sparse() {
            if sparse.count() > accessor.count() {
                return Err("Sparse count exceeds accessor".into());
            }
            let indices = sparse.indices();
            let iv = indices.view();
            let is = indices.index_type().size();
            let ie = indices
                .offset()
                .checked_add(sparse.count().checked_mul(is).ok_or("Sparse overflow")?)
                .ok_or("Sparse overflow")?;
            let values = sparse.values();
            let vv = values.view();
            let ve = values
                .offset()
                .checked_add(sparse.count().checked_mul(size).ok_or("Sparse overflow")?)
                .ok_or("Sparse overflow")?;
            if iv.stride().is_some()
                || vv.stride().is_some()
                || ie > iv.length()
                || ve > vv.length()
            {
                return Err("Invalid sparse buffer range".into());
            }
            let bytes =
                &buffers[iv.buffer().index()][iv.offset() + indices.offset()..iv.offset() + ie];
            let mut previous = None;
            for b in bytes.chunks_exact(is) {
                let index = match is {
                    1 => b[0] as usize,
                    2 => u16::from_le_bytes([b[0], b[1]]) as usize,
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
                };
                if index >= accessor.count() || previous.is_some_and(|prev| index <= prev) {
                    return Err("Sparse indices must increase and remain in range".into());
                }
                previous = Some(index);
            }
        }
    }
    Ok(buffers)
}
fn accessor_element_size(accessor: &gltf::Accessor<'_>) -> usize {
    let component = accessor.data_type().size();
    let columns = match accessor.dimensions() {
        gltf::accessor::Dimensions::Mat2 => 2,
        gltf::accessor::Dimensions::Mat3 => 3,
        gltf::accessor::Dimensions::Mat4 => 4,
        _ => return accessor.size(),
    };
    (columns * component).div_ceil(4) * 4 * columns
}
fn checked_attribute(
    accessor: &gltf::Accessor<'_>,
    count: usize,
    dimensions: gltf::accessor::Dimensions,
    types: &[gltf::accessor::DataType],
    normalized: bool,
) -> Result<(), String> {
    if accessor.count() != count
        || accessor.dimensions() != dimensions
        || !types.contains(&accessor.data_type())
        || (normalized
            && accessor.data_type() != gltf::accessor::DataType::F32
            && !accessor.normalized())
    {
        return Err(format!("Invalid attribute accessor {}", accessor.index()));
    }
    Ok(())
}

/// Full glTF 2.0 core CPU import. External resources are supplied by document index.
pub fn parse_model(
    bytes: &[u8],
    external_buffers: &[Vec<u8>],
    external_images: &[Vec<u8>],
) -> Result<crate::gltf_model::ModelAsset, String> {
    use crate::gltf_model::*;
    use gltf::accessor::{DataType as D, Dimensions as A};
    let gltf = core_document(bytes)?;
    let buffers = core_buffers(&gltf, external_buffers)?;
    let mut asset = ModelAsset {
        vertices: vec![],
        primitives: vec![],
        meshes: vec![],
        materials: vec![],
        images: vec![],
        textures: vec![],
        samplers: vec![],
        nodes: vec![],
        skins: vec![],
        animations: vec![],
        scenes: vec![],
        default_scene: gltf.default_scene().map(|s| s.index()),
        cameras: vec![],
        warnings: vec![],
    };
    for extension in gltf.extensions_used() {
        asset.warnings.push(format!(
            "Optional extension {extension} ignored; using glTF core fallback"
        ));
    }
    let texture_slot = |info: gltf::texture::Info<'_>| TextureSlot {
        texture: info.texture().index(),
        tex_coord: info.tex_coord(),
    };
    for material in gltf.materials() {
        let pbr = material.pbr_metallic_roughness();
        let m = ModelMaterial {
            name: material.name().map(str::to_owned),
            base_color_factor: pbr.base_color_factor(),
            metallic_factor: pbr.metallic_factor(),
            roughness_factor: pbr.roughness_factor(),
            emissive_factor: material.emissive_factor(),
            base_color_texture: pbr.base_color_texture().map(texture_slot),
            metallic_roughness_texture: pbr.metallic_roughness_texture().map(texture_slot),
            normal_texture: material.normal_texture().map(|i| TextureSlot {
                texture: i.texture().index(),
                tex_coord: i.tex_coord(),
            }),
            normal_scale: material.normal_texture().map_or(1., |i| i.scale()),
            occlusion_texture: material.occlusion_texture().map(|i| TextureSlot {
                texture: i.texture().index(),
                tex_coord: i.tex_coord(),
            }),
            occlusion_strength: material.occlusion_texture().map_or(1., |i| i.strength()),
            emissive_texture: material.emissive_texture().map(texture_slot),
            alpha_mode: match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask,
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            },
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5),
            double_sided: material.double_sided(),
        };
        let factors = m
            .base_color_factor
            .iter()
            .chain(&m.emissive_factor)
            .copied()
            .chain([m.metallic_factor, m.roughness_factor, m.occlusion_strength]);
        if factors
            .clone()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            || !m.normal_scale.is_finite()
            || !m.alpha_cutoff.is_finite()
        {
            return Err("Invalid material factors".into());
        }
        asset.materials.push(m);
    }
    let mut decoded_bytes = 0usize;
    let mut encoded_bytes = buffers.iter().map(|b| b.len()).sum::<usize>();
    for image in gltf.images() {
        let (encoded, mime) = match image.source() {
            gltf::image::Source::View { view, mime_type } => (
                &buffers[view.buffer().index()][view.offset()..view.offset() + view.length()],
                Some(mime_type),
            ),
            gltf::image::Source::Uri { mime_type, .. } => (
                external_images
                    .get(image.index())
                    .map(Vec::as_slice)
                    .ok_or("Missing external glTF image")?,
                mime_type,
            ),
        };
        if matches!(image.source(), gltf::image::Source::Uri { .. }) {
            encoded_bytes = encoded_bytes
                .checked_add(encoded.len())
                .ok_or("Image size overflow")?;
        }
        if encoded_bytes > MAX_BYTES {
            return Err("Encoded glTF resources exceed 128 MiB".into());
        }
        let decoded = decode_core_image(encoded, mime)?;
        decoded_bytes = decoded_bytes
            .checked_add(decoded.2.len())
            .ok_or("Image size overflow")?;
        if decoded_bytes > 256 * 1024 * 1024 {
            return Err("Decoded glTF images exceed 256 MiB".into());
        }
        asset.images.push(ModelImage {
            name: image.name().map(str::to_owned),
            width: decoded.0,
            height: decoded.1,
            rgba: decoded.2,
        });
    }
    for sampler in gltf.samplers() {
        asset.samplers.push(ModelSampler {
            wrap_s: wrap_mode(sampler.wrap_s()),
            wrap_t: wrap_mode(sampler.wrap_t()),
            mag_filter: sampler.mag_filter().map(|f| match f {
                gltf::texture::MagFilter::Nearest => FilterMode::Nearest,
                gltf::texture::MagFilter::Linear => FilterMode::Linear,
            }),
            min_filter: sampler.min_filter().map(|f| match f {
                gltf::texture::MinFilter::Nearest => FilterMode::Nearest,
                gltf::texture::MinFilter::Linear => FilterMode::Linear,
                gltf::texture::MinFilter::NearestMipmapNearest => FilterMode::NearestMipmapNearest,
                gltf::texture::MinFilter::LinearMipmapNearest => FilterMode::LinearMipmapNearest,
                gltf::texture::MinFilter::NearestMipmapLinear => FilterMode::NearestMipmapLinear,
                gltf::texture::MinFilter::LinearMipmapLinear => FilterMode::LinearMipmapLinear,
            }),
        });
    }
    for texture in gltf.textures() {
        asset.textures.push(ModelTexture {
            image: texture.source().index(),
            sampler: texture.sampler().index(),
        });
    }
    let mut total_indices = 0usize;
    let mut morph_elements = 0usize;
    let mut influence_elements = 0usize;
    for mesh in gltf.meshes() {
        let mut imported = ModelMesh {
            name: mesh.name().map(str::to_owned),
            primitives: vec![],
            weights: vec![],
        };
        let mut mesh_targets = None;
        for primitive in mesh.primitives() {
            let position = primitive.get(&gltf::Semantic::Positions).ok_or(
                "Primitive missing POSITION; extension-only geometry has no core fallback",
            )?;
            let count = position.count();
            checked_attribute(&position, count, A::Vec3, &[D::F32], false)?;
            if count == 0
                || asset
                    .vertices
                    .len()
                    .checked_add(count)
                    .filter(|&n| n <= MAX_VERTICES)
                    .is_none()
            {
                return Err("glTF vertex limit exceeded".into());
            }
            // Validate typed reader shapes before they reinterpret components.
            for (semantic, accessor) in primitive.attributes() {
                match semantic {
                    gltf::Semantic::Extras(_) => {
                        if !asset
                            .warnings
                            .iter()
                            .any(|w| w.contains("Custom vertex attributes"))
                        {
                            asset.warnings.push("Custom vertex attributes are application-specific and are not rendered".into());
                        }
                    }
                    gltf::Semantic::Positions | gltf::Semantic::Normals => {
                        checked_attribute(&accessor, count, A::Vec3, &[D::F32], false)?
                    }
                    gltf::Semantic::Tangents => {
                        checked_attribute(&accessor, count, A::Vec4, &[D::F32], false)?
                    }
                    gltf::Semantic::TexCoords(_) => checked_attribute(
                        &accessor,
                        count,
                        A::Vec2,
                        &[D::F32, D::U8, D::U16],
                        true,
                    )?,
                    gltf::Semantic::Colors(_) => {
                        if !matches!(accessor.dimensions(), A::Vec3 | A::Vec4) {
                            return Err("COLOR must be VEC3/VEC4".into());
                        }
                        checked_attribute(
                            &accessor,
                            count,
                            accessor.dimensions(),
                            &[D::F32, D::U8, D::U16],
                            true,
                        )?;
                    }
                    gltf::Semantic::Joints(_) => {
                        checked_attribute(&accessor, count, A::Vec4, &[D::U8, D::U16], false)?;
                        if accessor.normalized() {
                            return Err("JOINTS must not be normalized".into());
                        }
                    }
                    gltf::Semantic::Weights(_) => checked_attribute(
                        &accessor,
                        count,
                        A::Vec4,
                        &[D::F32, D::U8, D::U16],
                        true,
                    )?,
                }
            }
            if let Some(indices) = primitive.indices() {
                checked_attribute(
                    &indices,
                    indices.count(),
                    A::Scalar,
                    &[D::U8, D::U16, D::U32],
                    false,
                )?;
                if indices.normalized() {
                    return Err("Indices cannot be normalized".into());
                }
            }
            let reader = primitive.reader(|b| buffers.get(b.index()).copied());
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or("Cannot read positions")?
                .collect();
            let indices: Vec<u32> = if primitive.indices().is_some() {
                reader
                    .read_indices()
                    .ok_or("Cannot read indices")?
                    .into_u32()
                    .collect()
            } else {
                (0..count as u32).collect()
            };
            total_indices = total_indices
                .checked_add(indices.len())
                .ok_or("Index size overflow")?;
            if total_indices > MAX_INDICES || indices.iter().any(|&i| i as usize >= count) {
                return Err("Invalid or oversize glTF indices".into());
            }
            let mode = match primitive.mode() {
                gltf::mesh::Mode::Points => PrimitiveMode::Points,
                gltf::mesh::Mode::Lines => PrimitiveMode::Lines,
                gltf::mesh::Mode::LineLoop => PrimitiveMode::LineLoop,
                gltf::mesh::Mode::LineStrip => PrimitiveMode::LineStrip,
                gltf::mesh::Mode::Triangles => PrimitiveMode::Triangles,
                gltf::mesh::Mode::TriangleStrip => PrimitiveMode::TriangleStrip,
                gltf::mesh::Mode::TriangleFan => PrimitiveMode::TriangleFan,
            };
            validate_core_mode(mode, indices.len())?;
            let normals: Vec<_> = reader
                .read_normals()
                .map(|v| v.collect())
                .unwrap_or_else(|| derive_core_normals(&positions, &indices, mode));
            let flat_normals = primitive.get(&gltf::Semantic::Normals).is_none();
            let tangents: Vec<_> = if flat_normals {
                vec![[0., 0., 0., 1.]; count]
            } else {
                reader
                    .read_tangents()
                    .map(|v| v.collect())
                    .unwrap_or_else(|| vec![[0., 0., 0., 1.]; count])
            };
            let colors: Vec<_> = reader
                .read_colors(0)
                .map(|v| v.into_rgba_f32().collect())
                .unwrap_or_else(|| vec![[1.; 4]; count]);
            let tex_sets: Vec<_> = primitive
                .attributes()
                .filter_map(|(s, _)| {
                    if let gltf::Semantic::TexCoords(i) = s {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();
            if let Some(index) = primitive.material().index() {
                let material = &asset.materials[index];
                for slot in [
                    material.base_color_texture,
                    material.metallic_roughness_texture,
                    material.normal_texture,
                    material.occlusion_texture,
                    material.emissive_texture,
                ]
                .into_iter()
                .flatten()
                {
                    if !tex_sets.contains(&slot.tex_coord) {
                        return Err("Textured primitive lacks its referenced TEXCOORD set".into());
                    }
                }
            }
            if tex_sets.iter().any(|&set| set >= 32) {
                return Err("Texture coordinate set limit exceeded".into());
            }
            let max_tex = tex_sets.iter().max().copied().map_or(0, |n| n as usize + 1);
            let mut tex_coords = vec![vec![[0.; 2]; count]; max_tex];
            for set in tex_sets {
                tex_coords[set as usize] = reader
                    .read_tex_coords(set)
                    .ok_or("Cannot read texture coordinates")?
                    .into_f32()
                    .collect();
            }
            let sets: Vec<_> = primitive
                .attributes()
                .filter_map(|(s, _)| {
                    if let gltf::Semantic::Joints(i) = s {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();
            let weight_sets = primitive
                .attributes()
                .filter(|(s, _)| matches!(s, gltf::Semantic::Weights(_)))
                .count();
            if sets.len() != weight_sets {
                return Err("JOINTS/WEIGHTS sets must match".into());
            }
            let mut influences = vec![vec![]; count];
            let mut joints0 = vec![[0u16; 4]; count];
            let mut weights0 = vec![[0.; 4]; count];
            for set in sets {
                if primitive.get(&gltf::Semantic::Weights(set)).is_none() {
                    return Err("JOINTS/WEIGHTS sets must match".into());
                }
                influence_elements = influence_elements
                    .checked_add(count.checked_mul(4).ok_or("Influence size overflow")?)
                    .ok_or("Influence size overflow")?;
                if influence_elements > 8_000_000 {
                    return Err("glTF influence limit exceeded".into());
                }
                let joints: Vec<_> = reader
                    .read_joints(set)
                    .ok_or("Cannot read JOINTS")?
                    .into_u16()
                    .collect();
                let weights: Vec<_> = reader
                    .read_weights(set)
                    .ok_or("Cannot read WEIGHTS")?
                    .into_f32()
                    .collect();
                if joints.len() != count || weights.len() != count {
                    return Err("Invalid joint/weight count".into());
                }
                for i in 0..count {
                    for j in 0..4 {
                        if !weights[i][j].is_finite() || weights[i][j] < 0. {
                            return Err("Invalid joint weight".into());
                        }
                        influences[i].push((joints[i][j], weights[i][j]));
                    }
                }
                if set == 0 {
                    joints0 = joints;
                    weights0 = weights;
                }
            }
            for weights in &mut influences {
                if !weights.is_empty() {
                    let sum = weights.iter().map(|(_, w)| w).sum::<f32>();
                    if !sum.is_finite() || sum <= 0. {
                        return Err("Joint weights must have positive finite sum".into());
                    }
                    for (_, w) in weights {
                        *w /= sum;
                    }
                }
            }
            let start = asset.vertices.len();
            for i in 0..count {
                let v = ModelVertex {
                    position: positions[i],
                    normal: normals[i],
                    tangent: tangents[i],
                    uv0: tex_coords.first().map_or([0.; 2], |v| v[i]),
                    uv1: tex_coords.get(1).map_or([0.; 2], |v| v[i]),
                    color: colors[i],
                    joints: joints0[i],
                    weights: weights0[i],
                    influences: std::mem::take(&mut influences[i]),
                    tex_coords: tex_coords.iter().map(|v| v[i]).collect(),
                };
                if v.position
                    .iter()
                    .chain(&v.normal)
                    .chain(&v.tangent)
                    .chain(&v.uv0)
                    .chain(&v.uv1)
                    .chain(&v.color)
                    .any(|v| !v.is_finite())
                    || v.tex_coords.iter().flatten().any(|v| !v.is_finite())
                    || v.color.iter().any(|v| !(0.0..=1.0).contains(v))
                {
                    return Err("Non-finite or invalid vertex attribute".into());
                }
                asset.vertices.push(v);
            }
            let mut targets = vec![];
            for target in primitive.morph_targets() {
                for a in [target.positions(), target.normals(), target.tangents()]
                    .into_iter()
                    .flatten()
                {
                    checked_attribute(&a, count, A::Vec3, &[D::F32], false)?;
                }
            }
            for (p, n, t) in reader.read_morph_targets() {
                morph_elements = morph_elements
                    .checked_add(count * 3)
                    .ok_or("Morph size overflow")?;
                if morph_elements > 8_000_000 {
                    return Err("glTF morph limit exceeded".into());
                }
                let target = MorphTarget {
                    positions: p.map(|v| v.collect()).unwrap_or_default(),
                    normals: n.map(|v| v.collect()).unwrap_or_default(),
                    tangents: t.map(|v| v.collect()).unwrap_or_default(),
                };
                if target
                    .positions
                    .iter()
                    .chain(&target.normals)
                    .chain(&target.tangents)
                    .flatten()
                    .any(|v| !v.is_finite())
                {
                    return Err("Invalid morph target".into());
                }
                targets.push(target);
            }
            if mesh_targets
                .replace(targets.len())
                .is_some_and(|n| n != targets.len())
            {
                return Err("Mesh primitives must have matching morph target counts".into());
            }
            imported.primitives.push(asset.primitives.len());
            asset.primitives.push(ModelPrimitive {
                flat_normals,
                vertices: start..asset.vertices.len(),
                indices,
                mode,
                material: primitive.material().index(),
                morph_targets: targets,
            });
        }
        imported.weights = mesh
            .weights()
            .map(|w| w.to_vec())
            .unwrap_or_else(|| vec![0.; mesh_targets.unwrap_or(0)]);
        if imported.weights.len() != mesh_targets.unwrap_or(0)
            || imported.weights.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid mesh morph weights".into());
        }
        asset.meshes.push(imported);
    }
    for node in gltf.nodes() {
        let (trs, matrix) = match node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => (
                NodeTrs {
                    translation: Vec3::from_array(translation),
                    rotation: glam::Quat::from_array(rotation),
                    scale: Vec3::from_array(scale),
                },
                None,
            ),
            gltf::scene::Transform::Matrix { matrix } => (
                NodeTrs {
                    translation: Vec3::ZERO,
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::ONE,
                },
                Some(Mat4::from_cols_array_2d(&matrix)),
            ),
        };
        if !trs.translation.is_finite()
            || !trs.rotation.is_finite()
            || !trs.scale.is_finite()
            || (trs.rotation.length_squared() - 1.).abs() > 0.001
            || matrix.is_some_and(|m| {
                !m.is_finite()
                    || m.x_axis.w != 0.
                    || m.y_axis.w != 0.
                    || m.z_axis.w != 0.
                    || m.w_axis.w != 1.
            })
        {
            return Err("Invalid node transform".into());
        }
        let weights = node.weights().map(|w| w.to_vec());
        if let Some(w) = &weights {
            if node
                .mesh()
                .is_none_or(|m| w.len() != asset.meshes[m.index()].weights.len())
                || w.iter().any(|v| !v.is_finite())
            {
                return Err("Invalid node morph weights".into());
            }
        }
        asset.nodes.push(ModelNode {
            name: node.name().map(str::to_owned),
            children: node.children().map(|n| n.index()).collect(),
            mesh: node.mesh().map(|m| m.index()),
            skin: node.skin().map(|s| s.index()),
            camera: node.camera().map(|c| c.index()),
            trs,
            matrix,
            weights,
        });
    }
    for skin in gltf.skins() {
        let joints: Vec<_> = skin.joints().map(|j| j.index()).collect();
        if joints.is_empty() || joints.len() > 65536 {
            return Err("Invalid skin joint count".into());
        }
        if let Some(ibm) = skin.inverse_bind_matrices() {
            checked_attribute(&ibm, joints.len(), A::Mat4, &[D::F32], false)?;
        }
        let inverse_bind_matrices: Vec<_> = skin
            .reader(|b| buffers.get(b.index()).copied())
            .read_inverse_bind_matrices()
            .map(|m| m.map(|v| Mat4::from_cols_array_2d(&v)).collect())
            .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
        if inverse_bind_matrices.iter().any(|m| !m.is_finite()) {
            return Err("Invalid inverse bind matrices".into());
        }
        asset.skins.push(ModelSkin {
            name: skin.name().map(str::to_owned),
            joints,
            inverse_bind_matrices,
            skeleton: skin.skeleton().map(|n| n.index()),
        });
    }
    for node in &asset.nodes {
        if let (Some(mesh), Some(skin)) = (node.mesh, node.skin) {
            for &p in &asset.meshes[mesh].primitives {
                for v in &asset.vertices[asset.primitives[p].vertices.clone()] {
                    if v.influences.is_empty()
                        || v.influences
                            .iter()
                            .any(|&(joint, _)| joint as usize >= asset.skins[skin].joints.len())
                    {
                        return Err(
                            "Skinned primitive has missing or invalid joint influences".into()
                        );
                    }
                }
            }
        }
    }
    let mut expanded_animation_scalars = 0usize;
    for animation in gltf.animations() {
        let mut clip = AnimationClip {
            name: animation.name().map(str::to_owned),
            channels: vec![],
            duration: 0.,
        };
        let mut targets = std::collections::HashSet::new();
        for channel in animation.channels() {
            let node = channel.target().node().index();
            let property = match channel.target().property() {
                gltf::animation::Property::Translation => AnimationProperty::Translation,
                gltf::animation::Property::Rotation => AnimationProperty::Rotation,
                gltf::animation::Property::Scale => AnimationProperty::Scale,
                gltf::animation::Property::MorphTargetWeights => AnimationProperty::Weights,
            };
            if !targets.insert((node, property as u8)) {
                return Err("Animation channels duplicate a node property".into());
            }
            if property != AnimationProperty::Weights && asset.nodes[node].matrix.is_some() {
                return Err("Matrix-authored nodes cannot have TRS animations".into());
            }
            let components = match property {
                AnimationProperty::Rotation => 4,
                AnimationProperty::Weights => asset.nodes[node]
                    .mesh
                    .map(|m| asset.meshes[m].weights.len())
                    .ok_or("Weight animation targets node without mesh")?,
                _ => 3,
            };
            let sampler = channel.sampler();
            let interpolation = match sampler.interpolation() {
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };
            let input = sampler.input();
            checked_attribute(&input, input.count(), A::Scalar, &[D::F32], false)?;
            let output = sampler.output();
            checked_attribute(
                &output,
                output.count(),
                match property {
                    AnimationProperty::Rotation => A::Vec4,
                    AnimationProperty::Weights => A::Scalar,
                    _ => A::Vec3,
                },
                &[D::F32],
                false,
            )?;
            let output_scalars = output
                .count()
                .checked_mul(output.dimensions().multiplicity())
                .ok_or("Animation size overflow")?;
            let expected = input
                .count()
                .checked_mul(components)
                .and_then(|n| {
                    n.checked_mul(if interpolation == Interpolation::CubicSpline {
                        3
                    } else {
                        1
                    })
                })
                .ok_or("Animation size overflow")?;
            if components == 0 || output_scalars != expected {
                return Err("Invalid animation output count/data".into());
            }
            expanded_animation_scalars = expanded_animation_scalars
                .checked_add(input.count())
                .and_then(|n| n.checked_add(output_scalars))
                .ok_or("Animation size overflow")?;
            if expanded_animation_scalars > 16 * 1024 * 1024 {
                return Err("Expanded animation data exceeds memory limit".into());
            }
            let reader = channel.reader(|b| buffers.get(b.index()).copied());
            let times: Vec<_> = reader
                .read_inputs()
                .ok_or("Cannot read animation timestamps")?
                .collect();
            if times.is_empty()
                || times.iter().any(|t| !t.is_finite() || *t < 0.)
                || times.windows(2).any(|w| w[1] <= w[0])
            {
                return Err("Animation timestamps must be finite and strictly increasing".into());
            }
            let values: Vec<f32> = match reader
                .read_outputs()
                .ok_or("Cannot read animation output")?
            {
                gltf::animation::util::ReadOutputs::Translations(v)
                | gltf::animation::util::ReadOutputs::Scales(v) => v.flatten().collect(),
                gltf::animation::util::ReadOutputs::Rotations(v) => {
                    v.into_f32().flatten().collect()
                }
                gltf::animation::util::ReadOutputs::MorphTargetWeights(v) => v.into_f32().collect(),
            };
            if values.len() != expected || values.iter().any(|v| !v.is_finite()) {
                return Err("Invalid animation output count/data".into());
            }
            if property == AnimationProperty::Rotation {
                let width = components
                    * if interpolation == Interpolation::CubicSpline {
                        3
                    } else {
                        1
                    };
                for key in values.chunks_exact(width) {
                    let value = if interpolation == Interpolation::CubicSpline {
                        &key[4..8]
                    } else {
                        key
                    };
                    let length = value.iter().map(|v| v * v).sum::<f32>();
                    if (length - 1.).abs() > 0.001 {
                        return Err("Animation rotation key must be a unit quaternion".into());
                    }
                }
            }
            clip.duration = clip.duration.max(*times.last().unwrap());
            clip.channels.push(AnimationChannel {
                node,
                property,
                interpolation,
                times,
                values,
                components,
            });
        }
        asset.animations.push(clip);
    }
    for scene in gltf.scenes() {
        asset.scenes.push(ModelScene {
            name: scene.name().map(str::to_owned),
            nodes: scene.nodes().map(|n| n.index()).collect(),
        });
    }
    for camera in gltf.cameras() {
        match camera.projection() {
            gltf::camera::Projection::Perspective(p) => {
                if !p.yfov().is_finite()
                    || p.yfov() <= 0.
                    || p.yfov() >= std::f32::consts::PI
                    || !p.znear().is_finite()
                    || p.znear() <= 0.
                    || p.zfar()
                        .is_some_and(|far| !far.is_finite() || far <= p.znear())
                    || p.aspect_ratio()
                        .is_some_and(|ratio| !ratio.is_finite() || ratio <= 0.)
                {
                    return Err("Invalid perspective camera parameters".into());
                }
            }
            gltf::camera::Projection::Orthographic(o) => {
                if [o.xmag(), o.ymag(), o.znear(), o.zfar()]
                    .iter()
                    .any(|v| !v.is_finite())
                    || o.xmag() <= 0.
                    || o.ymag() <= 0.
                    || o.znear() < 0.
                    || o.zfar() <= o.znear()
                {
                    return Err("Invalid orthographic camera parameters".into());
                }
            }
        }
        asset.cameras.push(match camera.projection() {
            gltf::camera::Projection::Perspective(p) => ModelCamera::Perspective {
                name: camera.name().map(str::to_owned),
                yfov: p.yfov(),
                aspect_ratio: p.aspect_ratio(),
                znear: p.znear(),
                zfar: p.zfar(),
            },
            gltf::camera::Projection::Orthographic(o) => ModelCamera::Orthographic {
                name: camera.name().map(str::to_owned),
                xmag: o.xmag(),
                ymag: o.ymag(),
                znear: o.znear(),
                zfar: o.zfar(),
            },
        });
    }
    // Validate graph/skin relationships even when no scene is selected for rendering.
    crate::gltf_animation::rest_pose(&asset)?;
    let children: std::collections::HashSet<_> = asset
        .nodes
        .iter()
        .flat_map(|n| n.children.iter().copied())
        .collect();
    if asset.scenes.is_empty() {
        asset.scenes.push(ModelScene {
            name: None,
            nodes: (0..asset.nodes.len())
                .filter(|node| !children.contains(node))
                .collect(),
        });
        asset.default_scene = Some(0);
    }
    for scene in &asset.scenes {
        let mut roots = std::collections::HashSet::new();
        if scene
            .nodes
            .iter()
            .any(|&node| children.contains(&node) || !roots.insert(node))
        {
            return Err("Scene roots must be distinct nodes without parents".into());
        }
    }
    Ok(asset)
}
fn wrap_mode(mode: gltf::texture::WrappingMode) -> crate::gltf_model::WrapMode {
    match mode {
        gltf::texture::WrappingMode::ClampToEdge => crate::gltf_model::WrapMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => crate::gltf_model::WrapMode::MirroredRepeat,
        gltf::texture::WrappingMode::Repeat => crate::gltf_model::WrapMode::Repeat,
    }
}
fn decode_core_image(bytes: &[u8], mime: Option<&str>) -> Result<(u32, u32, Vec<u8>), String> {
    use std::io::Cursor;
    let format = image::guess_format(bytes).map_err(|e| format!("Invalid glTF image: {e}"))?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("glTF core images must be PNG or JPEG".into());
    }
    if mime.is_some_and(|m| {
        m != if format == image::ImageFormat::Png {
            "image/png"
        } else {
            "image/jpeg"
        }
    }) {
        return Err("Image MIME type does not match encoded image".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| format!("Cannot decode glTF image: {e}"))?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > 16_777_216 {
        return Err("glTF image pixel limit exceeded".into());
    }
    Ok((
        decoded.width(),
        decoded.height(),
        decoded.to_rgba8().into_raw(),
    ))
}
fn validate_core_mode(mode: crate::gltf_model::PrimitiveMode, count: usize) -> Result<(), String> {
    use crate::gltf_model::PrimitiveMode::*;
    let valid = match mode {
        Points => count >= 1,
        Lines => count >= 2 && count.is_multiple_of(2),
        LineLoop | LineStrip => count >= 2,
        Triangles => count >= 3 && count.is_multiple_of(3),
        TriangleStrip | TriangleFan => count >= 3,
    };
    if valid {
        Ok(())
    } else {
        Err("Invalid glTF primitive element count".into())
    }
}
pub fn triangle_indices(indices: &[u32], mode: crate::gltf_model::PrimitiveMode) -> Vec<u32> {
    use crate::gltf_model::PrimitiveMode::*;
    match mode {
        Triangles => indices.to_vec(),
        TriangleStrip => indices
            .windows(3)
            .enumerate()
            .flat_map(|(i, t)| {
                if i % 2 == 0 {
                    [t[0], t[1], t[2]]
                } else {
                    [t[1], t[0], t[2]]
                }
            })
            .collect(),
        TriangleFan => indices[1..]
            .windows(2)
            .flat_map(|t| [indices[0], t[0], t[1]])
            .collect(),
        _ => vec![],
    }
}
fn derive_core_normals(
    positions: &[[f32; 3]],
    indices: &[u32],
    mode: crate::gltf_model::PrimitiveMode,
) -> Vec<[f32; 3]> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for tri in triangle_indices(indices, mode).as_chunks::<3>().0 {
        let a = Vec3::from_array(positions[tri[0] as usize]);
        let face = (Vec3::from_array(positions[tri[1] as usize]) - a)
            .cross(Vec3::from_array(positions[tri[2] as usize]) - a);
        for &i in tri {
            normals[i as usize] += face;
        }
    }
    normals
        .into_iter()
        .map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array())
        .collect()
}

#[cfg(test)]
mod core_tests {
    use super::*;
    use crate::{gltf_animation::*, gltf_model::*};
    use serde_json::{json, Value};
    use std::sync::Arc;
    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
    fn add_accessor(
        doc: &mut Value,
        bytes: &mut Vec<u8>,
        data: &[u8],
        component: u32,
        count: usize,
        kind: &str,
    ) -> usize {
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        let view = doc["bufferViews"].as_array().unwrap().len();
        let offset = bytes.len();
        bytes.extend_from_slice(data);
        doc["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()}));
        let index = doc["accessors"].as_array().unwrap().len();
        let mut accessor =
            json!({"bufferView":view,"componentType":component,"count":count,"type":kind});
        if index == 0 {
            accessor["min"] = json!([0, 0, 0]);
            accessor["max"] = json!([1, 1, 0]);
        }
        doc["accessors"].as_array_mut().unwrap().push(accessor);
        index
    }
    fn fixture() -> (Value, Vec<u8>) {
        let mut doc = json!({"asset":{"version":"2.0"},"buffers":[{"uri":"a.bin","byteLength":36}],"bufferViews":[],"accessors":[],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"nodes":[{"mesh":0}],"scenes":[{"nodes":[0]}]});
        let mut bytes = vec![];
        add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[0., 0., 0., 1., 0., 0., 0., 1., 0.]),
            5126,
            3,
            "VEC3",
        );
        (doc, bytes)
    }
    fn parse_fixture(
        mut doc: Value,
        bytes: Vec<u8>,
        images: Vec<Vec<u8>>,
    ) -> Result<ModelAsset, String> {
        doc["buffers"][0]["byteLength"] = json!(bytes.len());
        parse_model(&serde_json::to_vec(&doc).unwrap(), &[bytes], &images)
    }
    #[test]
    fn shared_morph_weights_and_invalid_pose_lengths_are_rejected() {
        let (doc, bytes) = fixture();
        let mut asset = parse_fixture(doc, bytes, vec![]).unwrap();
        let mut pose = rest_pose(&asset).unwrap();
        pose.world_matrices.clear();
        assert!(recompute_world(&asset, &mut pose)
            .unwrap_err()
            .contains("node count mismatch"));
        asset.meshes[0].weights = vec![0.; 1024];
        asset.nodes = vec![asset.nodes[0].clone(); 20_000];
        assert!(rest_pose(&asset)
            .unwrap_err()
            .contains("pose exceeds memory limit"));
    }
    #[test]
    fn excessive_shared_skin_pose_is_rejected_before_expansion() {
        let (mut doc, bytes) = fixture();
        doc["nodes"] = json!((0..5000).map(|_| json!({"skin":0})).collect::<Vec<_>>());
        doc["skins"] = json!([{"joints":(0..5000).collect::<Vec<_>>()}]);
        let error = parse_fixture(doc, bytes, vec![]).unwrap_err();
        assert!(error.contains("pose exceeds memory limit"), "{error}");
    }
    #[test]
    fn shared_animation_samplers_have_an_expanded_memory_budget() {
        let (mut doc, mut bytes) = fixture();
        let keys = 4096;
        let times = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&(0..keys).map(|i| i as f32).collect::<Vec<_>>()),
            5126,
            keys,
            "SCALAR",
        );
        let values = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&vec![0.; keys * 3]),
            5126,
            keys,
            "VEC3",
        );
        doc["nodes"] = json!((0..1100).map(|_| json!({})).collect::<Vec<_>>());
        doc["animations"] = json!([{"samplers":[{"input":times,"output":values}],"channels":(0..1100).map(|node| json!({"sampler":0,"target":{"node":node,"path":"translation"}})).collect::<Vec<_>>()}]);
        let error = parse_fixture(doc, bytes, vec![]).unwrap_err();
        assert!(
            error.contains("animation data exceeds memory limit"),
            "{error}"
        );
    }
    #[test]
    fn texture_coordinate_indices_are_bounded_before_wasm_conversion() {
        for set in [31u32, 32, u32::MAX] {
            let (mut doc, mut bytes) = fixture();
            let uv = add_accessor(&mut doc, &mut bytes, &floats(&[0.; 6]), 5126, 3, "VEC2");
            doc["meshes"][0]["primitives"][0]["attributes"][format!("TEXCOORD_{set}")] = json!(uv);
            let result = parse_fixture(doc, bytes, vec![]);
            if set == 31 {
                assert!(result.is_ok());
            } else {
                assert!(result.unwrap_err().contains("coordinate set limit"));
            }
        }
    }
    #[test]
    fn real_fox_skin_animation_and_helmet_pbr_images() {
        let fox = parse_model(include_bytes!("../assets/models/fox/Fox.glb"), &[], &[]).unwrap();
        assert!(!fox.skins.is_empty());
        assert!(fox.animations.len() >= 3);
        assert!(!fox.images.is_empty());
        let mut instance = ModelInstance::new(Arc::new(fox), None).unwrap();
        let before = model_bounds(&instance.asset, &instance.pose).unwrap();
        instance.play(0, true).unwrap();
        instance.seek(0.5).unwrap();
        let after = model_bounds(&instance.asset, &instance.pose).unwrap();
        assert_ne!(before, after);
        assert!(after.0.is_finite() && after.1.is_finite());
        let helmet = parse_model(
            include_bytes!("../assets/models/damaged-helmet/DamagedHelmet.glb"),
            &[],
            &[],
        )
        .unwrap();
        let m = &helmet.materials[0];
        assert!(m.base_color_texture.is_some());
        assert!(m.metallic_roughness_texture.is_some());
        assert!(m.normal_texture.is_some());
        assert!(m.occlusion_texture.is_some());
        assert!(m.emissive_texture.is_some());
        assert_eq!(helmet.images.len(), 5);
        assert!(helmet.vertices.iter().all(|v| !v.tex_coords.is_empty()));
    }
    #[test]
    fn all_core_modes_and_external_png_jpeg_material_uv_sets() {
        for mode in 0..=6 {
            let (mut doc, mut bytes) = fixture();
            if mode == 1 {
                let accessor = add_accessor(&mut doc, &mut bytes, &[0, 1], 5121, 2, "SCALAR");
                doc["meshes"][0]["primitives"][0]["indices"] = json!(accessor);
            }
            doc["meshes"][0]["primitives"][0]["mode"] = json!(mode);
            assert!(parse_fixture(doc, bytes, vec![]).is_ok());
        }
        for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
            let (mut doc, mut bytes) = fixture();
            let uv = add_accessor(
                &mut doc,
                &mut bytes,
                &floats(&[0., 0., 1., 0., 0., 1.]),
                5126,
                3,
                "VEC2",
            );
            for set in 0..3 {
                doc["meshes"][0]["primitives"][0]["attributes"][format!("TEXCOORD_{set}")] =
                    json!(uv);
            }
            doc["meshes"][0]["primitives"][0]["material"] = json!(0);
            doc["materials"] = json!([{"pbrMetallicRoughness":{"baseColorFactor":[0.25,0.5,0.75,0.6],"metallicFactor":0.4,"roughnessFactor":0.7,"baseColorTexture":{"index":0,"texCoord":2}},"alphaMode":"BLEND","doubleSided":true}]);
            doc["images"] = json!([{"uri":"image.bin"}]);
            doc["textures"] = json!([{"source":0,"sampler":0}]);
            doc["samplers"] =
                json!([{"wrapS":33071,"wrapT":33648,"minFilter":9987,"magFilter":9728}]);
            let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                2,
                1,
                image::Rgb([12, 34, 56]),
            ));
            let mut cursor = std::io::Cursor::new(vec![]);
            image.write_to(&mut cursor, format).unwrap();
            let asset = parse_fixture(doc, bytes, vec![cursor.into_inner()]).unwrap();
            assert_eq!(asset.images[0].rgba.len(), 8);
            assert_eq!(asset.vertices[0].tex_coords.len(), 3);
            assert_eq!(asset.materials[0].alpha_mode, AlphaMode::Blend);
            assert!(asset.materials[0].double_sided);
            assert_eq!(asset.materials[0].base_color_texture.unwrap().tex_coord, 2);
            assert_eq!(asset.samplers[0].wrap_s, WrapMode::ClampToEdge);
            assert_eq!(asset.samplers[0].wrap_t, WrapMode::MirroredRepeat);
        }
    }
    #[test]
    fn sparse_positions_without_base_and_invalid_sparse_indices() {
        let (mut doc, mut bytes) = fixture();
        let indices = add_accessor(&mut doc, &mut bytes, &[0, 1, 2], 5121, 3, "SCALAR");
        let index_view = doc["accessors"][indices]["bufferView"].clone();
        doc["accessors"][0]
            .as_object_mut()
            .unwrap()
            .remove("bufferView");
        doc["accessors"][0]["sparse"] = json!({"count":3,"indices":{"bufferView":index_view,"componentType":5121},"values":{"bufferView":0}});
        let asset = parse_fixture(doc.clone(), bytes.clone(), vec![]).unwrap();
        assert_eq!(asset.vertices[1].position, [1., 0., 0.]);
        let offset = doc["bufferViews"][1]["byteOffset"].as_u64().unwrap() as usize;
        bytes[offset + 1] = 0;
        assert!(parse_fixture(doc, bytes, vec![])
            .unwrap_err()
            .contains("Sparse indices"));
    }
    #[test]
    fn all_joint_sets_morph_then_skin_and_singular_skin_fallback() {
        let (mut doc, mut bytes) = fixture();
        let joints0 = add_accessor(&mut doc, &mut bytes, &[0u8; 12], 5121, 3, "VEC4");
        let weights0 = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.]),
            5126,
            3,
            "VEC4",
        );
        let joints1 = add_accessor(&mut doc, &mut bytes, &[1u8; 12], 5121, 3, "VEC4");
        let weights1 = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.]),
            5126,
            3,
            "VEC4",
        );
        let morph = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[1., 0., 0., 1., 0., 0., 1., 0., 0.]),
            5126,
            3,
            "VEC3",
        );
        doc["meshes"][0]["primitives"][0]["attributes"]["JOINTS_0"] = json!(joints0);
        doc["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"] = json!(weights0);
        doc["meshes"][0]["primitives"][0]["attributes"]["JOINTS_1"] = json!(joints1);
        doc["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_1"] = json!(weights1);
        doc["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":morph}]);
        doc["meshes"][0]["weights"] = json!([0.5]);
        doc["nodes"] = json!([{"mesh":0,"skin":0},{"translation":[2,0,0]},{"translation":[4,0,0]}]);
        doc["skins"] = json!([{"joints":[1,2]}]);
        doc["scenes"][0]["nodes"] = json!([0, 1, 2]);
        let mut asset = parse_fixture(doc, bytes, vec![]).unwrap();
        let pose = default_pose(&asset, 0).unwrap();
        assert_eq!(asset.vertices[0].influences.len(), 8);
        assert_eq!(
            deformed_vertices(&asset, &pose, 0, 0).unwrap()[0].position,
            [3.5, 0., 0.]
        );
        asset.nodes[0].trs.scale = Vec3::ZERO;
        let pose = default_pose(&asset, 0).unwrap();
        assert!(pose.skin_world_space[0]);
        assert_eq!(node_render_matrix(&pose, 0), Mat4::IDENTITY);
        assert_eq!(
            deformed_vertices(&asset, &pose, 0, 0).unwrap()[0].position,
            [3.5, 0., 0.]
        );
    }
    #[test]
    fn model_mutations_and_updates_are_atomic_on_deformation_overflow() {
        let (mut doc, mut bytes) = fixture();
        let morph = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[10., 0., 0., 10., 0., 0., 10., 0., 0.]),
            5126,
            3,
            "VEC3",
        );
        doc["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":morph}]);
        doc["meshes"][0]["weights"] = json!([0.]);
        let times = add_accessor(&mut doc, &mut bytes, &floats(&[0., 1.]), 5126, 2, "SCALAR");
        let values = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[0., f32::MAX]),
            5126,
            2,
            "SCALAR",
        );
        doc["animations"] = json!([{"samplers":[{"input":times,"output":values}],"channels":[{"sampler":0,"target":{"node":0,"path":"weights"}}]}]);
        let asset = Arc::new(parse_fixture(doc, bytes, vec![]).unwrap());
        let mut scene = crate::scene::SceneManager::default();
        let id = scene.add_model(asset).unwrap();
        let revision = scene.model(id).unwrap().revision;
        let render_revision = scene.render_revision();
        assert!(scene.set_model_morph_weights(id, 0, &[f32::MAX]).is_err());
        assert_eq!(scene.model(id).unwrap().revision, revision);
        assert_eq!(scene.render_revision(), render_revision);
        assert_eq!(scene.model(id).unwrap().pose.node_weights[0], [0.]);
        scene.play_model_animation(id, 0, false, 1.).unwrap();
        let revision = scene.model(id).unwrap().revision;
        scene.update(1.);
        let model = scene.model(id).unwrap();
        assert_eq!(model.revision, revision);
        assert_eq!(model.pose.node_weights[0], [0.]);
        assert_eq!(model.playback.as_ref().unwrap().time, 0.);
        assert!(!model.playback.as_ref().unwrap().playing);
        assert!(model.last_error.as_ref().unwrap().contains("overflow"));
    }
    #[test]
    fn no_scene_synthesizes_node_roots_and_cameras_validate() {
        let (mut doc, bytes) = fixture();
        doc.as_object_mut().unwrap().remove("scenes");
        let asset = parse_fixture(doc, bytes, vec![]).unwrap();
        assert_eq!(asset.scenes.len(), 1);
        assert_eq!(asset.scenes[0].nodes, [0]);
        assert!(ModelInstance::new(Arc::new(asset), None).is_ok());
        let (mut doc, bytes) = fixture();
        doc["cameras"] = json!([{"type":"perspective","perspective":{"yfov":1.0,"znear":0.1}}]);
        doc["nodes"][0]["camera"] = json!(0);
        let asset = parse_fixture(doc.clone(), bytes.clone(), vec![]).unwrap();
        assert!(matches!(
            asset.cameras[0],
            ModelCamera::Perspective { zfar: None, .. }
        ));
        doc["cameras"][0]["perspective"]["yfov"] = json!(4.0);
        assert!(parse_fixture(doc, bytes, vec![]).is_err());
    }
    #[test]
    fn changing_clips_resets_unanimated_morph_weights_and_marks_flat_normals() {
        let (mut doc, mut bytes) = fixture();
        let morph = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[1., 0., 0., 1., 0., 0., 1., 0., 0.]),
            5126,
            3,
            "VEC3",
        );
        doc["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":morph}]);
        doc["meshes"][0]["weights"] = json!([0.25]);
        let mut asset = parse_fixture(doc, bytes, vec![]).unwrap();
        assert!(asset.primitives[0].flat_normals);
        assert_eq!(asset.vertices[0].tangent, [0., 0., 0., 1.]);
        asset.animations = vec![
            AnimationClip {
                name: None,
                duration: 1.,
                channels: vec![AnimationChannel {
                    node: 0,
                    property: AnimationProperty::Weights,
                    interpolation: Interpolation::Step,
                    times: vec![0.],
                    values: vec![0.75],
                    components: 1,
                }],
            },
            AnimationClip {
                name: None,
                duration: 1.,
                channels: vec![AnimationChannel {
                    node: 0,
                    property: AnimationProperty::Translation,
                    interpolation: Interpolation::Step,
                    times: vec![0.],
                    values: vec![0., 0., 0.],
                    components: 3,
                }],
            },
        ];
        let mut instance = ModelInstance::new(Arc::new(asset), None).unwrap();
        instance.play(0, false).unwrap();
        assert_eq!(instance.pose.node_weights[0], [0.75]);
        instance.play(1, false).unwrap();
        assert_eq!(instance.pose.node_weights[0], [0.25]);
        instance.set_morph_weights(0, &[0.4]).unwrap();
        instance.update(0.1).unwrap();
        assert_eq!(instance.pose.node_weights[0], [0.4]);
        instance.play(0, false).unwrap();
        instance.update(0.1).unwrap();
        assert_eq!(instance.pose.node_weights[0], [0.75]);
        instance.stop().unwrap();
        assert_eq!(instance.pose.node_weights[0], [0.25]);
        assert!(instance.morph_overrides.is_empty());
        assert_eq!(
            deformed_vertices(&instance.asset, &instance.pose, 0, 0).unwrap()[0].tangent,
            [0., 0., 0., 1.]
        );
    }
    #[test]
    fn imported_animation_and_scene_model_root_lifecycle() {
        let (mut doc, mut bytes) = fixture();
        let times = add_accessor(&mut doc, &mut bytes, &floats(&[0., 2.]), 5126, 2, "SCALAR");
        let values = add_accessor(
            &mut doc,
            &mut bytes,
            &floats(&[0., 0., 0., 4., 0., 0.]),
            5126,
            2,
            "VEC3",
        );
        doc["animations"] = json!([{"samplers":[{"input":times,"output":values,"interpolation":"LINEAR"}],"channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]}]);
        let asset = Arc::new(parse_fixture(doc, bytes, vec![]).unwrap());
        let mut scene = crate::scene::SceneManager::default();
        let id = scene.add_model(asset).unwrap();
        scene.play_model_animation(id, 0, false, 1.).unwrap();
        let revision = scene.model(id).unwrap().revision;
        scene.update(1.);
        assert!(scene.model(id).unwrap().revision > revision);
        assert_eq!(
            scene.model(id).unwrap().pose.local_trs[0].translation,
            Vec3::new(2., 0., 0.)
        );
        scene.pause_model_animation(id, true).unwrap();
        let revision = scene.model(id).unwrap().revision;
        scene.update(1.);
        assert_eq!(scene.model(id).unwrap().revision, revision);
        scene.seek_model_animation(id, 0.5).unwrap();
        assert_eq!(scene.model(id).unwrap().pose.local_trs[0].translation.x, 1.);
        scene.stop_model_animation(id).unwrap();
        assert_eq!(
            scene.model(id).unwrap().pose.local_trs[0].translation,
            Vec3::ZERO
        );
        scene.remove_object(id);
        assert!(scene.model(id).is_none());
        assert!(scene.play_model_animation(id, 0, false, 1.).is_err());
    }
}
