//! The retail start-up logo videos. play1.exe plays 1.avi (Cenega), 2.avi (Mirage) and 3.avi
//! (Lithtech) before lithtech.exe opens the main menu (code order 0x40108c, 0x4010aa, 0x4010c8);
//! tools/export_videos.py converts them to JPEG frames plus an mp3 sound track.
use bevy::{prelude::*,window::PrimaryWindow};
use serde::Deserialize;
use std::collections::BTreeMap;
use crate::{ViewerConfig,settings::Session};

#[derive(Deserialize,Clone)] struct Clip {width:f32,height:f32,fps:f32,frames:usize,dir:String,audio:Option<String>}
#[derive(Deserialize)] struct Manifest {order:Vec<u32>,clips:BTreeMap<String,Clip>}

#[derive(Resource,Default)]
pub struct Video {
    pub active:bool,clips:Vec<Clip>,index:usize,elapsed:f32,audio:Option<Entity>,started:bool,
    /// Prefetched frames of the current clip by 0-based index; the newest loaded one at or before the clock is shown.
    frames:BTreeMap<usize,Handle<Image>>,shown:Option<usize>,
}
#[derive(Component)] pub struct Screen;
#[derive(Component)] pub struct Picture;
/// Marks the sound track so settings::update leaves its sink alone (the menu behind is paused by design).
#[derive(Component)] pub struct VideoAudio;

const PREFETCH:usize=12;

/// 0-based frame shown `elapsed` seconds into a clip, clamped to the last frame.
fn frame_at(elapsed:f32,fps:f32,frames:usize)->usize {((elapsed*fps) as usize).min(frames.saturating_sub(1))}
/// Largest size of a `(w,h)` picture that fits `(box_w,box_h)` with the aspect kept.
fn fit(size:(f32,f32),available:(f32,f32))->(f32,f32) {let scale=(available.0/size.0).min(available.1/size.1);(size.0*scale,size.1*scale)}
fn finished(elapsed:f32,fps:f32,frames:usize)->bool {elapsed>=frames as f32/fps}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>,front:Res<crate::frontend::Frontend>,session:Res<Session>) {
    // Normal launches of the main menu only: capture runs, silent probes and MESTER_SKIP_VIDEOS go straight in
    // (MESTER_FORCE_VIDEOS plays them silently in a capture run to check the picture).
    let test_run=(config.capture.is_some() || session.silent) && std::env::var_os("MESTER_FORCE_VIDEOS").is_none();
    let skip=test_run || std::env::var_os("MESTER_SKIP_VIDEOS").is_some() || !front.main_active;
    let manifest=std::fs::read_to_string(config.output.join("videos/videos.json")).ok().and_then(|text|serde_json::from_str::<Manifest>(&text).ok());
    let clips:Vec<Clip>=manifest.map(|m|m.order.iter().filter_map(|n|m.clips.get(&n.to_string()).cloned()).collect()).unwrap_or_default();
    let active=!skip && !clips.is_empty();
    commands.insert_resource(Video {active,clips,..default()});
    if !active {return;}
    commands.spawn((Screen,Button,Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()},
        BackgroundColor(Color::BLACK),GlobalZIndex(500))).with_children(|screen| {screen.spawn((Picture,ImageNode::default(),Node::default()));});
}

pub fn tick(mut commands:Commands,session:Res<Session>,mut video:ResMut<Video>,time:Res<Time>,keys:Res<ButtonInput<KeyCode>>,buttons:Res<ButtonInput<MouseButton>>,assets:Res<AssetServer>,
    window:Single<&Window,With<PrimaryWindow>>,scale:Res<UiScale>,screen:Query<Entity,With<Screen>>,mut picture:Single<(&mut ImageNode,&mut Node),With<Picture>>) {
    if !video.active {return;}
    let Some(clip)=video.clips.get(video.index).cloned() else {return};
    if !video.started {
        video.started=true;video.elapsed=0.0;video.frames.clear();video.shown=None;
        if let Some(audio)=clip.audio.as_ref().filter(|_|session.spawns_audio()) {
            let id=commands.spawn((VideoAudio,AudioPlayer::new(assets.load(audio.clone())),PlaybackSettings::DESPAWN)).id();
            video.audio=Some(id);
        }
    }
    video.elapsed+=time.delta_secs().min(0.1);
    let escape=keys.just_pressed(KeyCode::Escape);
    let skip=escape || keys.get_just_pressed().next().is_some() || buttons.get_just_pressed().next().is_some();
    if skip || finished(video.elapsed,clip.fps,clip.frames) {
        if let Some(id)=video.audio.take() {commands.entity(id).try_despawn();}
        video.index+=1;video.started=false;
        if escape {video.index=video.clips.len();}
        if video.index>=video.clips.len() {
            video.active=false;
            for entity in &screen {commands.entity(entity).despawn();}
        }
        return;
    }
    let current=frame_at(video.elapsed,clip.fps,clip.frames);
    let dir=clip.dir.clone();
    for index in current..(current+PREFETCH).min(clip.frames) {
        video.frames.entry(index).or_insert_with(||assets.load(format!("{dir}/{:04}.jpg",index+1)));
    }
    video.frames.retain(|index,_|*index+1>=current);
    // Show the newest frame at or before the clock that has finished loading (hold the last one meanwhile).
    let ready=video.frames.range(..=current).rev().find(|(_,handle)|assets.is_loaded_with_dependencies(handle.id())).map(|(index,handle)|(*index,handle.clone()));
    if let Some((index,handle))=ready {
        if video.shown!=Some(index) {picture.0.image=handle;video.shown=Some(index);}
    }
    let available=(window.width()/scale.0,window.height()/scale.0);
    let (w,h)=fit((clip.width,clip.height),available);
    picture.1.width=px(w);picture.1.height=px(h);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn frame_index_follows_the_clock_and_clamps_to_the_last_frame() {
        assert_eq!(frame_at(0.0,25.0,355),0);assert_eq!(frame_at(1.0,25.0,355),25);assert_eq!(frame_at(99.0,25.0,355),354);
        assert_eq!(frame_at(2.5,1.0,4),2);
    }
    #[test] fn a_clip_ends_after_frames_over_fps_seconds() {
        assert!(!finished(13.9,25.0,355));assert!(finished(14.2,25.0,355));assert!(!finished(3.9,1.0,4));assert!(finished(4.0,1.0,4));
    }
    #[test] fn pictures_fit_with_their_aspect_ratio() {
        assert_eq!(fit((640.0,360.0),(1024.0,768.0)),(1024.0,576.0));
        assert_eq!(fit((640.0,480.0),(1600.0,720.0)),(960.0,720.0));
    }
    #[test] fn exported_videos_match_the_retail_order_and_lengths() {
        let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/videos/videos.json");
        let Ok(text)=std::fs::read_to_string(path) else {return};
        let manifest:Manifest=serde_json::from_str(&text).unwrap();
        assert_eq!(manifest.order,vec![1,2,3]);
        let seconds=|n:&str|{let c=&manifest.clips[n];c.frames as f32/c.fps};
        assert!((seconds("1")-14.2).abs()<0.1 && (seconds("2")-4.0).abs()<0.01 && (seconds("3")-17.77).abs()<0.3);
    }
}
