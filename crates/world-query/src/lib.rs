//! Engine-independent contact queries on triangle geometry.
//!
//! Coordinates stay in the source world's Y-up units. A renderer or physics
//! engine can submit its triangles without importing any game-specific code.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }

    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }

    fn scale(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    fn normalized(self) -> Option<Self> {
        let length = self.dot(self).sqrt();
        (length > 1e-8 && length.is_finite()).then(|| self.scale(1.0 / length))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

impl Triangle {
    pub const fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self { a, b, c }
    }

    pub fn normal(self) -> Option<Vec3> {
        self.b.sub(self.a).cross(self.c.sub(self.a)).normalized()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactKind {
    Ground,
    Wall,
    Ceiling,
}

impl ContactKind {
    pub fn from_normal(normal: Vec3) -> Self {
        if normal.y >= 0.65 {
            Self::Ground
        } else if normal.y <= -0.65 {
            Self::Ceiling
        } else {
            Self::Wall
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub point: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub kind: ContactKind,
    pub triangle_index: usize,
}

pub struct Mesh {
    triangles: Vec<Triangle>,
}

impl Mesh {
    pub fn triangles(&self) -> &[Triangle] { &self.triangles }

    pub fn new(triangles: Vec<Triangle>) -> Self {
        Self { triangles }
    }

    /// Load the vertex and face subset of Wavefront OBJ used by the exporter.
    pub fn from_obj_str(source: &str) -> Result<Self, String> {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for (line_index, line) in source.lines().enumerate() {
            let mut words = line.split_whitespace();
            match words.next() {
                Some("v") => {
                    let coordinates: Vec<f32> = words
                        .take(3)
                        .map(|word| word.parse::<f32>().map_err(|_| format!("invalid vertex on line {}", line_index + 1)))
                        .collect::<Result<_, _>>()?;
                    if coordinates.len() != 3 || !coordinates.iter().all(|value| value.is_finite()) {
                        return Err(format!("invalid vertex on line {}", line_index + 1));
                    }
                    vertices.push(Vec3::new(coordinates[0], coordinates[1], coordinates[2]));
                }
                Some("f") => {
                    let indices: Vec<usize> = words
                        .map(|word| obj_index(word, vertices.len(), line_index + 1))
                        .collect::<Result<_, _>>()?;
                    if indices.len() < 3 {
                        return Err(format!("face has fewer than 3 vertices on line {}", line_index + 1));
                    }
                    for index in 1..indices.len() - 1 {
                        triangles.push(Triangle::new(
                            vertices[indices[0]],
                            vertices[indices[index]],
                            vertices[indices[index + 1]],
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(Self::new(triangles))
    }

    /// Cast in any direction. `direction` is normalized internally.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<Hit> {
        self.cast(origin, direction, max_distance, None)
    }

    /// Find the nearest upward-facing surface below a point.
    pub fn ground_below(&self, origin: Vec3, max_distance: f32) -> Option<Hit> {
        self.cast(
            origin,
            Vec3::new(0.0, -1.0, 0.0),
            max_distance,
            Some(ContactKind::Ground),
        )
    }

    fn cast(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        filter: Option<ContactKind>,
    ) -> Option<Hit> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }
        let direction = direction.normalized()?;
        let mut best: Option<Hit> = None;
        for (triangle_index, triangle) in self.triangles.iter().copied().enumerate() {
            let Some(normal) = triangle.normal() else {
                continue;
            };
            let kind = ContactKind::from_normal(normal);
            if filter.is_some_and(|wanted| wanted != kind) {
                continue;
            }
            let edge1 = triangle.b.sub(triangle.a);
            let edge2 = triangle.c.sub(triangle.a);
            let h = direction.cross(edge2);
            let determinant = edge1.dot(h);
            if determinant.abs() < 1e-6 {
                continue;
            }
            let inverse = 1.0 / determinant;
            let s = origin.sub(triangle.a);
            let u = s.dot(h) * inverse;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(edge1);
            let v = direction.dot(q) * inverse;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let distance = edge2.dot(q) * inverse;
            if distance < 0.0 || distance > max_distance {
                continue;
            }
            if best.is_some_and(|hit| hit.distance <= distance) {
                continue;
            }
            best = Some(Hit {
                point: origin.add(direction.scale(distance)),
                normal,
                distance,
                kind,
                triangle_index,
            });
        }
        best
    }
}

fn obj_index(token: &str, count: usize, line: usize) -> Result<usize, String> {
    let raw = token.split('/').next().unwrap_or("");
    let index: isize = raw.parse().map_err(|_| format!("invalid face index on line {line}"))?;
    let resolved = if index > 0 { index - 1 } else { count as isize + index };
    if index == 0 || resolved < 0 || resolved >= count as isize {
        return Err(format!("face index outside vertex list on line {line}"));
    }
    Ok(resolved as usize)
}
