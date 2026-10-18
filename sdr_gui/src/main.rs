use std::sync::{Arc, OnceLock};

use iced::{Task, Theme};
use tokio::runtime::Handle;

use crate::gui::{Message, RelayHolders};
pub static TOKIO_HANDLE: OnceLock<Handle> = OnceLock::new();
mod gui;

#[tokio::main]
async fn main() -> iced::Result {
    let _ = TOKIO_HANDLE.set(Handle::current());

    iced::application(
        || {
            let app = RelayHolders::new();

            (app, Task::done(Message::Init))
        },
        RelayHolders::update,
        RelayHolders::view,
    )
    .theme(|_: &_| Theme::Dark)
    .run()
}
