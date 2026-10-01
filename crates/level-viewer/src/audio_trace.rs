//! MESTER_AUDIO_TRACE=1 (capture runs only): a NON-AUDIBLE audio diagnostic (docs/retail-audio.md "Diagnostic").
//! The whole Bevy pipeline stays live on the real output device (AudioPlayer -> asset load -> sink -> play/pause -> despawn),
//! but nothing can be heard: every loaded sound is replaced by silence of the same length before its sink is made, every sink
//! is muted (the volume the game asked for stays readable as the managed volume) and GlobalVolume is 0. The real bytes are
//! decoded with the game's decoder on worker threads, so a file the audio thread could not play is reported.
//! Once per second the log shows the players, sinks and their state; a sound's last line says when its sink appeared, how many
//! frames it played or was paused, and the volume it was driven to.
use bevy::{asset::AssetLoadFailedEvent,audio::{AudioSink,AudioSinkPlayback,AudioSource,Decodable,GlobalVolume,Source},prelude::*};
use std::{collections::{BTreeMap,HashSet},sync::{Arc,Mutex,mpsc},time::Duration};
use crate::{campaign::Speech,music::MusicTrack,opening::SceneAudio,settings::Session,sound::Gain,spatial::{Mix,Spatial},video::VideoAudio};

/// The environment variable only counts in a capture run (a third command-line argument), never in a normal, audible one.
pub fn requested()->bool {std::env::var_os("MESTER_AUDIO_TRACE").is_some() && std::env::args().nth(3).is_some()}

/// What the exact decoder Bevy uses (`AudioSource::decoder`, rodio 0.20 with the `wav` and `mp3` features) made of a file.
#[derive(Debug,Clone,PartialEq)]
pub struct Decoded {pub channels:u16,pub rate:u32,pub samples:usize,pub peak:u16,pub seconds:f32,pub length:Option<Duration>}

/// Decodes at most `limit` samples of `bytes` the way the audio thread would. A panic (Bevy's `decoder()` unwraps, the WAV
/// reader panics on unknown sample formats, debug builds check integer overflow) comes back as the error text.
pub fn decode(bytes:&[u8],limit:usize)->Result<Decoded,String> {
    let bytes:Arc<[u8]>=bytes.into();
    std::panic::catch_unwind(move||{
        let mut decoder=AudioSource {bytes}.decoder();
        let (channels,rate,length)=(decoder.channels(),decoder.sample_rate(),decoder.total_duration());
        let (mut samples,mut peak)=(0usize,0u16);
        for sample in decoder.by_ref().take(limit) {samples+=1;peak=peak.max(sample.unsigned_abs());}
        Decoded {channels,rate,samples,peak,seconds:samples as f32/(channels.max(1) as f32*rate.max(1) as f32),length}
    }).map_err(|payload|payload.downcast_ref::<String>().cloned().or_else(||payload.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_else(||"panic".into()))
}

/// An 8 kHz mono 8-bit WAV of `seconds` of silence (8-bit PCM is unsigned: 128 is zero).
pub fn silent_wav(seconds:f32)->Arc<[u8]> {
    let n=(seconds.max(0.05)*8000.0) as u32;
    let mut b=Vec::with_capacity(44+n as usize);
    b.extend(b"RIFF");b.extend((36+n).to_le_bytes());b.extend(b"WAVEfmt ");b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());b.extend(1u16.to_le_bytes());b.extend(8000u32.to_le_bytes());b.extend(8000u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());b.extend(8u16.to_le_bytes());b.extend(b"data");b.extend(n.to_le_bytes());
    b.resize(44+n as usize,128);
    b.into()
}

const PROBE_SAMPLES:usize=4096;
/// Retail music is 56 kbit/s MPEG Layer-3 (docs/retail-audio.md); the length of a stream whose decoder cannot tell.
fn estimated_seconds(bytes:usize)->f32 {bytes as f32*8.0/56_000.0}

struct Entry {path:String,kind:&'static str,born:u32,sink:Option<u32>,playing:u32,paused:u32,peak:f32,muted:bool,state:String,ears:Option<Mix>}
/// The per-ear rendering of a spatialised sink (spatial.rs): pan gains, far-ear delay in microseconds, low-pass cut-offs.
fn ears(mix:&Option<Mix>)->String {mix.map_or(String::new(),|m|format!(", SPATIAL ears L={:.3} R={:.3} itd L={:.0}us R={:.0}us lowpass L={:.0}Hz R={:.0}Hz",m.gain[0],m.gain[1],m.delay[0]*1e6,m.delay[1]*1e6,m.cutoff[0],m.cutoff[1]))}

#[derive(Resource)]
pub struct Trace {
    seen:HashSet<AssetId<AudioSource>>,entries:BTreeMap<Entity,Entry>,frame:u32,report_at:f32,
    sender:mpsc::Sender<(String,Result<Decoded,String>)>,results:Mutex<mpsc::Receiver<(String,Result<Decoded,String>)>>,
    assets:u32,decode_failures:u32,load_failures:u32,spawned:u32,with_sink:u32,without_sink:u32,
}
impl Default for Trace {
    fn default()->Self {
        let (sender,results)=mpsc::channel();
        Self {seen:default(),entries:default(),frame:0,report_at:0.0,sender,results:Mutex::new(results),assets:0,decode_failures:0,load_failures:0,spawned:0,with_sink:0,without_sink:0}
    }
}

/// Registers the diagnostic; does nothing unless `requested()`.
pub struct AudioTrace;
impl Plugin for AudioTrace {
    fn build(&self,app:&mut App) {
        if !requested() {return;}
        app.init_resource::<Trace>().add_systems(Update,silence).add_systems(Last,(enforce,watch).chain());
        warn!("AUDIOTRACE on: sounds are replaced by silence and every sink is muted; nothing is audible");
    }
}

/// Replaces each newly loaded sound by silence of the same length (before PostUpdate makes its sink) after decoding the real
/// bytes: the first samples here for the format, the whole file on a worker thread for everything the audio thread would do.
fn silence(mut sources:ResMut<Assets<AudioSource>>,server:Res<AssetServer>,mut trace:ResMut<Trace>) {
    let fresh:Vec<_>=sources.ids().filter(|id|!trace.seen.contains(id)).collect();
    for id in fresh {
        trace.seen.insert(id);trace.assets+=1;
        let path=server.get_path(id).map(|path|path.to_string()).unwrap_or_else(||id.to_string());
        let Some(source)=sources.get_mut(id) else {continue};
        let real=source.bytes.clone();
        let seconds=match decode(&real,PROBE_SAMPLES) {
            Ok(head)=>{
                info!("AUDIOTRACE asset {path}: {} ch {} Hz {} bytes{}",head.channels,head.rate,real.len(),head.length.map_or(String::new(),|length|format!(" {:.2} s",length.as_secs_f32())));
                head.length.map_or_else(||estimated_seconds(real.len()),|length|length.as_secs_f32())
            },
            Err(reason)=>{error!("AUDIOTRACE asset {path} DOES NOT DECODE: {reason}");trace.decode_failures+=1;1.0}
        };
        source.bytes=silent_wav(seconds);
        let (sender,name)=(trace.sender.clone(),path);
        std::thread::spawn(move||{let result=decode(&real,usize::MAX);let _=sender.send((name,result));});
    }
}

/// Belt and braces: no sink of a trace run is ever left unmuted.
fn enforce(mut sinks:Query<&mut AudioSink>) {for mut sink in &mut sinks {if !sink.is_muted() {sink.mute();}}}

fn kind(music:bool,video:bool,scene:bool,speech:bool,positional:bool)->&'static str {
    if music {"music"}else if video {"video"}else if scene {"cutscene"}else if speech {"speech"}else if positional {"3d"}else{"2d"}
}

fn watch(time:Res<Time>,server:Res<AssetServer>,session:Res<Session>,global:Res<GlobalVolume>,front:Res<crate::frontend::Frontend>,mut trace:ResMut<Trace>,mut failures:MessageReader<AssetLoadFailedEvent<AudioSource>>,mut exit:MessageReader<AppExit>,
    players:Query<(Entity,&AudioPlayer,Option<&AudioSink>,Has<MusicTrack>,Has<VideoAudio>,Has<SceneAudio>,Has<Speech>,Option<&Gain>,Option<&Spatial>)>) {
    trace.frame+=1;
    let frame=trace.frame;
    for failure in failures.read() {error!("AUDIOTRACE load FAILED {}: {}",failure.path,failure.error);trace.load_failures+=1;}
    let results:Vec<_>=trace.results.lock().unwrap().try_iter().collect();
    for (path,result) in results {
        match result {
            Ok(full) if full.samples==0 || full.channels==0 || full.rate==0=>{error!("AUDIOTRACE decode {path}: EMPTY {full:?}");trace.decode_failures+=1;}
            Ok(full)=>info!("AUDIOTRACE decode {path}: complete, {:.2} s, peak {}{}",full.seconds,full.peak,if full.peak==0 {" (all zero: the retail file itself is silent)"}else{""}),
            Err(reason)=>{error!("AUDIOTRACE decode {path}: FAILED {reason}");trace.decode_failures+=1;}
        }
    }
    let mut alive=HashSet::new();
    for (entity,player,sink,music,video,scene,speech,gain,spatial) in &players {
        alive.insert(entity);
        let path=player.0.path().map_or_else(||"?".to_owned(),|path|path.to_string());
        if !trace.entries.contains_key(&entity) {trace.spawned+=1;}
        let entry=trace.entries.entry(entity).or_insert_with(||Entry {path,kind:kind(music,video,scene,speech,gain.is_some()),born:frame,sink:None,playing:0,paused:0,peak:0.0,muted:false,state:String::new(),ears:None});
        if let Some(spatial)=spatial {entry.ears=Some(spatial.0.get());}
        match sink {
            Some(sink)=>{
                entry.sink.get_or_insert(frame);
                if sink.is_paused() {entry.paused+=1;}else{entry.playing+=1;}
                entry.peak=entry.peak.max(sink.volume().to_linear());entry.muted=sink.is_muted();
            },
            None=>entry.state=format!("{:?}",server.load_state(player.0.id())),
        }
    }
    let ended:Vec<_>=trace.entries.keys().filter(|entity|!alive.contains(*entity)).copied().collect();
    for entity in ended {
        let Some(entry)=trace.entries.remove(&entity) else {continue};
        match entry.sink {
            Some(at)=>{trace.with_sink+=1;info!("AUDIOTRACE end {} {}: sink after {} frames, {} frames playing, {} paused, intended volume {:.3}, muted={}{}",entry.kind,entry.path,at-entry.born,entry.playing,entry.paused,entry.peak,entry.muted,ears(&entry.ears));},
            None=>{trace.without_sink+=1;error!("AUDIOTRACE end {} {}: NEVER GOT A SINK ({} frames, load state {})",entry.kind,entry.path,frame-entry.born,entry.state);},
        }
    }
    if time.elapsed_secs()>=trace.report_at {
        trace.report_at=time.elapsed_secs()+1.0;
        let sinks=trace.entries.values().filter(|entry|entry.sink.is_some()).count();
        info!("AUDIOTRACE t={:.1} paused={} main_menu={} volume_pref={:.2} global={:.2} players={} sinks={}",time.elapsed_secs(),session.paused,front.main_active,session.preferences.volume,global.volume.to_linear(),trace.entries.len(),sinks);
        for entry in trace.entries.values() {
            if entry.sink.is_some() {info!("AUDIOTRACE   {} {}: sink, {} frames playing, {} paused, intended volume {:.3}, muted={}{}",entry.kind,entry.path,entry.playing,entry.paused,entry.peak,entry.muted,ears(&entry.ears));}
            else {warn!("AUDIOTRACE   {} {}: NO SINK yet ({} frames, load state {})",entry.kind,entry.path,frame-entry.born,entry.state);}
        }
    }
    if exit.read().next().is_some() {
        warn!("AUDIOTRACE FINAL assets={} decode_failures={} load_failures={} players={} with_sink={} without_sink={} still_alive={}",trace.assets,trace.decode_failures,trace.load_failures,trace.spawned,trace.with_sink,trace.without_sink,trace.entries.len());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::audio::AudioSinkPlayback;

    fn files(dir:&std::path::Path,out:&mut Vec<std::path::PathBuf>) {
        let mut entries:Vec<_>=std::fs::read_dir(dir).unwrap().flatten().map(|e|e.path()).collect();entries.sort();
        for path in entries {if path.is_dir() {files(&path,out);}else{out.push(path);}}
    }
    #[test]
    fn silence_decodes_as_the_requested_length_of_zeros() {
        let d=decode(&silent_wav(1.5),usize::MAX).unwrap();
        assert_eq!((d.channels,d.rate,d.samples,d.peak),(1,8000,12000,0));
        assert!((d.length.unwrap().as_secs_f32()-1.5).abs()<0.01);
        assert!(decode(&silent_wav(0.0),usize::MAX).unwrap().samples>0,"never empty");
    }
    /// The sink chain the cpal thread pulls (Sink -> Amplify -> Pausable -> ... -> queue) with no device, converted to the
    /// 48 kHz stereo a mixer would ask for. Returns the peak of `frames` frames.
    /// A volume, pause or mute change reaches the samples within 5 ms (rodio's periodic access), so 10 ms are discarded first.
    fn peak(output:&mut impl Iterator<Item=f32>,frames:usize)->f32 {output.by_ref().take(960).for_each(drop);output.take(frames*2).fold(0.0,|peak,sample|peak.max(sample.abs()))}
    /// 1 s of a 16-bit stereo 22.05 kHz tone at half scale.
    pub(crate) fn tone_wav()->Vec<u8> {
        let n=22050u32;
        let mut b=Vec::new();
        b.extend(b"RIFF");b.extend((36+n*4).to_le_bytes());b.extend(b"WAVEfmt ");b.extend(16u32.to_le_bytes());b.extend(1u16.to_le_bytes());b.extend(2u16.to_le_bytes());
        b.extend(22050u32.to_le_bytes());b.extend(88200u32.to_le_bytes());b.extend(4u16.to_le_bytes());b.extend(16u16.to_le_bytes());b.extend(b"data");b.extend((n*4).to_le_bytes());
        for i in 0..n {let v=((i as f32*440.0*std::f32::consts::TAU/22050.0).sin()*16384.0) as i16;b.extend(v.to_le_bytes());b.extend(v.to_le_bytes());}
        b
    }
    fn chain(bytes:Vec<u8>,looped:bool)->(AudioSink,impl Iterator<Item=f32>) {
        use bevy::audio::Decodable;
        let (sink,output)=rodio::Sink::new_idle();
        let decoder=AudioSource {bytes:bytes.into()}.decoder();
        if looped {sink.append(decoder.repeat_infinite());}else{sink.append(decoder);}
        (AudioSink::new(sink),rodio::source::UniformSourceIterator::new(output,2,48000))
    }
    fn session(silent:bool)->Session {
        let mut session=Session::new(default(),silent,false);session.preferences.volume=0.6;session
    }
    #[test]
    fn an_audible_run_drives_the_sink_chain_to_the_preference_and_a_silent_run_mutes_it() {
        let (mut sink,mut out)=chain(tone_wav(),false);
        let audible=session(false);
        audible.drive(&mut sink,1.0);
        assert!(!sink.is_muted() && (sink.volume().to_linear()-0.6).abs()<1e-6);
        let full=peak(&mut out,2400);
        assert!((0.27..0.31).contains(&full),"half-scale tone at 0.6 is 0.3, got {full}");
        audible.drive(&mut sink,0.5);
        let half=peak(&mut out,2400);
        assert!((0.13..0.16).contains(&half),"distance gain 0.5 halves it, got {half}");
        sink.pause();assert_eq!(peak(&mut out,2400),0.0,"a paused sink delivers silence");
        sink.play();assert!(peak(&mut out,2400)>0.1,"and resumes");
        // A silent run mutes instead of lowering: nothing comes out but the volume it would have stays readable.
        let silent=session(true);
        silent.drive(&mut sink,1.0);
        assert!(sink.is_muted() && (sink.volume().to_linear()-0.6).abs()<1e-6);
        assert_eq!(peak(&mut out,2400),0.0);
        audible.drive(&mut sink,1.0);
        assert!(!sink.is_muted() && peak(&mut out,2400)>0.2);
    }
    #[test]
    fn a_looped_source_keeps_flowing_past_its_end() {
        let (mut sink,mut out)=chain(tone_wav(),true);
        session(false).drive(&mut sink,1.0);
        // 3 s of output is three passes over the 1 s tone.
        for second in 0..3 {assert!(peak(&mut out,48000)>0.25,"second {second} of the loop is silent");}
    }
    /// The real thing through the real chain: the level music (looped MPEG Layer-3) and a footstep, if the export is here.
    #[test]
    fn exported_music_and_effects_reach_the_end_of_the_sink_chain() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/audio/sounds");
        let (Ok(music),Ok(step))=(std::fs::read(root.join("muza/prolog.mp3")),std::fs::read(root.join("hero/KROK1.WAV"))) else {return};
        for (name,bytes,looped) in [("prolog.mp3",music,true),("KROK1.WAV",step,false)] {
            let (mut sink,mut out)=chain(bytes,looped);
            session(false).drive(&mut sink,1.0);
            let heard=peak(&mut out,48000);
            assert!(heard>0.02 && heard<=0.61,"{name}: peak {heard}");
        }
    }
    /// How much of real time the audio thread spends in this (unoptimised) profile: pulls `seconds` of 48 kHz 4-channel output
    /// (the DualSense pad the default device can be) of the level music plus `voices` looping effects, timing it.
    #[test]
    #[ignore="timing probe"]
    fn audio_thread_load_in_the_debug_profile() {
        use bevy::audio::Decodable;
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/audio/sounds");
        let music=std::fs::read(root.join("muza/prolog.mp3")).unwrap();
        let step=std::fs::read(root.join("hero/KROK1.WAV")).unwrap();
        let mut cases:Vec<(String,Vec<u8>,bool)>=vec![("KROK1.WAV looped".into(),step.clone(),true),("KROK1.WAV once".into(),step.clone(),false)];
        cases.push(("outro/01.wav once (15 s stereo)".into(),std::fs::read(root.join("scenes/outro/01.wav")).unwrap(),false));
        cases.push(("prolog.mp3 once".into(),music.clone(),false));
        for (name,bytes,looped) in cases {
            let (sink,output)=rodio::Sink::new_idle();
            let decoder=AudioSource {bytes:bytes.into()}.decoder();
            if looped {sink.append(decoder.repeat_infinite());}else{sink.append(decoder);}
            let mut out=rodio::source::UniformSourceIterator::new(output,4,48000);
            let started=std::time::Instant::now();
            let mut total=0.0f32;
            for _ in 0..10*48000*4 {total+=out.next().unwrap_or(0.0);}
            println!("{name}: 10 s of 4ch 48k output took {:.2} s ({total})",started.elapsed().as_secs_f32());
            drop(sink);
        }
        for voices in [0usize,4,16] {
            let (sink,output)=rodio::Sink::new_idle();
            sink.append(AudioSource {bytes:music.clone().into()}.decoder().repeat_infinite());
            let extra:Vec<_>=(0..voices).map(|_|{let (s,o)=rodio::Sink::new_idle();s.append(AudioSource {bytes:step.clone().into()}.decoder().repeat_infinite());(s,o)}).collect();
            let mut mixed:Vec<Box<dyn Iterator<Item=f32>>>=vec![Box::new(rodio::source::UniformSourceIterator::new(output,4,48000))];
            let (alive,outputs):(Vec<_>,Vec<_>)=extra.into_iter().unzip();
            for o in outputs {mixed.push(Box::new(rodio::source::UniformSourceIterator::new(o,4,48000)));}
            let seconds=10usize;
            let started=std::time::Instant::now();
            let mut total=0.0f32;
            for _ in 0..seconds*48000*4 {for m in mixed.iter_mut() {total+=m.next().unwrap_or(0.0);}}
            let took=started.elapsed().as_secs_f32();
            println!("voices {voices}: {seconds} s of output took {took:.2} s (load {:.2}) {total}",took/seconds as f32);
            drop((sink,alive));
        }
    }
    /// Where this PC's sound goes: every output device, the system default, and where the game would put its sound. Nothing is played;
    /// opening the stream (silence only) is opt-in: MESTER_PROBE_OPEN=1.
    #[test]
    #[ignore="asks the sound system for its devices"]
    fn output_device_report() {
        use rodio::cpal::traits::{DeviceTrait,HostTrait};
        let host=rodio::cpal::default_host();
        let default=host.default_output_device().and_then(|device|device.name().ok());
        println!("default: {default:?}");
        for device in host.output_devices().unwrap() {
            println!("device: {} -> {:?}",device.name().unwrap_or_default(),device.default_output_config().map(|c|(c.channels(),c.sample_rate().0,c.sample_format())));
        }
        let names=crate::output::device_names();
        println!("the game tries: {:?}",crate::output::order(&names,default.as_deref(),"").into_iter().map(|index|names[index].clone()).collect::<Vec<_>>());
        if std::env::var_os("MESTER_PROBE_OPEN").is_some() {
            match crate::output::open("") {Some(opened)=>println!("opened {} ({})",opened.name,opened.note),None=>println!("NO device opens")}
        }
    }
    #[test]
    fn garbage_is_reported_not_propagated() {
        assert!(decode(b"not audio at all",64).is_err());
        assert!(decode(&[],64).is_err());
        assert!(decode(&silent_wav(1.0)[..30],64).is_err(),"truncated header");
    }
    /// Every exported sound (wav and mp3) must decode completely with the decoder the game uses, in the same (debug) profile
    /// the launcher runs. Prints every failure with its reason.
    #[test]
    #[ignore="requires local original audio exports; decodes about 230 MB"]
    fn every_exported_sound_decodes_with_the_game_decoder() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/audio");
        let mut all=Vec::new();files(&root,&mut all);
        let (mut failures,mut silent,mut total)=(Vec::new(),Vec::new(),0);
        for path in all {
            let name=path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\',"/");
            total+=1;
            match decode(&std::fs::read(&path).unwrap(),usize::MAX) {
                Ok(d) if d.samples==0 || d.channels==0 || d.rate==0=>failures.push(format!("{name}: empty {d:?}")),
                Ok(d)=>{if d.peak==0 {silent.push(name.clone());}println!("ok {name} {}ch {}Hz {:.2}s peak {}",d.channels,d.rate,d.seconds,d.peak);}
                Err(reason)=>failures.push(format!("{name}: {reason}")),
            }
        }
        println!("{total} files, {} failures, {} all-zero",failures.len(),silent.len());
        for failure in &failures {println!("FAIL {failure}");}
        for name in &silent {println!("SILENT {name}");}
        assert!(failures.is_empty(),"{} of {total} sounds do not decode:\n{}",failures.len(),failures.join("\n"));
        // The 22 all-zero files are the retail cutscene voice tracks (sounds/scenes/intro*, outro): timing placeholders that are
        // byte-identical in the retail install. Nothing else in the tree may be silent.
        assert!(silent.iter().all(|name|name.starts_with("sounds/scenes/")),"silent effects: {silent:?}");
    }
}
