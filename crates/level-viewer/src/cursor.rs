//! The retail golden arrow cursor (misc/menu/Cursor1024.pcx, see tools/export_cursor.py).
use bevy::{prelude::*,window::{CursorIcon,CustomCursor,CustomCursorImage,PrimaryWindow}};

/// The arrow's tip is its first opaque texel.
const HOTSPOT:(u16,u16)=(1,0);

pub fn setup(mut commands:Commands,config:Res<crate::ViewerConfig>,assets:Res<AssetServer>,window:Single<Entity,With<PrimaryWindow>>) {
    if !config.output.join("ui/cursor.png").is_file() {return;}
    commands.entity(*window).insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
        handle:assets.load("ui/cursor.png"),texture_atlas:None,flip_x:false,flip_y:false,rect:None,hotspot:HOTSPOT})));
}
