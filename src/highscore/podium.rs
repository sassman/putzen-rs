use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medal {
    Gold,
    Silver,
    Bronze,
}

impl Medal {
    pub fn sort_key(self) -> u8 {
        match self {
            Medal::Gold => 0,
            Medal::Silver => 1,
            Medal::Bronze => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub size: u64,
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Podium {
    pub gold: Option<Record>,
    pub silver: Option<Record>,
    pub bronze: Option<Record>,
}

impl Podium {
    /// Check if a given size would earn a medal, without mutating state.
    /// Returns the medal that would be earned, or None.
    pub fn would_place(&self, size: u64) -> Option<Medal> {
        match (&self.gold, &self.silver, &self.bronze) {
            (None, _, _) => Some(Medal::Gold),
            (Some(g), _, _) if size > g.size => Some(Medal::Gold),
            (_, None, _) => Some(Medal::Silver),
            (_, Some(s), _) if size > s.size => Some(Medal::Silver),
            (_, _, None) => Some(Medal::Bronze),
            (_, _, Some(b)) if size > b.size => Some(Medal::Bronze),
            _ => None,
        }
    }

    /// Place a record on the podium. Bumps existing records down.
    /// Returns the medal earned.
    pub fn place(&mut self, size: u64, date: &str) -> Option<Medal> {
        let medal = self.would_place(size)?;
        let record = Record {
            size,
            date: date.to_string(),
        };
        match medal {
            Medal::Gold => {
                self.bronze = self.silver.take();
                self.silver = self.gold.take();
                self.gold = Some(record);
            }
            Medal::Silver => {
                self.bronze = self.silver.take();
                self.silver = Some(record);
            }
            Medal::Bronze => {
                self.bronze = Some(record);
            }
        }
        Some(medal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_podium_places_gold() {
        let podium = Podium::default();
        assert_eq!(podium.would_place(100), Some(Medal::Gold));
    }

    #[test]
    fn second_smaller_entry_places_silver() {
        let mut podium = Podium::default();
        podium.place(200, "2026-03-10");
        assert_eq!(podium.would_place(100), Some(Medal::Silver));
    }

    #[test]
    fn third_smaller_entry_places_bronze() {
        let mut podium = Podium::default();
        podium.place(300, "2026-03-10");
        podium.place(200, "2026-03-10");
        assert_eq!(podium.would_place(100), Some(Medal::Bronze));
    }

    #[test]
    fn beating_gold_bumps_down() {
        let mut podium = Podium::default();
        podium.place(100, "2026-03-01");
        podium.place(200, "2026-03-02");
        podium.place(300, "2026-03-03");

        // Gold=300, Silver=200, Bronze=100
        assert_eq!(podium.gold.as_ref().unwrap().size, 300);
        assert_eq!(podium.silver.as_ref().unwrap().size, 200);
        assert_eq!(podium.bronze.as_ref().unwrap().size, 100);
    }

    #[test]
    fn new_gold_bumps_all_down_drops_bronze() {
        let mut podium = Podium::default();
        podium.place(100, "2026-03-01");
        podium.place(200, "2026-03-02");
        podium.place(300, "2026-03-03");

        // Now beat gold with 500
        podium.place(500, "2026-03-04");
        assert_eq!(podium.gold.as_ref().unwrap().size, 500);
        assert_eq!(podium.silver.as_ref().unwrap().size, 300);
        assert_eq!(podium.bronze.as_ref().unwrap().size, 200);
        // original bronze (100) is gone
    }

    #[test]
    fn no_placement_when_all_slots_filled_and_too_small() {
        let mut podium = Podium::default();
        podium.place(300, "2026-03-01");
        podium.place(200, "2026-03-02");
        podium.place(100, "2026-03-03");
        assert_eq!(podium.would_place(50), None);
    }

    #[test]
    fn equal_size_places_in_next_empty_slot() {
        let mut podium = Podium::default();
        podium.place(100, "2026-03-01");
        // Equal to gold should place silver (empty slot)
        assert_eq!(podium.would_place(100), Some(Medal::Silver));
    }
}
