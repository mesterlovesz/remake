use mission_runtime::{Context, Event, Mission, NpcState};

fn mission(condition: &str) -> Mission {
    Mission::from_sources("town", &format!("level worlds\\town\naction rule\n{condition}\nset fired\nendifs"), "", "").unwrap()
}

fn context() -> Context {
    Context {
        hostile_npcs: vec!["guard".into()],
        npcs: vec![
            NpcState { name: "guard".into(), position: [191., 0., 0.], alive: true, player_seen: true, contact: true, ..Default::default() },
            NpcState { name: "civilian".into(), position: [1., 0., 0.], alive: true, player_seen: true, ..Default::default() },
        ],
        ..Default::default()
    }
}

#[test]
fn hostile_sight_range_and_interaction_use_living_source_role() {
    for predicate in ["ifseenbyhostile", "ifhostileblizejniz 192", "ifactionhostile"] {
        let mut c = context(); c.action_target = Some("guard".into());
        let mut m = mission(predicate); m.tick(0.1, &c); assert!(m.flag("fired"), "{predicate}");
        c.npcs[0].alive = false;
        let mut m = mission(predicate); m.tick(0.1, &c); assert!(!m.flag("fired"), "{predicate}");
        c.npcs[0].alive = true; c.hostile_npcs.clear();
        let mut m = mission(predicate); m.tick(0.1, &c); assert!(!m.flag("fired"), "{predicate}");
    }
    let mut c = context(); c.npcs[0].position[0] = 193.;
    let mut m = mission("ifhostileblizejniz 192"); m.tick(0.1, &c); assert!(!m.flag("fired"));
    c.npcs[0].player_seen = false;c.npcs[0].contact = false;
    let mut m = mission("ifseenbyhostile"); m.tick(0.1, &c); assert!(!m.flag("fired"));
    c.action_target = Some("civilian".into());
    let mut m = mission("ifactionhostile"); m.tick(0.1, &c); assert!(!m.flag("fired"));
}

#[test]
fn quest_inventory_uses_exact_item_id() {
    let mut m = mission("ifplayerhas Golden cat.");
    let mut c = Context::default();
    c.inventory.push("Golden cat".into()); m.tick(0.1, &c); assert!(!m.flag("fired"));
    c.inventory.push("Golden cat.".into()); m.tick(0.1, &c); assert!(m.flag("fired"));
}

#[test]
fn authored_health_loss_and_cutscene_are_events() {
    let mut m = Mission::from_sources("town", "level worlds\\town\naction event\nifnot done\nzdrowie -1000\ncutscene intro\nset done\nendifs", "", "").unwrap();
    // The executor runs the effects of one action in its fixed kind order (cutscene before zdrowie, 0x1001a4e1 .. 0x1001a6f2), not in the written order.
    assert_eq!(m.tick(0.1, &Context::default()), vec![Event::PlayCutscene("intro".into()), Event::HealthDelta(-1000.)]);
    assert!(m.tick(0.1, &Context::default()).is_empty());
}

#[test]
#[ignore = "requires local campaign export"]
fn all_exported_campaign_and_legacy_rules_parse() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let sources: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(output.join("gameplay_scripts.json")).unwrap()).unwrap();
    // Every supported DAT with an authored gameai block, including introduction.
    for world in ["rh1-wiezienie1", "rh1-wiezienie2", "rh1-wiezienie3", "rh2-wiezienie1", "rh2-wiezienie2", "rh3-miasteczko0", "rh3-miasteczko1", "rh3-miasteczko2", "chapel_mniejszy", "rh7a-tunele", "rh9-fabryka", "rh12-lab1", "rh12-lab2", "burmistrz1", "burmistrz2", "chinatown", "chinatown2", "rh10-wiezowiec1", "rh10-wiezowiec2", "rh10-wiezowiec3", "podziemia1", "podziemia1a", "podziemia1b", "podziemia1c", "wiez_wn1", "wiez_wn2", "wiez_wn3", "knajpa"] {
        Mission::from_sources(world, sources["gameai"].as_str().unwrap(), sources["dialogues"].as_str().unwrap(), sources["text_keys"].as_str().unwrap()).unwrap_or_else(|e| panic!("{world}: {e}"));
    }
}


#[test]
fn use_prompt_only_when_an_action_would_run() {
    let m = Mission::from_sources("town", "level worlds\\town\naction talk\nifaction barman\nifnot talked\nset talked\nendifs\naction other\nif never\nset x\nendifs", "", "").unwrap();
    let c = context();
    assert!(m.responds_to_action("barman", &c));
    assert!(!m.responds_to_action("guard", &c));
    let mut done = m;
    done.set_flag("talked", true);
    assert!(!done.responds_to_action("barman", &c));
}

/// Retail runs the actions of a level newest first (cshell 0x10015be0 links every node at the head of the list, the driver 0x1001a898 walks it from there):
/// a flag set by a later action is seen in the same frame by the earlier ones only, and when two actions hold at once the script-earlier one wins a latch.
#[test]
fn actions_run_in_reverse_script_order() {
    // `second` sets Latch and runs first; `first` is gated by `ifnot Latch` and sees it set in the same frame, so it never fires.
    let mut m = Mission::from_sources("town", "level worlds\\town\nbool Latch\nbool Seen\naction first\nifnot Latch\nset Seen\nendifs\naction second\nset Latch\nendifs", "", "").unwrap();
    m.tick(0.1, &Context::default());
    let snapshot = m.snapshot();
    assert_eq!(snapshot.flags.get("Latch"), Some(&true));
    assert_ne!(snapshot.flags.get("Seen"), Some(&true), "the later action ran first and closed the latch");
    // A cascade arrives one frame later: `b` (written after `a`) reads what `a` sets, but runs before it.
    let mut m = Mission::from_sources("town", "level worlds\\town\nbool A\nbool B\naction a\nset A\nendifs\naction b\nif A\nset B\nendifs", "", "").unwrap();
    m.tick(0.1, &Context::default());
    assert_ne!(m.snapshot().flags.get("B"), Some(&true), "b ran before a in the first frame");
    m.tick(0.1, &Context::default());
    assert_eq!(m.snapshot().flags.get("B"), Some(&true), "and sees A in the second");
}

/// Level-scope keywords (cshell 0x10015fe8..0x10016215) are stored per level section: the prologue carries `nie_sprawdzaj_drzwi` and `gestosc_sciezek 4`.
#[test]
fn level_scope_keywords_belong_to_their_own_section() {
    let script = "level worlds\\town\nnie_sprawdzaj_drzwi\ngestosc_sciezek 4\naction a\nset X\nendifs\nlevel worlds\\other\npure_shooter\naction b\nset Y\nendifs";
    let mut m = Mission::from_sources("town", script, "", "").unwrap();
    assert_eq!(m.level_setting("nie_sprawdzaj_drzwi"), Some(""));
    assert_eq!(m.level_setting("gestosc_sciezek"), Some("4"));
    assert_eq!(m.level_setting("pure_shooter"), None);
    m.enter_world("other").unwrap();
    assert_eq!(m.level_setting("nie_sprawdzaj_drzwi"), None);
    assert_eq!(m.level_setting("pure_shooter"), Some(""));
}
