//! Mining loop QA: a captain flies mining trips the way a player does — undock, fly to an asteroid,
//! mine until the hold is full, fly back, dock, sell (or refine) — and the report says per trip
//! where the time went (flying out, mining, flying back, fighting) and what it earned. Mining is
//! about 70% of campaign play time (docs/QA_CAMPAIGN.md), so this loop is most of the game.
//!
//! The pilot does not fight back: pirates that find it show up as combat time, hull lost and deaths
//! (cargo lost with the ship) — the risk of mining there.
//!
//! Deterministic like the rest of the simulation (`cargo run -p sim --release --example mining`).

use crate::game::{COMBAT_LOCK, ConnId, Game};
use crate::playthrough::{Activity, THINK, next_hop, steer};
use crate::store::Store;
use shared::galaxy::{self, Ore, SystemId};
use shared::minerals::{Rarity, mineral_in};
use shared::protocol::{ClientMsg, Controls, PROTOCOL_VERSION, Profession, TICK_HZ};
use shared::ships::{HullClass, Module};

/// Which asteroid of the ore to mine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Closest to the station (what the campaign pilot does).
    Nearest,
    /// The rarest deposit, closest first among equals.
    Rarest,
    /// The steepest climb or dive from the station (`height_bonus`).
    Steepest,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub hull: HullClass,
    pub profession: Profession,
    /// Every module at this level.
    pub level: u8,
    /// System to mine in (None: the captain's home).
    pub system: Option<SystemId>,
    /// Ore to mine (None: any).
    pub ore: Option<Ore>,
    pub pick: Pick,
    /// Refine the ore at the station instead of selling it.
    pub refine: bool,
    pub trips: u32,
    /// Give up after this much game time.
    pub max_secs: f32,
    pub unix: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { hull: HullClass::Talon, profession: Profession::CombatPilot, level: 1, system: None, ore: None, pick: Pick::Nearest, refine: false, trips: 3, max_secs: 3600.0, unix: 1_760_000_000 }
    }
}

/// One round trip, station to station.
#[derive(Clone, Debug, Default)]
pub struct Trip {
    /// Seconds flying to the asteroid, mining, flying back (docking included), fighting, dead.
    pub out: f32,
    pub mine: f32,
    pub back: f32,
    pub combat: f32,
    pub dead: f32,
    pub units: u32,
    /// Credits earned in all (ore, specimens, bounties, missions), from the ore and from specimens;
    /// refined material units (refining and loot), specimens found.
    pub credits: u64,
    pub ore_credits: u64,
    pub specimen_credits: u64,
    pub refined: u32,
    pub specimens: u32,
    pub deaths: u32,
    /// Times the captain ran for the station before the hold was full.
    pub fled: u32,
    /// Lowest hull share during the trip.
    pub low_hull: f32,
}

impl Trip {
    pub fn secs(&self) -> f32 {
        self.out + self.mine + self.back + self.combat + self.dead
    }
}

/// The asteroid mined and the trips flown.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub system: SystemId,
    pub ore: Option<Ore>,
    pub rarity: Option<Rarity>,
    /// Station → asteroid distance and climb (degrees above or below the station's plane).
    pub distance: f32,
    pub elevation: f32,
    /// Hold size, mining rate (units/s) and cruise speed of the ship.
    pub cargo: f32,
    pub mine_rate: f32,
    pub speed: f32,
    pub trips: Vec<Trip>,
    pub stuck: Option<String>,
}

impl Report {
    pub fn total(&self) -> Trip {
        let mut s = Trip { low_hull: 1.0, ..Default::default() };
        for t in &self.trips {
            s.out += t.out;
            s.mine += t.mine;
            s.back += t.back;
            s.combat += t.combat;
            s.dead += t.dead;
            s.units += t.units;
            s.credits += t.credits;
            s.ore_credits += t.ore_credits;
            s.specimen_credits += t.specimen_credits;
            s.refined += t.refined;
            s.specimens += t.specimens;
            s.deaths += t.deaths;
            s.fled += t.fled;
            s.low_hull = s.low_hull.min(t.low_hull);
        }
        s
    }

    /// Credits (or refined units) per minute of trip time.
    pub fn per_min(&self) -> (f32, f32) {
        let t = self.total();
        let m = t.secs().max(1.0) / 60.0;
        (t.credits as f32 / m, t.refined as f32 / m)
    }

    pub fn text(&self) -> String {
        let mut s = format!(
            "system {} ({:?}) · {} {} · {:.0} u from the station, {:+.0}° · hold {:.0}, {:.2} units/s (full in {:.0} s), speed {:.0}\n",
            self.system,
            galaxy::system(self.system).tier,
            self.ore.map_or("?", |o| o.name()),
            self.rarity.map_or("?", |r| r.label()),
            self.distance,
            self.elevation,
            self.cargo,
            self.mine_rate,
            self.cargo / self.mine_rate.max(1e-3),
            self.speed,
        );
        for (i, t) in self.trips.iter().enumerate() {
            s += &format!(
                "  trip {}: {:>4.0} s — out {:.0} · mine {:.0} · back {:.0} · combat {:.0} · dead {:.0} — {} units → {} CR (ore {}, specimens {}){}{}{}\n",
                i + 1,
                t.secs(),
                t.out,
                t.mine,
                t.back,
                t.combat,
                t.dead,
                t.units,
                t.credits,
                t.ore_credits,
                t.specimen_credits,
                if t.refined > 0 { format!(", {} refined", t.refined) } else { String::new() },
                if t.specimens > 0 { format!(", {} specimens", t.specimens) } else { String::new() },
                if t.fled > 0 { format!(", fled {}", t.fled) } else { String::new() } + &if t.deaths > 0 { format!(", {} deaths (hull low {:.0}%)", t.deaths, t.low_hull * 100.0) } else if t.low_hull < 1.0 { format!(", hull low {:.0}%", t.low_hull * 100.0) } else { String::new() },
            );
        }
        let t = self.total();
        let (cr, refined) = self.per_min();
        let secs = t.secs().max(1e-3);
        s += &format!(
            "  per minute: {:.0} CR (ore {:.0}, specimens {:.0}){} · mining {:.0}% of the trip, flying {:.0}%, combat {:.0}%\n",
            cr,
            t.ore_credits as f32 / (secs / 60.0),
            t.specimen_credits as f32 / (secs / 60.0),
            if refined > 0.0 { format!(", {refined:.1} refined") } else { String::new() },
            t.mine / secs * 100.0,
            (t.out + t.back) / secs * 100.0,
            (t.combat + t.dead) / secs * 100.0,
        );
        if let Some(why) = &self.stuck {
            s += &format!("  STOPPED: {why}\n");
        }
        s
    }
}

/// A player who died this often mining somewhere would stop going there.
const MAX_DEATHS: u32 = 5;

#[derive(Clone, Copy, PartialEq)]
enum Leg {
    Out,
    Mine,
    Back,
}

/// Flies `o.trips` mining trips and reports them.
pub fn run(o: &Options) -> Report {
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), o.unix);
    let conn: ConnId = 1;
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "채굴QA".into(), token: None });
    g.handle(conn, ClientMsg::CreateCharacter { species: galaxy::Species::Human, profession: o.profession, hull: o.hull, ship_name: "광부호".into(), livery: 0 });
    g.out.clear();
    let mut rep = Report::default();
    let Some(pid) = g.sessions.get(&conn).and_then(|s| s.player) else {
        rep.stuck = Some("character could not be created".into());
        return rep;
    };
    let sys = o.system.unwrap_or_else(|| g.store.profile(pid).map_or(0, |p| p.home));
    if let Some(p) = g.store.profile_mut(pid) {
        p.scenarios_completed = 5;
        for m in Module::ALL {
            p.ship.modules.insert(m, o.level.max(1));
        }
    }
    g.spawn_player(pid, sys, true);
    g.out.clear();
    rep.system = sys;

    let layout = g.systems[&sys].layout.clone();
    let station = layout.station;
    let elevation = |p: shared::glam::Vec3| {
        let d = p - station;
        (d.y / d.length().max(1e-3)).asin().to_degrees()
    };
    let rarity = |a: &galaxy::Asteroid| mineral_in(&g.tuning.minerals, a.mineral).map_or(Rarity::Common, |m| m.rarity);
    let candidates: Vec<&galaxy::Asteroid> = layout.asteroids.iter().filter(|a| o.ore.is_none_or(|ore| a.ore == ore)).collect();
    let near = |a: &&galaxy::Asteroid| a.pos.distance(station);
    let Some(a) = (match o.pick {
        Pick::Nearest => candidates.iter().min_by(|x, y| near(x).total_cmp(&near(y))),
        Pick::Rarest => candidates.iter().max_by(|x, y| (rarity(x) as u8).cmp(&(rarity(y) as u8)).then(near(y).total_cmp(&near(x)))),
        Pick::Steepest => candidates.iter().max_by(|x, y| elevation(x.pos).abs().total_cmp(&elevation(y.pos).abs())),
    })
    .map(|a| (*a).clone()) else {
        rep.stuck = Some(format!("no {} asteroid in system {sys}", o.ore.map_or("", |o| o.name())));
        return rep;
    };
    (rep.ore, rep.rarity, rep.distance, rep.elevation) = (Some(a.ore), Some(rarity(&a)), a.pos.distance(station), elevation(a.pos));
    let stats = g.store.profile(pid).unwrap().ship.stats();
    (rep.cargo, rep.mine_rate, rep.speed) = (stats.cargo, stats.mine_rate * if o.profession == Profession::Miner { g.tuning.economy.miner_bonus } else { 1.0 }, stats.speed);

    let dt = 1.0 / TICK_HZ;
    let mut leg = Leg::Out;
    let mut trip = Trip { low_hull: 1.0, ..Default::default() };
    let mut was_dead = false;
    let mut wallet = g.store.profile(pid).map_or((0, 0), |p| (p.wallet.credits, p.wallet.alloy + p.wallet.crystal + p.wallet.datacore));
    g.handle(conn, ClientMsg::Undock);
    for tick in 0..(o.max_secs * TICK_HZ) as u64 {
        let s = &g.sessions[&conn];
        let dead = s.dead;
        if dead && !was_dead {
            trip.deaths += 1;
            leg = Leg::Out;
            if rep.trips.iter().map(|t| t.deaths).sum::<u32>() + trip.deaths >= MAX_DEATHS {
                break;
            }
        }
        was_dead = dead;
        if tick % THINK == 0 {
            let here = s.system;
            let warping = s.warp.is_some();
            if dead {
                g.handle(conn, ClientMsg::Respawn);
            } else if warping {
                g.handle(conn, ClientMsg::Controls(Controls::default()));
            } else if let Some(e) = s.entity.and_then(|id| g.ent(here, id)) {
                let (pos, rot, docked, mining, hull) = (e.pos, e.rot, e.docked, e.mining.is_some(), e.hull / e.stats.max_hull.max(1.0));
                let in_combat = g.now - e.last_hit < COMBAT_LOCK;
                let shooting: std::collections::BTreeSet<_> = e.attackers.iter().filter(|(a, _, at)| g.now - at < 8.0 && g.ent(here, *a).is_some_and(|x| !x.dead)).map(|(a, _, _)| *a).collect();
                let attacker = e.attackers.iter().rev().find(|(a, ..)| shooting.contains(a)).and_then(|(a, _, _)| g.ent(here, *a)).map(|x| x.pos);
                // Outnumbered or hurt: run for the station with what is in the hold, as a player would.
                let flee = !shooting.is_empty() && (shooting.len() >= 2 || hull < 0.5);
                trip.low_hull = trip.low_hull.min(hull);
                let prof = g.store.profile(pid).unwrap().clone();
                let full = prof.cargo_used() as f32 + 1.0 > stats.cargo;
                if docked && here != sys {
                    // Rebuilt at home after a death: back to the mining system.
                    g.handle(conn, ClientMsg::Undock);
                } else if here != sys {
                    if !in_combat && let Some(hop) = next_hop(here, sys, stats.warp_range) {
                        g.handle(conn, ClientMsg::Warp { system: hop });
                    }
                } else if docked {
                    // Respawned at the station, or back from a trip: sell (or refine), then go again.
                    for ore in Ore::ALL {
                        let have = prof.cargo.get(ore.name()).copied().unwrap_or(0);
                        if have > 0 {
                            trip.units += have;
                            let before = g.store.profile(pid).unwrap().wallet.credits;
                            g.handle(conn, if o.refine { ClientMsg::Refine { ore, units: have } } else { ClientMsg::Sell { ore, units: have } });
                            let left = g.store.profile(pid).unwrap().cargo.get(ore.name()).copied().unwrap_or(0);
                            if left > 0 {
                                g.handle(conn, ClientMsg::Sell { ore, units: left });
                            }
                            trip.ore_credits += g.store.profile(pid).unwrap().wallet.credits - before;
                        }
                    }
                    let before = g.store.profile(pid).unwrap().wallet.credits;
                    g.handle(conn, ClientMsg::SellSpecimens);
                    trip.specimen_credits += g.store.profile(pid).unwrap().wallet.credits - before;
                    let w = &g.store.profile(pid).unwrap().wallet;
                    let now = (w.credits, w.alloy + w.crystal + w.datacore);
                    trip.credits += now.0 - wallet.0;
                    trip.refined += now.1 - wallet.1;
                    wallet = now;
                    if leg == Leg::Back {
                        rep.trips.push(std::mem::replace(&mut trip, Trip { low_hull: 1.0, ..Default::default() }));
                        if rep.trips.len() as u32 >= o.trips {
                            break;
                        }
                    }
                    leg = Leg::Out;
                    g.handle(conn, ClientMsg::Undock);
                } else if flee {
                    if leg != Leg::Back {
                        trip.fled += 1;
                    }
                    leg = Leg::Back;
                    let mut c = steer(pos, rot, station, stats.radius + 60.0, stats.turn);
                    c.boost = pos.distance(station) > stats.radius + 400.0;
                    g.handle(conn, ClientMsg::Controls(c));
                    if pos.distance(station) < stats.radius + 200.0 {
                        g.handle(conn, ClientMsg::Dock);
                    }
                } else if let Some(tp) = attacker {
                    // A miner shoots back: no docking or quiet mining while under fire.
                    let mut c = steer(pos, rot, tp, stats.weapon_range * 0.55, stats.turn);
                    let local = rot.inverse() * (tp - pos).normalize_or_zero();
                    c.fire = -local.z > 0.9 && pos.distance(tp) < stats.weapon_range * 1.05;
                    g.handle(conn, ClientMsg::Controls(c));
                } else if full {
                    leg = Leg::Back;
                    if pos.distance(station) > stats.radius + 120.0 {
                        g.handle(conn, ClientMsg::Controls(steer(pos, rot, station, stats.radius + 60.0, stats.turn)));
                    } else {
                        g.handle(conn, ClientMsg::Controls(Controls::default()));
                        g.handle(conn, ClientMsg::Dock);
                    }
                } else {
                    let reach = a.radius + stats.radius + 60.0;
                    if pos.distance(a.pos) > reach + 80.0 {
                        g.handle(conn, ClientMsg::Controls(steer(pos, rot, a.pos, reach, stats.turn)));
                    } else {
                        leg = Leg::Mine;
                        g.handle(conn, ClientMsg::Controls(Controls::default()));
                        if !mining {
                            g.handle(conn, ClientMsg::Mine { asteroid: Some(a.id) });
                        }
                    }
                }
            }
        }
        let before = g.store.profile(pid).map_or(0, |p| p.specimens.values().sum::<u32>());
        match Activity::of(&g, conn) {
            Activity::Dead => trip.dead += dt,
            Activity::Combat => trip.combat += dt,
            _ => match leg {
                Leg::Out => trip.out += dt,
                Leg::Mine => trip.mine += dt,
                Leg::Back => trip.back += dt,
            },
        }
        g.tick(dt, o.unix + tick / TICK_HZ as u64);
        g.out.clear();
        g.kicked.clear();
        trip.specimens += g.store.profile(pid).map_or(0, |p| p.specimens.values().sum::<u32>()).saturating_sub(before);
    }
    if (rep.trips.len() as u32) < o.trips {
        let s = &g.sessions[&conn];
        let cargo = g.store.profile(pid).map_or(0, |p| p.cargo_used());
        rep.stuck = Some(format!(
            "{} of {} trips in {:.0} s — unfinished trip: {} deaths, {:.0} s mining, {:.0} s combat, {:.0} s dead, in system {} with {cargo} units",
            rep.trips.len(),
            o.trips,
            o.max_secs,
            trip.deaths,
            trip.mine,
            trip.combat,
            trip.dead,
            s.system
        ));
    }
    rep
}
