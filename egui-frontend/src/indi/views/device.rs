use std::collections::{BTreeSet, HashMap};
use futures::executor::block_on;

use egui::Ui;
use indi::client::active_device::ActiveDevice;

use super::tab;

#[derive(Default)]
pub struct Device {
    group: tab::TabView,
    parameters: HashMap<String, indi::Parameter>
}

impl Device {
    pub fn show(&mut self, ui: &mut Ui, device: &ActiveDevice) {
        let group = {
            let binding = block_on(device.lock());
            let groups: BTreeSet<String> = binding.get_parameters().iter().filter_map(|x| block_on(x.1.lock()).get_group().clone()).collect();
            self.group.show(ui, groups.into_iter())
        };
        ui.separator();
        if let Some(group) = group {
            ui.add(crate::indi::widgets::device::Device::new(device, &mut self.parameters, group));
        }
    }
}