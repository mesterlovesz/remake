use retail_movement::{CollisionWorld, Input, Player, Vec3};

/// A player past the 60 frozen start frames (cshell 0x10061906).
fn fresh(position: Vec3) -> Player { let mut p = Player::new(position); p.frames = 60; p }

fn room() -> CollisionWorld {
    CollisionWorld::from_obj("v -1000 0 -1000\nv -1000 0 1000\nv 1000 0 1000\nv 1000 0 -1000\nv -1000 500 -100\nv 1000 500 -100\nv 1000 0 -100\nv -1000 0 -100\nf 1 2 3 4\nf 5 6 7 8").unwrap()
}

#[test]
fn falls_onto_floor_without_sinking() {
    let mut p = fresh(Vec3::new(0.0, 200.0, 0.0));
    for _ in 0..180 { p.tick(&room(), &Input::default(), 1.0/60.0); }
    assert!((p.position.y - 58.1).abs() < 0.3, "{:?}", p.position);
    assert!(p.grounded);
}

#[test]
fn walking_hits_wall_and_can_slide_along_it() {
    let w = room();
    let mut p = fresh(Vec3::new(0.0, 58.1, 0.0));
    for _ in 0..120 { p.tick(&w, &Input { forward: 1.0, right: 1.0, ..Default::default() }, 1.0/60.0); }
    assert!(p.position.z >= -84.2 && p.position.z < -80.0, "{:?}", p.position);
    assert!(p.position.x > 200.0);
    assert!((p.position.y-58.1).abs()<0.3);
}

#[test]
fn jump_leaves_ground_and_returns() {
    let w=room(); let mut p=fresh(Vec3::new(0.0,58.1,0.0));
    p.tick(&w,&Input::default(),1.0/60.0);
    p.tick(&w,&Input { jump:true,..Default::default() },1.0/60.0);
    assert!(p.velocity.y > 280.0 && p.velocity.y <= 300.0);
    assert!(p.stamina<106.0);
    for _ in 0..100 { p.tick(&w,&Input::default(),1.0/60.0); }
    assert!(p.grounded);
}

#[test]
fn airborne_crouch_toggle_preserves_retail_flight_bug() {
    let w=room(); let mut p=fresh(Vec3::new(0.0,300.0,0.0));
    p.tick(&w,&Input { crouch:true,..Default::default() },0.01);
    assert!((p.velocity.y-54.0).abs()<0.01);
    let y=p.position.y;
    p.tick(&w,&Input::default(),0.01);
    assert!((p.velocity.y-108.0).abs()<0.01);
    assert!(p.position.y>y+32.0);
}

#[test]
fn crouch_with_run_held_recovers_stamina_instead_of_sprinting() {
    let w=room(); let mut p=fresh(Vec3::new(0.0,58.1,0.0));
    for _ in 0..120 {p.tick(&w,&Input {crouch:true,..Default::default()},1.0/60.0);}
    p.stamina=50.0;
    for _ in 0..60 {p.tick(&w,&Input {right:1.0,crouch:true,run:true,..Default::default()},1.0/60.0);}
    assert!((p.stamina-50.5).abs()<0.01,"{}",p.stamina);
}

#[test]
fn exhausted_player_cannot_jump_and_airborne_run_recovers_stamina() {
    let w=room();let mut p=fresh(Vec3::new(0.0,58.1,0.0));p.stamina=0.0;
    p.tick(&w,&Input::default(),0.01);
    p.tick(&w,&Input {jump:true,..Default::default()},0.01);
    assert!(p.grounded && p.velocity.y<=0.0);
    p.position.y=500.0;p.stamina=50.0;
    p.tick(&w,&Input {forward:1.0,run:true,..Default::default()},0.1);
    assert!((p.stamina-50.55).abs()<0.01,"{}",p.stamina);
}

fn step_world(height:f32)->CollisionWorld {
    CollisionWorld::from_obj(&format!("v -1000 0 -1000\nv -1000 0 1000\nv 1000 0 1000\nv 1000 0 -1000\nv 0 {height} -1000\nv 0 {height} 1000\nv 1000 {height} 1000\nv 1000 {height} -1000\nv 0 0 -1000\nv 0 0 1000\nf 1 2 3 4\nf 5 6 7 8\nf 5 9 10 6")).unwrap()
}

#[test]
fn eighteen_unit_stair_is_walkable_but_twenty_unit_ledge_blocks() {
    for height in [18.0,20.0] {
        let w=step_world(height);let mut p=fresh(Vec3::new(-100.0,58.1,0.0));
        for _ in 0..120 {p.tick(&w,&Input {right:1.0,..Default::default()},1.0/60.0);}
        if height==18.0 {assert!(p.position.x>100.0,"{:?}",p.position);}
        else {assert!(p.position.x<=-15.9,"{:?}",p.position);}
    }
}

#[test]
fn stair_clearance_is_consistent_across_triangle_contacts() {
    for height in [8.0,12.0,16.0,18.0,20.0] {
        let w=CollisionWorld::from_obj(&format!("v -1000 0 -1000\nv -1000 0 1000\nv 1000 0 1000\nv 1000 0 -1000\nv -1000 {height} -100\nv 1000 {height} -100\nv 1000 {height} -1000\nv -1000 {height} -1000\nv -1000 0 -100\nv 1000 0 -100\nf 1 2 3 4\nf 5 6 7 8\nf 5 9 10 6")).unwrap();
        for x in [-10.0,0.0,10.0] {
            let mut p=fresh(Vec3::new(x,58.1,0.0));
            for _ in 0..240 {p.tick(&w,&Input {forward:1.0,..Default::default()},1.0/60.0);}
            if height<=18.0 {
                assert!(p.position.z < -300.0,"step {height}, x {x}: {:?}",p.position);
                assert!((p.position.y-height-58.1).abs()<0.3,"{:?}",p.position);
            } else {
                assert!(p.position.z>=-84.2,"ledge {height}, x {x}: {:?}",p.position);
            }
        }
    }
}

#[test]
fn retail_running_footsteps_tick_every_four_tenths() {
    let w=room();let mut p=fresh(Vec3::new(0.0,58.2,0.0));let mut steps=0;
    for _ in 0..60 {p.tick(&w,&Input {right:1.0,run:true,..Default::default()},1.0/60.0);if p.footstep {steps+=1;}}
    // The timer starts at 0 (0x1005f7ab), so the first step lands on the first frame: frames 1, 26 and 51.
    assert_eq!(steps,3);
    for _ in 0..60 {p.tick(&w,&Input {right:1.0,..Default::default()},1.0/60.0);assert!(!p.footstep);}
}

#[test]
fn raycast_reports_the_obj_face_behind_fans_and_degenerate_triangles() {
    // Face 0 is a degenerate sliver, face 1 a floor quad (two fan triangles), face 2 a wall.
    let world = CollisionWorld::from_obj("v 0 0 0\nv 1 0 0\nv 2 0 0\nv -100 0 -100\nv -100 0 100\nv 100 0 100\nv 100 0 -100\nv -100 0 50\nv 100 0 50\nv 100 200 50\nf 1 2 3\nf 4 5 6 7\nf 8 9 10\n").unwrap();
    let (distance, normal, face) = world.raycast_face(Vec3::new(-50.0, 10.0, -50.0), Vec3::NEG_Y, 100.0).unwrap();
    assert!((distance - 10.0).abs() < 0.01 && normal.y.abs() > 0.99 && face == 1);
    assert_eq!(world.raycast_face(Vec3::new(50.0, 10.0, 60.0), Vec3::NEG_Y, 100.0).unwrap().2, 1);
    assert_eq!(world.raycast_face(Vec3::new(50.0, 20.0, 0.0), Vec3::Z, 100.0).unwrap().2, 2);
    assert_eq!(world.raycast_face(Vec3::new(50.0, 20.0, 90.0), Vec3::NEG_Z, 100.0).unwrap().2, 2, "back face");
}
