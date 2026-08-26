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
