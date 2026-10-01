use mission_runtime::{Context, Event, Mission, NpcState};

// Verbatim command structure from the retail merged gameai/dialogi sources.
const AI: &str = r"
bool IntroOdpalone
level worlds\rh1-wiezienie2
bool BuntWybuchl
bool Wiezienie1Walka
action CzyGeneralAttack
ifnot Wiezienie1Walka
ifweapondrawn
set Wiezienie1Walka
endifs
action GeneralAttack
if Wiezienie1Walka
hostileattack
endifs
action Bunt1
ifnot WiezienieWlazlWDrzwi
ifnot BuntWybuchl
ifgraczblizejniz 360 wiezien bije
setfaza napad wiezien bije
set BuntWybuchl
endifs
action straznik1
if IntroOdpalone
ifnot StraznikPrzyCeliOstrzegal
ifalive straznik przy celi
ifplayerseenby straznik przy celi
iflicznikwiekszyniz 3 CzasOdGadki
dialog StraznikPrzyCeliOstrzega
endifs
level worlds\rh1-wiezienie3
action GeneralAttack
hostileattack
endifs
action WiezienKuchnia1
ifnot WiezienKuchniaGadal
ifalive wiezien_kuchnia
ifgraczblizejniz 256 wiezien_kuchnia
ifplayerseenby wiezien_kuchnia
dialog Wiezien27
set WiezienKuchniaGadal
setfaza gada wiezien_kuchnia
endifs
";
const DIALOGUES: &str = r"
dialog MasonStart1
delay 2
title >Fight02T
titlesnd >Fight02S
set IntroOdpalone
unset CzasOdGadki
ontimeexceeded 2
dialog StraznikPrzyCeliOstrzega
person straznik przy celi
delay 5
title >Mutiny02T
set StraznikPrzyCeliOstrzegal
ontimeexceeded 5
dialog Wiezien27
person wiezien_kuchnia
title >Mutiny20T
ontimeexceeded 5
dialog WiezienieWlazlWDrzwi
set WiezienieWlazlWDrzwi
";
const KEYS: &str = ">Fight02T Ki kell jutnom.\n>Fight02S dialogs\\Fight02.wav\n>Mutiny02T Vissza a cellába!\n>Mutiny20T Keress fedezéket!";
fn mission() -> Mission { Mission::from_sources("rh1-wiezienie2", AI, DIALOGUES, KEYS).unwrap() }
fn npc(name: &str, distance: f32) -> NpcState { NpcState {name:name.into(), position:[distance,0.0,0.0],alive:true,player_seen:true,contact:true,..Default::default()} }

#[test]
fn riot_requires_authored_distance_and_fires_once() {
    let mut m=mission();
    let mut c=Context {npcs:vec![npc("wiezien bije",361.0)],..Default::default()};
    assert!(m.tick(0.1,&c).is_empty());
    c.npcs[0].position[0]=359.0;
    assert_eq!(m.tick(0.1,&c),vec![Event::SetNpcPhase{name:"wiezien bije".into(),phase:"napad".into()}]);
    assert!(m.flag("BuntWybuchl"));
    assert!(m.tick(0.1,&c).is_empty());
}

#[test]
fn weapon_drawing_activates_police_but_unarmed_start_is_peaceful() {
    let mut m=mission();
    assert!(m.tick(0.1,&Context::default()).is_empty());
    let c=Context {weapon_drawn:true,..Default::default()};
    // Actions run newest first (cshell 0x1001a898): `GeneralAttack` (written after `CzyGeneralAttack`) still sees the flag clear in the first frame.
    let first=m.tick(0.1,&c);
    assert!(!first.contains(&Event::HostileAttack) && m.flag("Wiezienie1Walka"));
    assert!(m.tick(0.1,&c).contains(&Event::HostileAttack));
}

#[test]
fn opening_marker_resolves_localized_voice_and_enables_guard_after_timer() {
    let mut m=mission();
    let events=m.trigger_dialog("MasonStart1");
    assert!(matches!(&events[0],Event::Dialogue(d) if d.title=="Ki kell jutnom." && d.speech.as_deref()==Some("dialogs/Fight02.wav")));
    assert!(m.flag("IntroOdpalone"));
    let c=Context {npcs:vec![npc("straznik przy celi",260.0)],..Default::default()};
    m.tick(2.0,&c);
    assert!(!m.flag("StraznikPrzyCeliOstrzegal"));
    let events=m.tick(1.1,&c);
    assert!(events.iter().any(|e|matches!(e,Event::Dialogue(d) if d.id=="StraznikPrzyCeliOstrzega")));
    assert!(m.flag("StraznikPrzyCeliOstrzegal"));
}

#[test]
fn map_transition_loads_new_rules_and_keeps_flags() {
    let mut m=mission();m.set_flag("BuntWybuchl",true);
    m.enter_world("worlds\\rh1-wiezienie3").unwrap();
    let c=Context {npcs:vec![npc("wiezien_kuchnia",200.0)],..Default::default()};
    let events=m.tick(0.1,&c);
    assert!(events.contains(&Event::HostileAttack));
    assert!(events.iter().any(|e|matches!(e,Event::Dialogue(d) if d.id=="Wiezien27")));
    assert!(m.flag("BuntWybuchl") && m.flag("WiezienKuchniaGadal"));
    assert!(!m.tick(0.1,&c).iter().any(|e|matches!(e,Event::Dialogue(_))));
}

#[test]
fn zero_time_pause_does_not_advance_rules_or_dialogues() {
    let mut m=mission();let c=Context {weapon_drawn:true,..Default::default()};
    for dt in [0.0,-1.0,f32::NAN] {assert!(m.tick(dt,&c).is_empty());}
    assert!(!m.flag("Wiezienie1Walka"));
}

#[test]
fn unsupported_action_and_missing_dialogue_are_explicit() {
    assert!(Mission::from_sources("test","level worlds\\test\naction bad\nteleport mystery","","").is_err());
    assert!(mission().trigger_dialog("missing").iter().any(|e|matches!(e,Event::Warning(_))));
}

#[test]
fn phase_callbacks_preserve_script_flags_and_animated_followup() {
    let mut m=mission();
    m.phase_enter("straznik przy celi",&[
        ("set".into(),"StraznikPrzyCeliBije".into()),
        ("unset".into(),"StraznikPrzyCeliIdzie".into()),
        ("on_koniec_anim0".into(),"atakuje1".into()),
        ("odepchnij_gracza".into(),"".into()),
    ]);
    assert!(m.flag("StraznikPrzyCeliBije"));
    assert_eq!(m.phase_event("straznik przy celi","on_koniec_anim",0),vec![Event::SetNpcPhase{name:"straznik przy celi".into(),phase:"atakuje1".into()}]);
}

#[test]
fn checkpoint_preserves_once_markers_and_dialogue_without_replaying_side_effects() {
    let mut m=mission();assert!(!m.trigger_once("marker1","MasonStart1").is_empty());
    assert!(m.trigger_once("marker1","MasonStart1").is_empty());
    m.tick(0.5,&Context::default());
    let json=serde_json::to_string(&m.snapshot()).unwrap();
    let mut loaded=mission();loaded.restore(serde_json::from_str(&json).unwrap()).unwrap();
    assert!(loaded.flag("IntroOdpalone"));
    assert!(loaded.trigger_once("marker1","MasonStart1").is_empty());
    assert_eq!(loaded.dialogue().unwrap().id,"MasonStart1");
    assert!(loaded.tick(1.6,&Context::default()).contains(&Event::EndDialogue));
}

#[test]
fn dialogue_choice_waits_for_answer_voice_before_next_node_and_grants_once() {
    let dialogues="dialog ask\ntitle Question\ndelay 1\nanswer1 Answer\nanswer1snd reply.wav\nanswer1delay 2\nonchoice1dialog reward\nontimeexceeded 10\ndialog reward\ntitle Thanks\nreceive Glock\nexpgained 50\nontimeexceeded 2";
    let mut m=Mission::from_sources("rh1-wiezienie2",AI,dialogues,"").unwrap();
    m.trigger_dialog("ask");assert!(m.choose(1).is_empty());
    m.tick(1.01,&Context::default());
    let events=m.choose(1);
    assert!(events.iter().any(|e|matches!(e,Event::Dialogue(d) if d.title=="Question" && d.answered.as_deref()==Some("Answer") && d.speech.as_deref()==Some("reply.wav"))));
    assert!(!events.contains(&Event::GrantItem("Glock".into())));
    assert!(!m.tick(1.0,&Context::default()).contains(&Event::GrantItem("Glock".into())));
    assert!(m.tick(1.1,&Context::default()).contains(&Event::GrantItem("Glock".into())));
    assert!(!m.tick(0.1,&Context::default()).contains(&Event::GrantItem("Glock".into())));
}

#[test]
#[ignore="requires local gameplay_scripts.json export"]
fn complete_original_prison_scripts_parse_and_first_riot_runs() {
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/gameplay_scripts.json");
    let source:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for world in ["rh1-wiezienie2","rh1-wiezienie3","rh2-wiezienie1","rh2-wiezienie2"] {
        let mut m=Mission::from_sources(world,source["gameai"].as_str().unwrap(),source["dialogues"].as_str().unwrap(),source["text_keys"].as_str().unwrap()).unwrap();
        let c=Context {npcs:vec![npc("wiezien bije",200.0)],..Default::default()};
        // The cascades of the reverse action order need a few frames.
        let mut events=Vec::new();for _ in 0..4 {events.extend(m.tick(0.1,&c));}
        assert!(!events.iter().any(|e|matches!(e,Event::Warning(_))),"{world}: {events:?}");
        if world=="rh1-wiezienie2" {assert!(m.flag("BuntWybuchl") && m.flag("PolicjantZabil"));}
        else {assert!(events.contains(&Event::HostileAttack));}
    }
}
