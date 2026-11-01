use zbus::{OwnedObjectPath, proxy};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub followed_unit: String,
    pub unit_object_path: OwnedObjectPath,
    pub job_id: u32,
    pub job_type: String,
    pub job_object_path: OwnedObjectPath
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
pub trait Manager {
    async fn list_units(&self) -> zbus::Result<Vec<(
        String,          // Name
        String,          // Description
        String,          // Load state
        String,          // Active state
        String,          // Sub state
        String,          // Followed unit
        OwnedObjectPath, // Unit object path
        u32,             // Job ID
        String,          // Job type
        OwnedObjectPath, // Job object path
    )>>;

    #[zbus(allow_interactive_auth)]
    async fn start_unit(&self, name: String, mode: String) -> zbus::Result<OwnedObjectPath>;

    #[zbus(allow_interactive_auth)]
    async fn stop_unit(&self, name: String, mode: String) -> zbus::Result<OwnedObjectPath>;
}
