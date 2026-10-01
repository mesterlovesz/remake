//! Route planning over the static collision hull for the automated walk-through (`level-viewer` walk_probe, tests/reach_levels.rs):
//! which standing positions a player can reach from the start with the controller's own step, stair, jump and crouch rules.
//! An approximation of the player controller (player.rs), not part of it: steps are checked with box sweeps on a 24-unit grid.
use crate::{CollisionWorld,Vec3,GROUND_NORMAL,STAIR_HEIGHT};

const SKIN: f32 = 0.1;

impl CollisionWorld {
    /// Whether a standing box (`half`) at `position` can walk the horizontal `delta` the way the controller does (level, an up-to-18 stair
    /// or a walkable ramp, or a drop of up to `max_drop`) or, with `max_climb` above the 18-unit stair, jump onto a ledge (a jump rises 45).
    /// Returns where the centre ends up. The automated walk-through planner searches with this.
    pub fn walk_step(&self, position:Vec3, half:Vec3, delta:Vec3, max_drop:f32, max_climb:f32) -> Option<Vec3> {
        let horizontal=Vec3::new(delta.x,0.0,delta.z);
        for rise in [0.0f32,STAIR_HEIGHT,30.0,45.0,max_climb] {
            if rise>max_climb.max(30.0) { continue; }
            if rise>0.0 && self.sweep(position,half,Vec3::Y*rise,SKIN).is_some() { break; }
            let lifted=position+Vec3::Y*rise;
            if self.sweep(lifted,half,horizontal,SKIN).is_some() { continue; }
            let moved=lifted+horizontal;
            let drop=Vec3::NEG_Y*(rise+max_drop);
            let Some(hit)=self.sweep(moved,half,drop,SKIN) else { continue };
            if hit.normal.y<GROUND_NORMAL { continue; }
            let landed=moved+drop*hit.time;
            let gain=landed.y-position.y;
            // Only a slanted floor may rise more than one stair without a jump; nothing rises more than a jump.
            if gain< -max_drop || gain>max_climb+0.5 || (gain>18.5 && gain>18.5 && hit.normal.y>0.98 && max_climb<=STAIR_HEIGHT) { continue; }
            return Some(landed);
        }
        None
    }
    /// A running jump across a gap. The jump peaks 45 units up after 0.3 s and the body then falls to the landing floor (gravity 1000);
    /// at a running speed of 270 the flight covers `270 * (0.3 + sqrt(2 (45 - rise) / 1000))`. The box is swept along the raised arc
    /// (lift, then across) and dropped onto a floor at most 45 above or 320 below where it left.
    pub fn jump_step(&self, position:Vec3, half:Vec3, direction:Vec3, distance:f32) -> Option<Vec3> {
        let lift=Vec3::Y*40.0;
        if self.sweep(position,half,lift,SKIN).is_some() { return None; }
        let peak=position+lift;let across=Vec3::new(direction.x,0.0,direction.z)*distance;
        if self.sweep(peak,half,across,SKIN).is_some() { return None; }
        let moved=peak+across;let drop=Vec3::NEG_Y*(40.0+320.0);
        let hit=self.sweep(moved,half,drop,SKIN)?;
        if hit.normal.y<GROUND_NORMAL { return None; }
        let landed=moved+drop*hit.time;
        let rise=landed.y-position.y;
        if rise>45.0 { return None; }
        let flight=0.3+(2.0*(45.0-rise).max(0.0)/1000.0).sqrt();
        (distance<=270.0*flight).then_some(landed)
    }
    /// A* over positions on a 24-unit grid where every step is legal under `walk_step`, standing or crouched (a crouched box is 24 half
    /// wide and 24 high: vents and ducts). Returns waypoints `(centre, crouched, reached by a jump)` thinned to one per 72 units, whether `goal` itself was
    /// reached (within 40 horizontally) and how many cells were expanded; when the goal is unreachable the route ends at the reachable
    /// cell closest to it. `portals` are door leaves' two sides; `cells` receives every visited centre when asked.
    pub fn plan_walk(&self, start:Vec3, goal:Vec3, limit:usize, portals:&[(Vec3,Vec3)], cells:Option<&mut Vec<Vec3>>) -> (Vec<(Vec3,bool,bool)>, bool, usize) {
        self.plan_walk_avoiding(start,goal,limit,portals,&[],cells)
    }
    /// `plan_walk` that also treats `avoid` boxes (centre, half extents: the solid props the level adds on top of the static hull) as walls.
    pub fn plan_walk_avoiding(&self, start:Vec3, goal:Vec3, limit:usize, portals:&[(Vec3,Vec3)], avoid:&[(Vec3,Vec3)], mut cells:Option<&mut Vec<Vec3>>) -> (Vec<(Vec3,bool,bool)>, bool, usize) {
        use std::{cmp::Reverse, collections::{BinaryHeap, HashMap}};
        const CELL:f32=24.0;
        type Key=(i32,i32,i32,bool);
        let key=|p:Vec3,crouch:bool|((p.x/CELL).round() as i32,(p.y/16.0).round() as i32,(p.z/CELL).round() as i32,crouch);
        let flat=|a:Vec3,b:Vec3|((a.x-b.x)*(a.x-b.x)+(a.z-b.z)*(a.z-b.z)).sqrt();
        // "Closest to an unreachable goal": horizontal distance plus twice the height difference beyond the 170 of the goal test (another floor is not close).
        let near=|a:Vec3,b:Vec3|flat(a,b)+((a.y-b.y).abs()-170.0).max(0.0)*2.0;
        let stand=Vec3::new(17.0,58.0,17.0);let crouch=Vec3::splat(24.0);
        let blocked=|p:Vec3,half:Vec3|avoid.iter().any(|(c,h)|(p-*c).abs().cmplt(half+*h).all());
        let mut best:HashMap<Key,(f32,Vec3,Key,bool)>=HashMap::new();
        let mut heap=BinaryHeap::new();
        let start_key=key(start,false);best.insert(start_key,(0.0,start,start_key,false));heap.push(Reverse(((start.distance(goal)*10.0) as u32,start_key)));
        let (mut closest,mut closest_distance,mut expanded)=(start_key,near(start,goal),0usize);
        let mut reached=None;
        while let Some(Reverse((_,at)))=heap.pop() {
            let (cost,position,..)=best[&at];
            let crouched=at.3;
            expanded+=1;
            if near(position,goal)<closest_distance { closest_distance=near(position,goal);closest=at; }
            if flat(position,goal)<40.0 && (position.y-goal.y).abs()<170.0 { reached=Some(at);break; }
            if expanded>limit { break; }
            let half=if crouched {crouch} else {stand};
            let mut ordinary=0;
            for i in 0..8 {
                let angle=i as f32*std::f32::consts::FRAC_PI_4;
                let delta=Vec3::new(angle.cos(),0.0,angle.sin())*CELL;
                let Some(next)=self.walk_step(position,half,delta,300.0,45.0) else { continue };
                if blocked(next,half) { continue; }
                ordinary+=1;
                // A fall is legal (the controller just falls) but costs extra so ordinary routes win; so does crawling.
                let g=cost+CELL*if i%2==1 {1.4142} else {1.0}*if crouched {1.6} else {1.0}+(position.y-next.y-40.0).max(0.0)*0.8+if next.y-position.y>18.5 {30.0} else {0.0};
                let k=key(next,crouched);let hop=next.y-position.y>18.5;
                if best.get(&k).is_none_or(|(old,..)|g<*old-0.5) { best.insert(k,(g,next,at,hop));heap.push(Reverse((((g+next.distance(goal)*1.3)*10.0) as u32,k))); }
            }
            // Running jumps over gaps (roof to roof, across a pit): only from the edge of the walkable region (few ordinary steps).
            if !crouched && ordinary<=5 {
                for i in 0..8 {
                    let angle=i as f32*std::f32::consts::FRAC_PI_4;let direction=Vec3::new(angle.cos(),0.0,angle.sin());
                    for distance in [72.0f32,108.0,144.0,192.0,240.0,288.0] {
                        let Some(next)=self.jump_step(position,half,direction,distance) else { continue };
                        if blocked(next,half) { continue; }
                        let g=cost+distance+70.0;let k=key(next,false);
                        if best.get(&k).is_none_or(|(old,..)|g<*old-0.5) { best.insert(k,(g,next,at,true));heap.push(Reverse((((g+next.distance(goal)*1.3)*10.0) as u32,k))); }
                    }
                }
            }
            // Change stance at the same feet: crouching always fits; standing up needs the standing box to be free.
            let feet=position.y-half.y;
            let (other_half,other_crouch)=if crouched {(stand,false)} else {(crouch,true)};
            let other=Vec3::new(position.x,feet+other_half.y,position.z);
            if (crouched && !self.box_overlaps(other,other_half)) || !crouched {
                let g=cost+8.0;let k=key(other,other_crouch);
                if best.get(&k).is_none_or(|(old,..)|g<*old-0.5) { best.insert(k,(g,other,at,false));heap.push(Reverse((((g+other.distance(goal)*1.3)*10.0) as u32,k))); }
            }
            // Door portals: a leaf is a movable brush, so the walker crosses it through its doorway; jump across when near either side.
            if !crouched {
                for &(a,b) in portals {
                    for (from,to) in [(a,b),(b,a)] {
                        if flat(position,from)<30.0 && (position.y-from.y).abs()<40.0 {
                            let g=cost+from.distance(to)+10.0;let k=key(to,false);
                            if best.get(&k).is_none_or(|(old,..)|g<*old-0.5) { best.insert(k,(g,to,at,false));heap.push(Reverse((((g+to.distance(goal)*1.3)*10.0) as u32,k))); }
                        }
                    }
                }
            }
        }
        if let Some(out)=cells.as_deref_mut() { out.extend(best.values().map(|(_,p,..)|*p)); }
        let end=reached.unwrap_or(closest);
        let mut path=vec![(best[&end].1,end.3,best[&end].3)];let mut at=end;
        while best[&at].2!=at { at=best[&at].2;path.push((best[&at].1,at.3,best[&at].3)); }
        path.reverse();
        // Keep a point every 72 units, every stance change, and both ends of every jump (the flag marks the step INTO a point).
        let mut waypoints=vec![path[0]];
        for i in 1..path.len() { let last=*waypoints.last().unwrap(); let jump_next=path.get(i+1).is_some_and(|n|n.2);
            if i+1==path.len() || path[i].2 || jump_next || last.1!=path[i].1 || path[i-1].1!=path[i].1 || last.0.distance(path[i].0)>=72.0 { waypoints.push(path[i]); } }
        if reached.is_some() { waypoints.push((goal,false,false)); }
        (waypoints,reached.is_some(),expanded)
    }
    /// The two standing points on either side of a door leaf with the given bounds (its thin horizontal axis is the doorway direction).
    pub fn portal_of(low:Vec3, high:Vec3) -> Option<(Vec3,Vec3)> {
        let size=high-low;
        if size.y<64.0 { return None; }
        let centre=(low+high)*0.5;let y=low.y+58.2;
        let (normal,reach)=if size.x<size.z {(Vec3::X,size.x*0.5+40.0)} else {(Vec3::Z,size.z*0.5+40.0)};
        let at=|sign:f32|{let p=centre+normal*reach*sign;Vec3::new(p.x,y,p.z)};
        Some((at(-1.0),at(1.0)))
    }
}
