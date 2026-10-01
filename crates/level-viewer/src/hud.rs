//! Retail HUD (cshell.dll, HUD object 0x10a0b420; evidence and numbers in docs/retail-hud.md).
//! Everything is laid out in the original 1024x768 space and tinted with the HUD colour (default palette 7 = 208,115,16,
//! alpha 196, 0x1001acdd); each group fades with its own timer: the image draws with min(timer, 1) and the timer runs down
//! at one per second once its condition ends.
use bevy::prelude::*;
use crate::{Walking,campaign::Campaign,settings::Session,panels::Panels,retail_ui::{label,BitmapText},retail_weapons::{NativeArsenal,NativeHud}};

#[derive(Component)] pub struct Help;
#[derive(Component)] pub struct GameHud;
#[derive(Component)] pub struct Crosshair;
#[derive(Component)] pub struct DeathOverlay;
/// HUD colour of the default palette 7 (0x1001acdd; table at 0x100b240c, 12 bytes per entry) and its base alpha (0x100b2408 = 196).
pub const HUD_PALETTE:[[f32;3];8]=[[175.0,19.0,19.0],[201.0,78.0,78.0],[131.0,51.0,121.0],[63.0,119.0,194.0],[91.0,205.0,204.0],[37.0,133.0,76.0],[232.0,227.0,73.0],[208.0,115.0,16.0]];
pub const HUD_RGB:[f32;3]=[HUD_PALETTE[7][0]/255.0,HUD_PALETTE[7][1]/255.0,HUD_PALETTE[7][2]/255.0];
pub const HUD_ALPHA:f32=196.0/255.0;
/// The HUD colour in use: the palette entry the options slider picked (`option_effects::hud_rgb()`, retail `[0x100b246c]`), read every frame.
pub fn hud_rgb()->[f32;3] {crate::option_effects::hud_rgb()}
fn tint(alpha:f32)->Color {let [r,g,b]=hud_rgb();Color::srgba(r,g,b,HUD_ALPHA*alpha)}

/// The HUD groups that fade: the health and stamina bars, the ammo panel and the three item-effect boxes.
#[derive(Component,Clone,Copy,Debug,PartialEq,Eq)] pub enum Fade {Health=0,Stamina=1,Ammo=2,PowerUp=3,PainKiller=4,Alcohol=5}
#[derive(Component)] struct Group(Fade);
/// An image that draws in the HUD colour with its group's fade.
#[derive(Component)] struct Faded(Fade);
#[derive(Component,Clone,Copy)] enum Fill {Health,Stamina,Ammo}
#[derive(Component)] struct FillBar(Fill);
#[derive(Component)] struct StatusDigits(Fade);
#[derive(Component)] struct AmmoDigits {reserve:bool}
#[derive(Component)] struct AmmoFrame;

/// What the fade timers watch each frame.
#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct Vitals {pub health:f32,pub max_health:f32,pub power_up:f32,pub pain_killer:f32,pub alcohol:f32,pub stamina:f32,pub max_stamina:f32,pub weapon:bool,pub used:u32}
/// The six fade timers in seconds (HUD object +0xaa2b8.. and the statics at 0x10ab56d8 / 0x10ab56dc).
#[derive(Clone,Debug,Default)]
pub struct HudFade {t:[f32;6],last:Option<Vitals>,epoch:u64}
impl HudFade {
    /// One frame. Timers count down 1/s (HUD update 0x1003b46e..); changes and conditions raise them:
    /// health or power-up changed -> 2.0 (0x10061caf, 0x10061daf), stamina changed -> 0.95 (0x10061ed5); the draw code then
    /// pins stamina below max, an active alcohol / painkiller / power-up effect (0x10037ad5, 0x100373ee, 0x1003803e, 0x100368be)
    /// and the ammo panel (a weapon in hand, 0x10038608) to 1.0, and the weapon bob pins health to 1.0 while a weapon is drawn (0x1001030a).
    pub fn step(&mut self,v:Vitals,dt:f32,epoch:u64) {
        for t in &mut self.t {*t=(*t-dt).max(0.0);}
        if epoch!=self.epoch {self.epoch=epoch;self.last=None;}
        if let Some(last)=self.last {
            if v.health!=last.health || v.power_up!=last.power_up || v.used!=last.used {self.t[Fade::Health as usize]=2.0;}
            if v.stamina!=last.stamina {self.t[Fade::Stamina as usize]=0.95;}
        }
        self.last=Some(v);
        if v.weapon {self.t[Fade::Health as usize]=1.0;self.t[Fade::Ammo as usize]=1.0;}
        if v.stamina!=v.max_stamina {self.t[Fade::Stamina as usize]=1.0;}
        if v.alcohol>0.0 {self.t[Fade::Alcohol as usize]=1.0;}
        if v.pain_killer>0.0 {self.t[Fade::PainKiller as usize]=1.0;}
        if v.power_up>0.0 {self.t[Fade::PowerUp as usize]=1.0;}
    }
    /// Alpha factor of a group: the timer capped at 1 (a group with an empty timer is not drawn).
    pub fn alpha(&self,fade:Fade)->f32 {self.t[fade as usize].clamp(0.0,1.0)}
    /// The item-effect boxes print their value only while the effect lasts (timer still 1.0), not during the fade-out.
    pub fn digits(&self,fade:Fade)->bool {self.t[fade as usize]>=1.0}
}
/// 0x10037740: (power-up + health) / max health, capped at 1.
pub fn health_fraction(health:f32,power_up:f32,max_health:f32)->f32 {((power_up+health)/max_health.max(1.0)).min(1.0)}
/// 0x10037aa0: stamina / max stamina, capped at 1.
pub fn stamina_fraction(stamina:f32,max_stamina:f32)->f32 {(stamina/max_stamina.max(1.0)).min(1.0)}
/// Magazine bar: rounds / capacity (melee and grenade weapons show a full bar), 0x10038925.
pub fn ammo_fraction(magazine:u32,capacity:u32)->f32 {(magazine as f32/capacity.max(1) as f32).clamp(0.0,1.0)}
/// Item-effect boxes: truncated, clamped to 0..=99 (0x10036ac8..0x10036ad8), two digits with a zero in front (0x10036af7).
pub fn two_digits(value:f32)->String {format!("{:02}",(value as i32).clamp(0,99))}
/// Magazine counter: padded to two digits but never clamped (0x10038a94).
pub fn magazine_digits(rounds:u32)->String {format!("{rounds:02}")}
/// Reserve counter: padded to three digits, never clamped (0x10038c16..0x10038c8b).
pub fn three_digits(value:u32)->String {format!("{value:03}")}

pub fn setup(mut commands:Commands,assets:Res<AssetServer>) {
    // UI renders after the isolated weapon pass, so menus cover the viewmodel.
    commands.spawn((Camera2d,Camera {order:2,clear_color:ClearColorConfig::None,..default()},
        IsDefaultUiCamera,bevy::camera::visibility::RenderLayers::layer(2)));
    let abs=|x:f32,y:f32,w:f32,h:f32|Node {position_type:PositionType::Absolute,left:px(x),top:px(y),width:px(w),height:px(h),..default()};
    let image=|name:&str|assets.load(format!("hud/{name}.png"));
    commands.spawn((GameHud,Node {width:percent(100),height:percent(100),position_type:PositionType::Absolute,..default()})).with_children(|root| {
        // Health and stamina: 256x32 frame at (16,680) / (16,720); the bar is 216x20 inset by (32,6) and cropped, not stretched (0x1003790c, 0x10037d19).
        for (fade,fill,name,y) in [(Fade::Health,Fill::Health,"health",680.0),(Fade::Stamina,Fill::Stamina,"stamina",720.0)] {
            root.spawn((Group(fade),Node {position_type:PositionType::Absolute,left:px(16),bottom:px(768.0-y-32.0),width:px(256),height:px(32),..default()},Visibility::Hidden)).with_children(|group| {
                group.spawn((Faded(fade),ImageNode {image:image(&format!("ramka_{name}")),color:tint(0.0),..default()},abs(0.0,0.0,256.0,32.0)));
                group.spawn((FillBar(fill),Node {overflow:Overflow::clip(),..abs(32.0,6.0,216.0,20.0)})).with_children(|bar| {
                    bar.spawn((Faded(fade),ImageNode {image:image(&format!("pasek_{name}")),color:tint(0.0),..default()},abs(0.0,0.0,216.0,20.0)));
                });
            });
        }
        // Item effects: 256x64 boxes stacked at y = 448 (power-up), 512 (painkiller), 576 (alcohol) with a two-digit value at (88,+10).
        for (fade,name,y) in [(Fade::PowerUp,"powerup",448.0),(Fade::PainKiller,"painkiller",512.0),(Fade::Alcohol,"alcohol",576.0)] {
            root.spawn((Group(fade),Node {position_type:PositionType::Absolute,left:px(16),bottom:px(768.0-y-64.0),width:px(256),height:px(64),..default()},Visibility::Hidden)).with_children(|group| {
                group.spawn((Faded(fade),ImageNode {image:image(&format!("ramka_{name}")),color:tint(0.0),..default()},abs(0.0,0.0,256.0,64.0)));
                group.spawn((StatusDigits(fade),label("ammo","",1.0,tint(1.0),72.0,10.0)));
            });
        }
        // Ammo panel: the weapon's 256x128 frame at (768,640), magazine bar at (782,729) up to 228x24, counters at (962,655) and (952,692).
        root.spawn((Group(Fade::Ammo),Node {position_type:PositionType::Absolute,right:px(0),bottom:px(0),width:px(256),height:px(128),..default()},Visibility::Hidden)).with_children(|group| {
            group.spawn((AmmoFrame,Faded(Fade::Ammo),ImageNode {color:tint(0.0),..default()},abs(0.0,0.0,256.0,128.0)));
            group.spawn((FillBar(Fill::Ammo),Node {overflow:Overflow::clip(),..abs(14.0,89.0,228.0,24.0)})).with_children(|bar| {
                bar.spawn((Faded(Fade::Ammo),ImageNode {image:image("pasek_ammo"),color:tint(0.0),..default()},abs(0.0,0.0,228.0,24.0)));
            });
            group.spawn((AmmoDigits {reserve:false},label("ammo","",1.0,tint(1.0),194.0,15.0)));
            group.spawn((AmmoDigits {reserve:true},label("ammo","",1.0,tint(1.0),184.0,52.0)));
        });
        // The crosshair texture is drawn as it is (white vertex colour, 32x32 at the screen centre, 0x10010bd0).
        root.spawn((Crosshair,ImageNode {image:image("celownik"),..default()},Node {position_type:PositionType::Absolute,left:percent(50),top:percent(50),margin:UiRect::all(px(-16)),width:px(32),height:px(32),..default()}));
    });
    // Retail has no such banner (the mouse is always captured there); the remake needs one line while the window has not grabbed the mouse yet.
    // It used to be a long key list in a dark strip over the top left corner, where it covered the dialogue text and every HUD message; it is now the
    // single short line at the bottom edge, centred, outlined by a text shadow.
    commands.spawn((Node {position_type:PositionType::Absolute,left:px(0),right:px(0),bottom:px(28),justify_content:JustifyContent::Center,..default()},GlobalZIndex(40))).with_children(|row| {
        row.spawn((Text::new("Kattints a játékhoz · Esc: menü"),TextFont {font:assets.load("hud/subtitles.ttf"),font_size:18.0,..default()},TextColor(Color::WHITE),
            TextShadow {offset:Vec2::splat(1.5),color:Color::srgba(0.0,0.0,0.0,0.95)},
            Node {padding:UiRect::axes(px(14),px(5)),..default()},BackgroundColor(Color::srgba(0.0,0.0,0.0,0.45)),Help));
    });
    // Retail has no death screen: the shell prints GameShell4 (character.rs) and Esc / F9 do the rest, so this overlay stays hidden.
    commands.spawn((DeathOverlay,Visibility::Hidden,GlobalZIndex(30),Node::default()));
}

/// The HUD parts `health` drives (bundled: Bevy systems take at most 16 parameters).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Parts<'w,'s> {
    groups:Query<'w,'s,(&'static Group,&'static mut Visibility),Without<NativeHud>>,images:Query<'w,'s,(&'static Faded,&'static mut ImageNode,Has<AmmoFrame>)>,fills:Query<'w,'s,(&'static FillBar,&'static mut Node)>,
    status:Query<'w,'s,(&'static StatusDigits,&'static mut BitmapText),Without<AmmoDigits>>,counters:Query<'w,'s,(&'static AmmoDigits,&'static mut BitmapText),Without<StatusDigits>>,
    legacy:Query<'w,'s,&'static mut Visibility,(With<NativeHud>,Without<Group>)>,
}
/// Fade timers, group visibility, bar fills and counters. The character-info and attribute screens hide the bars and effect boxes,
/// the inventory and attribute screens hide the ammo panel (0x10037748, 0x100385e6).
#[allow(clippy::too_many_arguments)]
pub fn health(mut fade:Local<HudFade>,mut shown_weapon:Local<Option<usize>>,mut script:Local<Script>,config:Res<crate::ViewerConfig>,time:Res<Time>,session:Res<Session>,opening:Res<crate::opening::Opening>,mut campaign:ResMut<Campaign>,mut walking:ResMut<Walking>,mut native:ResMut<NativeArsenal>,panels:Res<Panels>,travel:Res<crate::travel::Travel>,assets:Res<AssetServer>,mut parts:Parts) {
    if script.enabled(&config) {script.tick(time.delta_secs(),&mut campaign,&mut walking.player.stamina,&mut native,&parts);}
    // The old orange placeholder ammo panel of retail_weapons stays hidden: the retail panel is built here.
    for mut visibility in &mut parts.legacy {*visibility=Visibility::Hidden;}
    let selected=native.inventory.selected.filter(|_|native.inventory.equipped);
    let vitals=Vitals {health:campaign.health,max_health:campaign.max_health,power_up:campaign.stats.power_up,pain_killer:campaign.stats.pain_killer,alcohol:campaign.alcohol,
        stamina:walking.player.stamina,max_stamina:walking.player.max_stamina,weapon:selected.is_some(),used:campaign.health_events};
    if !session.paused && !opening.active {fade.step(vitals,time.delta_secs().min(0.05),travel.arrived);}
    let sheet=panels.char_info || panels.attributes;
    for (group,mut visibility) in &mut parts.groups {
        let hidden=match group.0 {Fade::Ammo=>panels.inventory || panels.attributes,_=>sheet};
        *visibility=if hidden || fade.alpha(group.0)<=0.0 {Visibility::Hidden}else{Visibility::Inherited};
    }
    let frame_image=selected.map(|slot|native.definitions[slot].hud.clone());
    for (faded,mut image,ammo_frame) in &mut parts.images {
        image.color=tint(fade.alpha(faded.0));
        if ammo_frame && *shown_weapon!=selected {if let Some(path)=&frame_image {image.image=assets.load(path.clone());}}
    }
    if selected.is_some() {*shown_weapon=selected;}
    let magazine_bar=selected.map(|slot|{let d=&native.definitions[slot];if d.melee || d.grenade {1.0}else{ammo_fraction(native.inventory.weapons[slot].magazine,d.capacity)}}).unwrap_or(0.0);
    for (bar,mut node) in &mut parts.fills {
        let (fraction,width)=match bar.0 {
            Fill::Health=>(health_fraction(campaign.health,campaign.stats.power_up,campaign.max_health),216.0),
            Fill::Stamina=>(stamina_fraction(walking.player.stamina,walking.player.max_stamina),216.0),
            Fill::Ammo=>(magazine_bar,228.0),
        };
        node.width=px(width*fraction);
    }
    for (digits,mut text) in &mut parts.status {
        let value=match digits.0 {Fade::PowerUp=>campaign.stats.power_up,Fade::PainKiller=>campaign.stats.pain_killer,_=>campaign.alcohol};
        text.set(if fade.digits(digits.0) {two_digits(value)}else{String::new()});
        if text.color!=tint(1.0) {text.color=tint(1.0);}
    }
    for (digits,mut text) in &mut parts.counters {
        let value=selected.map(|slot|{let d=&native.definitions[slot];
            let count=if d.melee {1}else if digits.reserve {native.inventory.reserve(&native.definitions,slot)}else{native.inventory.weapons[slot].magazine};
            if digits.reserve {three_digits(count)}else{magazine_digits(count)}}).unwrap_or_default();
        text.set(value);
        if text.color!=tint(1.0) {text.color=tint(1.0);}
    }
}

/// Headless capture aid: MESTER_TEST_SCENARIO=hud draws a weapon, holsters it, hurts the player, tires him and doses him,
/// and reports the alpha each HUD group is drawn with (capture at 2, 3.5, 4.6, 5.5, 7.5, 10.5 s to see every state).
#[derive(Default)] pub struct Script {on:Option<bool>,t:f32,act:usize,report:usize}
impl Script {
    fn enabled(&mut self,config:&crate::ViewerConfig)->bool {*self.on.get_or_insert_with(||config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("hud"))}
    fn tick(&mut self,dt:f32,campaign:&mut Campaign,stamina:&mut f32,native:&mut NativeArsenal,parts:&Parts) {
        const ACTIONS:[f32;5]=[0.5,3.0,5.0,7.0,10.0];const REPORTS:[f32;6]=[2.0,3.5,4.6,5.5,7.5,10.5];
        self.t+=dt;campaign.health=campaign.health.min(campaign.max_health);
        if self.act<ACTIONS.len() && self.t>=ACTIONS[self.act] {
            match self.act {
                0=>{native.acquire("Glock");if let Some(slot)=native.definitions.iter().position(|d|d.id=="Glock") {native.inventory.select(slot);}},
                1=>native.inventory.holster(),
                2=>campaign.health=65.0,
                3=>*stamina=40.0,
                _=>{campaign.alcohol=30.0;campaign.stats.pain_killer=40.0;campaign.stats.power_up=20.0;},
            }
            self.act+=1;
        }
        if self.report<REPORTS.len() && self.t>=REPORTS[self.report] {
            let alpha=|fade:Fade|parts.images.iter().find(|(f,..)|f.0==fade).map(|(_,image,_)|image.color.alpha()/HUD_ALPHA).unwrap_or(-1.0);
            info!("HUDPRÓBA t={:.1} alpha health={:.2} stamina={:.2} ammo={:.2} powerup={:.2} painkiller={:.2} alcohol={:.2}",self.t,alpha(Fade::Health),alpha(Fade::Stamina),alpha(Fade::Ammo),alpha(Fade::PowerUp),alpha(Fade::PainKiller),alpha(Fade::Alcohol));
            self.report+=1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(campaign:Res<Campaign>,native:Res<NativeArsenal>,panels:Res<Panels>,mut help:Single<&mut Visibility,(With<Help>,Without<GameHud>,Without<Crosshair>)>,cursor:Single<&bevy::window::CursorOptions>,window:Single<&Window>,mut scale:ResMut<UiScale>,opening:Res<crate::opening::Opening>,mut hud:Single<&mut Visibility,(With<GameHud>,Without<Help>,Without<Crosshair>)>,session:Res<Session>,mut crosshair:Single<&mut Visibility,(With<Crosshair>,Without<Help>,Without<GameHud>)>,mut death:Single<&mut Visibility,(With<DeathOverlay>,Without<GameHud>,Without<Help>,Without<Crosshair>)>) {
    **help=if cursor.visible && !opening.active && !session.paused && campaign.dialogue.is_none() && !panels.mouse_mode() {Visibility::Visible}else{Visibility::Hidden};
    // Retail draws the crosshair only with a weapon in hand and outside menus and cutscenes (0x10010bd0); the scope overlay hides it further (weapons_alt).
    let armed=native.inventory.selected.is_some() && native.inventory.equipped;
    **crosshair=if opening.active || session.paused || !armed {Visibility::Hidden}else{Visibility::Inherited};
    **hud=if opening.active {Visibility::Hidden}else{Visibility::Visible};
    **death=Visibility::Hidden;
    // Written only when it changes: a plain assignment marks `UiScale` changed every frame and Bevy relayouts the whole UI tree for it.
    let wanted=(window.height()/768.0).min(window.width()/1024.0);if scale.0!=wanted {scale.0=wanted;}
}

#[cfg(test)] mod tests {
    use super::*;
    fn calm() -> Vitals {Vitals {health:100.0,max_health:100.0,stamina:100.0,max_stamina:100.0,..default()}}
    fn run(fade:&mut HudFade,v:Vitals,seconds:f32) {for _ in 0..(seconds*60.0) as usize {fade.step(v,1.0/60.0,0);}}
    #[test] fn a_full_stamina_bar_fades_out_in_under_a_second_and_a_hurt_health_bar_holds_for_one_second_then_fades() {
        let mut fade=HudFade::default();run(&mut fade,calm(),1.0);
        assert_eq!((fade.alpha(Fade::Health),fade.alpha(Fade::Stamina)),(0.0,0.0),"at full values and no weapon nothing is drawn");
        // Running: stamina below max keeps the bar at full alpha.
        let tired=Vitals {stamina:60.0,..calm()};fade.step(tired,1.0/60.0,0);
        assert_eq!(fade.alpha(Fade::Stamina),1.0);
        // Back at max: the change of the last frame arms 0.95 s and the bar fades linearly.
        let mut v=tired;v.stamina=100.0;fade.step(v,1.0/60.0,0);
        assert!((fade.alpha(Fade::Stamina)-0.95).abs()<1e-6);
        run(&mut fade,calm(),0.5);assert!((fade.alpha(Fade::Stamina)-0.45).abs()<0.03);
        run(&mut fade,calm(),0.5);assert_eq!(fade.alpha(Fade::Stamina),0.0);
        // Damage: a 2.0 s timer = one second at full alpha, then one second of fade-out.
        fade.step(Vitals {health:80.0,..calm()},1.0/60.0,0);
        assert_eq!(fade.alpha(Fade::Health),1.0);
        run(&mut fade,Vitals {health:80.0,..calm()},0.9);assert_eq!(fade.alpha(Fade::Health),1.0);
        run(&mut fade,Vitals {health:80.0,..calm()},0.6);assert!((fade.alpha(Fade::Health)-0.5).abs()<0.05);
        run(&mut fade,Vitals {health:80.0,..calm()},0.6);assert_eq!(fade.alpha(Fade::Health),0.0);
    }
    #[test] fn a_drawn_weapon_keeps_the_health_bar_and_the_ammo_panel_and_holstering_fades_them() {
        let mut fade=HudFade::default();let armed=Vitals {weapon:true,..calm()};
        run(&mut fade,armed,2.0);assert_eq!((fade.alpha(Fade::Health),fade.alpha(Fade::Ammo)),(1.0,1.0));
        run(&mut fade,calm(),0.5);assert!((fade.alpha(Fade::Ammo)-0.5).abs()<0.05,"the ammo panel fades over a second");
        run(&mut fade,calm(),1.0);assert_eq!((fade.alpha(Fade::Health),fade.alpha(Fade::Ammo)),(0.0,0.0));
    }
    #[test] fn item_effect_boxes_show_their_digits_only_while_the_effect_lasts() {
        let mut fade=HudFade::default();
        let drunk=Vitals {alcohol:12.4,..calm()};fade.step(drunk,1.0/60.0,0);
        assert_eq!((fade.alpha(Fade::Alcohol),fade.digits(Fade::Alcohol)),(1.0,true));
        fade.step(calm(),1.0/60.0,0);assert!(!fade.digits(Fade::Alcohol) && fade.alpha(Fade::Alcohol)>0.9);
        run(&mut fade,calm(),1.0);assert_eq!(fade.alpha(Fade::Alcohol),0.0);
        // A power-up counts down every frame, which is a health change each frame: the health bar stays up.
        let mut v=Vitals {power_up:5.0,..calm()};fade.step(v,1.0/60.0,0);v.power_up=4.9;fade.step(v,1.0/60.0,0);
        assert_eq!((fade.alpha(Fade::PowerUp),fade.alpha(Fade::Health)),(1.0,1.0));
    }
    #[test] fn a_new_level_does_not_flash_the_bars_and_consumed_items_do() {
        let mut fade=HudFade::default();fade.step(calm(),1.0/60.0,0);
        fade.step(Vitals {health:40.0,stamina:50.0,..calm()},1.0/60.0,1);
        assert_eq!((fade.alpha(Fade::Health),fade.alpha(Fade::Stamina)),(0.0,1.0),"health jumped with the level start (no flash); stamina is below max");
        fade.step(Vitals {used:1,health:40.0,stamina:50.0,..calm()},1.0/60.0,1);
        assert_eq!(fade.alpha(Fade::Health),1.0,"eating at any health shows the bar (a health add of any size, 0x10061caf)");
    }
    #[test] fn fractions_and_counters_follow_the_retail_caps() {
        assert_eq!(health_fraction(15.0,0.0,100.0),0.15);assert_eq!(health_fraction(90.0,30.0,100.0),1.0,"power-up adds to the bar, capped");
        assert_eq!(stamina_fraction(55.0,110.0),0.5);assert_eq!(stamina_fraction(120.0,110.0),1.0);
        assert_eq!(ammo_fraction(6,12),0.5);assert_eq!(ammo_fraction(3,0),1.0);
        assert_eq!((two_digits(7.9).as_str(),two_digits(-3.0).as_str(),two_digits(250.0).as_str(),two_digits(42.0).as_str()),("07","00","99","42"));
        assert_eq!((three_digits(5).as_str(),three_digits(70).as_str(),three_digits(1234).as_str()),("005","070","1234"),"the reserve is never clamped");
        assert_eq!((magazine_digits(7).as_str(),magazine_digits(120).as_str()),("07","120"));
    }
}

/// The hidden console variable `DrawKilledCount` (autoexec.cfg, cshell 0x1003ae60): the number of characters the player has killed in digits at the top left.
#[derive(Component)] pub struct KilledCount;
pub fn killed_count(mut commands:Commands,campaign:Res<Campaign>,options:Res<crate::options::Options>,assets:Res<AssetServer>,mut shown:Query<(Entity,&mut Text),With<KilledCount>>) {
    match (options.draw_killed_count,shown.single_mut()) {
        (true,Ok((_,mut text)))=>{let now=campaign.kills.to_string();if text.0!=now {text.0=now;}},
        (true,Err(_))=>{commands.spawn((KilledCount,Text::new(campaign.kills.to_string()),TextFont {font:assets.load("hud/subtitles.ttf"),font_size:24.0,..default()},TextColor(Color::WHITE),
            Node {position_type:PositionType::Absolute,top:px(4),left:px(4),..default()},GlobalZIndex(24)));},
        (false,Ok((entity,_)))=>{commands.entity(entity).despawn();},
        (false,Err(_))=>{},
    }
}
