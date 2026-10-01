//! Original carried-item grid (cshell.dll 0x1001bba0..0x1001c3a0, panel 0x10034a00).
//!
//! One 4-column grid: rows 0..3 are the backpack, whose columns are unbounded
//! and scrolled one column per arrow press; rows 3..5 are the holster (keys 1..8,
//! slot = column + 4*(row-3)); row 5 is the belt (F1..F4). Ammunition never enters it.
use serde::{Deserialize,Serialize};
use crate::retail_ui::Catalog;

pub const COLUMNS:u32=4;
pub const BACKPACK_ROWS:u32=3;
pub const HOLSTER_ROW:u32=3;
pub const BELT_ROW:u32=5;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct Entry {pub item:String,pub count:u32,pub column:u32,pub row:u32}

#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
pub struct Items {pub entries:Vec<Entry>}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Placed {Stacked,Holster(u32),Backpack(u32,u32)}

/// Effects of consuming one `eaten` item; applied by the caller.
#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct Effect {pub health:f32,pub power_up:f32,pub pain_killer:f32,pub alcohol:f32}

impl Items {
    pub fn at(&self,column:u32,row:u32)->Option<&Entry> {self.entries.iter().find(|e|e.column==column && e.row==row)}
    fn index_at(&self,column:u32,row:u32)->Option<usize> {self.entries.iter().position(|e|e.column==column && e.row==row)}
    pub fn count(&self,item:&str)->u32 {self.entries.iter().filter(|e|e.item==item).map(|e|e.count).sum()}
    pub fn has(&self,item:&str)->bool {self.entries.iter().any(|e|e.item==item)}
    /// Exact, case-sensitive names for `ifplayerhas` (0x1001a0f0).
    pub fn names(&self)->Vec<String> {self.entries.iter().map(|e|e.item.clone()).collect()}
    pub fn clear(&mut self) {self.entries.clear();}
    /// 0x1001bb50: sum of count x unit weight, weapons included.
    pub fn weight(&self,catalog:&Catalog)->f32 {self.entries.iter().map(|e|e.count as f32*catalog.weight(&e.item)).sum()}
    /// Weapon id in holster slot 0..8 (0x1001c060 only returns weapons).
    pub fn holster(&self,slot:u32,catalog:&Catalog)->Option<&str> {
        self.at(slot%COLUMNS,HOLSTER_ROW+slot/COLUMNS).filter(|e|catalog.items.get(&e.item).is_some_and(|d|d.weapon)).map(|e|e.item.as_str())
    }
    fn free_holster(&self)->Option<(u32,u32)> {(0..8).map(|s|(s%COLUMNS,HOLSTER_ROW+s/COLUMNS)).find(|&(c,r)|self.at(c,r).is_none())}
    fn free_backpack(&self)->(u32,u32) {(0..).map(|i|(i/BACKPACK_ROWS,i%BACKPACK_ROWS)).find(|&(c,r)|self.at(c,r).is_none()).unwrap()}
    /// Routing of a newly carried item (0x1001c100 same-id merge, then 0x1001bba0).
    /// Weapons never stack; the caller turns a duplicate weapon into ammunition.
    pub fn add(&mut self,item:&str,count:u32,weapon:bool)->Placed {
        if !weapon {if let Some(entry)=self.entries.iter_mut().find(|e|e.item==item) {entry.count+=count;return Placed::Stacked;}}
        let (column,row)=if weapon {self.free_holster().unwrap_or_else(||self.free_backpack())}else{self.free_backpack()};
        self.entries.push(Entry {item:item.into(),count,column,row});
        if row>=HOLSTER_ROW {Placed::Holster(column+COLUMNS*(row-HOLSTER_ROW))}else{Placed::Backpack(column,row)}
    }
    /// Panel drop (0x10034a00): an empty cell receives the item, an occupied one swaps.
    /// Stacks never merge and no cell is restricted to a kind of item.
    pub fn move_item(&mut self,from:(u32,u32),to:(u32,u32)) {
        if from==to {return;}
        let Some(source)=self.index_at(from.0,from.1) else {return};
        if let Some(target)=self.index_at(to.0,to.1) {self.entries[target].column=from.0;self.entries[target].row=from.1;}
        self.entries[source].column=to.0;self.entries[source].row=to.1;
    }
    /// One unit leaves the cell; the item disappears when the last one goes.
    pub fn take_one(&mut self,cell:(u32,u32))->Option<String> {
        let index=self.index_at(cell.0,cell.1)?;let item=self.entries[index].item.clone();
        if self.entries[index].count<=1 {self.entries.remove(index);}else{self.entries[index].count-=1;}
        Some(item)
    }
    pub fn remove_item(&mut self,item:&str) {self.entries.retain(|e|e.item!=item);}
    /// Right mouse in the panel (0x10035399): `eaten` items only; the last unit is removed.
    pub fn use_in_panel(&mut self,cell:(u32,u32),catalog:&Catalog)->Option<Effect> {
        let effect=effect(catalog,&self.at(cell.0,cell.1)?.item)?;self.take_one(cell);Some(effect)
    }
    /// F1..F4 ("panel::ZjedzF", 0x100348e0). Retail decrements to zero and only
    /// removes an entry that is already at zero, so a belt stack of n gives n+1 uses.
    pub fn use_belt(&mut self,slot:u32,catalog:&Catalog)->Option<Effect> {
        let index=self.index_at(slot,BELT_ROW)?;let effect=effect(catalog,&self.entries[index].item)?;
        if self.entries[index].count==0 {self.entries.remove(index);}else{self.entries[index].count-=1;}
        Some(effect)
    }
    /// Old saves recorded every acquisition by name; rebuild the grid from them.
    pub fn from_legacy(names:&[String],catalog:&Catalog)->Self {
        let mut items=Self::default();
        for name in names {let Some(d)=catalog.items.get(name) else {continue};
            if d.ammo_for>=0 || (d.weapon && items.has(name)) {continue;}
            items.add(name,1,d.weapon);}
        items
    }
}
fn effect(catalog:&Catalog,item:&str)->Option<Effect> {
    let d=catalog.items.get(item).filter(|d|d.eaten)?;
    Some(Effect {health:d.health,power_up:d.power_up,pain_killer:d.pain_killer,alcohol:d.alcohol})
}

#[cfg(test)] mod tests {
    use super::*;
    use crate::retail_ui::ItemDef;
    fn catalog()->Catalog {
        let mut c=Catalog::default();
        let mut add=|name:&str,d:ItemDef|{c.items.insert(name.into(),d);};
        add("an apple",ItemDef {weight:0.15,health:5.0,eaten:true,ammo_for:-1,..default()});
        add("Police nightstick",ItemDef {weight:0.5,weapon:true,melee:true,ammo_for:-1,..default()});
        add("Glock",ItemDef {weight:0.63,weapon:true,ammo_for:-1,..default()});
        add("small syringe",ItemDef {weight:0.2,power_up:20.0,eaten:true,ammo_for:-1,..default()});
        add("vodka bottle",ItemDef {weight:0.75,health:40.0,alcohol:40.0,eaten:true,ammo_for:-1,..default()});
        add("Golden cat.",ItemDef {weight:5.0,fixed:true,ammo_for:-1,..default()});
        add("Glock ammo",ItemDef {ammo_for:0,amount:17,..default()});
        c
    }
    fn default<T:Default>()->T {T::default()}
    #[test] fn owner_screenshot_weight_is_the_sum_of_carried_items() {
        let c=catalog();let mut items=Items::default();
        items.add("an apple",1,false);items.add("Police nightstick",1,true);items.add("Glock",1,true);
        assert!((items.weight(&c)-1.28).abs()<1e-5);
        assert_eq!(items.holster(0,&c),Some("Police nightstick"));assert_eq!(items.holster(1,&c),Some("Glock"));
        assert_eq!(items.at(0,0).unwrap().item,"an apple");
    }
    #[test] fn food_stacks_anywhere_and_fills_the_backpack_column_by_column() {
        let mut items=Items::default();
        assert_eq!(items.add("an apple",1,false),Placed::Backpack(0,0));
        assert_eq!(items.add("small syringe",1,false),Placed::Backpack(0,1));
        assert_eq!(items.add("vodka bottle",1,false),Placed::Backpack(0,2));
        assert_eq!(items.add("Golden cat.",1,false),Placed::Backpack(1,0));
        items.move_item((0,0),(1,5));
        assert_eq!(items.add("an apple",1,false),Placed::Stacked);
        assert_eq!(items.at(1,5).unwrap().count,2);
        for i in 0..20 {items.add(&format!("x{i}"),1,false);}
        assert!(items.entries.iter().any(|e|e.column>=6),"backpack has no size limit");
    }
    #[test] fn weapons_fill_holster_rows_then_the_backpack_and_never_stack() {
        let c=catalog();let mut items=Items::default();
        for slot in 0..8 {assert_eq!(items.add(&format!("w{slot}"),1,true),Placed::Holster(slot));}
        assert_eq!(items.add("Glock",1,true),Placed::Backpack(0,0));
        assert_eq!(items.holster(4,&c),None,"only weapons count");
        assert_eq!(items.at(0,4).unwrap().item,"w4");
    }
    #[test] fn old_saves_rebuild_the_grid_from_their_acquisition_log() {
        let c=catalog();
        let items=Items::from_legacy(&["an apple".into(),"Glock".into(),"an apple".into(),"Glock ammo".into(),"Glock".into(),"unknown thing".into()],&c);
        assert_eq!(items.count("an apple"),2,"food stacks");assert_eq!(items.count("Glock"),1,"a second weapon was only ammunition");
        assert!(!items.has("Glock ammo") && !items.has("unknown thing"),"ammunition never enters the grid");
        assert_eq!(items.holster(0,&c),Some("Glock"));
    }
    #[test] fn drops_swap_and_never_merge() {
        let mut items=Items::default();items.add("an apple",1,false);items.add("an apple",1,false);items.add("small syringe",1,false);
        items.move_item((0,0),(0,1));
        assert_eq!(items.at(0,1).unwrap().item,"an apple");assert_eq!(items.at(0,0).unwrap().item,"small syringe");
        items.move_item((0,1),(3,5));assert_eq!(items.at(3,5).unwrap().count,2);assert!(items.at(0,1).is_none());
    }
    #[test] fn panel_use_and_belt_use_follow_retail_counting() {
        let c=catalog();let mut items=Items::default();
        items.add("an apple",1,false);items.add("an apple",1,false);items.add("Golden cat.",1,false);
        assert_eq!(items.use_in_panel((0,1),&c),None,"only eaten items are usable");
        assert_eq!(items.use_in_panel((0,0),&c).unwrap().health,5.0);assert_eq!(items.count("an apple"),1);
        items.use_in_panel((0,0),&c);assert!(!items.has("an apple"));
        items.add("vodka bottle",1,false);items.add("vodka bottle",1,false);
        let cell=(items.entries.iter().find(|e|e.item=="vodka bottle").unwrap().column,0);let cell=(cell.0,items.entries.iter().find(|e|e.item=="vodka bottle").unwrap().row);
        items.move_item(cell,(0,5));
        let uses=(0..5).filter(|_|items.use_belt(0,&c).is_some()).count();
        assert_eq!(uses,3,"retail F-key quirk: n+1 uses");assert!(!items.has("vodka bottle"));
    }
}
