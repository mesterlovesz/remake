//! The sound output (docs/retail-audio.md "Diagnostic"). Bevy's AudioPlugin plays on the Windows default device and cannot be told
//! otherwise; on a PC whose default is a game pad's speaker (the DualSense of the owner's PC) that is a game without sound.
//! This plugin replaces it: the same rodio sinks under the same `AudioPlayer` / `AudioSink` components (Bevy 0.18.1 audio_output.rs,
//! non-spatial branch), on the device chosen here. `Preferences::output_device` names it; empty is automatic: the system default,
//! unless that is a game pad, then the first real speaker device. Runs without sound (captures) open nothing.
use std::io::Cursor;
use bevy::{asset::AssetApp,audio::{AudioLoader,AudioPlayer,AudioSink,AudioSinkPlayback,AudioSource,GlobalVolume,PlaybackMode,PlaybackSettings},prelude::*};
use rodio::cpal::traits::{DeviceTrait,HostTrait};
use crate::{settings::Session,sound::Positional,spatial::{Spatial,SpatialAudio,Spatialized}};

/// A game pad's speaker (or headset jack): never what a player wants the game on.
fn pad(name:&str)->bool {let name=name.to_lowercase();["dualsense","dualshock","wireless controller","xbox","gamepad"].iter().any(|word|name.contains(word))}
/// Steam's remote-play sink: right while streaming (then it is the default), no fallback otherwise.
fn streaming(name:&str)->bool {name.to_lowercase().contains("steam streaming")}
/// Windows names the analogue speaker endpoints "Speakers (...)" in the language of the system; the rest are digital outs and displays.
fn speakers(name:&str)->bool {let name=name.to_lowercase();["speakers","hangszórók","lautsprecher","haut-parleurs","altavoces"].iter().any(|word|name.starts_with(word))}

/// The order the devices are tried in: the chosen one; the system default unless it is a game pad; then the others, real speakers
/// first and the pad and the streaming sink last (a pad still beats no sound at all).
pub fn order(devices:&[String],default:Option<&str>,wanted:&str)->Vec<usize> {
    let find=|name:&str|devices.iter().position(|device|device==name);
    let default=default.and_then(find);
    let mut order:Vec<usize>=Vec::new();
    if !wanted.is_empty() {order.extend(find(wanted));}
    let keep=default.filter(|&index|!pad(&devices[index]) && !order.contains(&index));
    order.extend(keep);
    let mut rest:Vec<usize>=(0..devices.len()).filter(|index|!order.contains(index)).collect();
    rest.sort_by_key(|&index|(pad(&devices[index]) || streaming(&devices[index]),!speakers(&devices[index]),Some(index)!=default));
    order.extend(rest);
    order
}

/// The next choice when the button is pressed: "" (automatic), then every device in turn, wrapping around.
pub fn next_choice(current:&str,devices:&[String],step:i32)->String {
    let count=devices.len() as i32+1;
    let at=devices.iter().position(|device|device==current).map_or(0,|index|index as i32+1);
    match (at+step).rem_euclid(count) {0=>String::new(),index=>devices[index as usize-1].clone()}
}
/// `next_choice` over the devices this PC has now.
pub fn cycle(current:&str,step:i32)->String {next_choice(current,&device_names(),step)}

/// The device line of the option screens: where the sound landed, marked "(auto)" when the game chose, cut to 44 characters.
pub fn label(choice:&str,opened:&str)->String {
    let (opened,mark)=(if opened.is_empty() {"nincs hangkimenet"} else {opened},if choice.is_empty() {" (auto)"}else{""});
    let room=44-mark.chars().count();
    let name=if opened.chars().count()>room {format!("{}…",opened.chars().take(room-1).collect::<String>())}else{opened.to_owned()};
    format!("{name}{mark}")
}

pub fn device_names()->Vec<String> {
    rodio::cpal::default_host().output_devices().map(|devices|devices.filter_map(|device|device.name().ok()).collect()).unwrap_or_default()
}

/// What `open` reports: the device, its format and why it is not the system default.
pub struct Opened {stream:rodio::OutputStream,handle:rodio::OutputStreamHandle,pub name:String,pub note:String,pub channels:u16}
/// Opens the first device of `order` that works; `None` when this PC has no sound output at all.
pub fn open(wanted:&str)->Option<Opened> {
    let host=rodio::cpal::default_host();
    let devices:Vec<(String,rodio::cpal::Device)>=host.output_devices().map(|list|list.filter_map(|device|device.name().ok().map(|name|(name,device))).collect()).unwrap_or_default();
    let names:Vec<String>=devices.iter().map(|(name,_)|name.clone()).collect();
    let default=host.default_output_device().and_then(|device|device.name().ok());
    for index in order(&names,default.as_deref(),wanted) {
        let (name,device)=&devices[index];
        match rodio::OutputStream::try_from_device(device) {
            Ok((stream,handle))=>{
                let format=device.default_output_config().map_or_else(|_|String::new(),|config|format!("{} csatorna, {} Hz",config.channels(),config.sample_rate().0));
                let note=match &default {Some(default) if default!=name=>format!("a rendszer alapértelmezettje ({default}) helyett"),_=>String::new()};
                let channels=device.default_output_config().map_or(2,|config|config.channels());
                return Some(Opened {stream,handle,name:name.clone(),channels,note:if note.is_empty() {format} else {format!("{format}, {note}")}});
            },
            Err(error)=>warn!("Hangkimenet {name}: nem nyitható meg: {error}"),
        }
    }
    None
}

/// Makes the sink of a sound; `None` while nothing is open. A closure so tests can run the players without a device.
#[derive(Resource,Default)]
pub struct Sinks(pub Option<Box<dyn Fn()->Option<rodio::Sink>+Send+Sync>>);
/// The open device, kept alive here (a cpal stream cannot leave the main thread) with the choice it was opened for.
#[derive(Default)]
struct Stream {stream:Option<rodio::OutputStream>,opened_for:Option<String>}
/// A `PlaybackMode::Despawn` sound: gone when its sink has played out.
#[derive(Component)]
struct Ends;

/// Opens the device named by the preference when the game starts and again whenever it changes; the sounds playing at that moment
/// restart on the new one. Silent runs open nothing; a trace run opens it (everything is muted and silenced there).
fn follow(mut session:ResMut<Session>,mut stream:NonSendMut<Stream>,mut sinks:ResMut<Sinks>,mut spatial:ResMut<SpatialAudio>,playing:Query<Entity,With<AudioSink>>,mut commands:Commands) {
    if session.silent && !session.trace {return;}
    let wanted=session.preferences.output_device.clone();
    if stream.opened_for.as_ref()==Some(&wanted) {return;}
    stream.opened_for=Some(wanted.clone());
    sinks.0=None;stream.stream=None;
    for entity in &playing {commands.entity(entity).remove::<AudioSink>();}
    match open(&wanted) {
        Some(opened)=>{
            info!("Hangkimenet: {} ({})",opened.name,opened.note);
            let handle=opened.handle;
            sinks.0=Some(Box::new(move||rodio::Sink::try_new(&handle).ok()));
            stream.stream=Some(opened.stream);session.output=opened.name;spatial.device_channels=opened.channels;
        },
        None=>{warn!("Nincs hangkimenet: a gépen nincs megnyitható hangeszköz.");session.output.clear();},
    }
}

/// Starts every `AudioPlayer` whose sound has loaded (Bevy's play_queued_audio_system without the spatial branch).
fn play_queued(sinks:Res<Sinks>,sources:Res<Assets<AudioSource>>,global:Res<GlobalVolume>,spatial:Option<Res<SpatialAudio>>,queued:Query<(Entity,&AudioPlayer,&PlaybackSettings,Option<&Positional>),Without<AudioSink>>,mut commands:Commands) {
    let Some(make)=&sinks.0 else {return};
    for (entity,player,settings,positional) in &queued {
        let Some(source)=sources.get(&player.0) else {continue};
        let Some(sink)=make() else {warn!("Hangcsatorna nem hozható létre.");continue};
        // Bevy unwraps here and would take the game down with a file that does not decode.
        let decoder=match rodio::Decoder::new(Cursor::new(source.bytes.clone())) {Ok(decoder)=>decoder,Err(error)=>{error!("A hang nem dekódolható: {error}");commands.entity(entity).try_despawn();continue}};
        // "Javított 3D hangzás": a positional sound gets the per-ear effect in its chain, starting at its real pan (spatial.rs).
        let effect=spatial.as_ref().zip(positional).and_then(|(audio,at)|audio.start(at.position).map(|state|(state,audio.device_channels==1)));
        match (settings.mode,&effect) {
            (PlaybackMode::Loop,Some((state,mono)))=>sink.append(Spatialized::new(rodio::Source::repeat_infinite(decoder),state.clone(),*mono)),
            (_,Some((state,mono)))=>sink.append(Spatialized::new(decoder,state.clone(),*mono)),
            (PlaybackMode::Loop,None)=>sink.append(rodio::Source::repeat_infinite(decoder)),
            (_,None)=>sink.append(decoder),
        }
        let mut sink=AudioSink::new(sink);
        if settings.muted {sink.mute();}
        sink.set_speed(settings.speed);
        sink.set_volume(settings.volume*global.volume);
        if settings.paused {sink.pause();}
        let mut sound=commands.entity(entity);
        sound.insert(sink);
        if let Some((state,_))=effect {sound.insert(Spatial(state));}
        if matches!(settings.mode,PlaybackMode::Despawn) {sound.insert(Ends);}
    }
}

fn cleanup_finished(ended:Query<(Entity,&AudioSink),With<Ends>>,mut commands:Commands) {
    for (entity,sink) in &ended {if sink.empty() {commands.entity(entity).try_despawn();}}
}

pub struct Output;
impl Plugin for Output {
    fn build(&self,app:&mut App) {
        app.init_asset::<AudioSource>().init_asset_loader::<AudioLoader>().init_resource::<Sinks>().init_resource::<SpatialAudio>().insert_non_send_resource(Stream::default())
            .add_systems(Update,follow).add_systems(PostUpdate,(play_queued,cleanup_finished).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc,Mutex};
    fn names(list:&[&str])->Vec<String> {list.iter().map(|name|name.to_string()).collect()}
    /// The output devices of the owner's PC in the order cpal lists them; the DualSense is the Windows default.
    fn owners_pc()->Vec<String> {names(&["Hangszórók (2 - DualSense Wireless Controller)","SPDIF illesztő (Realtek USB Audio)","Hangszórók (Steam Streaming Microphone)","Hangszórók (Focusrite USB Audio)","1 - VG258QM (2 - AMD High Definition Audio Device)"])}
    #[test]
    fn a_game_pad_default_gives_way_to_the_real_speakers() {
        let devices=owners_pc();
        let default=Some(devices[0].as_str());
        let tried:Vec<&str>=order(&devices,default,"").into_iter().map(|index|devices[index].as_str()).collect();
        assert_eq!(tried[0],"Hangszórók (Focusrite USB Audio)");
        assert_eq!(tried[1..3],["SPDIF illesztő (Realtek USB Audio)","1 - VG258QM (2 - AMD High Definition Audio Device)"]);
        assert_eq!(tried[3..],["Hangszórók (2 - DualSense Wireless Controller)","Hangszórók (Steam Streaming Microphone)"],"the pad still beats silence, the streaming sink comes last");
        assert_eq!(order(&devices,default,"").len(),devices.len(),"every device is a fallback exactly once");
    }
    #[test]
    fn an_ordinary_default_stays_and_a_saved_choice_wins() {
        let devices=owners_pc();
        assert_eq!(order(&devices,Some(&devices[1]),"")[0],1,"the system default is kept when it is not a pad");
        assert_eq!(order(&devices,Some(&devices[0]),&devices[4])[0],4,"the saved device beats everything");
        assert_eq!(order(&devices,Some(&devices[0]),&devices[0])[0],0,"even a pad, when the player asks for it");
        assert_eq!(order(&devices,Some(&devices[0]),"unplugged")[0],3,"a saved device that is gone means automatic");
        assert_eq!(order(&devices,Some(&devices[2]),"")[0],2,"while streaming, Steam's sink is the default and stays");
        assert_eq!(order(&names(&["Xbox Controller"]),Some("Xbox Controller"),""),vec![0],"a pad alone is still used");
        assert!(order(&[],None,"").is_empty());
    }
    #[test]
    fn the_button_cycles_automatic_and_every_device() {
        let devices=owners_pc();
        let mut choice=String::new();
        let mut seen=vec![];
        for _ in 0..=devices.len() {choice=next_choice(&choice,&devices,1);seen.push(choice.clone());}
        assert_eq!(seen[0],devices[0]);assert_eq!(seen[devices.len()-1],devices[devices.len()-1]);assert_eq!(seen[devices.len()],"","wraps to automatic");
        assert_eq!(next_choice("",&devices,-1),devices[4],"backwards from automatic");
        assert_eq!(next_choice("gone",&devices,1),devices[0]);assert_eq!(next_choice("",&[],1),"");
    }
    #[test]
    fn the_option_line_names_the_device_and_fits() {
        assert_eq!(label("","Hangszórók (Focusrite USB Audio)"),"Hangszórók (Focusrite USB Audio) (auto)");
        assert_eq!(label("SPDIF","SPDIF"),"SPDIF");
        assert_eq!(label("",""),"nincs hangkimenet (auto)");
        assert_eq!((label("x",&"é".repeat(80)).chars().count(),label("",&"é".repeat(80)).chars().count()),(44,44));
    }
    /// The players without a device: `Sinks` hands out idle rodio sinks whose queues the test reads, as the cpal thread would.
    #[test]
    fn a_player_gets_a_sink_plays_pauses_loops_and_ends() {
        use crate::audio_trace::tests::tone_wav;
        let outputs:Arc<Mutex<Vec<rodio::queue::SourcesQueueOutput<f32>>>>=default();
        let stash=outputs.clone();
        let mut app=App::new();
        app.init_resource::<Assets<AudioSource>>().insert_resource(GlobalVolume::new(bevy::audio::Volume::Linear(0.5)))
            .insert_resource(Sinks(Some(Box::new(move||{let (sink,output)=rodio::Sink::new_idle();stash.lock().unwrap().push(output);Some(sink)}))))
            .add_systems(Update,(play_queued,cleanup_finished).chain());
        let handle=app.world_mut().resource_mut::<Assets<AudioSource>>().add(AudioSource {bytes:tone_wav().into()});
        let once=app.world_mut().spawn((AudioPlayer::new(handle.clone()),PlaybackSettings::DESPAWN)).id();
        let looped=app.world_mut().spawn((AudioPlayer::new(handle),PlaybackSettings::LOOP)).id();
        app.update();
        assert!(app.world().get::<AudioSink>(once).is_some() && app.world().get::<AudioSink>(looped).is_some(),"a loaded sound gets its sink");
        assert!((app.world().get::<AudioSink>(once).unwrap().volume().to_linear()-0.5).abs()<1e-6,"it starts at the global volume");
        let mut queues=std::mem::take(&mut *outputs.lock().unwrap());
        let mut second=rodio::source::UniformSourceIterator::new(queues.pop().unwrap(),2,48000);
        let mut first=rodio::source::UniformSourceIterator::new(queues.pop().unwrap(),2,48000);
        let peak=|out:&mut dyn Iterator<Item=f32>,frames:usize|out.take(frames*2).fold(0.0f32,|peak,sample|peak.max(sample.abs()));
        assert!(peak(&mut first,4800)>0.1 && peak(&mut second,4800)>0.1,"and the samples come out");
        // 1 s tone: the looped one is still playing after 2 s, the once one has ended and its entity goes.
        assert!(peak(&mut second,96000)>0.1,"the loop keeps flowing");
        for _ in 0..96000*2 {if first.next().is_none() {break}}
        app.update();
        assert!(app.world().get_entity(once).is_err(),"a despawn sound leaves with its last sample");
        assert!(app.world().get_entity(looped).is_ok());
    }
    /// "Javított 3D hangzás" through the real `play_queued` and the real sink chain, no device: the players' idle sinks are read as the
    /// device would read them (`channels` output channels at 48 kHz); returns the RMS per output channel.
    fn spatial_run(enabled:bool,source:Option<Vec3>,channels:u16,switch_off_after_start:bool)->(Vec<f32>,bool) {
        use crate::{audio_trace::tests::tone_wav,spatial::{Listener,SpatialAudio}};
        let outputs:Arc<Mutex<Vec<rodio::queue::SourcesQueueOutput<f32>>>>=default();
        let stash=outputs.clone();
        let mut app=App::new();
        app.init_resource::<Assets<AudioSource>>().insert_resource(GlobalVolume::default())
            .insert_resource(Sinks(Some(Box::new(move||{let (sink,output)=rodio::Sink::new_idle();stash.lock().unwrap().push(output);Some(sink)}))))
            .insert_resource(SpatialAudio {enabled,listener:Some(Listener {position:Vec3::ZERO,right:Vec3::X,forward:Vec3::Z,up:Vec3::Y}),device_channels:channels})
            .add_systems(Update,(crate::spatial::drive,play_queued).chain());
        let handle=app.world_mut().resource_mut::<Assets<AudioSource>>().add(AudioSource {bytes:tone_wav().into()});
        let mut sound=app.world_mut().spawn((AudioPlayer::new(handle),PlaybackSettings::DESPAWN));
        if let Some(position)=source {sound.insert(Positional {position,radius:1280.0});}
        let sound=sound.id();
        app.update();
        let spatialised=app.world().get::<Spatial>(sound).is_some();
        if switch_off_after_start {app.world_mut().resource_mut::<SpatialAudio>().enabled=false;app.update();}
        let queue=outputs.lock().unwrap().pop().unwrap();
        let mut out=rodio::source::UniformSourceIterator::<_,f32>::new(queue,channels,48000);
        // 0.3 s to let any slew settle, then 0.1 s to measure.
        out.by_ref().take(14400*channels as usize).for_each(drop);
        let mut energy=vec![0.0f32;channels as usize];
        for frame in 0..4800 {for channel in 0..channels as usize {energy[channel]+=out.next().unwrap().powi(2);let _=frame;}}
        (energy.iter().map(|e|(e/4800.0).sqrt()).collect(),spatialised)
    }
    #[test]
    fn a_positional_sound_on_the_left_is_louder_in_the_left_channel_through_the_sink_chain() {
        let (left,spatialised)=spatial_run(true,Some(Vec3::new(-300.0,0.0,0.0)),2,false);
        assert!(spatialised && left[0]>0.25 && left[1]<0.05*left[0],"left source: {left:?}");
        let (right,_)=spatial_run(true,Some(Vec3::new(300.0,0.0,0.0)),2,false);
        assert!(right[1]>0.25 && right[0]<0.05*right[1],"right source: {right:?}");
        let (front,_)=spatial_run(true,Some(Vec3::new(0.0,0.0,300.0)),2,false);
        assert!((front[0]-front[1]).abs()<1e-3 && front[0]>0.2,"straight ahead: {front:?}");
        let (behind,_)=spatial_run(true,Some(Vec3::new(0.0,0.0,-300.0)),2,false);
        assert!((behind[0]-behind[1]).abs()<1e-3 && behind[0]<front[0]*0.75,"behind is quieter and duller: {behind:?} {front:?}");
        // A device with more channels gets the front pair like the baseline; a mono device gets one channel.
        let (quad,_)=spatial_run(true,Some(Vec3::new(-300.0,0.0,0.0)),4,false);
        assert!(quad[0]>0.25 && quad[1]<0.05*quad[0] && quad[2]==0.0 && quad[3]==0.0,"4 channels: {quad:?}");
        let (mono,_)=spatial_run(true,Some(Vec3::new(-300.0,0.0,0.0)),1,false);
        assert!(mono[0]>0.1,"mono device: {mono:?}");
    }
    #[test]
    fn sounds_without_a_position_and_the_switched_off_option_play_as_the_baseline() {
        let (twod,spatialised)=spatial_run(true,None,2,false);
        assert!(!spatialised && (twod[0]-twod[1]).abs()<1e-6 && twod[0]>0.25,"a 2D sound is untouched: {twod:?}");
        let (off,spatialised)=spatial_run(false,Some(Vec3::new(-300.0,0.0,0.0)),2,false);
        assert!(!spatialised && (off[0]-off[1]).abs()<1e-6 && (off[0]-twod[0]).abs()<1e-6,"option off: the plain duplicated mono of before: {off:?}");
        // Switched off while it plays: the same sound glides back to the baseline instead of stopping or clicking.
        let (later,spatialised)=spatial_run(true,Some(Vec3::new(-300.0,0.0,0.0)),2,true);
        assert!(spatialised && (later[0]-later[1]).abs()<0.02*later[0] && (later[0]-twod[0]).abs()<0.02,"{later:?}");
    }
    /// The real thing, silent: opens the devices whose names contain the comma-separated MESTER_PROBE_DEVICES (two of them), plays a
    /// muted zero-volume loop of silence, changes the preference and checks that the sound restarts on the other device.
    #[test]
    #[ignore="opens sound devices (silence only): MESTER_PROBE_DEVICES=Focusrite,DualSense"]
    fn switching_the_device_restarts_the_sounds_on_the_new_one() {
        let Some(wanted)=std::env::var("MESTER_PROBE_DEVICES").ok() else {return};
        let all=device_names();
        let pick:Vec<String>=wanted.split(',').filter_map(|part|all.iter().find(|name|name.contains(part)).cloned()).collect();
        assert_eq!(pick.len(),2,"two devices out of {all:?}");
        let preferences=|device:&str|crate::settings::Preferences {output_device:device.to_owned(),..default()};
        let mut app=App::new();
        app.add_plugins((bevy::app::TaskPoolPlugin::default(),bevy::asset::AssetPlugin::default(),Output)).insert_resource(GlobalVolume::new(bevy::audio::Volume::Linear(0.0)))
            .insert_resource(Session::new(preferences(&pick[0]),true,true));
        let handle=app.world_mut().resource_mut::<Assets<AudioSource>>().add(AudioSource {bytes:crate::audio_trace::silent_wav(30.0)});
        let sound=app.world_mut().spawn((AudioPlayer::new(handle),PlaybackSettings {muted:true,volume:bevy::audio::Volume::Linear(0.0),..PlaybackSettings::LOOP})).id();
        for _ in 0..3 {app.update();}
        assert_eq!(app.world().resource::<Session>().output,pick[0]);
        assert!(app.world().get::<AudioSink>(sound).is_some(),"the sound plays on the first device");
        app.world_mut().resource_mut::<Session>().preferences.output_device=pick[1].clone();
        for _ in 0..3 {app.update();}
        assert_eq!(app.world().resource::<Session>().output,pick[1]);
        assert!(app.world().get::<AudioSink>(sound).is_some(),"and again, from the start, on the second");
        app.world_mut().resource_mut::<Session>().preferences.output_device="unplugged".into();
        for _ in 0..3 {app.update();}
        assert!(!app.world().resource::<Session>().output.is_empty() && app.world().get::<AudioSink>(sound).is_some(),"a saved device that is gone falls back to automatic");
    }
    #[test]
    fn nothing_starts_without_an_open_device_and_a_broken_file_does_not_stop_the_game() {
        let mut app=App::new();
        app.init_resource::<Assets<AudioSource>>().insert_resource(GlobalVolume::default()).init_resource::<Sinks>().add_systems(Update,play_queued);
        let handle=app.world_mut().resource_mut::<Assets<AudioSource>>().add(AudioSource {bytes:b"not audio"[..].into()});
        let player=app.world_mut().spawn((AudioPlayer::new(handle),PlaybackSettings::DESPAWN)).id();
        app.update();
        assert!(app.world().get::<AudioSink>(player).is_none(),"no device, no sink; the sound waits");
        app.insert_resource(Sinks(Some(Box::new(||Some(rodio::Sink::new_idle().0)))));
        app.update();
        assert!(app.world().get_entity(player).is_err(),"a sound that does not decode is dropped, not unwrapped");
    }
}
