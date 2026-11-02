mod dbus;

use std::error::Error;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::{Element, Length, Subscription, Task, stream};
use iced::widget::{button, center, column, container, scrollable, table, text};
use iced_aw::ContextMenu;
use zbus::Connection;

use crate::dbus::{HashableManagerProxy, ManagerProxy, Unit};

enum Status {
    Loading,

    Loaded(State),
    Failed(String)
}

struct App {
    proxy: Option<ManagerProxy<'static>>,
    status: Status
}

#[derive(Debug, Clone)]
enum Message {
    DbusConnected(Result<ManagerProxy<'static>, zbus::Error>),

    JobRemoved,

    RefreshUnits,
    LoadedUnits(Result<Vec<Unit>, zbus::Error>),

    StartUnit(String),
    UnitStarted(Result<(), zbus::Error>),

    StopUnit(String),
    UnitStopped(Result<(), zbus::Error>),

    RestartUnit(String),
    UnitRestarted(Result<(), zbus::Error>)
}

struct State {
    units: Vec<Unit>
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self { proxy: None, status: Status::Loading },
            Task::perform(dbus_connect(), Message::DbusConnected)
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DbusConnected(Ok(proxy)) => {
                println!("D-Bus connected");
                self.proxy = Some(proxy);
                self.update(Message::RefreshUnits)
            },
            Message::DbusConnected(Err(e)) => {
                eprintln!("Failed to connect to D-Bus: {:?}", e);
                self.status = Status::Failed(e.to_string());
                Task::none()
            },

            Message::JobRemoved => self.update(Message::RefreshUnits),

            Message::RefreshUnits => match &self.proxy {
                Some(proxy) => {
                    println!("Refreshing units");
                    Task::perform(get_units(proxy.clone()), Message::LoadedUnits)
                },
                // This can only be reached if the initial connection failed
                None => {
                    self.status = Status::Loading;
                    Task::perform(dbus_connect(), Message::DbusConnected)
                }
            },
            Message::LoadedUnits(Ok(units)) => {
                println!("Loaded units");
                self.status = Status::Loaded(State { units });
                Task::none()
            }
            Message::LoadedUnits(Err(e)) => {
                eprintln!("Failed to load units: {:?}", e);
                self.status = Status::Failed(e.to_string());
                Task::none()
            }

            Message::StartUnit(name) => match self.proxy.clone() {
                Some(proxy) => {
                    println!("Starting unit: {}", name);
                    Task::perform(start_unit(proxy, name), Message::UnitStarted)
                },
                None => Task::none(),
            },
            Message::UnitStarted(Ok(_)) => self.update(Message::RefreshUnits),
            Message::UnitStarted(Err(e)) => {
                eprintln!("Failed to start unit: {:?}", e);
                Task::none()
            },

            Message::StopUnit(name) => match self.proxy.clone() {
                Some(proxy) => {
                    println!("Stopping unit: {}", name);
                    Task::perform(stop_unit(proxy, name), Message::UnitStopped)
                },
                None => Task::none(),
            },
            Message::UnitStopped(Ok(_)) => self.update(Message::RefreshUnits),
            Message::UnitStopped(Err(e)) => {
                eprintln!("Failed to stop unit: {:?}", e);
                Task::none()
            },

            Message::RestartUnit(name) => match self.proxy.clone() {
                Some(proxy) => {
                    println!("Restarting unit: {}", name);
                    Task::perform(restart_unit(proxy, name), Message::UnitRestarted)
                },
                None => Task::none(),
            },
            Message::UnitRestarted(Ok(_)) => self.update(Message::RefreshUnits),
            Message::UnitRestarted(Err(e)) => {
                eprintln!("Failed to restart unit: {:?}", e);
                Task::none()
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.status {
            Status::Loading => center(text("Loading services...")).into(),
            Status::Failed(e) => {
                center(column![
                    text(format!("Failed to load units: {e}")),
                    button("Try again").on_press(Message::RefreshUnits)
                ]).into()
            },
            Status::Loaded(state) => {
                let columns = vec![
                    table::column(text("Name"), |row: Unit| ContextMenu::new(text(row.name.clone()), move || {
                        let mut start = button("Start").width(Length::Fill);
                        let mut stop = button("Stop").width(Length::Fill);
                        let mut restart = button("Restart").width(Length::Fill);

                        if row.active_state != "active".to_string() {
                            start = start.on_press(Message::StartUnit(row.name.clone()));
                        } else {
                            stop = stop.on_press(Message::StopUnit(row.name.clone()));
                            restart = restart.on_press(Message::RestartUnit(row.name.clone()));
                        }

                        container(
                            column![
                                start,
                                stop,
                                restart
                            ]
                        )
                        .width(Length::Fixed(128.0))
                        .style(container::primary)
                        .into()
                    })),
                    table::column(text("Description"), |row: Unit| text(row.description)),
                    table::column(text("State"), |row: Unit| text(row.active_state))
                ];

                let services_table = table(columns, state.units.clone());

                column![
                    button("Refresh").on_press(Message::RefreshUnits),
                    scrollable(services_table),
                ].into()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match &self.proxy {
            Some(proxy) => Subscription::run_with(HashableManagerProxy(proxy.clone()), signal_listener),
            None => Subscription::none()
        }
    }
}

// The `use<>` here tells the compiler that the generic data returned does not borrow from the arguments
fn signal_listener(proxy: &HashableManagerProxy) -> impl Stream<Item = Message> + use<> {
    let proxy = proxy.0.clone();
    stream::channel(100, async move |mut output| {
        let mut stream = proxy.receive_job_removed().await.unwrap();
        while let Some(signal) = stream.next().await {
            let args = match signal.args() {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Failed to get args from signal: {:?}", e);
                    continue;
                }
            };

            println!("{:?}", args);
            // It doesn't matter too much if the message fails to send, so just ignore the result
            let _ = output.send(Message::JobRemoved).await;
        }
    })
}

async fn dbus_connect() -> Result<ManagerProxy<'static>, zbus::Error> {
    let conn = Connection::system().await?;
    ManagerProxy::new(&conn).await
}

async fn get_units(proxy: ManagerProxy<'_>) -> Result<Vec<Unit>, zbus::Error> {
    let mut units = proxy
        .list_units()
        .await?
        .into_iter()
        .map(|u| Unit {
            name: u.0,
            description: u.1,
            load_state: u.2,
            active_state: u.3,
            sub_state: u.4,
            followed_unit: u.5,
            unit_object_path: u.6,
            job_id: u.7,
            job_type: u.8,
            job_object_path: u.9
        }).collect::<Vec<_>>();
    units.sort_by_key(|u| u.name.clone());

    Ok(units)
}

async fn start_unit(proxy: ManagerProxy<'_>, name: String) -> Result<(), zbus::Error> {
    proxy.start_unit(name, "replace".to_string()).await?;
    Ok(())
}

async fn stop_unit(proxy: ManagerProxy<'_>, name: String) -> Result<(), zbus::Error> {
    proxy.stop_unit(name, "replace".to_string()).await?;
    Ok(())
}

async fn restart_unit(proxy: ManagerProxy<'_>, name: String) -> Result<(), zbus::Error> {
    proxy.restart_unit(name, "replace".to_string()).await?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    iced::application(App::new, App::update, App::view)
        .title("Services")
        .subscription(App::subscription)
        .run()?;
    Ok(())
}
