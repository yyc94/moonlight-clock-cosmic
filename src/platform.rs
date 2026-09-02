use anyhow::{Context, Result};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};

pub struct WaylandCapabilities {
    pub layer_shell: bool,
}

pub fn probe_wayland() -> Result<WaylandCapabilities> {
    let connection =
        Connection::connect_to_env().context("cannot connect to the Wayland compositor")?;
    let (globals, _queue) = registry_queue_init::<RegistryState>(&connection)
        .context("cannot read the Wayland protocol registry")?;
    let layer_shell = globals.contents().with_list(|items| {
        items
            .iter()
            .any(|item| item.interface == "zwlr_layer_shell_v1")
    });
    Ok(WaylandCapabilities { layer_shell })
}

struct RegistryState;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RegistryState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}
