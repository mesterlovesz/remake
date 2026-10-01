//! The end of the game: cshell.dll c_outromgr (0x10031a00..0x10032756) shows three story slides, the scrolling credits and a
//! last slide over `sounds\muza\credits.wav`, then the client shuts down (0x10059cc0); the remake returns to the main menu instead (owner decision, `Travel::menu`). Evidence: docs/retail-scenes.md.
use bevy::prelude::*;
use serde::Deserialize;
use crate::{ViewerConfig,retail_ui::{RetailUi,label}};

/// The three story slides last 10 s each, the last slide 20 s (floats at 0x10066170 and 0x10066418).
const SLIDE_SECONDS:f32=10.0;
const LAST_SECONDS:f32=20.0;
/// Text row pitch and text scale in the 1024x768 canvas (0x100661b0 = 32, 0x100660a0 = 0.5).
const ROW:f32=32.0;
const TEXT_SCALE:f32=0.5;
/// The credits scroll starts at the screen bottom (768) and both per-frame routines of c_outromgr subtract from the same offset: the update
/// 0x100323a0 dt*20 (0x10066418) and the draw 0x10032530 dt*16 (0x10066128, times width/1024): 36 px/s at 1024x768 (about 78 s, the credits
/// track is 89 s).
const SCROLL:f32=36.0;
const CANVAS:Vec2=Vec2::new(1024.0,768.0);

#[derive(Deserialize,Clone,Default)] pub struct Slide {pub image:String,pub lines:Vec<String>,pub y:f32}
#[derive(Deserialize,Clone,Default)] pub struct Credits {pub image:String,pub lines:Vec<String>}
#[derive(Deserialize,Clone,Default,Resource)] pub struct EndData {pub slides:Vec<Slide>,pub credits:Credits,pub last:Slide}

#[derive(Clone,Copy,PartialEq,Eq,Debug)] pub enum Screen {Slide(usize),Credits,Last}
/// State of the sequence; `advance` is one update of c_outromgr (0x10032420).
#[derive(Clone,Debug)]
pub struct EndGame {pub screen:Screen,pub timer:f32,pub scroll:f32,built:Option<Screen>}
impl EndGame {
    pub fn new()->Self {Self {screen:Screen::Slide(0),timer:0.0,scroll:CANVAS.y,built:None}}
    /// Adds `dt`; true when the sequence is over (retail: the client shuts down; the remake: back to the main menu).
    pub fn advance(&mut self,dt:f32,slides:usize,credit_lines:usize)->bool {
        self.timer+=dt;
        match self.screen {
            Screen::Slide(i) if self.timer>=SLIDE_SECONDS=>{
                self.timer=0.0;
                self.screen=if i+1<slides {Screen::Slide(i+1)}else{self.scroll=CANVAS.y;Screen::Credits};
            },
            Screen::Credits=>{
                self.scroll-=SCROLL*dt;
                // The last row has left the top edge (0x10032740: rows*32 + scroll - 1 < -32).
                if credit_lines as f32*ROW+self.scroll-1.0< -ROW {self.screen=Screen::Last;self.timer=0.0;}
            },
            Screen::Last if self.timer>=LAST_SECONDS=>return true,
            _=>{},
        }
        false
    }
}

/// One drawn text row in canvas pixels.
#[derive(Clone,Debug,PartialEq)] pub struct Row {pub text:String,pub x:f32,pub y:f32,pub scale:f32,pub color:[u8;3]}
fn centered(width:f32)->f32 {((CANVAS.x-width.trunc()) as i32>>1) as f32}
/// Slide rows: centred, scale 0.5, white, 32 px apart from `y` (0x10031a40 and its siblings).
pub fn slide_rows(lines:&[String],y:f32,width:&dyn Fn(&str,f32)->f32)->Vec<Row> {
    lines.iter().enumerate().map(|(i,text)|Row {text:text.clone(),x:centered(width(text,TEXT_SCALE)),y:y+ROW*i as f32,scale:TEXT_SCALE,color:[255,255,255]}).collect()
}
/// Credits rows at `scroll`: every row is drawn twice, dark (32,32,32) at (x, y) and again at (x-1, y-1) in its own colour. Rows
/// alternate by index mod 3 (scale 0.3 grey c0, 0.45 grey f0, 0.5 stays dark); the drawing code does not care what the row holds
/// (0x100325a7..0x100326d3), so with the file's leading two-row block the role titles land on the dark rows.
pub fn credit_rows(lines:&[String],scroll:f32,width:&dyn Fn(&str,f32)->f32)->Vec<(Row,Row)> {
    lines.iter().enumerate().map(|(i,text)|{
        let (scale,color)=match i%3 {0=>(0.3,[0xc0,0xc0,0xc0]),1=>(0.45,[0xf0,0xf0,0xf0]),_=>(TEXT_SCALE,[0x20,0x20,0x20])};
        let x=centered(width(text,scale));let y=(ROW*i as f32+scroll).trunc();
        (Row {text:text.clone(),x,y,scale,color:[0x20,0x20,0x20]},Row {text:text.clone(),x:x-1.0,y:y-1.0,scale,color})
    }).collect()
}

#[derive(Component)] pub struct EndScreen;
#[derive(Component)] pub struct CreditRow {index:usize,shadow:bool}

pub fn setup(mut commands:Commands,config:Res<ViewerConfig>) {
    let data=std::fs::read_to_string(config.output.join("endgame.json")).ok().and_then(|text|serde_json::from_str::<EndData>(&text).ok()).unwrap_or_default();
    commands.insert_resource(data);
}

fn text_width(ui:&RetailUi)->impl Fn(&str,f32)->f32+'_ {move|text,scale|ui.fonts.get("mincho").map_or(text.chars().count() as f32*32.0*scale,|font|font.width(text,scale))}

pub fn tick(mut commands:Commands,mut opening:ResMut<crate::opening::Opening>,data:Res<EndData>,time:Res<Time>,assets:Res<AssetServer>,ui:Res<RetailUi>,session:Res<crate::settings::Session>,
    mut travel:ResMut<crate::travel::Travel>,screens:Query<Entity,With<EndScreen>>,mut rows:Query<(&CreditRow,&mut Node,&mut Visibility)>) {
    let Some(end)=opening.end.as_mut() else {
        for entity in &screens {commands.entity(entity).despawn();}
        return;
    };
    if session.paused {return;}
    // A stale or missing export (run `python -m tools.export_scenes GYARI output`) must not crash the ending.
    if data.slides.is_empty() || data.last.image.is_empty() {warn!("endgame.json hiányzik vagy régi formátumú: a stáblista kimarad");opening.end=None;travel.menu=true;return;}
    let width=text_width(&ui);
    let over=end.advance(time.delta_secs(),data.slides.len(),data.credits.lines.len());
    if end.built!=Some(end.screen) {
        end.built=Some(end.screen);
        for entity in &screens {commands.entity(entity).despawn();}
        let (image,slide)=match end.screen {
            Screen::Slide(i)=>(data.slides[i].image.clone(),Some(&data.slides[i])),
            Screen::Credits=>(data.credits.image.clone(),None),
            Screen::Last=>(data.last.image.clone(),Some(&data.last)),
        };
        commands.spawn((EndScreen,Button,GlobalZIndex(600),Node {position_type:PositionType::Absolute,width:percent(100),height:percent(100),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()},BackgroundColor(Color::BLACK)))
            .with_children(|root| {
                root.spawn((ImageNode::new(assets.load(image)),Node {width:px(CANVAS.x),height:px(CANVAS.y),flex_shrink:0.0,..default()})).with_children(|canvas| {
                    let white=|c:[u8;3]|Color::srgb_u8(c[0],c[1],c[2]);
                    if let Some(slide)=slide {
                        for row in slide_rows(&slide.lines,slide.y,&width) {canvas.spawn(label("mincho",row.text,row.scale,white(row.color),row.x,row.y));}
                    } else {
                        for (index,(shadow,top)) in credit_rows(&data.credits.lines,end.scroll,&width).into_iter().enumerate() {
                            for (row,is_shadow) in [(shadow,true),(top,false)] {canvas.spawn((CreditRow {index,shadow:is_shadow},label("mincho",row.text,row.scale,white(row.color),row.x,row.y)));}
                        }
                    }
                });
            });
    }
    if end.screen==Screen::Credits {
        for (row,mut node,mut visibility) in &mut rows {
            let y=(ROW*row.index as f32+end.scroll).trunc();
            node.top=px(if row.shadow {y}else{y-1.0});
            *visibility=if y< -64.0 || y>CANVAS.y {Visibility::Hidden}else{Visibility::Inherited};
        }
    }
    if over {
        // Retail calls ILTClient::Shutdown here (cshell.dll 0x10059cc0); the owner wants the main menu instead (docs/retail-scenes.md).
        info!("A játék véget ért: vissza a főmenübe (retail: kilépés, cshell.dll 0x10059cc0)");
        opening.end=None;travel.menu=true;
    }
}

#[cfg(test)] mod tests {
    use super::*;
    fn narrow(text:&str,scale:f32)->f32 {text.chars().count() as f32*32.0*scale}
    #[test] fn the_story_slides_credits_and_last_slide_run_in_retail_order() {
        let mut end=EndGame::new();let mut seen=vec![end.screen];let mut t=0.0f32;
        loop {
            let over=end.advance(0.1,3,63);t+=0.1;
            if seen.last()!=Some(&end.screen) {seen.push(end.screen);}
            if over {break;}
            assert!(t<400.0);
        }
        assert_eq!(seen,vec![Screen::Slide(0),Screen::Slide(1),Screen::Slide(2),Screen::Credits,Screen::Last]);
        // 3 x 10 s + 2815 px at 36 px/s (about 78 s) + 20 s.
        assert!((t-(30.0+78.2+20.0)).abs()<1.0,"{t}");
    }
    #[test] fn slide_rows_are_centred_and_32_apart() {
        let rows=slide_rows(&["abcd".to_owned(),"ab".to_owned()],540.0,&narrow);
        assert_eq!((rows[0].x,rows[0].y,rows[0].scale),(480.0,540.0,0.5));assert_eq!((rows[1].x,rows[1].y),(496.0,572.0));
    }
    #[test] fn credits_rows_cycle_scale_and_colour_and_carry_a_dark_shadow() {
        let lines:Vec<String>=["a","b","c","d"].iter().map(|s|s.to_string()).collect();
        let rows=credit_rows(&lines,768.0,&narrow);
        assert_eq!(rows[0].1.scale,0.3);assert_eq!(rows[1].1.scale,0.45);assert_eq!(rows[2].1.scale,0.5);assert_eq!(rows[3].1.scale,0.3);
        assert_eq!(rows[0].1.color,[0xc0;3]);assert_eq!(rows[1].1.color,[0xf0;3]);assert_eq!(rows[2].1.color,[0x20;3]);
        assert_eq!((rows[1].0.y,rows[1].1.y,rows[1].1.x-rows[1].0.x),(800.0,799.0,-1.0));assert_eq!(rows[0].0.color,[0x20;3]);
    }
    #[test] #[ignore] fn the_export_has_three_slides_and_at_most_63_credit_rows_with_existing_images() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let data:EndData=serde_json::from_str(&std::fs::read_to_string(root.join("endgame.json")).unwrap()).unwrap();
        assert_eq!(data.slides.len(),3);assert!(data.credits.lines.len()<=63 && data.credits.lines.len()>=60);
        for image in data.slides.iter().map(|s|&s.image).chain([&data.credits.image,&data.last.image]) {assert!(root.join(image).is_file(),"{image}");}
        assert_eq!(data.slides[2].lines[1],data.slides[2].lines[3],"outro3.txt repeats >Outro32 (retail)");
    }
}
