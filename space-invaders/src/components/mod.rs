//! Reusable entity views. Simulation state stays in `AppState` and is advanced
//! by the reducer; each component receives only the data needed to draw it.

pub mod alien;
pub mod bullet;
pub mod debug;
pub mod explosion;
pub mod layout;
pub mod player;
pub mod saucer;
pub mod shield;
pub mod sprite;

pub use alien::{AlienComponent, AlienProps};
pub use bullet::{BulletComponent, BulletProps};
pub use debug::ComponentRegistry;
pub use explosion::{ExplosionComponent, ExplosionProps};
pub use layout::BoardLayout;
pub use player::{PlayerComponent, PlayerProps};
pub use saucer::{SaucerComponent, SaucerProps};
pub use shield::{ShieldComponent, ShieldProps};
pub use sprite::{SpriteComponent, SpriteProps};
