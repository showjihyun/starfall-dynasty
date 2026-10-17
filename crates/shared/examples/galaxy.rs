//! Prints the galaxy (`tuning/galaxy.ron`): id, name, tier, star, sector, position, nearest neighbour.
fn main() {
    use shared::galaxy::{self, coords};
    for s in galaxy::systems() {
        let n = galaxy::systems().iter().filter(|o| o.id != s.id).map(|o| galaxy::distance_ly(s.id, o.id)).fold(f32::MAX, f32::min);
        println!("{:>2} {:<20} {:<9} {:<11} {:<4} ({:+.2}, {:+.2}) nearest {n:.2} ly", s.id, s.name, s.tier.label(), format!("{:?}", s.star), coords::sector_of(s.map_pos), s.map_pos[0], s.map_pos[2]);
    }
}
