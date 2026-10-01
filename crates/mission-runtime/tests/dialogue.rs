//! Dialogue and condition semantics reverse-engineered from cshell.dll
//! (StartDialog 0x100194d0, dialogue tick 0x10018ab0, gameai tick 0x1001a750).
use mission_runtime::{Context, Event, Mission, NpcState};

fn guard(id: &str, x: f32, seen: bool) -> NpcState {
    NpcState { name: "chiniol".into(), id: id.into(), position: [x, 0., 0.], alive: true, player_seen: seen, contact: seen }
}
fn hostile_context(npcs: Vec<NpcState>) -> Context {
    Context { hostile_npcs: vec!["chiniol".into()], npcs, ..Default::default() }
}
fn dialogues(ai: &str, dialogues: &str) -> Mission {
    Mission::from_sources("town", &format!("level worlds\\town\n{ai}"), dialogues, "").unwrap()
}
fn started(events: &[Event], id: &str) -> bool { events.iter().any(|e| matches!(e, Event::Dialogue(d) if d.id == id)) }

// chinatown2 ChiniolZauwaza2: the condition stays true for the whole speech.
const WARN: &str = "action ChiniolZauwaza2\nifseenbyhostile\nifhostileblizejniz 192\ndialog Chiniole13\nendifs";
const ASK: &str = "dialog Chiniole13\ndelay 4\nperson hostile\ntitle Stop\ntitlesnd stop.wav\nanswer1 Yes\nonchoice1dialog Done\nontimeexceeded 20 Done\ndialog Done\ntitle Ok\nontimeexceeded 1";

#[test]
fn an_active_dialogue_is_not_restarted_by_its_still_true_action() {
    let mut m = dialogues(WARN, ASK);
    let c = hostile_context(vec![guard("o_postac1", 100., true)]);
    assert!(started(&m.tick(0.1, &c), "Chiniole13"));
    for _ in 0..45 { assert!(m.tick(0.1, &c).is_empty(), "the speech restarted"); }
    assert!(m.choices_ready());
    assert!(m.choose(1).contains(&Event::EndDialogue), "the answer is accepted");
    // The answer is spoken for answer1delay (default 0.2 s) before onchoice1dialog.
    assert!(started(&m.tick(0.3, &c), "Done"));
}

#[test]
fn speech_length_replaces_delay_and_title_only_nodes_end_after_it() {
    let mut m = dialogues("", "dialog Mono\ntitle Hello\ntitlesnd hello.wav\ndialog Short\ntitle Hi");
    assert!(started(&m.trigger_dialog("Mono"), "Mono"));
    m.set_speech_length(3.0);
    assert!(m.tick(2.9, &Context::default()).is_empty());
    assert_eq!(m.tick(0.2, &Context::default()), vec![Event::EndDialogue]);
    // Without speech the retail defaults end it after 0.2 s.
    m.trigger_dialog("Short");
    assert!(m.tick(0.15, &Context::default()).is_empty());
    assert_eq!(m.tick(0.1, &Context::default()), vec![Event::EndDialogue]);
}

#[test]
fn walking_away_from_the_speaker_ends_a_node_at_the_default_320() {
    let mut m = dialogues("", "dialog Talk\nperson chiniol\ntitle Wait\nontimeexceeded 30");
    m.trigger_dialog("Talk");
    let mut c = hostile_context(vec![guard("o_postac1", 300., true)]);
    assert!(m.tick(0.1, &c).is_empty());
    c.npcs[0].position[0] = 330.;
    assert_eq!(m.tick(0.1, &c), vec![Event::EndDialogue]);
}

#[test]
fn person_hostile_speaks_through_the_nearest_seeing_hostile_that_matched() {
    let mut m = dialogues(WARN, "dialog Chiniole13\nperson hostile\ntitle Stop\nontimeexceeded 30");
    // The far guard sees the player too, but the condition picks the near one.
    let mut c = hostile_context(vec![guard("far", 170., true), guard("near", 60., true)]);
    assert!(started(&m.tick(0.1, &c), "Chiniole13"));
    c.npcs[0].position[0] = 500.;
    assert!(m.tick(0.1, &c).is_empty(), "the far guard is not the speaker");
    c.npcs[1].position[0] = 400.;
    // The end of the node releases the tagged speaker (its phase is re-entered, cshell 0x10019550..0x10019585).
    assert_eq!(m.tick(0.1, &c), vec![Event::EndDialogue, Event::Released(vec!["near".to_string()])]);
}

#[test]
fn first_instance_conditions_but_any_seeing_instance_for_ifplayerseenby() {
    let rule = |condition: &str| dialogues(&format!("action a\n{condition}\nset fired\nendifs"), "");
    let mut first_dead = guard("a", 50., false); first_dead.alive = false;
    let c = Context { npcs: vec![first_dead, guard("b", 50., true)], ..Default::default() };
    for (condition, expected) in [("ifalive chiniol", false), ("ifdead chiniol", true), ("ifplayerseenby chiniol", true), ("ifdead nobody", false), ("ifgraczblizejniz 1 chiniol", false)] {
        let mut m = rule(condition); m.tick(0.1, &c); assert_eq!(m.flag("fired"), expected, "{condition}");
    }
}

#[test]
fn setallfaza_and_setfaza_are_distinct_events() {
    let mut m = dialogues("action a\nsetfaza ucieka cywil1\nsetallfaza ucieka cywil2\nendifs", "");
    assert_eq!(m.tick(0.1, &Context::default()), vec![
        Event::SetNpcPhase { name: "cywil1".into(), phase: "ucieka".into() },
        Event::SetAllNpcPhase { name: "cywil2".into(), phase: "ucieka".into() }]);
}

#[test]
fn counters_restart_on_set_and_unset_but_not_when_a_dialogue_starts() {
    let mut m = Mission::from_sources("town", "level worlds\\town\nlicznik Licznik1\naction a\niflicznikwiekszyniz 5 Licznik1\nset fired\nendifs",
        "dialog Talk\ntitle Hi\nontimeexceeded 1", "").unwrap();
    m.tick(3.0, &Context::default());
    m.trigger_dialog("Talk"); m.tick(1.5, &Context::default());
    m.tick(0.6, &Context::default()); assert!(m.flag("fired"), "CzasOdGadki-style counters keep running through dialogues");
    m.set_flag("Licznik1", true); m.set_flag("fired", false);
    m.tick(4.0, &Context::default()); assert!(!m.flag("fired"));
}

/// Every retail dialogue without answers must end on its own (title-only nodes end after their speech or `delay`).
#[test]
#[ignore = "requires the local gameplay_scripts.json export"]
fn every_exported_answerless_dialogue_ends_by_itself() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
    let scripts: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join("gameplay_scripts.json")).unwrap()).unwrap();
    let text = |k: &str| scripts[k].as_str().unwrap().to_owned();
    let ids: Vec<String> = text("dialogues").lines().filter_map(|l| l.trim().strip_prefix("dialog ").map(|v| v.trim().to_owned())).collect();
    let mut endless = Vec::new();
    for id in &ids {
        let mut m = Mission::from_sources("rh3-miasteczko0", &text("gameai"), &text("dialogues"), &text("text_keys")).unwrap();
        let events = m.trigger_dialog(id);
        if !events.iter().any(|e| matches!(e, Event::Dialogue(_))) { continue; }
        let mut speech = 0.0;
        for step in 0..6000 {
            if let Some(d) = m.dialogue() { if !d.choices.is_empty() { speech = -1.0; break; } }
            else { break; }
            m.tick(0.1, &Context::default());
            speech = step as f32 * 0.1;
        }
        if speech >= 0.0 && m.dialogue().is_some() { endless.push(id.clone()); }
    }
    assert!(endless.is_empty(), "dialogues that never end: {endless:?}");
}

// ---- the click state machine (0x10018bb0..0x10018d34), the parser limits (0x173a0..) and StartDialog side effects (0x100194d0)
const QUESTION: &str = "dialog Ask\ntitle Q\ndelay 3\nanswer1 A1\nanswer1delay 2\nonchoice1dialog Next\nanswer2 A2\nanswer2delay 1\nonchoice2dialog Other\nontimeexceeded 30\ndialog Next\ntitle N\nontimeexceeded 1\ndialog Other\ntitle O\nontimeexceeded 1";

#[test]
fn a_click_shows_the_choices_early_and_is_used_up_but_taking_one_needs_the_delay() {
    let mut m = dialogues("", QUESTION);
    m.trigger_dialog("Ask");
    m.tick(0.5, &Context::default());
    assert!(!m.choices_ready());
    assert!(m.click(0).is_empty() && m.choices_ready(), "the click only shows the list");
    assert!(m.click(1).is_empty() && !m.dialogue().unwrap().choices.is_empty() && m.dialogue().unwrap().answered.is_none(), "elapsed <= delay: the second click picks nothing");
    m.tick(2.6, &Context::default());
    let events = m.click(1);
    assert!(matches!(&events[..], [Event::EndDialogue, Event::Dialogue(d)] if d.answered.as_deref() == Some("A2") && d.title == "Q"), "{events:?}");
}

#[test]
fn choices_appear_when_elapsed_passes_delay_strictly_and_no_wrap_is_needed_for_answer_numbers() {
    let mut m = dialogues("", QUESTION);
    m.trigger_dialog("Ask");
    m.tick(3.0, &Context::default());
    assert!(!m.choices_ready(), "elapsed > delay is strict");
    m.tick(0.01, &Context::default());
    assert!(m.choices_ready());
    assert!(m.choose(3).is_empty(), "only answers that exist");
    assert!(!m.choose(2).is_empty());
}

#[test]
fn a_click_skips_the_answer_delay_and_starts_the_next_node() {
    let mut m = dialogues("", QUESTION);
    m.trigger_dialog("Ask");
    m.tick(3.1, &Context::default());
    m.choose(1);
    m.tick(0.5, &Context::default());
    assert!(started(&m.click(0), "Next"));
}

#[test]
fn ontimeexceeded_below_a_tenth_waits_for_a_click() {
    let mut m = dialogues("", "dialog Hold\ntitle H\ndelay 1\nontimeexceeded 0\ndialog After\ntitle X\nontimeexceeded 1");
    m.trigger_dialog("Hold");
    assert!(m.tick(50.0, &Context::default()).is_empty(), "no time limit");
    assert_eq!(m.click(0), vec![Event::EndDialogue]);
    // ...but the node delay still applies: a click on the first frame does nothing.
    m.trigger_dialog("Hold");
    assert!(m.click(0).is_empty() && m.dialogue().is_some());
}

#[test]
fn only_four_answers_exist_and_a_repeated_key_keeps_its_last_value() {
    let mut m = dialogues("", "dialog Many\ntitle T\nanswer1 a\nanswer2 b\nanswer3 c\nanswer4 d\nanswer5 e\nset First\nset Second\nontimeexceeded 5");
    m.trigger_dialog("Many");
    assert_eq!(m.dialogue().unwrap().choices.len(), 4);
    assert!(!m.flag("First") && m.flag("Second"));
}

#[test]
fn a_dialogue_start_resets_the_tagged_speaker_and_the_dialogue_hostileattack_names_it() {
    let rule = "action a\nifhostileblizejniz 192\ndialog Bark\nendifs";
    let mut m = dialogues(rule, "dialog Bark\nperson hostile\ntitle Halt\nhostileattack\nontimeexceeded 2");
    let c = hostile_context(vec![guard("o_postac7", 100., true)]);
    let events = m.tick(0.1, &c);
    assert!(events.contains(&Event::SpeakerDefaultPhase("o_postac7".into())), "{events:?}");
    assert!(events.contains(&Event::SpeakerAttack(Some("o_postac7".into()))), "{events:?}");
    // Nothing tagged (a marker dialogue): no reset, no speaker.
    let mut m = dialogues("", "dialog Bark\nperson hostile\ntitle Halt\nhostileattack\nontimeexceeded 2");
    let events = m.trigger_dialog("Bark");
    assert!(!events.iter().any(|e| matches!(e, Event::SpeakerDefaultPhase(_))) && events.contains(&Event::SpeakerAttack(None)));
}

#[test]
fn the_speaker_is_the_first_tagged_actor_whatever_person_says() {
    let mut m = dialogues("action a\nifplayerseenby chiniol\ndialog Talk\nendifs", "dialog Talk\nperson barman\ntitle Hi\nontimeexceeded 30");
    let mut c = hostile_context(vec![guard("o_postac3", 100., true)]);
    m.tick(0.1, &c);
    c.npcs[0].position[0] = 400.;
    assert_eq!(m.tick(0.1, &c).first(), Some(&Event::EndDialogue), "walked 400 away from the tagged actor, not from a barman");
}
