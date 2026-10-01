//! World transitions use the original door destinations and keep the selected mod.
use bevy::prelude::*;
use crate::{ViewerConfig,opening,settings::Session};

pub const LEVELS:[(&str,&str);28]=[
    ("rh3-miasteczko0","Előszó"),
    ("rh1-wiezienie1","Bevezetés"),
    ("rh1-wiezienie2","A Lázadó"),
    ("rh1-wiezienie3","Keress fedezéket!"),
    ("rh2-wiezienie1","Találj kiutat!"),
    ("rh2-wiezienie2","A Menekülés"),
    ("rh3-miasteczko1","Az Álmos Üreg"),
    ("rh3-miasteczko2","Az Utcán"),
    ("burmistrz1","A Polgármesternél"),
    ("burmistrz2","A Kulisszák mögött"),
    ("chapel_mniejszy","Az öreg kolostor"),
    ("knajpa","A Bullseye Kocsma"),
    ("rh7a-tunele","A Titkos Átjáró"),
    ("podziemia1","Alagutak"),
    ("podziemia1a","Irány a sötétség"),
    ("podziemia1b","Közel a szabadság"),
    ("podziemia1c","Égő város"),
    ("chinatown2","A Templom"),
    ("rh9-fabryka","A Gyár"),
    ("rh10-wiezowiec1","Az utolsó ítélet"),
    ("rh10-wiezowiec2","A Felhőkarcoló"),
    ("rh10-wiezowiec3","A Küzdelem"),
    ("wiez_wn1","Harcmezők"),
    ("wiez_wn2","Vadászat a vadászra"),
    ("wiez_wn3","wiez_wn3"),
    ("rh12-lab1","Kínzókamrák"),
    ("rh12-lab2","Leszámolás"),
    ("chinatown","Kiskína · kimaradt pálya"),];
/// Hungarian level title of a world id (the loading screen and save list name).
pub fn level_title(id:&str)->&'static str {LEVELS.iter().find(|(level,_)|level.eq_ignore_ascii_case(id)).map_or("",|(_,title)|*title)}
/// Level ids are lowercase, but two retail exports keep their DAT file case
/// (Rh7a-Tunele, RH9-fabryka); resolve the exported file stem case-insensitively.
pub fn exported_stem(output:&std::path::Path,id:&str)->Option<String> {
    let wanted=format!("{}.scene.json",id.to_ascii_lowercase());
    std::fs::read_dir(output).ok()?.flatten().filter_map(|entry|entry.file_name().into_string().ok())
        .find(|name|name.to_ascii_lowercase()==wanted).map(|name|name[..name.len()-".scene.json".len()].to_owned())
}
#[derive(Resource,Default)] pub struct Travel {pub pending:Option<String>,pub staged:Option<String>,pub arrived:u64,/// Owner deviation from retail's shutdown after the credits: reload the main-menu backdrop world (`request` below).
    pub menu:bool}

pub fn request(mut commands:Commands,mut travel:ResMut<Travel>,mut config:ResMut<ViewerConfig>,mut session:ResMut<Session>,mut intro:ResMut<opening::Opening>,mut front:ResMut<crate::frontend::Frontend>,assets:Res<AssetServer>,mut actors:Query<&mut Visibility,With<opening::Actor>>,audio:Query<Entity,With<AudioPlayer>>,mut subtitle:Query<&mut crate::retail_ui::BitmapText,With<opening::SubtitleLine>>,mut cursor:Single<&mut bevy::window::CursorOptions>,mut campaign:ResMut<crate::campaign::Campaign>) {
    // Back to the main menu (the end of the game): unload the level, restart the menu backdrop world exactly like the launch does (paused, menu music, a fresh campaign on the next new game).
    if std::mem::take(&mut travel.menu) {
        travel.pending=None;travel.staged=None;campaign.reset_requested=true;campaign.clear_restore();
        let Some(world)=exported_stem(&config.output,"rh1-wiezienie1") else {return};
        for entity in &audio {commands.entity(entity).despawn();}
        for mut visibility in &mut actors {*visibility=Visibility::Hidden;}
        for mut line in &mut subtitle {line.set("");}
        front.bonus=None;front.loading=None;front.load_started=false;front.main_active=true;front.in_game=false;front.page=crate::menu_layout::MAIN;front.character_started=false;front.draft=Default::default();
        session.paused=true;session.dialogue_active=false;cursor.visible=true;cursor.grab_mode=bevy::window::CursorGrabMode::None;
        config.world=world;config.story=false;intro.restart(false);travel.arrived+=1;
        commands.run_system_cached(crate::setup);
        info!("Vissza a főmenübe (a játék vége)");return;
    }
    if let Some(requested)=travel.pending.take() {
    // A bonus run never continues into the campaign: any exit other than the bonus level itself (a reload of it after a death) returns to the main menu like the ending does.
    if front.bonus.as_deref().is_some_and(|bonus|!bonus.eq_ignore_ascii_case(&requested)) {info!("Bónusz pálya vége: {requested} helyett főmenü");travel.menu=true;return;}
    let id=requested.to_ascii_lowercase();
    let stem=(LEVELS.iter().any(|(level,_)|*level==id) && !id.contains(['/', '\\', '.'])).then(||exported_stem(&config.output,&id)).flatten()
        .filter(|stem|["visual.obj","collision.obj","scene.json"].iter().all(|ext|config.output.join(format!("{stem}.{ext}")).is_file()));
    let Some(world)=stem else {
        // A refused load must not leave a new-game reset or save restore armed for the next door.
        campaign.reset_requested=false;campaign.clear_restore();front.bonus=None;
        session.notice="Ez a pálya még nincs exportálva.".into();session.paused=true;cursor.visible=true;cursor.grab_mode=bevy::window::CursorGrabMode::None;return;
    };
    travel.staged=Some(world);front.begin_load(&id);session.paused=true;cursor.visible=false;cursor.grab_mode=bevy::window::CursorGrabMode::None;return;
    }
    let Some(world)=travel.staged.clone() else{return};
    if !front.screen_ready(&assets) {return;}
    travel.staged=None;front.load_started=true;
    for entity in &audio {commands.entity(entity).despawn();}
    for mut visibility in &mut actors {*visibility=Visibility::Hidden;}
    for mut line in &mut subtitle {line.set("");}
    config.world=world.clone();config.story=world.eq_ignore_ascii_case("rh1-wiezienie1");
    intro.restart(config.story);
    travel.arrived+=1;
    commands.run_system_cached(crate::setup);
    info!("Küldetésbetöltés: {world}");
}
#[cfg(test)] mod tests {
    #[test] fn lowercase_level_id_resolves_the_retail_file_case() {
        let dir=std::env::temp_dir().join(format!("mester-travel-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("RH9-fabryka.scene.json"),"{}").unwrap();std::fs::write(dir.join("knajpa.scene.json"),"{}").unwrap();
        assert_eq!(super::exported_stem(&dir,"rh9-fabryka").as_deref(),Some("RH9-fabryka"));
        assert_eq!(super::exported_stem(&dir,"KNAJPA").as_deref(),Some("knajpa"));
        assert_eq!(super::exported_stem(&dir,"chinatown"),None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
