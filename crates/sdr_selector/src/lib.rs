pub mod error;
pub mod firewall;
pub mod games;
pub mod picker;
pub mod relay;
pub mod scanner;
pub mod system;

type Result<T> = std::result::Result<T, error::Error>;
