mod dbus;

use std::error::Error;
use iced::{Element, Task};
use iced::widget::{button, center, column, scrollable, table, text};
use iced_aw::ContextMenu;
use zbus::Connection;

use crate::dbus::{ManagerProxy, Unit};

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
    RefreshUnits,
    LoadedUnits(Result<Vec<Unit>, zbus::Error>),

    StartUnit(String),
    UnitStarted(Result<(), zbus::Error>),

    StopUnit(String),
    UnitStopped(Result<(), zbus::Error>)
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
                self.proxy = Some(proxy);
                self.update(Message::RefreshUnits)
            },
            Message::DbusConnected(Err(e)) => {
                self.status = Status::Failed(e.to_string());
                Task::none()
            },
            Message::RefreshUnits => match &self.proxy {
                Some(proxy) => Task::perform(get_units(proxy.clone()), Message::LoadedUnits),
                // This can only be reached if the initial connection failed
                None => {
                    self.status = Status::Loading;
                    Task::perform(dbus_connect(), Message::DbusConnected)
                }
            },
            Message::LoadedUnits(Ok(units)) => {
                self.status = Status::Loaded(State { units });
                Task::none()
            }
            Message::LoadedUnits(Err(e)) => {
                self.status = Status::Failed(e.to_string());
                Task::none()
            }

            Message::StartUnit(name) => match self.proxy.clone() {
                Some(proxy) => Task::perform(start_unit(proxy, name), Message::UnitStarted),
                None => Task::none(),
            },
            Message::UnitStarted(Ok(_)) => self.update(Message::RefreshUnits),
            Message::UnitStarted(Err(e)) => {
                eprintln!("Failed to start unit: {:?}", e);
                Task::none()
            },

            Message::StopUnit(name) => match self.proxy.clone() {
                Some(proxy) => Task::perform(stop_unit(proxy, name), Message::UnitStopped),
                None => Task::none(),
            },
            Message::UnitStopped(Ok(_)) => self.update(Message::RefreshUnits),
            Message::UnitStopped(Err(e)) => {
                eprintln!("Failed to stop unit: {:?}", e);
                Task::none()
            }
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
                        let mut start = button("Start");
                        let mut stop = button("Stop");

                        if row.active_state != "active".to_string() {
                            start = start.on_press(Message::StartUnit(row.name.clone()));
                        } else {
                            stop = stop.on_press(Message::StopUnit(row.name.clone()));
                        }

                        column![
                            start,
                            stop
                        ].into()
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

fn main() -> Result<(), Box<dyn Error>> {
    iced::application(App::new, App::update, App::view)
        .title("Services")
        .run()?;
    Ok(())
}
