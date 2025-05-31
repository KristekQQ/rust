use crate::render::types::Vertex;

pub const VERTICES: &[Vertex] = &[
    // front - red
    Vertex { position: [-0.5, -0.5, 0.5], color: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, -0.5, 0.5], color: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, 0.5, 0.5], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, 0.5, 0.5], color: [1.0, 0.0, 0.0] },
    // back - green
    Vertex { position: [0.5, -0.5, -0.5], color: [0.0, 1.0, 0.0] },
    Vertex { position: [-0.5, -0.5, -0.5], color: [0.0, 1.0, 0.0] },
    Vertex { position: [-0.5, 0.5, -0.5], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, 0.5, -0.5], color: [0.0, 1.0, 0.0] },
    // left - blue
    Vertex { position: [-0.5, -0.5, -0.5], color: [0.0, 0.0, 1.0] },
    Vertex { position: [-0.5, -0.5, 0.5], color: [0.0, 0.0, 1.0] },
    Vertex { position: [-0.5, 0.5, 0.5], color: [0.0, 0.0, 1.0] },
    Vertex { position: [-0.5, 0.5, -0.5], color: [0.0, 0.0, 1.0] },
    // right - yellow
    Vertex { position: [0.5, -0.5, 0.5], color: [1.0, 1.0, 0.0] },
    Vertex { position: [0.5, -0.5, -0.5], color: [1.0, 1.0, 0.0] },
    Vertex { position: [0.5, 0.5, -0.5], color: [1.0, 1.0, 0.0] },
    Vertex { position: [0.5, 0.5, 0.5], color: [1.0, 1.0, 0.0] },
    // top - cyan
    Vertex { position: [-0.5, 0.5, 0.5], color: [0.0, 1.0, 1.0] },
    Vertex { position: [0.5, 0.5, 0.5], color: [0.0, 1.0, 1.0] },
    Vertex { position: [0.5, 0.5, -0.5], color: [0.0, 1.0, 1.0] },
    Vertex { position: [-0.5, 0.5, -0.5], color: [0.0, 1.0, 1.0] },
    // bottom - magenta
    Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0] },
    Vertex { position: [0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0] },
    Vertex { position: [0.5, -0.5, 0.5], color: [1.0, 0.0, 1.0] },
    Vertex { position: [-0.5, -0.5, 0.5], color: [1.0, 0.0, 1.0] },
];

pub const INDICES: &[u16] = &[
    0, 1, 2, 0, 2, 3,
    4, 5, 6, 4, 6, 7,
    8, 9, 10, 8, 10, 11,
    12, 13, 14, 12, 14, 15,
    16, 17, 18, 16, 18, 19,
    20, 21, 22, 20, 22, 23,
];
