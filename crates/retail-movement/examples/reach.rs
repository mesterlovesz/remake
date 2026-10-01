//! `cargo run --release --example reach -- <collision.obj> x y z gx gy gz [dump.json]`: can a standing player walk from start to goal?
fn main() {
    let a:Vec<String>=std::env::args().collect();
    if a[1]=="scan" {
        // scan <obj> x y z0 z1 dx dz : rays from (x,y,z) for z in z0..z1 step 8 along (dx,0,dz)? -> prints openings (hit farther than 250)
        let world=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
        let n=|i:usize|a[i].parse::<f32>().unwrap();
        let (x,y,z0,z1)=(n(3),n(4),n(5),n(6));let mut z=z0;let mut open=None;
        while z<=z1 {
            let far=world.raycast(retail_movement::Vec3::new(x,y,z),retail_movement::Vec3::new(-1.0,0.0,0.0),600.0).map_or(true,|h|h.0>250.0);
            if far && open.is_none() {open=Some(z);} if !far {if let Some(o)=open.take() {println!("opening z {o}..{}",z-8.0);}}
            z+=8.0;
        }
        if let Some(o)=open {println!("opening z {o}..{z1}");}
        return;
    }
    if a[1]=="ceil" {
        // ceil <obj> <cells.json> : free height above each cell, clustered
        let world=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
        let text=std::fs::read_to_string(&a[3]).unwrap();
        let nums:Vec<f32>=text.replace(['[',']'],"").split(',').filter_map(|v|v.trim().parse().ok()).collect();
        let mut tall=std::collections::BTreeMap::<(i32,i32),(usize,f32)>::new();
        for c in nums.chunks(3) {
            let h=world.raycast(retail_movement::Vec3::new(c[0],c[1],c[2]),retail_movement::Vec3::Y,1000.0).map_or(1000.0,|h|h.0);
            if h>200.0 {let e=tall.entry(((c[0]/100.0).floor() as i32*100,(c[2]/100.0).floor() as i32*100)).or_insert((0,0.0));e.0+=1;e.1=e.1.max(h);}
        }
        for (k,v) in tall {println!("{k:?} cells {} maxfree {}",v.0,v.1);}
        return;
    }
    if a[1]=="ray" {
        // ray <obj> x y z dx dy dz [max]
        let world=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
        let n=|i:usize|a[i].parse::<f32>().unwrap();
        let hit=world.raycast(retail_movement::Vec3::new(n(3),n(4),n(5)),retail_movement::Vec3::new(n(6),n(7),n(8)).normalize(),a.get(9).map_or(4000.0,|m|m.parse().unwrap()));
        println!("{hit:?}");return;
    }
    let world=retail_movement::CollisionWorld::from_obj(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let n=|i:usize|a[i].parse::<f32>().unwrap();
    let (start,goal)=(retail_movement::Vec3::new(n(2),n(3),n(4)),retail_movement::Vec3::new(n(5),n(6),n(7)));
    let mut cells=Vec::new();
    let t=std::time::Instant::now();
    let (route,reached,expanded)=world.plan_walk(start,goal,400000,&[],Some(&mut cells));
    if std::env::var_os("SHOW_ROUTE").is_some() {for (p,c,j) in &route {println!("  ({:.0},{:.0},{:.0}){}{}",p.x,p.y,p.z,if *c {" crouch"} else {""},if *j {" JUMP"} else {""});}}
    println!("reached={reached} expanded={expanded} waypoints={} closest={:?} in {:?}",route.len(),route.last().map(|p|p.0),t.elapsed());
    if let Some(path)=a.get(8) {std::fs::write(path,format!("[{}]",cells.iter().map(|p|format!("[{},{},{}]",p.x,p.y,p.z)).collect::<Vec<_>>().join(","))).unwrap();}
}
