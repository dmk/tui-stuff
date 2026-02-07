use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Skill {
    Athletics,
    Acrobatics,
    Stealth,
    Perception,
    Persuasion,
    Arcana,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AbilityScores {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

impl Default for AbilityScores {
    fn default() -> Self {
        Self {
            strength: 8,
            dexterity: 8,
            constitution: 8,
            intelligence: 8,
            wisdom: 8,
            charisma: 8,
        }
    }
}

impl AbilityScores {
    pub fn get(&self, ability: Ability) -> i32 {
        match ability {
            Ability::Strength => self.strength,
            Ability::Dexterity => self.dexterity,
            Ability::Constitution => self.constitution,
            Ability::Intelligence => self.intelligence,
            Ability::Wisdom => self.wisdom,
            Ability::Charisma => self.charisma,
        }
    }

    pub fn set(&mut self, ability: Ability, value: i32) {
        match ability {
            Ability::Strength => self.strength = value,
            Ability::Dexterity => self.dexterity = value,
            Ability::Constitution => self.constitution = value,
            Ability::Intelligence => self.intelligence = value,
            Ability::Wisdom => self.wisdom = value,
            Ability::Charisma => self.charisma = value,
        }
    }

    pub fn modifier(&self, ability: Ability) -> i32 {
        ability_modifier(self.get(ability))
    }
}

pub fn ability_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

pub fn skill_to_ability(skill: Skill) -> Ability {
    match skill {
        Skill::Athletics => Ability::Strength,
        Skill::Acrobatics => Ability::Dexterity,
        Skill::Stealth => Ability::Dexterity,
        Skill::Perception => Ability::Wisdom,
        Skill::Persuasion => Ability::Charisma,
        Skill::Arcana => Ability::Intelligence,
    }
}

pub fn difficulty_dc(difficulty: Difficulty) -> i32 {
    match difficulty {
        Difficulty::Easy => 10,
        Difficulty::Medium => 15,
        Difficulty::Hard => 20,
    }
}

pub fn parse_skill_or_ability(input: &str) -> CheckKind {
    let key = input.trim().to_lowercase();
    match key.as_str() {
        "athletics" => CheckKind::Skill(Skill::Athletics),
        "acrobatics" => CheckKind::Skill(Skill::Acrobatics),
        "stealth" => CheckKind::Skill(Skill::Stealth),
        "perception" => CheckKind::Skill(Skill::Perception),
        "persuasion" => CheckKind::Skill(Skill::Persuasion),
        "arcana" => CheckKind::Skill(Skill::Arcana),
        "strength" => CheckKind::Ability(Ability::Strength),
        "dexterity" => CheckKind::Ability(Ability::Dexterity),
        "constitution" => CheckKind::Ability(Ability::Constitution),
        "intelligence" => CheckKind::Ability(Ability::Intelligence),
        "wisdom" => CheckKind::Ability(Ability::Wisdom),
        "charisma" => CheckKind::Ability(Ability::Charisma),
        _ => CheckKind::Ability(Ability::Wisdom),
    }
}

pub fn parse_difficulty(input: &str) -> Difficulty {
    match input.trim().to_lowercase().as_str() {
        "easy" => Difficulty::Easy,
        "hard" => Difficulty::Hard,
        _ => Difficulty::Medium,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CheckKind {
    Skill(Skill),
    Ability(Ability),
}

pub fn point_cost(score: i32) -> i32 {
    match score {
        8 => 0,
        9 => 1,
        10 => 2,
        11 => 3,
        12 => 4,
        13 => 5,
        14 => 7,
        15 => 9,
        _ => 99,
    }
}

pub fn points_remaining(scores: &AbilityScores, total: i32) -> i32 {
    total
        - point_cost(scores.strength)
        - point_cost(scores.dexterity)
        - point_cost(scores.constitution)
        - point_cost(scores.intelligence)
        - point_cost(scores.wisdom)
        - point_cost(scores.charisma)
}

pub fn clamp_score(value: i32) -> i32 {
    value.max(8).min(15)
}

pub fn roll_d20(seed: &mut u64) -> i32 {
    (next_u32(seed) % 20) as i32 + 1
}

pub fn roll_damage(seed: &mut u64, sides: i32) -> i32 {
    (next_u32(seed) % sides as u32) as i32 + 1
}

pub fn next_u32(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 32) as u32
}

pub const CLASS_OPTIONS: &[&str] = &[
    "Fighter",
    "Rogue",
    "Wizard",
    "Ranger",
    "Cleric",
];

pub const BACKGROUND_OPTIONS: &[&str] = &[
    "Soldier",
    "Outlander",
    "Scholar",
    "Merchant",
    "Acolyte",
];

pub fn class_base_hp(class_name: &str) -> i32 {
    match class_name.to_lowercase().as_str() {
        "fighter" => 12,
        "rogue" => 10,
        "wizard" => 8,
        "ranger" => 10,
        "cleric" => 10,
        _ => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_buy_math() {
        let mut scores = AbilityScores::default();
        scores.strength = 15;
        scores.dexterity = 14;
        let remaining = points_remaining(&scores, 27);
        assert!(remaining < 27);
    }

    #[test]
    fn modifiers() {
        assert_eq!(ability_modifier(10), 0);
        assert_eq!(ability_modifier(12), 1);
        assert_eq!(ability_modifier(8), -1);
    }

    #[test]
    fn dc_map() {
        assert_eq!(difficulty_dc(Difficulty::Easy), 10);
        assert_eq!(difficulty_dc(Difficulty::Medium), 15);
        assert_eq!(difficulty_dc(Difficulty::Hard), 20);
    }
}
