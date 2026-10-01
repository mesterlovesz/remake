use std::{env, fs};
use world_query::{Mesh, Vec3};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: probe_world <collision.obj> <x> <y> <z>".into());
    }
    let mesh = Mesh::from_obj_str(&fs::read_to_string(&args[1])?)?;
    let point = Vec3::new(args[2].parse()?, args[3].parse()?, args[4].parse()?);
    match mesh.ground_below(point, 500.0) {
        Some(hit) => println!(
            "ground distance={:.3} point=({:.3}, {:.3}, {:.3}) normal=({:.3}, {:.3}, {:.3})",
            hit.distance, hit.point.x, hit.point.y, hit.point.z, hit.normal.x, hit.normal.y, hit.normal.z
        ),
        None => println!("no ground within 500 retail units"),
    }
    Ok(())
}
