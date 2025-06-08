use glam::Mat4;

#[derive(Clone)]
pub enum SceneObject {
    Cube { size: f32, color: [f32; 3], model: Mat4 },
    Light { position: [f32; 3], color: [f32; 3] },
    // TODO: Model { mesh_id: usize }
}

#[derive(Debug)]
pub enum SceneError {
    InvalidIndex,
}

pub struct SceneManager {
    objects: Vec<SceneObject>,
}

impl SceneManager {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    pub fn add_cube(
        &mut self,
        size: f32,
        color: [f32; 3],
        model: Mat4,
    ) -> Result<usize, SceneError> {
        self.objects.push(SceneObject::Cube { size, color, model });
        Ok(self.objects.len() - 1)
    }

    pub fn add_light(
        &mut self,
        position: [f32; 3],
        color: [f32; 3],
    ) -> Result<usize, SceneError> {
        self.objects.push(SceneObject::Light { position, color });
        Ok(self.objects.len() - 1)
    }

    pub fn remove(&mut self, idx: usize) -> Result<(), SceneError> {
        if idx < self.objects.len() {
            self.objects.remove(idx);
            Ok(())
        } else {
            Err(SceneError::InvalidIndex)
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &SceneObject> {
        self.objects.iter()
    }
}
