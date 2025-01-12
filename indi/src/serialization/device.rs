use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use std::fmt::Debug;
use twinkle_client::notify::{self, Notify};

use crate::*;

// Define the main trait for types that provide notify-like behavior
pub trait AsyncLockable<T> {
    type Lock<'a>: Deref<Target = T> + DerefMut + 'a
    where
        Self: 'a;

    fn new(value: T) -> Self;

    fn lock(&self) -> impl std::future::Future<Output = Self::Lock<'_>> + Send;
}

impl<T: Clone + Debug + Send + Sync + 'static> AsyncLockable<T> for Notify<T> {
    type Lock<'a> = notify::NotifyMutexGuard<'a, T>;

    fn new(value: T) -> Self {
        Notify::new(value)
    }

    async fn lock(&self) -> Self::Lock<'_> {
        Notify::lock(self).await
    }
}
/// Internal representation of a device.
#[derive(Debug)]
pub struct Device<T: AsyncLockable<Parameter> + Debug> {
    name: String,
    parameters: HashMap<String, Arc<T>>,
    names: Vec<String>,
    groups: Vec<Option<String>>,
}

impl<T: AsyncLockable<Parameter> + Debug> Clone for Device<T> {
    fn clone(&self) -> Self {
        Device {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            names: self.names.clone(),
            groups: self.groups.clone(),
        }
    }
}

#[derive(Clone)]
pub enum DeviceUpdate {
    AddParameter(String),
    UpdateParameter(String),
    DeleteParameter(Option<String>),
}

impl<T: AsyncLockable<Parameter> + Debug> Device<T> {
    /// Creates a new device named `name` with no parameters.
    pub fn new(name: String) -> Self {
        Device {
            name,
            parameters: HashMap::new(),
            names: vec![],
            groups: vec![],
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
    pub fn parameter_groups(&self) -> &Vec<Option<String>> {
        return &self.groups;
    }

    /// Returns a `&Vec<String>` of all current parameters.
    pub fn get_parameters(&self) -> &HashMap<String, Arc<T>> {
        return &self.parameters;
    }

    /// Updates the current device based on `command`.
    pub async fn update(
        &mut self,
        command: serialization::Command,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        match command {
            Command::Message(_) => Ok(None),
            Command::GetProperties(_) => Ok(None),
            Command::DefSwitchVector(command) => self.new_param(command),
            Command::SetSwitchVector(command) => self.update_param(command).await,
            Command::NewSwitchVector(_) => Ok(None),
            Command::DefNumberVector(command) => self.new_param(command),
            Command::SetNumberVector(command) => self.update_param(command).await,
            Command::NewNumberVector(_) => Ok(None),
            Command::DefTextVector(command) => self.new_param(command),
            Command::SetTextVector(command) => self.update_param(command).await,
            Command::NewTextVector(_) => Ok(None),
            Command::DefBlobVector(command) => self.new_param(command),
            Command::SetBlobVector(command) => self.update_param(command).await,
            Command::DefLightVector(command) => self.new_param(command),
            Command::SetLightVector(command) => self.update_param(command).await,
            Command::DelProperty(command) => self.delete_param(command.name),
            Command::EnableBlob(_) => Ok(None),
        }
    }
    pub fn new_param<'a, C: CommandtoParam + std::fmt::Debug>(
        &'a mut self,
        def: C,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        let name = def.get_name().clone();

        self.names.push(name.clone());
        if let None = self.groups.iter().find(|&x| x == def.get_group()) {
            self.groups.push(def.get_group().clone());
        }

        if !self.parameters.contains_key(&name) {
            let param = def.to_param();
            // let value: Notify<Parameter> = param.into();
            self.parameters
                .insert(name.clone(), Arc::new(T::new(param)));
        }
        Ok(Some(DeviceUpdate::AddParameter(name.clone())))
    }

    pub async fn update_param<'a, C: CommandToUpdate>(
        &'a mut self,
        new_command: C,
    ) -> Result<Option<DeviceUpdate>, UpdateError> {
        match self.parameters.get_mut(&new_command.get_name().clone()) {
            Some(param) => {
                let mut param = param.lock().await;
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

    fn delete_param(&mut self, name: Option<String>) -> Result<Option<DeviceUpdate>, UpdateError> {
        match &name {
            Some(name) => {
                self.names.retain(|n| *n != *name);
                self.parameters.remove(name);
            }
            None => {
                self.names.clear();
                self.parameters.drain();
            }
        };

        Ok(Some(DeviceUpdate::DeleteParameter(name)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use std::sync::{Mutex, MutexGuard};

    #[derive(Debug)]
    struct TestLock<T>(std::sync::Mutex<T>);
    impl<T: Send> AsyncLockable<T> for TestLock<T> {
        type Lock<'a> = MutexGuard<'a, T>
        where Self: 'a;

        fn new(value: T) -> Self {
            Self(Mutex::new(value))
        }

        async fn lock(&self) -> Self::Lock<'_> {
            self.0.lock().unwrap()
        }
    }
    #[tokio::test]
    async fn test_update_switch() {
        let mut device: Device<TestLock<Parameter>> = Device::new(String::from("CCD Simulator"));
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
        device
            .update(serialization::Command::DefSwitchVector(def_switch))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);
        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
        device
            .update(serialization::Command::SetSwitchVector(set_switch))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
        let mut device: Device<TestLock<Parameter>> = Device::new(String::from("CCD Simulator"));
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
        device
            .update(serialization::Command::DefNumberVector(def_number))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
        device
            .update(serialization::Command::SetNumberVector(set_number))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
        let mut device: Device<TestLock<Parameter>> = Device::new(String::from("CCD Simulator"));
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
        device
            .update(serialization::Command::DefTextVector(def_text))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
        device
            .update(serialization::Command::SetTextVector(set_number))
            .await
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
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
}
