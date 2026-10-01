//! Retail panel art, item catalogue and bitmap fonts exported by tools.export_inventory.
use bevy::{prelude::*,color::Alpha,asset::RenderAssetUsages,image::{ImageSampler,ImageSamplerDescriptor,ImageFilterMode,ImageAddressMode,ImageType,CompressedImageFormats},render::render_resource::{Extent3d,TextureDimension,TextureFormat},sprite::{TextureSlicer,BorderRect,SliceScaleMode}};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone,Debug,Default,Deserialize)]
pub struct ItemDef {
    pub title:String,pub description:String,pub weight:f32,pub health:f32,pub alcohol:f32,pub power_up:f32,pub pain_killer:f32,
    pub eaten:bool,pub fixed:bool,pub weapon:bool,pub grenade:bool,pub melee:bool,pub ammo_for:i32,pub amount:u32,pub ammo_index:i32,pub ammo_amount:u32,
    pub experience_index:Option<i32>,pub icon:Option<String>,pub pickup_icon:Option<String>,
}
#[derive(Clone,Debug,Deserialize)] pub struct Glyph {pub image:String,pub width:f32,pub height:f32}
#[derive(Clone,Debug,Deserialize)] pub struct Font {pub height:f32,pub glyphs:BTreeMap<char,Glyph>}
#[derive(Clone,Debug,Deserialize)] pub struct PanelImage {pub image:String,pub keyed:String,pub width:f32,pub height:f32}
#[derive(Clone,Debug,Default,Deserialize)]
pub struct Catalog {pub items:BTreeMap<String,ItemDef>,#[serde(default)] pub panel:BTreeMap<String,PanelImage>,#[serde(default)] pub fonts:BTreeMap<String,Font>,#[serde(default)] pub text:BTreeMap<String,String>}
impl Catalog {
    pub fn load(root:&std::path::Path)->Self {
        std::fs::read_to_string(root.join("retail_inventory.json")).ok().and_then(|s|serde_json::from_str(&s).map_err(|e|warn!("retail_inventory.json: {e}")).ok())
            .unwrap_or_else(||{warn!("A gyári felszerelés-export hiányzik: python -m tools.export_inventory ../GYARI output");Self::default()})
    }
    pub fn text(&self,key:&str)->String {self.text.get(key).cloned().unwrap_or_else(||key.to_owned())}
    pub fn weight(&self,item:&str)->f32 {self.items.get(item).map_or(0.0,|d|d.weight)}
}

/// One drawable glyph: image (optionally an atlas cell), drawn size and pen advance.
#[derive(Clone)] pub struct GlyphImage {pub image:Handle<Image>,pub rect:Option<Rect>,pub size:Vec2,pub advance:f32,/// Cell in the halo atlas (dark outline shape of the glyph, `HALO_PAD` larger on every side).
    pub halo:Option<Rect>}
#[derive(Clone,Default)] pub struct GlyphSet {pub glyphs:BTreeMap<char,GlyphImage>,pub height:f32,pub halo:Option<Handle<Image>>}
impl GlyphSet {
    fn get(&self,c:char)->Option<&GlyphImage> {self.glyphs.get(&c).or_else(||self.glyphs.get(&fold(c))).or_else(||self.glyphs.get(&fold(c).to_ascii_uppercase()))}
    pub fn width(&self,text:&str,scale:f32)->f32 {text.chars().map(|c|self.get(c).map_or(self.height*0.3,|g|g.advance)).sum::<f32>()*scale}
    /// The widest line of a newline separated text.
    pub fn block_width(&self,text:&str,scale:f32)->f32 {text.split('\n').map(|l|self.width(l,scale)).fold(0.0,f32::max)}
}

/// The loaded catalogue plus image handles and glyph sets, shared by every retail panel.
#[derive(Resource,Default)]
pub struct RetailUi {/// Soft dark blob, nine-sliced behind text lines (see `Aid`).
    pub blob:Handle<Image>,pub catalog:Catalog,images:BTreeMap<String,Handle<Image>>,pub fonts:BTreeMap<&'static str,GlyphSet>,/// scripts	ext_keys.txt as exported into retail_ui.json (keys carry the '>' of the retail file).
    pub locale:BTreeMap<String,String>}
impl RetailUi {
    pub fn image(&mut self,assets:&AssetServer,path:&str)->Handle<Image> {self.images.entry(path.to_owned()).or_insert_with(||assets.load(path.to_owned())).clone()}
    pub fn panel(&mut self,assets:&AssetServer,name:&str)->Handle<Image> {
        let path=self.catalog.panel.get(name).map(|p|p.keyed.clone()).unwrap_or_default();self.image(assets,&path)
    }
    pub fn icon(&mut self,assets:&AssetServer,item:&str)->Option<Handle<Image>> {
        let path=self.catalog.items.get(item)?.icon.clone()?;Some(self.image(assets,&path))
    }
    /// The Hungarian retail text of a key; the inventory export first, then the complete text_keys.txt.
    pub fn text(&self,key:&str)->String {self.catalog.text.get(key).or_else(||self.locale.get(&format!(">{key}"))).cloned().unwrap_or_else(||key.to_owned())}
}
pub fn setup(mut commands:Commands,config:Res<crate::ViewerConfig>,assets:Res<AssetServer>,mut images:ResMut<Assets<Image>>) {
    let mut ui=RetailUi {catalog:Catalog::load(&config.output),..default()};
    for (name,key) in [("info","info"),("cyfry","cyfry"),("un","un"),("un_big","un_big")] {
        let Some(font)=ui.catalog.fonts.get(key).cloned() else {continue};
        let glyphs=font.glyphs.iter().map(|(c,g)|(*c,GlyphImage {image:ui.image(&assets,&g.image),rect:None,size:Vec2::new(g.width,g.height),advance:g.width,halo:None})).collect();
        ui.fonts.insert(name,GlyphSet {glyphs,height:font.height,halo:None});
    }
    // Mincho table font: 32x64 cells, fixed advance (menu and panel labels).
    let menu:serde_json::Value=std::fs::read_to_string(config.output.join("retail_ui.json")).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default();
    if let Some(locale)=menu["localization"].as_object() {ui.locale=locale.iter().filter_map(|(k,v)|Some((k.clone(),v.as_str()?.trim().to_owned()))).collect();}
    if let (Some(atlas),Some(glyphs))=(menu["font"]["image"].as_str(),menu["font"]["glyphs"].as_object()) {
        // The crisp version of the atlas (clean white glyphs, mip levels, dark halo atlas); the plain asset if the file cannot be decoded.
        let crisp=std::fs::read(config.output.join(atlas)).ok().and_then(|bytes|crisp_atlas(&bytes));
        let (atlas,halo_atlas)=match crisp {Some((fill,halo))=>(images.add(fill),Some(images.add(halo))),None=>(ui.image(&assets,atlas),None)};
        let glyphs=glyphs.iter().filter_map(|(c,g)|{let r=g["rect"].as_array()?;let v=|i:usize|r[i].as_f64().unwrap_or(0.0) as f32;
            let (col,row)=((v(0)/CELL.0 as f32).floor(),(v(1)/CELL.1 as f32).floor());let (hw,hh)=(v(2)+2.0*HALO_PAD as f32,v(3)+2.0*HALO_PAD as f32);
            Some((c.chars().next()?,GlyphImage {image:atlas.clone(),rect:Some(Rect::new(v(0),v(1),v(0)+v(2),v(1)+v(3))),size:Vec2::new(v(2),v(3)),advance:g["advance"].as_f64().unwrap_or(32.0) as f32,
                halo:halo_atlas.as_ref().map(|_|Rect::new(col*hw,row*hh,col*hw+hw,row*hh+hh))}))}).collect();
        ui.fonts.insert("mincho",GlyphSet {glyphs,height:64.0,halo:halo_atlas});
    }
    // HUD ammo digits (misc\panel\ammo\fonts): 32x32 quads with a 16-pixel advance.
    let ammo=('0'..='9').map(|c|(c,format!("hud/{c}.png"))).chain([('.',"hud/kropka.png".to_owned())])
        .map(|(c,p)|(c,GlyphImage {image:ui.image(&assets,&p),rect:None,size:Vec2::splat(32.0),advance:16.0,halo:None})).collect();
    ui.fonts.insert("ammo",GlyphSet {glyphs:ammo,height:32.0,halo:None});
    ui.blob=images.add(blob_image());
    commands.insert_resource(ui);
}

/// Legibility aid of the retail bitmap text (owner feedback: the subtitles and dialogue lines were hard to read). The retail colours, font, positions and
/// wording stay; the aid adds a crisp dark outline around every glyph (a second, dilated copy of the atlas drawn underneath) and, where a text asks
/// for it, a faint soft dark strip behind the line. One switch (`TextAid`, display page "Olvasható szöveg") turns all of it off.
#[derive(Resource,Clone,Copy,PartialEq)] pub struct TextAid(pub bool);
impl Default for TextAid {fn default()->Self {Self(true)}}
/// Which parts of the aid a text uses. `backing` is the strip's peak opacity (0 = none).
#[derive(Clone,Copy,PartialEq,Default,Debug)] pub struct Aid {pub outline:bool,pub backing:f32}
impl Aid {
    pub const OUTLINE:Aid=Aid {outline:true,backing:0.0};
    /// Outline plus a faint strip (bright backgrounds).
    pub const BACKED:Aid=Aid {outline:true,backing:0.5};
    /// Only the strip (the retail drop shadow copy of a dialogue line already is the outline).
    pub const STRIP:Aid=Aid {outline:false,backing:0.5};
}
/// Copies the display-page switch into `TextAid`; `MESTER_TEXT_AID=0` forces it off (screenshot comparisons).
pub fn aid_sync(session:Res<crate::settings::Session>,mut aid:ResMut<TextAid>,mut forced:Local<Option<Option<bool>>>) {
    let forced=*forced.get_or_insert_with(||std::env::var("MESTER_TEXT_AID").ok().map(|v|v!="0"));
    let want=forced.unwrap_or(session.preferences.text_aid);
    if aid.0!=want {aid.0=want;}
}
/// Glyph cell of the Mincho atlas and the halo padding / radius in atlas pixels (about 1 to 1.5 screen pixels at the dialogue and subtitle sizes).
const CELL:(u32,u32)=(32,64);
const HALO_PAD:u32=4;
const HALO_RADIUS:i32=3;
const MIPS:usize=4;
/// A four-level mip chain of an RGBA image whose colour is constant: only the alpha plane is filtered (2x2 box).
fn mipped(width:u32,height:u32,rgb:[u8;3],alpha:&[u8])->Image {
    let mut data=Vec::new();let (mut w,mut h)=(width as usize,height as usize);let mut plane=alpha.to_vec();
    for level in 0..MIPS {
        for a in &plane {data.extend_from_slice(&[rgb[0],rgb[1],rgb[2],*a]);}
        if level+1==MIPS {break;}
        let (nw,nh)=((w/2).max(1),(h/2).max(1));let mut next=vec![0u8;nw*nh];
        for y in 0..nh {for x in 0..nw {
            let mut sum=0u32;for (dx,dy) in [(0,0),(1,0),(0,1),(1,1)] {sum+=plane[((y*2+dy).min(h-1))*w+(x*2+dx).min(w-1)] as u32;}
            next[y*nw+x]=((sum+2)/4) as u8;
        }}
        plane=next;w=nw;h=nh;
    }
    let mut image=Image::new_fill(Extent3d {width,height,depth_or_array_layers:1},TextureDimension::D2,&[0,0,0,0],TextureFormat::Rgba8UnormSrgb,RenderAssetUsages::default());
    image.texture_descriptor.mip_level_count=MIPS as u32;image.data=Some(data);
    image.sampler=ImageSampler::Descriptor(ImageSamplerDescriptor {mag_filter:ImageFilterMode::Linear,min_filter:ImageFilterMode::Linear,mipmap_filter:ImageFilterMode::Linear,
        address_mode_u:ImageAddressMode::ClampToEdge,address_mode_v:ImageAddressMode::ClampToEdge,..default()});
    image
}
/// The Mincho atlas as clean white glyphs with mip levels (the source has grey colour bleeding under transparent pixels, which showed as dark
/// fringes when it is scaled down to 0.3x), and the halo atlas: every cell dilated by `HALO_RADIUS` inside a cell padded by `HALO_PAD`.
pub fn crisp_atlas(png:&[u8])->Option<(Image,Image)> {
    let source=Image::from_buffer(png,ImageType::Extension("png"),CompressedImageFormats::NONE,true,ImageSampler::Default,RenderAssetUsages::default()).ok()?;
    let (w,h)=(source.width(),source.height());let data=source.data.as_ref()?;
    if data.len()!=(w*h*4) as usize || w%CELL.0!=0 || h%CELL.1!=0 {return None;}
    let alpha:Vec<u8>=data.chunks_exact(4).map(|p|p[3]).collect();
    let (cols,rows)=(w/CELL.0,h/CELL.1);let (cw,ch)=(CELL.0+2*HALO_PAD,CELL.1+2*HALO_PAD);
    let mut halo=vec![0u8;(cols*cw*rows*ch) as usize];
    let offsets:Vec<(i32,i32)>=(-HALO_RADIUS..=HALO_RADIUS).flat_map(|dy|(-HALO_RADIUS..=HALO_RADIUS).map(move|dx|(dx,dy))).filter(|(dx,dy)|dx*dx+dy*dy<=HALO_RADIUS*HALO_RADIUS+1).collect();
    for row in 0..rows {for col in 0..cols {
        for oy in 0..ch as i32 {for ox in 0..cw as i32 {
            let (sx,sy)=(ox-HALO_PAD as i32,oy-HALO_PAD as i32);let mut best=0u8;
            for (dx,dy) in &offsets {
                let (x,y)=(sx+dx,sy+dy);
                if x<0 || y<0 || x>=CELL.0 as i32 || y>=CELL.1 as i32 {continue;}
                best=best.max(alpha[((row*CELL.1) as i32+y) as usize*w as usize+((col*CELL.0) as i32+x) as usize]);
            }
            halo[((row*ch) as i32+oy) as usize*(cols*cw) as usize+((col*cw) as i32+ox) as usize]=best;
        }}
    }}
    Some((mipped(w,h,[255,255,255],&alpha),mipped(cols*cw,rows*ch,[0,0,0],&halo)))
}
const BLOB_BORDER:f32=10.0;
/// 64x64 soft blob: opaque in the middle, fading to nothing over a 10 pixel border (nine-sliced behind a text line).
pub fn blob_image()->Image {
    let n=64usize;let border=BLOB_BORDER;
    let alpha:Vec<u8>=(0..n*n).map(|i|{
        let (x,y)=((i%n) as f32+0.5,(i/n) as f32+0.5);let edge=x.min(n as f32-x).min(y.min(n as f32-y));
        let t=(edge/border).clamp(0.0,1.0);(t*t*(3.0-2.0*t)*255.0).round() as u8
    }).collect();
    let mut image=mipped(n as u32,n as u32,[0,0,0],&alpha);
    // one level only: a nine-slice never needs a mip chain
    image.texture_descriptor.mip_level_count=1;if let Some(d)=&mut image.data {d.truncate(n*n*4);}
    image
}
/// Padding of the strip around a line of `height` pixels: (horizontal, vertical).
pub fn strip_pad(height:f32)->(f32,f32) {(height*0.5,height*0.35)}

/// Text drawn with a retail bitmap font; one image per glyph like the original blitter.
#[derive(Component,Clone)]
pub struct BitmapText {pub font:&'static str,pub text:String,pub color:Color,pub scale:f32,pub aid:Aid,/// Line pitch of a `\n` separated text in glyph heights.
    pub pitch:f32,shown:Option<(String,Color,f32,Aid,f32,bool)>}
impl BitmapText {
    pub fn new(font:&'static str,text:impl Into<String>,scale:f32,color:Color)->Self {Self {font,text:text.into(),color,scale,aid:Aid::default(),pitch:1.0,shown:None}}
    pub fn aided(mut self,aid:Aid)->Self {self.aid=aid;self}
    pub fn set(&mut self,text:impl Into<String>) {let text=text.into();if self.text!=text {self.text=text;}}
}
/// A positioned bitmap-text node (top-left anchored, like the retail text calls).
pub fn label(font:&'static str,text:impl Into<String>,scale:f32,color:Color,x:f32,y:f32)->impl Bundle {
    (BitmapText::new(font,text,scale,color),Node {position_type:PositionType::Absolute,left:px(x),top:px(y),flex_direction:FlexDirection::Row,..default()})
}
/// A `label` with the legibility aid.
pub fn aided_label(font:&'static str,text:impl Into<String>,scale:f32,color:Color,x:f32,y:f32,aid:Aid)->impl Bundle {
    (BitmapText::new(font,text,scale,color).aided(aid),Node {position_type:PositionType::Absolute,left:px(x),top:px(y),flex_direction:FlexDirection::Row,..default()})
}

/// The shipped PCX glyph sets have no accented letters; the base letter stands in.
pub fn fold(c:char)->char {
    match c {'á'=>'a','é'=>'e','í'=>'i','ó'|'ö'|'ő'=>'o','ú'|'ü'|'ű'=>'u','Á'=>'A','É'=>'E','Í'=>'I','Ó'|'Ö'|'Ő'=>'O','Ú'|'Ü'|'Ű'=>'U',_=>c}
}
/// Greedy word wrap for a fixed-advance font.
pub fn wrap(text:&str,max_chars:usize)->Vec<String> {
    let mut lines=Vec::new();let mut line=String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count()+1+word.chars().count()>max_chars {lines.push(std::mem::take(&mut line));}
        if !line.is_empty() {line.push(' ');}line.push_str(word);
    }
    if !line.is_empty() {lines.push(line);}lines
}
pub fn draw_text(mut commands:Commands,ui:Res<RetailUi>,aid:Res<TextAid>,mut texts:Query<(Entity,&mut BitmapText,Option<&Children>)>) {
    for (entity,mut text,children) in &mut texts {
        // Compared in place: cloning every text's string every frame was a per-frame allocation for nothing.
        if text.shown.as_ref().is_some_and(|(t,c,s,a,p,on)|*t==text.text && *c==text.color && *s==text.scale && *a==text.aid && *p==text.pitch && *on==aid.0) {continue;}
        let key=(text.text.clone(),text.color,text.scale,text.aid,text.pitch,aid.0);
        if let Some(children)=children {for child in children.iter() {commands.entity(child).despawn();}}
        text.shown=Some(key);
        let Some(font)=ui.fonts.get(text.font) else {continue};
        let (scale,color,style)=(text.scale,text.color,if aid.0 {text.aid}else{Aid::default()});
        let (pitch,line_height)=(font.height*scale*text.pitch,font.height*scale);
        let halo_color=Color::srgba(0.0,0.0,0.0,0.9*color.alpha());
        commands.entity(entity).with_children(|root| {
            for (row_index,line) in text.text.split('\n').enumerate() {
                let top=row_index as f32*pitch;
                if !line.trim().is_empty() {
                    if style.backing>0.0 {
                        let (px_pad,py_pad)=strip_pad(line_height);
                        root.spawn((ImageNode {image:ui.blob.clone(),color:Color::srgba(0.0,0.0,0.0,style.backing*color.alpha()),image_mode:NodeImageMode::Sliced(TextureSlicer {border:BorderRect::all(BLOB_BORDER),center_scale_mode:SliceScaleMode::Stretch,sides_scale_mode:SliceScaleMode::Stretch,max_corner_scale:1.0}),..default()},
                            Node {position_type:PositionType::Absolute,left:px(-px_pad),top:px(top-py_pad),width:px(font.width(line,scale)+2.0*px_pad),height:px(line_height+2.0*py_pad),..default()}));
                    }
                    if let (true,Some(halo))=(style.outline,&font.halo) {
                        root.spawn(Node {position_type:PositionType::Absolute,left:px(0),top:px(top),flex_direction:FlexDirection::Row,..default()}).with_children(|row| {
                            for c in line.chars() {
                                let Some(g)=font.get(c) else {row.spawn(Node {width:px(font.height*0.3*scale),height:px(line_height),flex_shrink:0.0,..default()});continue};
                                row.spawn(Node {width:px(g.advance*scale),height:px(g.size.y*scale),flex_shrink:0.0,..default()}).with_children(|cell| {
                                    if let Some(rect)=g.halo {
                                        let pad=HALO_PAD as f32*scale;
                                        cell.spawn((ImageNode {image:halo.clone(),rect:Some(rect),color:halo_color,..default()},Node {position_type:PositionType::Absolute,left:px(-pad),top:px(-pad),width:px(rect.width()*scale),height:px(rect.height()*scale),..default()}));
                                    }
                                });
                            }
                        });
                    }
                }
                let glyphs=|row:&mut ChildSpawnerCommands| for c in line.chars() {
                    match font.get(c) {
                        Some(g)=>{row.spawn(Node {width:px(g.advance*scale),height:px(g.size.y*scale),flex_shrink:0.0,..default()}).with_children(|cell| {
                            cell.spawn((ImageNode {image:g.image.clone(),rect:g.rect,color,..default()},Node {position_type:PositionType::Absolute,width:px(g.size.x*scale),height:px(g.size.y*scale),..default()}));
                        });},
                        None=>{row.spawn(Node {width:px(font.height*0.3*scale),height:px(font.height*scale),flex_shrink:0.0,..default()});},
                    }
                };
                // The first line stays in the flow (the node is as wide as its text); further lines are absolute rows below it.
                if row_index==0 {glyphs(root);}
                else {root.spawn(Node {position_type:PositionType::Absolute,left:px(0),top:px(top),flex_direction:FlexDirection::Row,..default()}).with_children(|row|glyphs(row));}
            }
        });
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn the_crisp_atlas_has_mips_a_halo_larger_than_the_glyph_and_the_blob_fades_out() {
        let mut png=Vec::new();
        {let img=image_crate_free_png();png.extend(img);}
        let (fill,halo)=crisp_atlas(&png).expect("atlas");
        assert_eq!((fill.width(),fill.texture_descriptor.mip_level_count),(64,MIPS as u32));
        assert_eq!(halo.width(),2*(CELL.0+2*HALO_PAD));
        // a single opaque pixel at (10,10) of cell 0 spreads over its neighbours in the halo but not into the fill
        let a=|img:&Image,x:usize,y:usize,w:usize|img.data.as_ref().unwrap()[(y*w+x)*4+3];
        assert_eq!(a(&fill,10,10,64),255);assert_eq!(a(&fill,11,10,64),0);
        let hw=halo.width() as usize;assert_eq!(a(&halo,10+HALO_PAD as usize+2,10+HALO_PAD as usize,hw),255);
        let blob=blob_image();let n=64;let d=blob.data.as_ref().unwrap();
        assert!(d[3]<8);assert_eq!(d[(32*n+32)*4+3],255);
    }
    /// A 64x64 RGBA png with one opaque pixel at (10,10) (stored uncompressed, hand-made so the test needs no encoder).
    fn image_crate_free_png()->Vec<u8> {
        fn crc(data:&[u8])->u32 {let mut c=!0u32;for b in data {c^=*b as u32;for _ in 0..8 {c=if c&1!=0 {0xedb88320^(c>>1)}else{c>>1};}}!c}
        fn adler(data:&[u8])->u32 {let (mut a,mut b)=(1u32,0u32);for d in data {a=(a+*d as u32)%65521;b=(b+a)%65521;}(b<<16)|a}
        fn chunk(out:&mut Vec<u8>,kind:&[u8;4],data:&[u8]) {out.extend((data.len() as u32).to_be_bytes());let mut body=kind.to_vec();body.extend(data);out.extend(&body);out.extend(crc(&body).to_be_bytes());}
        let (w,h)=(64usize,64usize);let mut raw=Vec::new();
        for y in 0..h {raw.push(0u8);for x in 0..w {raw.extend(if (x,y)==(10,10) {[255,255,255,255]}else{[0,0,0,0]});}}
        let mut z=vec![0x78,0x01];
        for (i,block) in raw.chunks(65535).enumerate() {let last=(i+1)*65535>=raw.len();z.push(last as u8);z.extend((block.len() as u16).to_le_bytes());z.extend((!(block.len() as u16)).to_le_bytes());z.extend(block);}
        z.extend(adler(&raw).to_be_bytes());
        let mut out=vec![0x89,b'P',b'N',b'G',13,10,26,10];
        let mut head=Vec::new();head.extend(64u32.to_be_bytes());head.extend(64u32.to_be_bytes());head.extend([8,6,0,0,0]);
        chunk(&mut out,b"IHDR",&head);chunk(&mut out,b"IDAT",&z);chunk(&mut out,b"IEND",&[]);out
    }
    #[test] fn wrap_keeps_words_whole() {
        assert_eq!(wrap("Újabb szintet értél el! A pontok",14),vec!["Újabb szintet","értél el! A","pontok"]);
        assert_eq!(fold('ő'),'o');assert_eq!(fold('Ű'),'U');
    }
    #[test] #[ignore] fn exported_catalog_has_every_item_icon_and_the_panel_art() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output");
        let catalog=Catalog::load(&root);
        assert!(catalog.items.len()>=36);
        for (name,item) in catalog.items.iter().filter(|(_,d)|d.icon.is_some()) {assert!(root.join(item.icon.as_ref().unwrap()).is_file(),"{name}");}
        for name in ["PanelInv1024","PanelChar1024","player_attrib_1024","InvPrzyciskL1024","InvPrzyciskP1024","player_attrib_button","cursor1024"] {assert!(root.join(&catalog.panel[name].keyed).is_file(),"{name}");}
        assert_eq!(catalog.text("IPBackpack"),"Hátizsák");assert_eq!(catalog.text("PIPNextLev"),"Következő szint");
        assert!((catalog.weight("an apple")+catalog.weight("Police nightstick")+catalog.weight("Glock")-1.28).abs()<1e-5);
    }
    /// Every non-weapon record of GYARI/scripts/items.txt (effects, weight, flags) against the exported catalogue the game reads.
    #[test] #[ignore] fn consumables_and_flags_match_items_txt() {
        let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let catalog=Catalog::load(&root.join("output"));
        let text=std::fs::read(root.join("../GYARI/scripts/items.txt")).unwrap().iter().map(|b|*b as char).collect::<String>();
        let mut checked=0;let mut name="";let mut fields:Vec<(&str,&str)>=Vec::new();
        let mut records:Vec<(&str,Vec<(&str,&str)>)>=Vec::new();
        for line in text.lines().map(str::trim) {
            if let Some(n)=line.strip_prefix("item ") {if !name.is_empty() {records.push((name,std::mem::take(&mut fields)));}name=n;}
            else if !line.is_empty() {let (k,v)=line.split_once(char::is_whitespace).unwrap_or((line,""));fields.push((k,v.trim()));}
        }
        records.push((name,fields));
        for (name,fields) in records {
            let Some(d)=catalog.items.get(name) else {continue};
            let get=|key:&str|fields.iter().find(|(k,_)|*k==key).and_then(|(_,v)|v.parse::<f32>().ok()).unwrap_or(0.0);
            let has=|key:&str|fields.iter().any(|(k,_)|*k==key);
            assert_eq!((d.health,d.alcohol,d.power_up,d.pain_killer,d.weight),(get("health"),get("alcohol"),get("power_up"),get("pain_killer"),get("weight")),"{name}");
            assert_eq!((d.eaten,d.fixed,d.weapon),(has("eaten"),has("nie_ruszaj"),has("weapon")),"{name}");
            checked+=1;
        }
        assert!(checked>=36,"only {checked} records compared");
    }
}
