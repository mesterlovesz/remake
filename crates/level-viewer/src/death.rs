//! The player dies (cshell.dll 0x10060070): the controller is flagged dead, every living character re-enters its default phase
//! (`default_faza`, so attackers stop attacking a body), the death camera runs (view.rs) and the shell prints GameShell4 (character.rs).
//! Esc opens the menus and F9 loads the quick save (menu.rs). There is no death screen: the overlay of earlier builds is not retail.
use bevy::prelude::*;
use crate::{campaign::Campaign,npcs::NpcRoster};

pub fn tick(campaign:Res<Campaign>,mut roster:ResMut<NpcRoster>,mut was_dead:Local<bool>) {
    let dead=campaign.dead();
    if dead && !*was_dead {roster.player_died();}
    *was_dead=dead;
}
