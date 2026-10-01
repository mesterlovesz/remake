//! Save slots like the retail save manager `c_savemgr` (cshell.dll 0x1004b500..0x1004d9a0): `save\quick.sav` plus `save\save0.sav`..`save99.sav`,
//! each with a local time stamp, the player's health, the level and a 320x240 RGB565 thumbnail of the screen (0x1004b550 grabs it when the
//! menu opens). The retail file holds the whole LithTech server state; this one holds the remake's [`Checkpoint`](crate::campaign) as JSON.
use serde::{Deserialize,Serialize};
use std::{io::Write,path::{Path,PathBuf}};

pub const THUMB_W:usize=320;
pub const THUMB_H:usize=240;
/// The tag string the retail file starts with after its system time (`RatHunt save game.` + newline).
pub const MAGIC:&[u8]=b"RatHunt save game.\n";
pub const SLOTS:usize=100;

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct SaveTime {pub year:u16,pub month:u16,pub day:u16,pub hour:u16,pub minute:u16,pub second:u16}
#[cfg(windows)] mod local {
    #[repr(C)] #[derive(Default)] pub struct SystemTime {pub year:u16,pub month:u16,pub day_of_week:u16,pub day:u16,pub hour:u16,pub minute:u16,pub second:u16,pub milliseconds:u16}
    unsafe extern "system" {pub fn GetLocalTime(time:*mut SystemTime);}
}
impl SaveTime {
    /// The local clock, like the retail `GetLocalTime` call (0x1004b8b8).
    pub fn now()->Self {
        #[cfg(windows)] {
            let mut time=local::SystemTime::default();
            unsafe {local::GetLocalTime(&mut time);}
            Self {year:time.year,month:time.month,day:time.day,hour:time.hour,minute:time.minute,second:time.second}
        }
        #[cfg(not(windows))] {Self::from_system_time(std::time::SystemTime::now())}
    }
    /// UTC calendar time of a file time stamp (days-from-civil, no leap seconds).
    pub fn from_system_time(time:std::time::SystemTime)->Self {
        let seconds=time.duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_secs());
        let days=(seconds/86400) as i64;let rest=seconds%86400;
        let z=days+719468;let era=z.div_euclid(146097);let doe=z.rem_euclid(146097);let yoe=(doe-doe/1460+doe/36524-doe/146096)/365;
        let year=yoe+era*400;let doy=doe-(365*yoe+yoe/4-yoe/100);let mp=(5*doy+2)/153;let day=doy-(153*mp+2)/5+1;let month=if mp<10 {mp+3}else{mp-9};
        Self {year:(year+(month<=2) as i64) as u16,month:month as u16,day:day as u16,hour:(rest/3600) as u16,minute:(rest%3600/60) as u16,second:(rest%60) as u16}
    }
}
#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
pub struct Header {pub time:SaveTime,pub world:String,pub title:String,pub health:f32}
#[derive(Clone,Debug,Default)]
pub struct SaveFile {pub header:Header,pub payload:serde_json::Value,pub thumbnail:Option<Vec<u8>>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind {Quick,Numbered(u8),/// `manual_save.json` of the earlier remake, kept loadable.
    Legacy}
#[derive(Clone,Debug)]
pub struct Slot {pub path:PathBuf,pub kind:Kind,pub header:Header}

fn u32_at(bytes:&[u8],at:usize)->Result<usize,String> {bytes.get(at..at+4).map(|b|u32::from_le_bytes([b[0],b[1],b[2],b[3]]) as usize).ok_or_else(||"csonka mentés".to_owned())}
pub fn encode(file:&SaveFile)->Result<Vec<u8>,String> {
    let header=serde_json::to_vec(&file.header).map_err(|e|e.to_string())?;let payload=serde_json::to_vec(&file.payload).map_err(|e|e.to_string())?;
    let mut out=Vec::with_capacity(MAGIC.len()+8+header.len()+payload.len()+THUMB_W*THUMB_H*2);
    out.extend_from_slice(MAGIC);out.extend_from_slice(&(header.len() as u32).to_le_bytes());out.extend_from_slice(&header);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());out.extend_from_slice(&payload);
    if let Some(thumbnail) = file.thumbnail.as_ref().filter(|t|t.len()==THUMB_W*THUMB_H*2) {out.extend_from_slice(thumbnail);}
    Ok(out)
}
/// Parses a save. `with_payload` = false stops after the header and the thumbnail (what the load list needs).
pub fn decode(bytes:&[u8],with_payload:bool)->Result<SaveFile,String> {
    if !bytes.starts_with(MAGIC) {return Err("nem mentésfájl".into());}
    let mut at=MAGIC.len();let header_len=u32_at(bytes,at)?;at+=4;
    let header:Header=serde_json::from_slice(bytes.get(at..at+header_len).ok_or("csonka mentés")?).map_err(|e|format!("sérült fejléc: {e}"))?;at+=header_len;
    let payload_len=u32_at(bytes,at)?;at+=4;let payload_at=at;at+=payload_len;
    if at>bytes.len() {return Err("csonka mentés".into());}
    let payload=if with_payload {serde_json::from_slice(&bytes[payload_at..at]).map_err(|e|format!("sérült mentés: {e}"))?}else{serde_json::Value::Null};
    let thumbnail=(bytes.len()-at==THUMB_W*THUMB_H*2).then(||bytes[at..].to_vec());
    Ok(SaveFile {header,payload,thumbnail})
}
/// Writes through a temporary file so an interrupted save never leaves a half-written slot.
pub fn write(path:&Path,file:&SaveFile)->Result<(),String> {
    let bytes=encode(file)?;
    if let Some(dir)=path.parent() {std::fs::create_dir_all(dir).map_err(|e|e.to_string())?;}
    let temp=path.with_extension("tmp");
    let mut out=std::fs::File::create(&temp).map_err(|e|e.to_string())?;out.write_all(&bytes).and_then(|_|out.sync_all()).map_err(|e|e.to_string())?;drop(out);
    std::fs::rename(&temp,path).map_err(|e|e.to_string())
}
pub fn read(path:&Path,with_payload:bool)->Result<SaveFile,String> {
    let bytes=std::fs::read(path).map_err(|e|match e.kind() {std::io::ErrorKind::NotFound=>"nincs ilyen mentés".to_owned(),_=>e.to_string()})?;
    if path.extension().is_some_and(|e|e=="json") {return read_legacy(path,&bytes);}
    decode(&bytes,with_payload)
}
/// `manual_save.json` was the bare checkpoint; its level and health are enough for a list entry.
fn read_legacy(path:&Path,bytes:&[u8])->Result<SaveFile,String> {
    let payload:serde_json::Value=serde_json::from_slice(bytes).map_err(|e|format!("sérült mentés: {e}"))?;
    let world=payload["world"].as_str().ok_or("sérült mentés")?.to_owned();
    let time=std::fs::metadata(path).and_then(|m|m.modified()).map(SaveTime::from_system_time).unwrap_or_default();
    Ok(SaveFile {header:Header {time,title:crate::travel::level_title(&world).to_owned(),health:payload["health"].as_f64().unwrap_or(0.0) as f32,world},payload,thumbnail:None})
}
/// The slot list in the retail order: quick.sav first, then save99 down to save0 (each new node became the head of the list, 0x1004d5e0).
pub fn list(dir:&Path)->Vec<Slot> {
    let mut slots=Vec::new();
    let mut add=|kind:Kind,path:PathBuf|{if let Ok(file)=read(&path,false) {slots.push(Slot {path,kind,header:file.header});}};
    add(Kind::Quick,dir.join("quick.sav"));
    for n in (0..SLOTS as u8).rev() {add(Kind::Numbered(n),dir.join(format!("save{n}.sav")));}
    if let Some(root)=dir.parent() {let legacy=root.join("manual_save.json");if legacy.is_file() {add(Kind::Legacy,legacy);}}
    slots
}
/// The file name the "new save" entry writes to: the lowest free `saveN.sav`, `save99.sav` when all are taken (0x1004d7d0).
pub fn next_free(dir:&Path)->PathBuf {
    (0..SLOTS-1).map(|n|dir.join(format!("save{n}.sav"))).find(|p|!p.exists()).unwrap_or_else(||dir.join("save99.sav"))
}
/// "Mentve: 9-19, 2003 at: 15:18": the created line of the details box (0x1004d04b..0x1004d155; numbers are not zero padded).
pub fn created_line(created:&str,at:&str,time:&SaveTime)->String {format!("{created} {}-{}, {} {at} {}:{}",time.month,time.day,time.year,time.hour,time.minute)}
/// "Játékos egészsége: 100.00" (0x1004d48d, the health is appended with two decimals).
pub fn health_line(label:&str,health:f32)->String {format!("{label} {health:.2}")}

/// Scales a screenshot (4 bytes per pixel, `bgra` for the swap chain order) to the 320x240 RGB565 thumbnail with a box filter.
pub fn thumbnail_from_pixels(width:usize,height:usize,pixels:&[u8],bgra:bool)->Option<Vec<u8>> {
    if width==0 || height==0 || pixels.len()<width*height*4 {return None;}
    let mut out=Vec::with_capacity(THUMB_W*THUMB_H*2);
    for ty in 0..THUMB_H {
        let (y0,y1)=(ty*height/THUMB_H,((ty+1)*height/THUMB_H).max(ty*height/THUMB_H+1).min(height));
        for tx in 0..THUMB_W {
            let (x0,x1)=(tx*width/THUMB_W,((tx+1)*width/THUMB_W).max(tx*width/THUMB_W+1).min(width));
            let (mut r,mut g,mut b,mut n)=(0u32,0u32,0u32,0u32);
            for y in y0..y1 {for x in x0..x1 {let p=(y*width+x)*4;let (pr,pb)=if bgra {(pixels[p+2],pixels[p])}else{(pixels[p],pixels[p+2])};r+=pr as u32;g+=pixels[p+1] as u32;b+=pb as u32;n+=1;}}
            let (r,g,b)=((r/n) as u16,(g/n) as u16,(b/n) as u16);
            out.extend_from_slice(&(((r>>3)<<11)|((g>>2)<<5)|(b>>3)).to_le_bytes());
        }
    }
    Some(out)
}
/// RGBA8 of an RGB565 thumbnail, for the menu picture.
pub fn rgba_from_thumbnail(thumbnail:&[u8])->Vec<u8> {
    thumbnail.chunks_exact(2).flat_map(|c|{let v=u16::from_le_bytes([c[0],c[1]]);let (r,g,b)=((v>>11) as u8,((v>>5)&63) as u8,(v&31) as u8);[(r<<3)|(r>>2),(g<<2)|(g>>4),(b<<3)|(b>>2),255]}).collect()
}

#[cfg(test)] mod tests {
    use super::*;
    fn temp(name:&str)->PathBuf {let dir=std::env::temp_dir().join(format!("mester-{name}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();dir}
    fn file(world:&str,health:f32)->SaveFile {SaveFile {header:Header {time:SaveTime {year:2026,month:9,day:29,hour:8,minute:5,second:1},world:world.into(),title:"Előszó".into(),health},payload:serde_json::json!({"world":world,"health":health,"nested":{"doors":[1,2,3]}}),thumbnail:Some(vec![7;THUMB_W*THUMB_H*2])}}
    #[test] fn a_save_round_trips_with_thumbnail_and_the_header_reads_alone() {
        let dir=temp("save-roundtrip");let path=dir.join("save").join("save3.sav");
        write(&path,&file("rh3-miasteczko0",87.5)).unwrap();
        let back=read(&path,true).unwrap();
        assert_eq!(back.header.world,"rh3-miasteczko0");assert_eq!(back.header.health,87.5);assert_eq!(back.payload["nested"]["doors"][2],3);assert_eq!(back.thumbnail.as_ref().map(Vec::len),Some(THUMB_W*THUMB_H*2));
        let header_only=read(&path,false).unwrap();assert!(header_only.payload.is_null() && header_only.thumbnail.is_some());
        assert!(!path.with_extension("tmp").exists(),"the temporary file is renamed away");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn the_list_follows_the_retail_order_and_skips_damaged_or_foreign_files() {
        let dir=temp("save-list");let saves=dir.join("save");
        for n in [0u8,2,5] {write(&saves.join(format!("save{n}.sav")),&file("rh1-wiezienie2",50.0+n as f32)).unwrap();}
        write(&saves.join("quick.sav"),&file("burmistrz1",100.0)).unwrap();
        std::fs::write(saves.join("save7.sav"),b"garbage").unwrap();std::fs::write(saves.join("save9.sav"),&MAGIC[..10]).unwrap();
        let mut truncated=encode(&file("x",1.0)).unwrap();truncated.truncate(MAGIC.len()+6);std::fs::write(saves.join("save8.sav"),truncated).unwrap();
        std::fs::write(dir.join("manual_save.json"),br#"{"world":"knajpa","health":42.0}"#).unwrap();
        let slots=list(&saves);
        let order:Vec<Kind>=slots.iter().map(|s|s.kind).collect();
        assert_eq!(order,vec![Kind::Quick,Kind::Numbered(5),Kind::Numbered(2),Kind::Numbered(0),Kind::Legacy]);
        let legacy=slots.last().unwrap();assert_eq!((legacy.header.world.as_str(),legacy.header.health),("knajpa",42.0));
        assert_eq!(next_free(&saves).file_name().unwrap(),"save1.sav");
        for n in 0..99 {std::fs::write(saves.join(format!("save{n}.sav")),b"x").unwrap();}
        assert_eq!(next_free(&saves).file_name().unwrap(),"save99.sav");
        assert!(read(&saves.join("save7.sav"),true).is_err() && read(&saves.join("nope.sav"),true).unwrap_err().contains("nincs"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test] fn detail_lines_are_formatted_like_the_retail_menu() {
        let time=SaveTime {year:2003,month:9,day:19,hour:15,minute:18,second:21};
        assert_eq!(created_line("Mentve:","at:",&time),"Mentve: 9-19, 2003 at: 15:18");assert_eq!(health_line("Játékos egészsége:",100.0),"Játékos egészsége: 100.00");
        // The sample quick.sav of the retail install starts with this system time.
        assert_eq!(SaveTime::from_system_time(std::time::UNIX_EPOCH+std::time::Duration::from_secs(1_063_984_701)),SaveTime {year:2003,month:9,day:19,hour:15,minute:18,second:21});
    }
    #[test] fn screenshots_scale_to_the_rgb565_thumbnail_and_back() {
        // A 640x480 image: left half red, right half blue (RGBA), then BGRA order.
        let mut pixels=vec![0u8;640*480*4];
        for y in 0..480 {for x in 0..640 {let p=(y*640+x)*4;if x<320 {pixels[p]=255;}else{pixels[p+2]=255;}pixels[p+3]=255;}}
        let thumb=thumbnail_from_pixels(640,480,&pixels,false).unwrap();assert_eq!(thumb.len(),THUMB_W*THUMB_H*2);
        let rgba=rgba_from_thumbnail(&thumb);assert_eq!(&rgba[..4],&[255,0,0,255]);assert_eq!(&rgba[(THUMB_W-1)*4..(THUMB_W-1)*4+4],&[0,0,255,255]);
        let bgra=thumbnail_from_pixels(640,480,&pixels,true).unwrap();assert_eq!(&rgba_from_thumbnail(&bgra)[..4],&[0,0,255,255]);
        assert!(thumbnail_from_pixels(0,0,&[],false).is_none() && thumbnail_from_pixels(10,10,&[0;8],false).is_none());
    }
}
