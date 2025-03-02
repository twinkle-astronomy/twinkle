use egui::{Ui, WidgetText};

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct TabView {
    selected: Option<String>,
}

impl TabView {
    pub fn show<'a, I: PartialEq + Into<WidgetText> + ToString, T: Iterator<Item = I>>(
        &'a mut self,
        ui: &mut Ui,
        tabs: T,
    ) ->  Option<&'a String> {
        ui.horizontal(|ui| {
            for tab in tabs {
                let selected = self.selected.get_or_insert_with(|| tab.to_string());
                ui.selectable_value(selected, tab.to_string(), tab);
            }
        });

        self.selected.as_ref()
    }
}
