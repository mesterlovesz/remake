//! "Javított 3D hangzás" (docs/retail-audio.md "Improved 3D audio"): every positional sound (`sound::Positional`) is rendered for the
//! listener (the camera) instead of only being attenuated. Per ear: an equal-power pan by the lateral direction, a rear level
//! cut, an interaural time difference (Woodworth) on the far ear, and a one-pole low-pass for the head shadow, sources behind and
//! sources far above. The distance gain (`distance_gain`, inverse distance faded out at the retail radius) stays the sink volume
//! (`Gain`, settings::update); this module only shapes the two ears. Switch off: `Options::spatial_audio`, then the game plays the
//! retail baseline (linear falloff, no pan) exactly as before.
//!
//! The effect is a rodio `Source` wrapper (`Spatialized`) in the sink chain that `output::play_queued` builds: it downmixes the sound
//! to mono, delays / filters / scales it per ear and hands the device stereo (a mono device gets one channel, devices with more
//! channels get the front pair like the baseline: their other channels are haptics, headphone jacks or other outputs, never rear
//! speakers we can rely on). The game thread writes the targets (`Mix`) into `SpatialState` every frame; the audio thread slews the
//! per-ear gains, delays and cut-offs sample by sample (20 ms), so a turning player never hears a click.
use std::{sync::{Arc,atomic::{AtomicU32,Ordering}},time::Duration};
use bevy::{prelude::*,ecs::system::SystemParam};
use rodio::{Sample,Source};
use crate::{InspectionCamera,SCALE,options::Options,sound::Positional};

/// A sound closer than this (native units, 60 cm) has no direction: the pan fades to the centre.
pub const NEAR:f32=60.0;
/// Distance gain: full level up to `REFERENCE` x radius, then inverse distance (rolloff 0.35) faded to silence at the retail radius.
pub const REFERENCE:f32=0.1;
pub const ROLLOFF:f32=0.35;
pub const FADE_FROM:f32=0.6;
/// Head radius 8.75 cm, speed of sound 343 m/s, and "gentle": 70 % of the Woodworth time difference (0.46 ms at 90 degrees).
const HEAD:f32=0.0875;const SOUND_SPEED:f32=343.0;const ITD_SCALE:f32=0.7;
/// Level of a source directly behind (retail cut it up to 25 %, Lithtech.exe 0x4804a0).
const REAR_CUT:f32=0.25;
pub const OPEN:f32=20000.0;const CLOSED:f32=2500.0;

/// Retail-shaped linear falloff of the baseline (Lithtech.exe 0x480940): 1 - d/R.
pub fn retail_gain(distance:f32,radius:f32)->f32 {if radius<=0.0 {0.0}else{(1.0-distance/radius).clamp(0.0,1.0)}}
/// Improved distance gain: 1 within 10 % of the radius, then inverse distance (rolloff 0.35), a cosine fade from 60 % of the radius
/// to exactly 0 at the retail radius (so nothing pops at the audible limit, and the retail range is kept).
pub fn distance_gain(distance:f32,radius:f32)->f32 {
    if radius<=0.0 || distance>=radius {return 0.0;}
    let reference=radius*REFERENCE;
    let inverse=reference/(reference+(distance-reference).max(0.0)*ROLLOFF);
    let t=distance/radius;
    let fade=if t<FADE_FROM {1.0}else{0.5*(1.0+(std::f32::consts::PI*(t-FADE_FROM)/(1.0-FADE_FROM)).cos())};
    inverse*fade
}

/// The ears of the player: native-unit position and the unit vectors of the head. `right` is the direction of the player's RIGHT EAR
/// as seen on screen (already handed for the display mirror).
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Listener {pub position:Vec3,pub right:Vec3,pub forward:Vec3,pub up:Vec3}
impl Listener {
    /// From the camera transform (render units) and its position in native units. mirror.rs shows the image left/right flipped
    /// (LithTech is left-handed): a source on the right of the picture is on the LEFT of a Bevy camera, so the ear axis flips too.
    pub fn from_camera(camera:&Transform,mirrored:bool)->Self {
        Self {position:camera.translation/SCALE,right:*camera.right()*if mirrored {-1.0}else{1.0},forward:*camera.forward(),up:*camera.up()}
    }
}

/// What the two ears get: gains (pan, rear cut), far-ear delays in seconds and low-pass cut-offs in Hz; index 0 left, 1 right.
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Mix {pub gain:[f32;2],pub delay:[f32;2],pub cutoff:[f32;2]}
impl Mix {
    /// A plain mono sound duplicated to both ears (the baseline).
    pub const NEUTRAL:Mix=Mix {gain:[1.0,1.0],delay:[0.0,0.0],cutoff:[OPEN,OPEN]};
}

/// The per-ear rendering of a source at `source` (native units) for `listener`.
pub fn mix(listener:&Listener,source:Vec3)->Mix {
    let offset=source-listener.position;
    let distance=offset.length();
    let direction=if distance>1e-3 {offset/distance}else{listener.forward};
    // Close to the head the direction is unreliable and the sound is "inside": fade all cues in with the distance.
    let near=(distance/NEAR).clamp(0.0,1.0);
    let lateral=(direction.dot(listener.right)*near).clamp(-1.0,1.0);
    let behind=(-direction.dot(listener.forward)).max(0.0)*near;
    let elevation=direction.dot(listener.up).abs()*near;
    // Equal-power pan on the lateral axis (front and back sound alike on two speakers; the rear cues below tell them apart).
    let angle=(lateral+1.0)*std::f32::consts::FRAC_PI_4;
    let rear=1.0-REAR_CUT*behind;
    let gain=[angle.cos().max(0.0)*rear,angle.sin().max(0.0)*rear];
    // Woodworth: the far ear (left when the source is on the right) hears it (a/c)(theta + sin theta) later.
    let theta=lateral.abs().asin();
    let itd=ITD_SCALE*(HEAD/SOUND_SPEED)*(theta+theta.sin());
    let delay=[if lateral>0.0 {itd}else{0.0},if lateral<0.0 {itd}else{0.0}];
    // Low-pass: sources behind and well above (or below) the head lose highs; the far ear also sits in the head shadow.
    let common=0.9*behind+0.25*((elevation-0.3).max(0.0)/0.7);
    let closing=|shadow:f32|OPEN*(CLOSED/OPEN).powf((common+0.6*shadow).clamp(0.0,1.0));
    let cutoff=[closing((lateral).max(0.0)),closing((-lateral).max(0.0))];
    Mix {gain,delay,cutoff}
}

/// Targets shared with the audio thread (f32 bits): gain L R, delay L R, cutoff L R.
pub struct SpatialState {slots:[AtomicU32;6]}
impl SpatialState {
    pub fn new(mix:&Mix)->Arc<Self> {let state=Arc::new(Self {slots:Default::default()});state.set(mix);state}
    pub fn set(&self,mix:&Mix) {
        for (slot,value) in self.slots.iter().zip([mix.gain[0],mix.gain[1],mix.delay[0],mix.delay[1],mix.cutoff[0],mix.cutoff[1]]) {slot.store(value.to_bits(),Ordering::Relaxed);}
    }
    pub fn get(&self)->Mix {
        let v:Vec<f32>=self.slots.iter().map(|slot|f32::from_bits(slot.load(Ordering::Relaxed))).collect();
        Mix {gain:[v[0],v[1]],delay:[v[2],v[3]],cutoff:[v[4],v[5]]}
    }
}
/// The state of a spatialised sink, on the sound entity.
#[derive(Component)] pub struct Spatial(pub Arc<SpatialState>);

/// Switch, listener and device format of the run; `output::play_queued` reads it when a sink is made.
#[derive(Resource)]
pub struct SpatialAudio {pub enabled:bool,pub listener:Option<Listener>,pub device_channels:u16}
impl Default for SpatialAudio {fn default()->Self {Self {enabled:false,listener:None,device_channels:2}}}
impl SpatialAudio {
    /// The state a new positional sound starts with: its real mix for the listener now, so nothing ramps in.
    pub fn start(&self,source:Vec3)->Option<Arc<SpatialState>> {
        self.enabled.then(||SpatialState::new(&self.listener.as_ref().map_or(Mix::NEUTRAL,|listener|mix(listener,source))))
    }
}
/// Whether new sounds are spatialised (false without the resource: tests, menus before the setup).
pub fn enabled(world:&World)->bool {world.get_resource::<SpatialAudio>().is_some_and(|audio|audio.enabled)}

const CONTROL:u32=32;
const BUFFER:usize=128;
/// Slew time of gains, delays and cut-offs.
const SLEW:f32=0.02;

/// The sink-chain effect: any source in, `out` channels out (stereo, or one for a mono device).
pub struct Spatialized<S:Source> where S::Item:Sample {
    inner:S,in_channels:u16,out_channels:u16,rate:f32,state:Arc<SpatialState>,
    now:Mix,goal:Mix,slew:f32,coefficient:[f32;2],ticks:u32,
    line:[f32;BUFFER],head:usize,low:[f32;2],frame:[f32;2],cursor:u16,
}
impl<S:Source> Spatialized<S> where S::Item:Sample {
    pub fn new(inner:S,state:Arc<SpatialState>,mono_device:bool)->Self {
        let (in_channels,rate)=(inner.channels().max(1),inner.sample_rate().max(1) as f32);
        let goal=state.get();
        let mut source=Self {inner,in_channels,out_channels:if mono_device {1}else{2},rate,state,now:goal,goal,slew:1.0-(-1.0/(rate*SLEW)).exp(),coefficient:[1.0;2],ticks:0,line:[0.0;BUFFER],head:0,low:[0.0;2],frame:[0.0;2],cursor:2};
        source.coefficient=source.coefficients();
        source
    }
    fn coefficients(&self)->[f32;2] {
        let one=|cutoff:f32|if cutoff>=self.rate*0.45 {1.0}else{1.0-(-std::f32::consts::TAU*cutoff/self.rate).exp()};
        [one(self.now.cutoff[0]),one(self.now.cutoff[1])]
    }
    fn render(&mut self,mono:f32) {
        self.head=(self.head+1)%BUFFER;self.line[self.head]=mono;
        for ear in 0..2 {
            self.now.gain[ear]+=(self.goal.gain[ear]-self.now.gain[ear])*self.slew;
            self.now.delay[ear]+=(self.goal.delay[ear]-self.now.delay[ear])*self.slew;
            self.now.cutoff[ear]+=(self.goal.cutoff[ear]-self.now.cutoff[ear])*self.slew;
        }
        self.ticks+=1;
        if self.ticks%CONTROL==0 {self.goal=self.state.get();self.coefficient=self.coefficients();}
        for ear in 0..2 {
            let samples=(self.now.delay[ear]*self.rate).clamp(0.0,(BUFFER-2) as f32);
            let (whole,fraction)=(samples as usize,samples.fract());
            let (a,b)=(self.line[(self.head+BUFFER-whole)%BUFFER],self.line[(self.head+BUFFER-whole-1)%BUFFER]);
            let delayed=a+(b-a)*fraction;
            self.low[ear]+=self.coefficient[ear]*(delayed-self.low[ear]);
            self.frame[ear]=self.low[ear]*self.now.gain[ear];
        }
        if self.out_channels==1 {self.frame[0]=(self.frame[0]+self.frame[1])*0.5;}
    }
}
impl<S:Source> Iterator for Spatialized<S> where S::Item:Sample {
    type Item=f32;
    fn next(&mut self)->Option<f32> {
        if self.cursor>=self.out_channels {
            let mut sum=0.0;
            for channel in 0..self.in_channels {match self.inner.next() {Some(sample)=>sum+=sample.to_f32(),None=>if channel==0 {return None}}}
            self.render(sum/self.in_channels as f32);self.cursor=0;
        }
        let sample=self.frame[self.cursor as usize];self.cursor+=1;Some(sample)
    }
}
impl<S:Source> Source for Spatialized<S> where S::Item:Sample {
    fn current_frame_len(&self)->Option<usize> {self.inner.current_frame_len().map(|len|(len/self.in_channels as usize).max(1)*self.out_channels as usize)}
    fn channels(&self)->u16 {self.out_channels}
    fn sample_rate(&self)->u32 {self.inner.sample_rate()}
    fn total_duration(&self)->Option<Duration> {self.inner.total_duration()}
}

/// The game-thread side: the listener from the camera and the option, every frame.
#[derive(SystemParam)]
pub struct Ears<'w,'s> {options:Option<Res<'w,Options>>,camera:Query<'w,'s,&'static Transform,With<InspectionCamera>>}
pub fn listen(ears:Ears,mut audio:ResMut<SpatialAudio>) {
    audio.enabled=ears.options.as_ref().is_none_or(|options|options.spatial_audio);
    audio.listener=ears.camera.iter().next().map(|camera|Listener::from_camera(camera,crate::mirror::MIRRORED));
}
/// Writes the per-ear targets of every spatialised sound: the real mix while the option is on, the neutral one when it was switched
/// off under a sound that is still playing (it then continues as the baseline, without a click).
pub fn drive(audio:Res<SpatialAudio>,sounds:Query<(&Positional,&Spatial)>) {
    for (source,state) in &sounds {
        let target=match (&audio.listener,audio.enabled) {(Some(listener),true)=>mix(listener,source.position),_=>Mix::NEUTRAL};
        state.0.set(&target);
    }
}

pub struct SpatialPlugin;
impl Plugin for SpatialPlugin {
    fn build(&self,app:&mut App) {
        app.init_resource::<SpatialAudio>().add_systems(Update,(listen,drive).chain().before(crate::sound::update));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    /// Listener at the origin facing LithTech +Z with LithTech +X on its right (what the player sees under the display mirror).
    fn player()->Listener {Listener {position:Vec3::ZERO,right:Vec3::X,forward:Vec3::Z,up:Vec3::Y}}
    fn at(x:f32,y:f32,z:f32)->Mix {mix(&player(),Vec3::new(x,y,z))}
    #[test]
    fn a_source_ahead_is_centred_dry_and_equal_power() {
        let front=at(0.0,0.0,300.0);
        assert!((front.gain[0]-front.gain[1]).abs()<1e-6 && (front.gain[0]-std::f32::consts::FRAC_1_SQRT_2).abs()<1e-5,"{front:?}");
        assert!((front.gain[0].powi(2)+front.gain[1].powi(2)-1.0).abs()<1e-5,"equal power");
        assert_eq!((front.delay,front.cutoff),([0.0,0.0],[OPEN,OPEN]),"no time difference, no roll-off in front");
    }
    #[test]
    fn a_source_on_the_left_is_louder_in_the_left_ear_and_reaches_the_right_ear_later_and_duller() {
        let left=at(-300.0,0.0,0.0);
        assert!(left.gain[0]>0.999 && left.gain[1]<1e-3,"{left:?}");
        assert!((left.delay[1]-0.00046).abs()<0.00003 && left.delay[0]==0.0,"gentle ITD, far (right) ear delayed: {:?}",left.delay);
        assert!(left.cutoff[1]<left.cutoff[0] && left.cutoff[0]==OPEN,"head shadow on the far ear");
        let right=at(300.0,0.0,0.0);
        assert_eq!((right.gain,right.delay,right.cutoff),([left.gain[1],left.gain[0]],[left.delay[1],left.delay[0]],[left.cutoff[1],left.cutoff[0]]),"mirror image");
        let half=at(-300.0,0.0,300.0);
        assert!(half.gain[0]>half.gain[1] && half.gain[1]>0.2 && half.delay[1]>0.0 && half.delay[1]<left.delay[1],"45 degrees is in between: {half:?}");
    }
    #[test]
    fn a_source_behind_is_quieter_and_duller_than_the_same_pan_in_front() {
        let (front,back)=(at(-100.0,0.0,300.0),at(-100.0,0.0,-300.0));
        assert!((front.gain[0]/front.gain[1]-back.gain[0]/back.gain[1]).abs()<0.02 || back.gain[0]<front.gain[0],"same lateral direction, same ear balance");
        assert!(back.gain[0]<front.gain[0] && back.gain[1]<front.gain[1]);
        assert!(back.cutoff[0]<front.cutoff[0]*0.5,"the high frequencies go: {:?} vs {:?}",back.cutoff,front.cutoff);
        let straight=at(0.0,0.0,-300.0);
        assert!((straight.gain[0]-straight.gain[1]).abs()<1e-6 && straight.gain[0]<0.75*std::f32::consts::FRAC_1_SQRT_2+1e-3,"directly behind: centre, 25 % cut: {straight:?}");
        assert!(straight.cutoff[0]<3600.0,"and dull: {:?}",straight.cutoff);
    }
    #[test]
    fn a_source_above_is_centred_and_a_little_duller() {
        let above=at(0.0,300.0,0.0);
        assert!((above.gain[0]-above.gain[1]).abs()<1e-6 && above.delay==[0.0,0.0]);
        assert!(above.cutoff[0]<OPEN && above.cutoff[0]>at(0.0,0.0,-300.0).cutoff[0],"elevation cue is milder than the rear one");
    }
    #[test]
    fn a_source_at_the_head_has_no_direction_and_no_nan() {
        for source in [Vec3::ZERO,Vec3::new(1.0,0.0,0.0),Vec3::new(-20.0,0.0,0.0)] {
            let near=mix(&player(),source);
            assert!(near.gain.iter().chain(&near.delay).chain(&near.cutoff).all(|v|v.is_finite()),"{near:?}");
            assert!((near.gain[0]-near.gain[1]).abs()<0.4,"the pan fades to the centre close to the head: {near:?}");
        }
        assert!((mix(&player(),Vec3::ZERO).gain[0]-std::f32::consts::FRAC_1_SQRT_2).abs()<1e-5);
    }
    #[test]
    fn the_distance_gain_is_full_up_close_falls_with_distance_and_ends_at_the_retail_radius() {
        let r=1280.0;
        assert_eq!((distance_gain(0.0,r),distance_gain(r*0.1,r),distance_gain(r,r),distance_gain(r*2.0,r),distance_gain(10.0,0.0)),(1.0,1.0,0.0,0.0,0.0));
        let mut last=1.0;
        for step in 1..=100 {let g=distance_gain(r*step as f32/100.0,r);assert!(g<=last+1e-6,"monotone at {step}");last=g;}
        assert!(distance_gain(r*0.99,r)<0.01,"no pop at the audible limit");
        // Comparable to the retail linear curve (never far louder or much quieter in the middle of the range).
        for t in [0.25,0.5,0.75] {let (improved,retail)=(distance_gain(r*t,r),retail_gain(r*t,r));assert!(improved>0.5*retail-0.02 && improved<1.0,"t={t}: {improved} vs {retail}");}
        assert!((distance_gain(r*0.5,r)-0.417).abs()<0.01);
        assert_eq!(retail_gain(640.0,1280.0),0.5);
    }
    #[test]
    fn the_ears_follow_the_picture_under_the_display_mirror() {
        // A Bevy camera faces LithTech +Z with its own right = world -X. LithTech +X is on the RIGHT of the mirrored picture.
        let camera=Transform::IDENTITY.looking_to(Vec3::Z,Vec3::Y);
        let mirrored=Listener::from_camera(&camera,true);
        assert!((mirrored.right-Vec3::X).length()<1e-6,"{:?}",mirrored.right);
        let on_screen_right=mix(&mirrored,Vec3::new(300.0,0.0,0.0));
        assert!(on_screen_right.gain[1]>0.99 && on_screen_right.gain[0]<0.01,"right of the image, right ear");
        let unmirrored=Listener::from_camera(&camera,false);
        assert!(mix(&unmirrored,Vec3::new(300.0,0.0,0.0)).gain[0]>0.99,"without the mirror the same source is on the left");
        // Turn the camera 90 degrees to the right (yaw towards LithTech +X): the +Z source is now on the left of the picture.
        let turned=Transform::IDENTITY.looking_to(Vec3::X,Vec3::Y);
        let (l,r)=(Listener::from_camera(&turned,true),Vec3::new(0.0,0.0,300.0));
        assert!(mix(&l,r).gain[0]>0.99,"turning right puts the +Z source on the left: {:?}",mix(&l,r));
        // The camera translation is in render units: the listener is in native ones.
        let moved=Listener::from_camera(&Transform::from_translation(Vec3::new(1.0,2.0,3.0)),true);
        assert!((moved.position-Vec3::new(1.0,2.0,3.0)/SCALE).length()<1e-3);
    }
    fn tone(rate:u32,seconds:f32)->Vec<i16> {(0..(rate as f32*seconds) as usize).map(|i|((i as f32*440.0*std::f32::consts::TAU/rate as f32).sin()*16000.0) as i16).collect()}
    fn render(mix:&Mix,frames:usize)->(Vec<f32>,Vec<f32>) {
        let state=SpatialState::new(mix);
        let out:Vec<f32>=Spatialized::new(SamplesBuffer::new(1,22050,tone(22050,1.0)),state,false).take(frames*2).collect();
        (out.iter().step_by(2).copied().collect(),out.iter().skip(1).step_by(2).copied().collect())
    }
    fn rms(samples:&[f32])->f32 {(samples.iter().map(|s|s*s).sum::<f32>()/samples.len().max(1) as f32).sqrt()}
    #[test]
    fn the_effect_renders_a_mono_source_left_or_right_and_the_neutral_mix_changes_nothing() {
        let (l,r)=render(&at(-300.0,0.0,0.0),10000);
        assert!(rms(&l)>0.25 && rms(&r)<0.01,"left source, left ear: {} {}",rms(&l),rms(&r));
        let (l,r)=render(&at(300.0,0.0,0.0),10000);
        assert!(rms(&r)>0.25 && rms(&l)<0.01);
        let (l,r)=render(&Mix::NEUTRAL,2000);
        let plain=tone(22050,1.0);
        assert!(l.iter().zip(&r).zip(&plain).all(|((l,r),p)|(l-r).abs()<1e-6 && (l-*p as f32/32768.0).abs()<1e-4),"neutral = the source, both ears");
    }
    #[test]
    fn the_far_ear_is_delayed_by_the_interaural_time_difference() {
        // A single click at the first frame: the far ear's copy arrives itd seconds later (10 samples at 22.05 kHz = 0.45 ms).
        let state=SpatialState::new(&Mix {gain:[1.0,1.0],delay:[0.0,10.0/22050.0],cutoff:[OPEN,OPEN]});
        let mut click=vec![0i16;200];click[20]=20000;
        let out:Vec<f32>=Spatialized::new(SamplesBuffer::new(1,22050,click),state,false).collect();
        let peak=|ear:usize|out.iter().skip(ear).step_by(2).enumerate().max_by(|a,b|a.1.abs().total_cmp(&b.1.abs())).unwrap().0;
        assert_eq!(peak(1)-peak(0),10);
    }
    #[test]
    fn a_stereo_source_becomes_positional_and_a_mono_device_gets_one_channel() {
        let stereo:Vec<i16>=tone(22050,0.2).iter().flat_map(|s|[*s,*s]).collect();
        let state=SpatialState::new(&at(-300.0,0.0,0.0));
        let source=Spatialized::new(SamplesBuffer::new(2,22050,stereo.clone()),state.clone(),false);
        assert_eq!((source.channels(),source.sample_rate()),(2,22050));
        let out:Vec<f32>=source.collect();
        assert_eq!(out.len(),stereo.len());
        assert!(rms(&out.iter().step_by(2).copied().collect::<Vec<_>>())>0.25 && rms(&out.iter().skip(1).step_by(2).copied().collect::<Vec<_>>())<0.01);
        let mono=Spatialized::new(SamplesBuffer::new(1,22050,tone(22050,0.2)),SpatialState::new(&Mix::NEUTRAL),true);
        assert_eq!(mono.channels(),1);
        let out:Vec<f32>=mono.collect();
        assert_eq!(out.len(),4410);assert!((rms(&out)-rms(&tone(22050,0.2).iter().map(|s|*s as f32/32768.0).collect::<Vec<_>>())).abs()<0.01);
    }
    #[test]
    fn changing_the_target_slews_instead_of_clicking() {
        // A DC signal makes every jump audible: the pan flips from hard left to hard right while it plays.
        let state=SpatialState::new(&at(-300.0,0.0,0.0));
        let mut source=Spatialized::new(SamplesBuffer::new(1,22050,vec![16000i16;22050]),state.clone(),false);
        let mut left=Vec::new();
        for frame in 0..8000 {
            if frame==2000 {state.set(&at(300.0,0.0,0.0));}
            let l=source.next().unwrap();let _r=source.next().unwrap();left.push(l);
        }
        let biggest=left.windows(2).map(|w|(w[1]-w[0]).abs()).fold(0.0f32,f32::max);
        assert!(biggest<0.02,"largest step of the left ear {biggest} (an abrupt flip would be 0.49)");
        assert!(left[1990]>0.48 && left[7999]<0.01,"and it does get there: {} {}",left[1990],left[7999]);
    }
    #[test]
    fn the_state_survives_the_round_trip_and_the_start_mix_is_the_real_one() {
        let want=at(-300.0,50.0,100.0);
        assert_eq!(SpatialState::new(&want).get(),want);
        let mut audio=SpatialAudio::default();
        assert!(audio.start(Vec3::X).is_none(),"switched off: no effect in the chain");
        audio.enabled=true;
        assert_eq!(audio.start(Vec3::X).unwrap().get(),Mix::NEUTRAL,"no listener yet: neutral");
        audio.listener=Some(player());
        assert_eq!(audio.start(Vec3::new(-300.0,0.0,0.0)).unwrap().get(),at(-300.0,0.0,0.0),"a new sound starts at its real pan");
    }
}
