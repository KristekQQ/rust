use glam::{Mat4, Vec3};

pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    (Mat4::from_cols_array_2d(&a) * Mat4::from_cols_array_2d(&b)).to_cols_array_2d()
}

pub fn rotation_y(angle: f32) -> [[f32; 4]; 4] {
    Mat4::from_rotation_y(angle).to_cols_array_2d()
}

pub fn rotation_z(angle: f32) -> [[f32; 4]; 4] {
    Mat4::from_rotation_z(angle).to_cols_array_2d()
}

pub fn translation(tx: f32, ty: f32, tz: f32) -> [[f32; 4]; 4] {
    Mat4::from_translation(Vec3::new(tx, ty, tz)).to_cols_array_2d()
}

pub fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    Mat4::look_at_lh(Vec3::from(eye), Vec3::from(center), Vec3::from(up)).to_cols_array_2d()
}

pub fn perspective(aspect: f32, fovy: f32, znear: f32, zfar: f32) -> [[f32; 4]; 4] {
    Mat4::perspective_lh(fovy, aspect, znear, zfar).to_cols_array_2d()
}

/// Levostranná perspektivní matice pro WebGPU (z ∈ [0‥1])
pub fn perspective_lh(aspect: f32, fovy: f32, znear: f32, zfar: f32) -> [[f32; 4]; 4] {
    Mat4::perspective_lh(fovy, aspect, znear, zfar).to_cols_array_2d()
}

pub fn transpose(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    Mat4::from_cols_array_2d(&m).transpose().to_cols_array_2d()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_z_pi() {
        let r = rotation_z(std::f32::consts::PI);
        let expected = [
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        for i in 0..4 {
            for j in 0..4 {
                assert!((r[i][j] - expected[i][j]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn perspective_identity_aspect_one() {
        let m = perspective(1.0, std::f32::consts::FRAC_PI_2, 0.1, 10.0);
        assert!((m[0][0] - 1.0).abs() < 0.0001);
        assert!((m[1][1] - 1.0).abs() < 0.0001);
    }

    #[test]
    fn transpose_roundtrip() {
        let m = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
            [13.0, 14.0, 15.0, 16.0],
        ];
        let t = transpose(transpose(m));
        assert_eq!(m, t);
    }

}
