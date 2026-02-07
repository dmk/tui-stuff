#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    GenerateFloor {
        floor_index: u32,
        seed: u64,
        width: u16,
        height: u16,
    },
}
