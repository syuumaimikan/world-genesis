use crate::npc::Profession;
use genesis_core::time::SimTick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PersonId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyMember {
    pub id: PersonId,
    pub name: String,
    pub dynasty_id: u32,
    pub father_id: Option<PersonId>,
    pub mother_id: Option<PersonId>,
    pub spouse_id: Option<PersonId>,
    pub children_ids: Vec<PersonId>,
    pub birth_tick: SimTick,
    pub death_tick: Option<SimTick>,
    pub health: f32, // 0.0 to 100.0
    pub age_years: u16,
    pub loyalty_to_crown: f32, // 0.0 to 1.0
    pub ambition: f32,         // 0.0 to 1.0
    pub profession: Profession,
}

impl FamilyMember {
    pub fn is_alive(&self) -> bool {
        self.death_tick.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dynasty {
    pub id: u32,
    pub house_name: String,
    pub founder_id: PersonId,
    pub prestige: f32,
    pub head_of_house: PersonId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DemographyLedger {
    pub people: HashMap<PersonId, FamilyMember>,
    pub dynasties: HashMap<u32, Dynasty>,
    next_person_id: u64,
}

impl DemographyLedger {
    pub fn new() -> Self {
        Self {
            people: HashMap::new(),
            dynasties: HashMap::new(),
            next_person_id: 1,
        }
    }

    pub fn birth_child(
        &mut self,
        tick: SimTick,
        name: impl Into<String>,
        dynasty_id: u32,
        father: Option<PersonId>,
        mother: Option<PersonId>,
    ) -> PersonId {
        let id = PersonId(self.next_person_id);
        self.next_person_id += 1;

        let member = FamilyMember {
            id,
            name: name.into(),
            dynasty_id,
            father_id: father,
            mother_id: mother,
            spouse_id: None,
            children_ids: Vec::new(),
            birth_tick: tick,
            death_tick: None,
            health: 100.0,
            age_years: 0,
            loyalty_to_crown: 0.8,
            ambition: 0.5,
            profession: Profession::Farmer,
        };

        if let Some(f_id) = father {
            if let Some(f) = self.people.get_mut(&f_id) {
                f.children_ids.push(id);
            }
        }
        if let Some(m_id) = mother {
            if let Some(m) = self.people.get_mut(&m_id) {
                m.children_ids.push(id);
            }
        }

        self.people.insert(id, member);
        id
    }

    pub fn record_death(&mut self, id: PersonId, tick: SimTick) {
        if let Some(person) = self.people.get_mut(&id) {
            person.death_tick = Some(tick);
            person.health = 0.0;
        }
    }

    /// 長子相続制 (Primogeniture) に基づく正当な後継者探索
    pub fn resolve_primogeniture_successor(&self, current_ruler_id: PersonId) -> Option<PersonId> {
        let ruler = self.people.get(&current_ruler_id)?;

        // 生存している実子から年長順に探索
        let mut eligible_children: Vec<&FamilyMember> = ruler
            .children_ids
            .iter()
            .filter_map(|cid| self.people.get(cid))
            .filter(|c| c.is_alive())
            .collect();

        eligible_children.sort_by(|a, b| b.age_years.cmp(&a.age_years));
        if let Some(eldest) = eligible_children.first() {
            return Some(eldest.id);
        }

        // 直系が途絶えた場合、兄弟姉妹を探索
        if let Some(father_id) = ruler.father_id {
            if let Some(father) = self.people.get(&father_id) {
                let mut eligible_siblings: Vec<&FamilyMember> = father
                    .children_ids
                    .iter()
                    .filter(|&&cid| cid != current_ruler_id)
                    .filter_map(|cid| self.people.get(cid))
                    .filter(|c| c.is_alive())
                    .collect();

                eligible_siblings.sort_by(|a, b| b.age_years.cmp(&a.age_years));
                if let Some(eldest_sib) = eligible_siblings.first() {
                    return Some(eldest_sib.id);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger_with_family() -> (DemographyLedger, PersonId) {
        let mut ledger = DemographyLedger::new();
        let grandfather = ledger.birth_child(SimTick(0), "Aldric", 1, None, None);
        let ruler = ledger.birth_child(SimTick(10), "Berin", 1, Some(grandfather), None);
        (ledger, ruler)
    }

    #[test]
    fn new_ledger_is_empty_and_ids_start_at_one() {
        let mut ledger = DemographyLedger::new();
        assert!(ledger.people.is_empty());
        assert!(ledger.dynasties.is_empty());
        assert_eq!(
            ledger.birth_child(SimTick(0), "First", 1, None, None),
            PersonId(1)
        );
        assert_eq!(
            ledger.birth_child(SimTick(0), "Second", 1, None, None),
            PersonId(2)
        );
    }

    #[test]
    fn newborns_start_healthy_loyal_and_alive() {
        let mut ledger = DemographyLedger::new();
        let id = ledger.birth_child(SimTick(42), "Cael", 3, None, None);
        let child = &ledger.people[&id];
        assert_eq!(child.name, "Cael");
        assert_eq!(child.dynasty_id, 3);
        assert_eq!(child.birth_tick, SimTick(42));
        assert_eq!(child.health, 100.0);
        assert_eq!(child.age_years, 0);
        assert_eq!(child.profession, Profession::Farmer);
        assert!(child.is_alive());
    }

    #[test]
    fn births_register_the_child_with_both_parents() {
        let mut ledger = DemographyLedger::new();
        let father = ledger.birth_child(SimTick(0), "Dain", 1, None, None);
        let mother = ledger.birth_child(SimTick(0), "Eira", 2, None, None);
        let child = ledger.birth_child(SimTick(5), "Fen", 1, Some(father), Some(mother));

        assert_eq!(ledger.people[&father].children_ids, vec![child]);
        assert_eq!(ledger.people[&mother].children_ids, vec![child]);
        assert_eq!(ledger.people[&child].father_id, Some(father));
        assert_eq!(ledger.people[&child].mother_id, Some(mother));
    }

    #[test]
    fn unknown_parents_are_ignored_by_births() {
        let mut ledger = DemographyLedger::new();
        let child = ledger.birth_child(SimTick(0), "Gale", 1, Some(PersonId(999)), None);
        assert_eq!(ledger.people.len(), 1);
        assert_eq!(ledger.people[&child].father_id, Some(PersonId(999)));
    }

    #[test]
    fn recorded_deaths_zero_out_health_and_end_the_life() {
        let mut ledger = DemographyLedger::new();
        let id = ledger.birth_child(SimTick(0), "Hild", 1, None, None);
        ledger.record_death(id, SimTick(500));

        let person = &ledger.people[&id];
        assert_eq!(person.death_tick, Some(SimTick(500)));
        assert_eq!(person.health, 0.0);
        assert!(!person.is_alive());
    }

    #[test]
    fn recording_a_death_for_an_unknown_person_is_a_no_op() {
        let mut ledger = DemographyLedger::new();
        ledger.record_death(PersonId(404), SimTick(1));
        assert!(ledger.people.is_empty());
    }

    #[test]
    fn primogeniture_picks_the_eldest_living_child() {
        let (mut ledger, ruler) = ledger_with_family();
        let young = ledger.birth_child(SimTick(20), "Ivar", 1, Some(ruler), None);
        let eldest = ledger.birth_child(SimTick(15), "Jora", 1, Some(ruler), None);
        ledger.people.get_mut(&young).unwrap().age_years = 12;
        ledger.people.get_mut(&eldest).unwrap().age_years = 30;

        assert_eq!(ledger.resolve_primogeniture_successor(ruler), Some(eldest));
    }

    #[test]
    fn dead_children_are_skipped_in_favour_of_living_ones() {
        let (mut ledger, ruler) = ledger_with_family();
        let dead_heir = ledger.birth_child(SimTick(15), "Kera", 1, Some(ruler), None);
        let spare = ledger.birth_child(SimTick(20), "Lodr", 1, Some(ruler), None);
        ledger.people.get_mut(&dead_heir).unwrap().age_years = 40;
        ledger.people.get_mut(&spare).unwrap().age_years = 20;
        ledger.record_death(dead_heir, SimTick(100));

        assert_eq!(ledger.resolve_primogeniture_successor(ruler), Some(spare));
    }

    #[test]
    fn childless_rulers_are_succeeded_by_their_eldest_living_sibling() {
        let mut ledger = DemographyLedger::new();
        let father = ledger.birth_child(SimTick(0), "Mard", 1, None, None);
        let ruler = ledger.birth_child(SimTick(10), "Nils", 1, Some(father), None);
        let younger_sibling = ledger.birth_child(SimTick(12), "Orin", 1, Some(father), None);
        let elder_sibling = ledger.birth_child(SimTick(11), "Perr", 1, Some(father), None);
        ledger.people.get_mut(&younger_sibling).unwrap().age_years = 18;
        ledger.people.get_mut(&elder_sibling).unwrap().age_years = 25;

        assert_eq!(
            ledger.resolve_primogeniture_successor(ruler),
            Some(elder_sibling)
        );
    }

    #[test]
    fn extinct_lines_have_no_successor() {
        let (ledger, ruler) = ledger_with_family();
        assert_eq!(ledger.resolve_primogeniture_successor(ruler), None);

        let mut orphan_ledger = DemographyLedger::new();
        let orphan = orphan_ledger.birth_child(SimTick(0), "Quin", 1, None, None);
        assert_eq!(orphan_ledger.resolve_primogeniture_successor(orphan), None);
    }

    #[test]
    fn unknown_rulers_have_no_successor() {
        let ledger = DemographyLedger::new();
        assert_eq!(ledger.resolve_primogeniture_successor(PersonId(1)), None);
    }
}
