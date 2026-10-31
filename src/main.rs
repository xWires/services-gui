mod dbus;

use std::error::Error;
use iced::{Element, Task};
use iced::widget::{button, center, column, scrollable, table, text};
use iced_aw::ContextMenu;
use zbus::Connection;

use crate::dbus::{ManagerProxy, Unit};

#[derive(Default)]
enum App {
    #[default]
    Loading,

    Loaded(State),
    Failed(String)
}

#[derive(Debug, Clone)]
enum Message {
    Refresh,
    Loaded(Result<Vec<Unit>, zbus::Error>),

    StartUnit(String),
    UnitStarted(Result<(), zbus::Error>)
}

struct State {
    units: Vec<Unit>
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (Self::Loading, Task::perform(App::get_units(), Message::Loaded))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => Task::perform(App::get_units(), Message::Loaded),
            Message::Loaded(Ok(units)) => {
                *self = App::Loaded(State { units });
                Task::none()
            }
            Message::Loaded(Err(e)) => {
                *self = App::Failed(e.to_string());
                Task::none()
            }

            Message::StartUnit(name) => Task::perform(App::start_unit(name), Message::UnitStarted),
            Message::UnitStarted(Ok(_)) => self.update(Message::Refresh),
            Message::UnitStarted(Err(e)) => {
                eprintln!("Failed to start unit: {:?}", e);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::Loading => center(text("Loading services...")).into(),
            App::Failed(e) => {
                center(column![
                    text(format!("Failed to load units: {e}")),
                    button("Try again").on_press(Message::Refresh)
                ]).into()
            },
            App::Loaded(state) => {
                let columns = vec![
                    table::column(text("Name"), |row: Unit| ContextMenu::new(text(row.name.clone()), move || {
                        column![
                            button("Start").on_press(Message::StartUnit(row.name.clone()))
                        ].into()
                    })),
                    table::column(text("Description"), |row: Unit| text(row.description)),
                    table::column(text("State"), |row: Unit| text(row.active_state))
                ];

                let services_table = table(columns, state.units.clone());

                column![
                    button("Refresh").on_press(Message::Refresh),
                    scrollable(services_table),
                ].into()
            }
        }
    }

    async fn get_units() -> Result<Vec<Unit>, zbus::Error> {
        let conn = Connection::system().await?;
        let proxy = ManagerProxy::new(&conn).await?;

        let mut units = proxy.list_units().await?.into_iter().map(|u| Unit {
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

    async fn start_unit(name: String) -> Result<(), zbus::Error> {
        let conn = Connection::system().await?;
        let proxy = ManagerProxy::new(&conn).await?;
        
        proxy.start_unit(name, "replace".to_string()).await?;

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    iced::application(App::new, App::update, App::view).run()?;
    Ok(())
}
