use world_query::{ContactKind, Mesh, Triangle, Vec3};

#[test]
fn downward_probe_finds_ground_and_horizontal_probe_finds_wall() {
    let ground = Triangle::new(
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, -1.0),
    );
    let wall = Triangle::new(
        Vec3::new(2.0, 0.0, -1.0),
        Vec3::new(2.0, 0.0, 1.0),
        Vec3::new(2.0, 2.0, -1.0),
    );
    let mesh = Mesh::new(vec![ground, wall]);

    let floor = mesh.ground_below(Vec3::new(0.0, 2.0, 0.0), 3.0).unwrap();
    assert_eq!(floor.kind, ContactKind::Ground);
    assert!((floor.distance - 2.0).abs() < 1e-5);

    let obstacle = mesh
        .raycast(Vec3::new(0.0, 0.5, 0.0), Vec3::new(1.0, 0.0, 0.0), 3.0)
        .unwrap();
    assert_eq!(obstacle.kind, ContactKind::Wall);
    assert!((obstacle.distance - 2.0).abs() < 1e-5);
}

#[test]
fn probes_ignore_surfaces_outside_range() {
    let ground = Triangle::new(
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, -1.0),
    );
    let mesh = Mesh::new(vec![ground]);
    assert!(mesh.ground_below(Vec3::new(0.0, 2.0, 0.0), 1.0).is_none());
}

#[test]
fn portable_obj_triangles_feed_the_same_queries() {
    let obj = "v -1 0 -1\nv 0 0 1\nv 1 0 -1\ng ground\nf 1 2 3\n";
    let mesh = Mesh::from_obj_str(obj).unwrap();
    let hit = mesh.ground_below(Vec3::new(0.0, 1.0, 0.0), 2.0).unwrap();
    assert_eq!(hit.kind, ContactKind::Ground);
    assert_eq!(hit.point.y, 0.0);
}

#[test]
fn obj_rejects_faces_with_missing_vertices() {
    let obj = "v 0 0 0\nf 1 2 3\n";
    assert!(Mesh::from_obj_str(obj).is_err());
}
