use std::ops::{Deref, DerefMut};
use std::{collections::HashMap, sync::Arc};

use std::fmt::Debug;
use twinkle_client::notify::Notify;

use crate::*;

/// Internal representation of a device.
#[derive(Debug)]
pub struct Device {
    name: String,
    parameters: HashMap<String, Arc<Notify<Parameter>>>,
    names: Vec<String>,
    groups: Vec<String>,
    group_counts: HashMap<String, usize>,
}

impl Clone for Device {
    fn clone(&self) -> Self {
        Device {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            names: self.names.clone(),
            groups: self.groups.clone(),
            group_counts: self.group_counts.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DeviceUpdate {
    AddParameter(String),
    UpdateParameter(String),
    DeleteParameter(Option<String>),
}

impl Device {
    /// Creates a new device named `name` with no parameters.
    pub fn new(name: String) -> Self {
        Device {
            name,
            parameters: HashMap::new(),
            names: Default::default(),
            groups: Default::default(),
            group_counts: Default::default(),
        }
    }

    pub fn get_name(&self) -> &String {
        return &self.name;
    }

    /// Returns a `&Vec<String>` of all currently know parameter names.
    pub fn parameter_names(&self) -> &Vec<String> {
        return &self.names;
    }

    /// Returns a `&Vec<Option<String>>` of all currently know parameter groups.
    pub fn parameter_groups(&self) -> &Vec<String> {
        return &self.groups;
    }

    /// Returns a `&Vec<String>` of all current parameters.
    pub fn get_parameters(&self) -> &HashMap<String, Arc<Notify<Parameter>>> {
        return &self.parameters;
    }

    /// Updates the current device based on `command`.
    pub async fn update(
        mut this: impl Deref<Target = Device> + DerefMut,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        match command {
            Command::Message(_) => Ok(None),
            Command::GetProperties(_) => Ok(None),
            Command::DefSwitchVector(command) => this.new_param(command).await,
            Command::SetSwitchVector(command) => this.update_param(command).await,
            Command::NewSwitchVector(_) => Ok(None),
            Command::DefNumberVector(command) => this.new_param(command).await,
            Command::SetNumberVector(command) => this.update_param(command).await,
            Command::NewNumberVector(_) => Ok(None),
            Command::DefTextVector(command) => this.new_param(command).await,
            Command::SetTextVector(command) => this.update_param(command).await,
            Command::NewTextVector(_) => Ok(None),
            Command::DefBlobVector(command) => this.new_param(command).await,
            Command::SetBlobVector(command) => {this.update_param(command).await},
            Command::DefLightVector(command) => this.new_param(command).await,
            Command::SetLightVector(command) => this.update_param(command).await,
            Command::DelProperty(command) => this.delete_param(command.name).await,
            Command::EnableBlob(_) => Ok(None),
        }
    }
    pub async fn new_param<'a, C: CommandtoParam + std::fmt::Debug>(
        &'a mut self,
        def: C,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        let name = def.get_name().clone();

        if let Some(group) = def.get_group() {
            let group_counts = self.group_counts.entry(group.clone()).or_insert(0);
            *group_counts += 1;
            if *group_counts == 1 {
                self.groups.push(group.clone());
            }
        }

        if !self.parameters.contains_key(&name) {
            self.names.push(name.clone());
        }
        
        let param = def.to_param();
        match self.parameters.get_mut(&name) {
            Some(entry) => {
                *entry.write().await = param;
            },
            None => {
                self.parameters.insert(name.clone(), Arc::new(Notify::new(param)));
            }
        };

        Ok(Some(DeviceUpdate::AddParameter(name.clone())))
    }

    pub async fn update_param<'a, C: CommandToUpdate>(
        &'a self,
        new_command: C,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        match self.parameters.get(&new_command.get_name().clone()) {
            Some(param) => {
                let mut param = param.write().await;

                new_command.update_param(&mut param)?;
                Ok(Some(DeviceUpdate::UpdateParameter(
                    param.get_name().clone(),
                )))
            }
            None => Err(UpdateError::ParameterMissing(
                new_command.get_name().clone(),
            )),
        }
    }

    pub async fn delete_param(
        &mut self,
        name: Option<String>,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        match &name {
            Some(name) => {
                self.names.retain(|n| *n != *name);

                match self.parameters.remove(name) {
                    Some(param) => {
                        if let Some(group) = param.write().await.get_group() {
                            let group_count = self.group_counts.entry(group.clone()).or_insert(0);
                            *group_count -= 1;
                            if group_count == &0 {
                                self.groups.retain(|g| *g != *group);
                            }
                        }
                    }
                    None => {
                        return Err(UpdateError::ParameterMissing(name.clone()));
                    }
                }
            }
            None => {
                self.names.clear();
                self.groups.clear();
                self.group_counts.clear();
                self.parameters.clear();
            }
        };

        Ok(Some(DeviceUpdate::DeleteParameter(name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use std::ops::Deref;

    #[tokio::test]
    async fn test_update_switch() {
        let mut device: Device = Device::new(String::from("CCD Simulator"));
        let timestamp = Timestamp(DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap());

        let def_switch = DefSwitchVector {
            device: String::from("CCD Simulator"),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            rule: SwitchRule::AtMostOne,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            switches: vec![DefSwitch {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                value: SwitchState::On,
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device.new_param(def_switch).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);
        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::SwitchVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &SwitchVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        rule: SwitchRule::AtMostOne,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Switch {
                                label: Some(String::from_str("asdf").unwrap()),
                                value: SwitchState::On
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
        let timestamp = Timestamp(DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap());
        let set_switch = SetSwitchVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            state: PropertyState::Ok,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            switches: vec![OneSwitch {
                name: String::from_str("seconds").unwrap(),
                value: SwitchState::Off,
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device.update_param(set_switch).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::SwitchVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &SwitchVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        rule: SwitchRule::AtMostOne,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Switch {
                                label: Some(String::from_str("asdf").unwrap()),
                                value: SwitchState::Off
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
    }

    #[tokio::test]
    async fn test_update_number() {
        let mut device: Device = Device::new(String::from("CCD Simulator"));
        let timestamp = Timestamp(DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap());

        let def_number = DefNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            numbers: vec![DefNumber {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                format: String::from_str("%4.0f").unwrap(),
                min: 0.0,
                max: 100.0,
                step: 1.0,
                value: 13.3.into(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device.new_param(def_number).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::NumberVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &NumberVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Number {
                                label: Some(String::from_str("asdf").unwrap()),
                                format: String::from_str("%4.0f").unwrap(),
                                min: 0.0,
                                max: 100.0,
                                step: 1.0,
                                value: 13.3.into(),
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }

        let timestamp = Timestamp(DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap());
        let set_number = SetNumberVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            state: PropertyState::Ok,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            numbers: vec![SetOneNumber {
                name: String::from_str("seconds").unwrap(),
                min: None,
                max: None,
                step: None,
                value: 5.0.into(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device.update_param(set_number).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::NumberVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &NumberVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Number {
                                label: Some(String::from_str("asdf").unwrap()),
                                format: String::from_str("%4.0f").unwrap(),
                                min: 0.0,
                                max: 100.0,
                                step: 1.0,
                                value: 5.0.into()
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
    }

    #[tokio::test]
    async fn test_update_text() {
        let mut device: Device = Device::new(String::from("CCD Simulator"));
        let timestamp = Timestamp(DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap());

        let def_text = DefTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            texts: vec![DefText {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                value: String::from_str("something").unwrap(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device.new_param(def_text).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::TextVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &TextVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Text {
                                label: Some(String::from_str("asdf").unwrap()),
                                value: String::from_str("something").unwrap(),
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z")
            .unwrap()
            .into();
        let set_number = SetTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            state: PropertyState::Ok,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            texts: vec![OneText {
                name: String::from_str("seconds").unwrap(),
                value: String::from_str("something else").unwrap(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device.update_param(set_number).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::TextVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &TextVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Text {
                                label: Some(String::from_str("asdf").unwrap()),
                                value: String::from_str("something else").unwrap(),
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
    }

    #[tokio::test]
    async fn test_delete() {
        let mut device: Device = Device::new(String::from("CCD Simulator"));
        let timestamp = Timestamp(DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap());

        let def_text = DefTextVector {
            device: String::from_str("CCD Simulator").unwrap(),
            name: String::from_str("Exposure").unwrap(),
            label: Some(String::from_str("thingo").unwrap()),
            group: Some(String::from_str("group").unwrap()),
            state: PropertyState::Ok,
            perm: PropertyPerm::RW,
            timeout: Some(60),
            timestamp: Some(timestamp),
            message: None,
            texts: vec![DefText {
                name: String::from_str("seconds").unwrap(),
                label: Some(String::from_str("asdf").unwrap()),
                value: String::from_str("something").unwrap(),
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device.new_param(def_text).await.unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .read()
                .await;
            if let Parameter::TextVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &TextVector {
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp.into_inner()),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Text {
                                label: Some(String::from_str("asdf").unwrap()),
                                value: String::from_str("something").unwrap(),
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z")
            .unwrap()
            .into();
        let del_number = DelProperty {
            device: String::from_str("CCD Simulator").unwrap(),
            name: Some(String::from_str("Exposure").unwrap()),
            timestamp: Some(timestamp),
            message: None,
        };
        assert_eq!(device.get_parameters().len(), 1);
        device.delete_param(del_number.name).await.unwrap();
        assert_eq!(device.get_parameters().len(), 0);
    }
}
