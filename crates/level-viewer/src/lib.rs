//! Minimal reader for the diagnostic OBJ/MTL exported from a retail level.

use std::collections::BTreeMap;

/// The level cameras render into an offscreen image that the window camera shows flipped horizontally (see mirror.rs): a
/// left-handed LithTech view on Bevy's right-handed camera. World textures then keep their authored U; without the flip the
/// old per-texture U mirror (`1 - u`) was the only correction, right for U-horizontal surfaces only.
pub const DISPLAY_MIRRORED: bool = true;

#[derive(Default, Debug)]
pub struct VisualMesh {
    pub material: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub invalid_uvs: usize,
    /// Atlas UV of the lightmap under each vertex (`<world>.visual.lightmap.bin`); empty when the read had none,
    /// NaN for surfaces without a lightmap.
    pub lightmap_uvs: Vec<[f32; 2]>,
    /// Index of each vertex in the OBJ's `v` list (per-vertex side data such as the light group intensities).
    pub source_vertices: Vec<u32>,
}

pub fn read_obj(source: &str) -> Result<Vec<VisualMesh>, String> {
    read_obj_lightmapped(source, None)
}

/// `lightmap`: six f32 per `f` line of the OBJ (u, v of its three vertices), from tools/lightmaps.py.
pub fn read_obj_lightmapped(source: &str, lightmap: Option<&[f32]>) -> Result<Vec<VisualMesh>, String> {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut meshes: BTreeMap<String, VisualMesh> = BTreeMap::new();
    let mut material = String::new();
    let mut face = 0usize;
    for (line_no, line) in source.lines().enumerate() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                positions.push(parse_vec3(&mut parts, line_no)?);
                let rgb = if let Some(r) = parts.next() {
                    [parse_float(Some(r), line_no)?, parse_float(parts.next(), line_no)?, parse_float(parts.next(), line_no)?, 1.0]
                } else { [1.0; 4] };
                colors.push(rgb);
            }
            Some("vt") => uvs.push(parse_uv(parts, line_no)?),
            Some("vn") => normals.push(parse_vec3(parts, line_no)?),
            Some("usemtl") => material = parts.next().unwrap_or("").to_owned(),
            Some("f") => {
                let indices = parts.map(|p| parse_face_vertex(p, line_no)).collect::<Result<Vec<_>, _>>()?;
                if indices.len() != 3 {
                    return Err(format!("expected a triangle on line {}", line_no + 1));
                }
                let mesh = meshes.entry(material.clone()).or_insert_with(|| VisualMesh {
                    material: material.clone(),
                    ..Default::default()
                });
                for (corner, (v, t, n)) in indices.into_iter().enumerate() {
                    if let Some(values) = lightmap {
                        let at = face * 6 + corner * 2;
                        mesh.lightmap_uvs.push([values.get(at).copied().unwrap_or(f32::NAN), values.get(at + 1).copied().unwrap_or(f32::NAN)]);
                    }
                    mesh.source_vertices.push(v as u32);
                    mesh.positions.push(*positions.get(v).ok_or_else(|| format!("missing position on line {}", line_no + 1))?);
                    mesh.colors.push(colors[v]);
                    let uv = *uvs.get(t).ok_or_else(|| format!("missing UV on line {}", line_no + 1))?;
                    if !uv.iter().all(|value| value.is_finite()) {
                        // The raw OBJ keeps the retail value. Only the diagnostic
                        // renderer substitutes a safe coordinate and marks the mesh.
                        mesh.invalid_uvs += 1;
                        mesh.uvs.push([0.0, 0.0]);
                    } else {
                        // OBJ V is bottom-left, whereas D3D and Bevy use top-left. U is mirrored only when the
                        // view itself is not (DISPLAY_MIRRORED).
                        mesh.uvs.push([if DISPLAY_MIRRORED { uv[0] } else { 1.0 - uv[0] }, 1.0 - uv[1]]);
                    }
                    mesh.normals.push(*normals.get(n).ok_or_else(|| format!("missing normal on line {}", line_no + 1))?);
                }
                face += 1;
            }
            _ => {}
        }
    }
    Ok(meshes.into_values().collect())
}

/// Optional remastered ("HD") textures (docs/retail-hd-textures.md): `tools/upscale_textures.py` mirrors the albedo textures of the 3D
/// surfaces (world, movable world models, model/prop/character skins) under `textures_hd/<same relative path>`.  With the option on
/// (options menu "HD textúrák", or MESTER_HD_TEXTURES=1) a load asks `hd::path` first and falls back to the original when the HD file
/// does not exist.  Sprites, HUD, menus, lightmaps and effect textures never have an HD file, so they always stay retail.
pub mod hd {
    use std::path::{Path, PathBuf};
    use std::sync::{atomic::{AtomicBool, Ordering}, OnceLock};

    pub const DIR: &str = "textures_hd/";
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static ROOT: OnceLock<PathBuf> = OnceLock::new();

    /// The export folder the HD files are looked up in (set once at start-up).
    pub fn set_root(output: &Path) { let _ = ROOT.set(output.to_owned()); }
    pub fn set_enabled(on: bool) { ENABLED.store(on, Ordering::Relaxed); }
    pub fn enabled() -> bool { ENABLED.load(Ordering::Relaxed) }
    /// MESTER_HD_TEXTURES=1 forces the option on (headless captures); 0 / unset leaves it to the saved option.
    pub fn forced_by_env() -> bool { std::env::var("MESTER_HD_TEXTURES").is_ok_and(|v| !matches!(v.trim(), "" | "0" | "false")) }
    /// The asset path to load for `rel`: `textures_hd/<rel>` when `on` and that file exists under `output`, else `rel` itself.
    pub fn choose(output: &Path, rel: &str, on: bool) -> String {
        let rel = rel.replace('\\', "/");
        if !on || rel.starts_with(DIR) { return rel; }
        let hd = format!("{DIR}{rel}");
        if output.join(&hd).is_file() { hd } else { rel }
    }
    /// `choose` with the global switch and export folder.
    pub fn path(rel: &str) -> String {
        match ROOT.get() { Some(root) if enabled() => { let c = choose(root, rel, true); if std::env::var_os("MESTER_HD_LOG").is_some() && c != rel { eprintln!("HDSWAP {c}"); } c } _ => rel.replace('\\', "/") }
    }
    pub fn is_hd(rel: &str) -> bool { rel.starts_with(DIR) }
}

pub fn read_mtl(source: &str) -> BTreeMap<String, String> {
    let mut textures = BTreeMap::new();
    let mut material = String::new();
    for line in source.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("newmtl") => material = parts.next().unwrap_or("").to_owned(),
            Some("map_Kd") if !material.is_empty() => {
                textures.insert(material.clone(), parts.collect::<Vec<_>>().join(" "));
            }
            _ => {}
        }
    }
    textures
}

fn parse_vec3<'a>(mut parts: impl Iterator<Item = &'a str>, line: usize) -> Result<[f32; 3], String> {
    Ok([parse_float(parts.next(), line)?, parse_float(parts.next(), line)?, parse_float(parts.next(), line)?])
}

fn parse_uv<'a>(mut parts: impl Iterator<Item = &'a str>, line: usize) -> Result<[f32; 2], String> {
    Ok([parse_number(parts.next(), line)?, parse_number(parts.next(), line)?])
}

fn parse_float(value: Option<&str>, line: usize) -> Result<f32, String> {
    let number = parse_number(value, line)?;
    if !number.is_finite() { return Err(format!("nonfinite number on line {}", line + 1)); }
    Ok(number)
}

fn parse_number(value: Option<&str>, line: usize) -> Result<f32, String> {
    let value = value.ok_or_else(|| format!("missing number on line {}", line + 1))?;
    let number = value.parse::<f32>().map_err(|_| format!("invalid number on line {}", line + 1))?;
    Ok(number)
}

fn parse_face_vertex(value: &str, line: usize) -> Result<(usize, usize, usize), String> {
    let parts = value.split('/').map(|p| p.parse::<usize>()).collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("invalid face on line {}", line + 1))?;
    if parts.len() != 3 || parts.contains(&0) {
        return Err(format!("invalid face on line {}", line + 1));
    }
    Ok((parts[0] - 1, parts[1] - 1, parts[2] - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_exported_triangle_and_material() {
        let obj = "v 0 0 0\nvt 0 1\nvn 0 1 0\nv 1 0 0\nvt 1 1\nvn 0 1 0\nv 0 0 1\nvt 0 0\nvn 0 1 0\nusemtl mat0000\nf 1/1/1 2/2/2 3/3/3\n";
        let meshes = read_obj(obj).unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].positions.len(), 3);
        assert_eq!(meshes[0].uvs[2], [if DISPLAY_MIRRORED { 0.0 } else { 1.0 }, 1.0]);
        assert_eq!(read_mtl("newmtl mat0000\nmap_Kd textures/a.png\n")["mat0000"], "textures/a.png");
    }

    #[test]
    fn hd_paths_are_preferred_only_when_on_and_present() {
        let dir = std::env::temp_dir().join(format!("mester-hd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("textures_hd/textures/w")).unwrap();
        std::fs::write(dir.join("textures_hd/textures/w/mat0000.png"), b"x").unwrap();
        // Off (the default): the retail path, whatever exists.
        assert_eq!(hd::choose(&dir, "textures/w/mat0000.png", false), "textures/w/mat0000.png");
        // On and present: the mirrored HD file (backslashes are normalised).
        assert_eq!(hd::choose(&dir, "textures/w/mat0000.png", true), "textures_hd/textures/w/mat0000.png");
        assert_eq!(hd::choose(&dir, "textures\\w\\mat0000.png", true), "textures_hd/textures/w/mat0000.png");
        // On but no HD file (sprites, HUD, lightmaps, not yet upscaled levels): fall back to the original.
        assert_eq!(hd::choose(&dir, "textures/w/mat0001.png", true), "textures/w/mat0001.png");
        assert_eq!(hd::choose(&dir, "textures/w/lightmap.png", true), "textures/w/lightmap.png");
        // An HD path is never prefixed twice.
        assert_eq!(hd::choose(&dir, "textures_hd/textures/w/mat0000.png", true), "textures_hd/textures/w/mat0000.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retail_sign_uv_faces_right_way_in_the_displayed_view() {
        // The original left-handed renderer sees +X as right when facing -Z; the displayed (flipped) view does too,
        // so U is kept (without the flip Bevy needed 1 - U).
        let obj = "v 0 0 0\nvt 0 1\nv 0 0 1\nvt 0.25 1\nv 0 1 0\nvt 0 0\nvn 1 0 0\nusemtl sign\nf 1/1/1 2/2/1 3/3/1\n";
        let sign = &read_obj(obj).unwrap()[0];
        if DISPLAY_MIRRORED {
            assert_eq!(sign.uvs[0], [0.0, 0.0]);
            assert_eq!(sign.uvs[1], [0.25, 0.0]);
        } else {
            assert_eq!(sign.uvs[0], [1.0, 0.0]);
            assert_eq!(sign.uvs[1], [0.75, 0.0]);
        }
    }

    #[test]
    fn invalid_face_does_not_panic() {
        assert!(read_obj("usemtl x\nf 1/1/1 2/2/2 3/3/3\n").is_err());
    }

    #[test]
    fn nonfinite_retail_uv_is_reported_and_safe_for_diagnostic_renderer() {
        let obj = "v 0 0 0\nvt inf -inf\nvn 0 1 0\nusemtl x\nf 1/1/1 1/1/1 1/1/1\n";
        let meshes = read_obj(obj).unwrap();
        assert_eq!(meshes[0].invalid_uvs, 3);
        assert_eq!(meshes[0].uvs[0], [0.0, 0.0]);
    }
}
