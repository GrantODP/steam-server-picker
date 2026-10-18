use std::{
    collections::HashMap,
    println,
    sync::{Arc, RwLock},
    time::Duration,
};

use iced::{
    Alignment, Element, Length, Task,
    widget::{Button, Column, Row, button, column, container, pick_list, row, scrollable, text},
};

use sdr_selector::{
    firewall::{FirewallState, RelayPolicy, Rule},
    games::{GameID, IntoEnumIterator, SteamGame},
    picker::{GameServers, PolicyProducer},
    relay::Ping,
    scanner::{RelayId, fetch_save_game_relays_cache},
    system::get_cache_dir,
};
use tokio::sync::Mutex;
use crate::TOKIO_HANDLE;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Message {
    #[default]
    None,
    TaskComplete,
    UpdatePing(RelayId, Duration),
    UpdateRelays,
    ChangeGame(GameID),
    Relay(RelayMessage),
    Init,
    LoadRelays,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum RelayMessage {
    #[default]
    None,
    RequestPing(RelayId),
    UpdateFirewall(RelayId, Rule),
}

pub struct RelayMessageBuilder {
    id: RelayId,
}

impl RelayMessageBuilder {
    pub fn ping(self, ping: Duration) -> RelayMessage {
        RelayMessage::RequestPing(self.id)
    }

    pub fn firewall(self, rule: Rule) -> RelayMessage {
        RelayMessage::UpdateFirewall(self.id, rule)
    }
}

impl RelayMessage {
    pub fn message(id: RelayId) -> RelayMessageBuilder {
        RelayMessageBuilder { id }
    }

    pub fn none() -> Self {
        Self::None
    }
}
static EMPTY_ID: RelayId = RelayId(0);

pub fn relay_state_view(
    id: RelayId,
    shrt_name: &str,
    name: &str,
    rule_state: &Rule,
    latency_ms: Duration,
) -> Element<'static, Message> {
    let (status, action) = match rule_state {
        Rule::Allow => (
            text("Allowed").style(text::success),
            button("Block")
                .style(button::danger)
                .on_press(Message::Relay(RelayMessage::UpdateFirewall(id, Rule::Deny))),
        ),
        Rule::Deny => (
            text("Blocked").style(text::danger),
            button("Allow")
                .style(button::success)
                .on_press(Message::Relay(RelayMessage::UpdateFirewall(
                    id,
                    Rule::Allow,
                ))),
        ),
    };

    let content = row![
        text!("{}", shrt_name).size(18).width(60),
        text!("{}", name).width(Length::Fill), // takes the leftover space
        text(format!("{} ms", latency_ms.as_millis()))
            .style(text::secondary)
            .width(80),
        status.width(80),
        action,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    container(content)
        .padding([8, 12])
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

pub struct RelayHolders {
    game: GameID,
    picker: Arc<RwLock<GameServers>>,
    firewall: Arc<RwLock<FirewallState>>,
    pings: Arc<Mutex<HashMap<RelayId, Duration>>>,
}

impl RelayHolders {
    pub fn new() -> Self {
        let game = GameID::DeadLock;
        Self {
            game: game.clone(),
            pings: Arc::new(Mutex::new(HashMap::new())),
            picker: Arc::new(RwLock::new(GameServers::new(game))),
            firewall: Arc::new(RwLock::new(FirewallState::default())),
        }
    }

    pub fn init(&self) -> Task<Message> {
        println!("initializing");
        self.load_relays();

        Task::none()
    }

    fn update_relays(&self) -> Task<Message> {
        if let Ok(mut picker) = self.picker.write() {
            let cache_dir = get_cache_dir();
            picker.load_clusters(cache_dir);
        }
        Task::none()
    }

    fn load_relays(&self) -> Task<Message> {
        if let Ok(mut picker) = self.picker.write() {
            println!("got picker");
            let cache_dir = get_cache_dir();
            let game = picker.game();

            let loaded = picker.load_clusters(cache_dir);
            if let Err(_) = loaded {
                println!("not loaded now fetching");
                return Task::perform(
                    async move {
                        if let Some(handle) = TOKIO_HANDLE.get() {
                            let _ = handle
                                .spawn(async move { fetch_save_game_relays_cache(&game).await })
                                .await;
                            println!("Fetched relay data")
                        }
                    },
                    move |_| Message::UpdateRelays,
                );
            }
        }

        Task::none()
    }

    fn change_game(&mut self, game: GameID) -> Task<Message> {
        if let Ok(mut guard) = self.picker.write() {
            if game == guard.game() {
                return Task::none();
            }
            println!("Changing game");
            self.game = game.clone();
            *guard = GameServers::new(game);
            return Task::done(Message::LoadRelays);
        }
        Task::none()
    }

    //     pub fn load(&mut self) -> Task<Message> {
    //         let mut picker = self.picker.clone();
    //         picker.load_clusters();
    //         Task::none()
    //     }
    //
    pub fn view(&self) -> Element<'_, Message> {
        // 1. Initialize an empty Column layout
        let mut list = Column::new().spacing(8).padding(12).width(Length::Fill);
        if let Ok(firewall) = self.firewall.read() {
            if let Ok(picker) = self.picker.read() {
                for (id, relay) in picker.relays() {
                    let latency_ms = self
                        .pings
                        .try_lock()
                        .ok()
                        .and_then(|pings| pings.get(id).copied())
                        .unwrap_or_default();

                    let rule_state = firewall.relay_state(id).copied().unwrap_or(Rule::Allow);

                    let row_view =
                        relay_state_view(*id, &relay.sname, &relay.desc, &rule_state, latency_ms);

                    list = list.push(row_view);
                }
            }
        }

        // 3. Wrap the column in a scrollable view container
        let list = scrollable(list).height(Length::Fill);
        let ping_all = self.ping_button();
        let games_view = self.game_name_view();
        let col = column![games_view, list, ping_all].spacing(10);

        col.spacing(10).padding(20).into()
    }

    fn game_name_view(&self) -> Element<'_, Message> {
        let mut games: Vec<GameID> = GameID::iter().collect();
        games.remove(0);
        let content = column![
            text("Select a game:").size(16),
            // 2. Create the PickList dropdown widget
            pick_list(
                games,                   // The list of options
                Some(self.game.clone()), // Currently selected option (as an Option<&str>)
                Message::ChangeGame      // Message produced when an option is clicked
            )
            .placeholder("Choose a game..."), // Text shown when nothing is selected
        ]
        .spacing(10);

        content.into()
    }

    pub fn ping_button(&self) -> Element<'_, Message> {
        let message = Message::Relay((RelayMessage::RequestPing(EMPTY_ID)));
        container(button("Ping All").style(button::success).on_press(message))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::None => todo!(),
            Message::Relay(relay_message) => self.handle_relay(relay_message),
            Message::UpdatePing(id, latency) => self.update_ping(id.0, latency),
            Message::Init => self.init(),
            // _ => Task::none(),
            Message::UpdateRelays => self.update_relays(),
            Message::LoadRelays => self.load_relays(),
            Message::ChangeGame(game_id) => self.change_game(game_id),

            _ => Task::none(),
        }
    }

    fn request_ping(&self, id: RelayId) -> Task<Message> {
        if id == EMPTY_ID {
            return self.ping_all();
        }

        if let Ok(picker) = &self.picker.read() {
            let data = picker.relay_owned(&id);
            if let Some(data) = data {
                println!("MAKING PINGS");
                return Task::perform(
                    async move {
                        let ping = data.ping().await;
                        println!("PING {}", ping.latency_ms.as_millis());
                        ping
                    },
                    move |latency| Message::UpdatePing(id, latency.latency_ms),
                );
            }
        }

        Task::none()
    }

    fn handle_firewall_rule(&self, id: RelayId, rule: Rule) -> Task<Message> {
        if let (Ok(mut firewall), Ok(picker)) = (self.firewall.write(), self.picker.read()) {
            println!("UPDATE FIREWALL RULE: {}", rule.to_string());
            // if let Ok(_) = firewall.apply_rule(id, rule) {
            // } else {
            let relay = picker.get_relay(&id);
            let policy: Option<RelayPolicy> = relay.map(|r| r.into());

            if let Some(mut policy) = policy {
                policy.change(rule);
                let r = firewall.add_policy(id, policy);
            }
            // }
        }
        Task::none()
    }

    pub fn ping_all(&self) -> Task<Message> {
        if let Ok(picker) = self.picker.read() {
            let ids: Vec<RelayId> = picker.relay_ids().map(|id| *id).collect();
            return Task::batch(ids.into_iter().map(|id| self.request_ping(id)));
        }
        Task::none()
    }

    fn handle_relay(&mut self, relay_message: RelayMessage) -> Task<Message> {
        match relay_message {
            RelayMessage::RequestPing(id) => self.request_ping(id),
            RelayMessage::UpdateFirewall(id, rule) => self.handle_firewall_rule(id, rule),

            _ => Task::none(),
        }
    }

    fn update_ping(&mut self, id: u64, latency: Duration) -> Task<Message> {
        if let Ok(mut pings) = self.pings.try_lock() {
            pings.insert(id.into(), latency);
        }
        Task::none()
    }
}
