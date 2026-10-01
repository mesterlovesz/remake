//! Silent regression: MESTER_TEST_SCENARIO=input presses real keys and mouse buttons into the input resources and checks what the game
//! systems do with them: Space jumps and the right mouse button does not, the right mouse button scopes the M-14 and Space does not,
//! the X screen frees the OS cursor while walking and jumping stay live, the inventory refuses the jump, Escape closes the inventory
//! before it may open the menu, and the mouse is taken back afterwards (docs/retail-input.md). Exit code 1 on any failure.
use bevy::{prelude::*,window::{CursorGrabMode,CursorOptions}};
use crate::{ViewerConfig,Walking,settings::Session,campaign::Campaign,panels::Panels,view::ViewState,retail_weapons::NativeArsenal,probe_kit::{Cursor,Step}};

#[derive(Default)] pub struct Script {on:Option<bool>,t:f32,cursor:Cursor,failed:bool,done:bool}

/// Each stage acts once (press) and then waits for what the game must do about it, so the script does not depend on frame counts.
#[allow(clippy::too_many_arguments)]
pub fn tick(mut script:Local<Script>,config:Res<ViewerConfig>,time:Res<Time>,mut keys:ResMut<ButtonInput<KeyCode>>,mut mouse:ResMut<ButtonInput<MouseButton>>,mut cursor:Single<&mut CursorOptions>,session:Res<Session>,
    mut campaign:ResMut<Campaign>,walking:Res<Walking>,panels:Res<Panels>,view:Res<ViewState>,mut native:ResMut<NativeArsenal>,mut exit:MessageWriter<AppExit>) {
    if !*script.on.get_or_insert_with(||config.capture.is_some() && std::env::var("MESTER_TEST_SCENARIO").as_deref()==Ok("input")) {return;}
    if script.failed || script.done {return;}
    // A focused input check, not a survival run.
    campaign.health=campaign.max_health;script.t+=time.delta_secs();
    let now=script.t;
    let player=&walking.player;let airborne=!player.grounded || player.velocity.y>1.0;
    let locked=cursor.grab_mode==CursorGrabMode::Locked;
    // A key press held for a quarter second must have had its effect by then (jumps last ~0.4 s; nothing here is timing-critical).
    let held=script.cursor.age(now)>=0.25;
    let stage=script.cursor.stage;
    let step=match stage {
        // Take the mouse like the click-to-play does; the level starts with the player on the ground.
        0=>{cursor.visible=false;cursor.grab_mode=CursorGrabMode::Locked;Step::when(player.grounded,"the player to stand on the ground")},
        1=>{keys.press(KeyCode::Space);Step::Done},
        2=>{if airborne {keys.release(KeyCode::Space);Step::Done}else{Step::Wait("Space to jump")}},
        // Landed: the right mouse button must not jump any more.
        3=>Step::when(player.grounded,"the landing"),
        4=>{mouse.press(MouseButton::Right);Step::Done},
        5=>{if !held {Step::Wait("the right button to be held")}else{mouse.release(MouseButton::Right);
            Step::check(!airborne && !view.scoped,||format!("right mouse jumped or scoped without a scope weapon (airborne={airborne} scoped={})",view.scoped))}},
        // Scope: the right button toggles the M-14 zoom (0x100037a0), Space and E do not. The rifle is drawn first (it takes a moment).
        6=>{native.acquire_and_draw("M-14");Step::when(native.inventory.equipped && !native.inventory.switching(),"the M-14 to be drawn")},
        7=>{keys.press(KeyCode::Space);Step::Done},
        8=>{if !held {Step::Wait("Space to be held")}else{keys.release(KeyCode::Space);Step::check(!view.scoped,||"Space scoped the M-14".to_owned())}},
        9=>Step::when(player.grounded,"the landing"),
        10=>{mouse.press(MouseButton::Right);Step::Done},
        11=>{if view.scoped {mouse.release(MouseButton::Right);Step::Done}else{Step::Wait("the right mouse button to scope the M-14")}},
        12=>{mouse.press(MouseButton::Right);Step::Done},
        13=>{if !view.scoped {mouse.release(MouseButton::Right);Step::Done}else{Step::Wait("a second right click to leave the scope")}},
        // X: the attribute screen frees the OS cursor, walking and jumping stay live.
        14=>{keys.press(KeyCode::KeyX);Step::Done},
        15=>{keys.release(KeyCode::KeyX);
            if !panels.attributes {Step::Wait("the X screen")}
            else if !panels.mouse_mode() || cursor.grab_mode!=CursorGrabMode::None || !cursor.visible {Step::Fail(format!("X screen: attributes={} free={} grab={:?} visible={}",panels.attributes,panels.mouse_mode(),cursor.grab_mode,cursor.visible))}
            else {keys.press(KeyCode::Space);Step::Done}},
        16=>{if airborne {keys.release(KeyCode::Space);Step::Done}else{Step::Wait("the jump under the X screen")}},
        17=>Step::when(player.grounded,"the landing"),
        18=>{keys.press(KeyCode::KeyX);Step::Done},
        19=>{keys.release(KeyCode::KeyX);
            if panels.attributes {Step::Wait("the X screen to close")}
            else {Step::check(!panels.mouse_mode() && locked && !cursor.visible,||format!("X closed but the mouse was not taken back: free={} grab={:?} visible={}",panels.mouse_mode(),cursor.grab_mode,cursor.visible))}},
        // C: the inventory refuses the jump (0x10060ba8); Escape closes it (not the game) and the mouse comes back.
        20=>{keys.press(KeyCode::KeyC);Step::Done},
        21=>{keys.release(KeyCode::KeyC);
            if !panels.inventory {Step::Wait("the inventory")}
            else if cursor.grab_mode!=CursorGrabMode::None {Step::Fail("inventory did not free the mouse".to_owned())}
            else {keys.press(KeyCode::Space);Step::Done}},
        22=>{if !held {Step::Wait("Space to be held")}else{keys.release(KeyCode::Space);Step::check(!airborne,||"the inventory did not refuse the jump".to_owned())}},
        23=>{keys.press(KeyCode::Escape);Step::Done},
        24=>{keys.release(KeyCode::Escape);
            if panels.inventory {Step::Wait("Escape to close the inventory")}
            else {Step::check(!session.paused && locked && !cursor.visible,||format!("Escape must close the inventory only: inventory={} paused={} grab={:?}",panels.inventory,session.paused,cursor.grab_mode))}},
        _=>{info!("BEMENETPRÓBA kész: hiba=false");script.done=true;return;},
    };
    let mut failure=None;
    let cursor_ref=&mut script.cursor;
    let stopped=cursor_ref.apply(step,now,5.0,&mut failure);
    if stopped {error!("BEMENETPRÓBA {stage}: {failure:?}");script.failed=true;exit.write(AppExit::error());}
    else if script.cursor.stage!=stage {info!("BEMENETPRÓBA {stage}: rendben (grounded={} vy={:.1} scoped={} inventory={} attributes={} free={} grab={:?})",player.grounded,player.velocity.y,view.scoped,panels.inventory,panels.attributes,panels.mouse_mode(),cursor.grab_mode);}
}

#[cfg(test)] mod tests {
    use bevy::prelude::*;
    use crate::keys_cfg::{cmd,keys_of_action,KeysCfg,SLOTS};
    fn keys_of(config:&KeysCfg,command:usize)->Vec<KeyCode> {config.bound(command).into_iter().flat_map(keys_of_action).collect()}
    #[test] fn the_default_table_is_the_retail_one_with_only_the_owners_three_changes() {
        let (retail,owner)=(KeysCfg::retail(),KeysCfg::default());
        let changed:Vec<usize>=(0..SLOTS).filter(|slot|retail.keys[*slot]!=owner.keys[*slot]).collect();
        assert_eq!(changed,vec![cmd::JUMP,cmd::ALT_FIRE,cmd::ACTION]);
        assert_eq!((owner.bound(cmd::JUMP),owner.bound(cmd::ALT_FIRE),owner.bound(cmd::ACTION)),([30,0],[85,86],[5,0]));
        assert_eq!((retail.bound(cmd::JUMP),retail.bound(cmd::ALT_FIRE),retail.bound(cmd::ACTION)),([85,0],[86,29],[30,5]),"retail: right mouse jumps, Space and E use");
    }
    #[test] fn jump_is_space_the_scope_is_the_right_mouse_button_and_use_is_only_e() {
        let config=KeysCfg::default();
        let held=|command:usize,setup:&dyn Fn(&mut ButtonInput<KeyCode>,&mut ButtonInput<MouseButton>)|{
            let (mut keys,mut mouse)=(ButtonInput::<KeyCode>::default(),ButtonInput::<MouseButton>::default());setup(&mut keys,&mut mouse);
            config.pressed(command,&keys,&mouse)};
        assert!(held(cmd::JUMP,&|k,_|k.press(KeyCode::Space)) && !held(cmd::JUMP,&|_,m|m.press(MouseButton::Right)),"the right button no longer jumps");
        assert!(held(cmd::ALT_FIRE,&|_,m|m.press(MouseButton::Right)) && held(cmd::ALT_FIRE,&|_,m|m.press(MouseButton::Middle)) && !held(cmd::ALT_FIRE,&|k,_|k.press(KeyCode::Space)));
        assert!(held(cmd::ACTION,&|k,_|k.press(KeyCode::KeyE)) && !held(cmd::ACTION,&|k,_|k.press(KeyCode::Space)),"Space no longer uses");
    }
    /// Retail polls every movement command level-triggered each frame (IsPressed 0x10011960, jump poll 0x10060b8b): a key held over N frames is "pressed" on
    /// every one of them, and only its first frame is "just pressed" (the edge commands: run toggle 0x10060601, panels, alternate fire). Whether a held jump
    /// repeats is the controller's latch (+0xdc, retail-movement `a_held_jump_key_repeats_on_landing_but_never_in_the_air`), not the input path.
    #[test] fn a_held_key_is_level_triggered_for_pressed_and_edge_triggered_for_just_pressed() {
        let config=KeysCfg::default();
        let (mut keys,mouse)=(ButtonInput::<KeyCode>::default(),ButtonInput::<MouseButton>::default());
        let (mut pressed,mut edges)=(0,0);
        keys.press(KeyCode::Space);
        for frame in 0..30 {
            if config.pressed(cmd::JUMP,&keys,&mouse) {pressed+=1;}
            if config.just_pressed(cmd::JUMP,&keys,&mouse) {edges+=1;}
            // A held key does not re-press: the OS key repeat arrives as `press` on an already pressed key and must not count as a new edge.
            keys.clear();if frame%5==0 {keys.press(KeyCode::Space);}
        }
        assert_eq!(pressed,30,"held Space is pressed on every frame");
        assert_eq!(edges,1,"but it is one press edge");
        keys.release(KeyCode::Space);assert!(!config.pressed(cmd::JUMP,&keys,&mouse));
    }
    #[test] fn movement_run_crouch_panel_and_weapon_keys_keep_their_retail_bindings() {
        let config=KeysCfg::default();
        assert_eq!(keys_of(&config,cmd::FORWARD),vec![KeyCode::KeyW,KeyCode::ArrowUp]);assert_eq!(keys_of(&config,cmd::STEP_RIGHT),vec![KeyCode::KeyD,KeyCode::ArrowRight]);
        assert_eq!(keys_of(&config,cmd::RUN),vec![KeyCode::ShiftLeft,KeyCode::ShiftRight]);assert_eq!(keys_of(&config,cmd::TOGGLE_RUN),vec![KeyCode::KeyQ]);
        assert_eq!(keys_of(&config,cmd::CROUCH),vec![KeyCode::ControlLeft,KeyCode::ControlRight],"both control keys are action 67 in the autoexec");
        assert_eq!(keys_of(&config,cmd::RELOAD),vec![KeyCode::KeyR]);assert_eq!(keys_of(&config,cmd::HOLSTER),vec![KeyCode::KeyH]);
        assert_eq!((keys_of(&config,cmd::INVENTORY_PANEL),keys_of(&config,cmd::CHARACTER_PANEL),keys_of(&config,cmd::PLAYER_PANEL)),(vec![KeyCode::KeyC],vec![KeyCode::KeyZ],vec![KeyCode::KeyX]));
        for slot in 0..8 {assert_eq!(keys_of(&config,cmd::WEAPON1+slot),vec![[KeyCode::Digit1,KeyCode::Digit2,KeyCode::Digit3,KeyCode::Digit4,KeyCode::Digit5,KeyCode::Digit6,KeyCode::Digit7,KeyCode::Digit8][slot]]);}
    }
}
