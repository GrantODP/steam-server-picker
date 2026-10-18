pub use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, EnumIter, Display)]
pub enum GameID {
    #[default]
    #[strum(serialize = "Unknown")]
    Unknown = 0,

    #[strum(serialize = "Deadlock")]
    DeadLock = 1422450,

    #[strum(serialize = "Counter-Strike 2")]
    CS2 = 730,
}

pub trait SteamGame {
    fn id(&self) -> u32;

    fn name(&self) -> &'static str;

    fn cache_name(&self) -> &'static str;
}

impl SteamGame for GameID {
    fn id(&self) -> u32 {
        self.clone() as u32
    }

    fn name(&self) -> &'static str {
        match self {
            GameID::CS2 => "Counter-Strike 2",
            GameID::DeadLock => "Deadlock",
            GameID::Unknown => "Unknown Game",
        }
    }

    fn cache_name(&self) -> &'static str {
        match self {
            GameID::Unknown => "",
            GameID::DeadLock => "deadlock.json",
            GameID::CS2 => "cs2.json",
        }
    }
}
