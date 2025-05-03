use twinkle_api::indi::api::IndiDriverParams;
use twinkle_client::task::{spawn, Abortable};

use crate::get_http_base;

pub struct Control<const N: usize> {
    pub drivers: [&'static str; N],
    pub current_driver: usize,
}

fn get_url() -> String {
    format!("{}indi/fifo", get_http_base())
}

impl<const N: usize> egui::Widget for &mut Control<N> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.current_driver != 0, |ui| {
                    if ui.button("Start").clicked() {
                        let params = IndiDriverParams { driver_name: self.drivers[self.current_driver].to_string()};
                        spawn((), |_| async move {
                            let _ = reqwest::Client::new().post(get_url()).json(&params).send().await;
                        })
                        .abort_on_drop(false);
                    }
                    if ui.button("Stop").clicked() {
                        let params = IndiDriverParams { driver_name: self.drivers[self.current_driver].to_string()};
                        spawn((), |_| async move {
                            let _ = reqwest::Client::new().delete(get_url()).json(&params).send().await;
                        })
                        .abort_on_drop(false);
                    }
                });
            });
            egui::ComboBox::from_label("Driver")
                .selected_text(self.drivers[self.current_driver])
                .show_ui(ui, |ui| {
                    for (i, driver) in self.drivers.iter().enumerate() {
                        ui.selectable_value(&mut self.current_driver, i, *driver);
                    }
                });
        })
        .response
    }
}
