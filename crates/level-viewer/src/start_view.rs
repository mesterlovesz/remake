//! Retail start pose of the levels whose first view changed with the `Kierunek` yaw (docs/retail-start-view.md), shared by the unit tests
//! and the `startpose` probe. LithTech is left-handed (+X right, +Z forward, north = +Z, east = +X); `forward` is a horizontal (x, z) vector.
use bevy::prelude::Vec2;

/// (world, StartPoint0 `Pos` in native units, `Kierunek`) exactly as exported to `<world>.scene.json`; the 10 levels where the
/// Rotation-based start of the first build faced a different way (cshell 0x1005b6df reads only the compass direction).
pub const STARTS:[(&str,[f32;3],&str);10]=[
    ("podziemia1b",[324.0,-368.0,1116.0],"zachod"),
    ("rh1-wiezienie1",[4028.0,125.0,698.0],"polnoc"),
    ("rh1-wiezienie2",[124.0,13.0,938.0],"poludnie"),
    ("rh10-wiezowiec1",[1616.0,-1329.0,-1800.0],"zachod"),
    ("rh2-wiezienie2",[-2714.0,-446.0,-1144.0],"wschod"),
    ("rh3-miasteczko0",[-6045.0,3.0,268.0],"wschod"),
    ("rh3-miasteczko1",[1484.0,-304.0,1356.162109375],"zachod"),
    ("wiez_wn1",[-1778.0,-224.0,-456.0],"wschod"),
    ("wiez_wn2",[-1000.0,-55.9375,352.0],"wschod"),
    ("wiez_wn3",[2316.0,408.0,116.0],"poludnie"),
];

/// The world-space heading (x, z) of a compass direction: polnoc +Z, wschod +X, poludnie -Z, zachod -X (anything else: polnoc).
pub fn forward(kierunek:&str)->Vec2 {
    match kierunek.to_ascii_lowercase().as_str() {"wschod"=>Vec2::X,"poludnie"=>Vec2::NEG_Y,"zachod"=>Vec2::NEG_X,_=>Vec2::Y}
}
