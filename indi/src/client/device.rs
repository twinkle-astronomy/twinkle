use std::{
    collections::HashMap,
    num::Wrapping,
    ops::Deref,
    sync::Arc,
    time::{Duration, Instant},
};

use fitsio::FitsFile;

use crate::{
    batch, serialization, Blob, BlobEnable, ChangeError, Client, ClientConnection, Command,
    CommandToUpdate, CommandtoParam, EnableBlob, Number, Parameter, PropertyState, ToCommand,
    TryEq, UpdateError,
};

use super::notify::{self, Notify};

#[derive(Debug)]
pub struct Device {
    name: String,
    parameters: HashMap<String, Arc<Notify<Parameter>>>,
    names: Vec<String>,
    groups: Vec<Option<String>>,
}

impl Device {
    pub fn new(name: String) -> Device {
        Device {
            name,
            parameters: HashMap::new(),
            names: vec![],
            groups: vec![],
        }
    }

    pub fn update(
        &mut self,
        command: serialization::Command,
    ) -> Result<Option<notify::NotifyMutexGuard<'_, Parameter>>, UpdateError> {
        match command {
            Command::Message(_) => Ok(None),
            Command::GetProperties(_) => Ok(None),
            Command::DefSwitchVector(command) => self.new_param(command),
            Command::SetSwitchVector(command) => self.update_param(command),
            Command::NewSwitchVector(_) => Ok(None),
            Command::DefNumberVector(command) => self.new_param(command),
            Command::SetNumberVector(command) => self.update_param(command),
            Command::NewNumberVector(_) => Ok(None),
            Command::DefTextVector(command) => self.new_param(command),
            Command::SetTextVector(command) => self.update_param(command),
            Command::NewTextVector(_) => Ok(None),
            Command::DefBlobVector(command) => self.new_param(command),
            Command::SetBlobVector(command) => self.update_param(command),
            Command::DefLightVector(command) => self.new_param(command),
            Command::SetLightVector(command) => self.update_param(command),
            Command::DelProperty(command) => self.delete_param(command.name),
            Command::EnableBlob(_) => Ok(None),
        }
    }

    pub fn parameter_names(&self) -> &Vec<String> {
        return &self.names;
    }

    pub fn parameter_groups(&self) -> &Vec<Option<String>> {
        return &self.groups;
    }

    pub fn get_parameters(&self) -> &HashMap<String, Arc<Notify<Parameter>>> {
        return &self.parameters;
    }

    fn new_param<T: CommandtoParam + std::fmt::Debug>(
        &mut self,
        def: T,
    ) -> Result<Option<notify::NotifyMutexGuard<'_, Parameter>>, UpdateError> {
        let name = def.get_name().clone();
        if self.name == "ZWO CCD ASI294MM Pro" && name == "CCD1" {
            // dbg!(&def);
        }

        self.names.push(name.clone());
        if let None = self.groups.iter().find(|&x| x == def.get_group()) {
            self.groups.push(def.get_group().clone());
        }

        if let Some(existing) = self.parameters.get(&name) {
            let mut l = existing.lock();

            let param = def.to_param(l.gen() + Wrapping(1));
            *l = param;
        } else {
            let param = def.to_param(Wrapping(0));
            self.parameters
                .insert(name.clone(), Arc::new(Notify::new(param)));
        }
        Ok(self.parameters.get(&name).map(|x| x.lock()))
    }

    fn update_param<T: CommandToUpdate>(
        &mut self,
        new_command: T,
    ) -> Result<Option<notify::NotifyMutexGuard<'_, Parameter>>, UpdateError> {
        match self.parameters.get_mut(&new_command.get_name().clone()) {
            Some(param) => {
                let mut param = param.lock();
                *param.gen_mut() += Wrapping(1);
                new_command.update_param(&mut param)?;
                Ok(Some(param))
            }
            None => Err(UpdateError::ParameterMissing(
                new_command.get_name().clone(),
            )),
        }
    }

    fn delete_param(
        &mut self,
        name: Option<String>,
    ) -> Result<Option<notify::NotifyMutexGuard<'_, Parameter>>, UpdateError> {
        match name {
            Some(name) => {
                self.names.retain(|n| *n != name);
                self.parameters.remove(&name);
            }
            None => {
                self.names.clear();
                self.parameters.drain();
            }
        };
        Ok(None)
    }

    pub fn image_from_fits(bytes: &Vec<u8>) -> ndarray::ArrayD<u16> {
        let mut ptr_size = bytes.capacity();
        let mut ptr = bytes.as_ptr();

        // now we have a pointer to the data, let's open this in `fitsio_sys`
        let mut fptr = std::ptr::null_mut();
        let mut status = 0;

        let c_filename = std::ffi::CString::new("memory.fits").expect("creating c string");
        unsafe {
            fitsio::sys::ffomem(
                &mut fptr as *mut *mut _,
                c_filename.as_ptr(),
                fitsio::sys::READONLY as _,
                &mut ptr as *const _ as *mut *mut libc::c_void,
                &mut ptr_size as *mut _,
                0,
                None,
                &mut status,
            );
        }

        fitsio::errors::check_status(status).expect("checking internal fitsio status");

        let mut f = unsafe { FitsFile::from_raw(fptr, fitsio::FileOpenMode::READONLY) }
            .expect("Creating a FitsFile");

        let hdu = f.primary_hdu().expect("Getting primary image");
        hdu.read_image(&mut f)
            .expect("reading image from in memory fits file")
    }
}

pub struct ActiveDevice<'a, T: ClientConnection + 'static> {
    device: Arc<Notify<Device>>,
    client: &'a Client<T>,
}
impl<'a, T: ClientConnection> ActiveDevice<'a, T> {
    pub fn new(device: Arc<Notify<Device>>, client: &'a Client<T>) -> ActiveDevice<'a, T> {
        ActiveDevice { device, client }
    }
}

impl<'a, T: ClientConnection> Deref for ActiveDevice<'a, T> {
    type Target = Arc<Notify<Device>>;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl<'a, T: ClientConnection> ActiveDevice<'a, T> {
    pub fn get_parameter(
        &self,
        param_name: &str,
    ) -> Result<Arc<Notify<Parameter>>, notify::Error<()>> {
        self.device
            .wait_fn::<_, (), _>(Duration::from_secs(1), |device| {
                Ok(match device.get_parameters().get(param_name) {
                    Some(param) => notify::Status::Complete(param.clone()),
                    None => notify::Status::Pending,
                })
            })
    }

    pub fn change<P: Clone + TryEq<Parameter, ()> + ToCommand<P> + 'static>(
        &self,
        param_name: &str,
        values: P,
    ) -> Result<
        Box<dyn FnOnce() -> Result<Arc<Notify<Parameter>>, notify::Error<()>>>,
        ChangeError<()>,
    > {
        let (device_name, param) =
            self.device
                .wait_fn::<_, (), _>(Duration::from_secs(1), |device| {
                    Ok(match device.get_parameters().get(param_name) {
                        Some(param) => {
                            notify::Status::Complete((device.name.clone(), param.clone()))
                        }
                        None => notify::Status::Pending,
                    })
                })?;

        let timeout = {
            let param = param.lock();

            if !values.try_eq(&param)? {
                let c = values
                    .clone()
                    .to_command(device_name, String::from(param_name));
                self.client.connection.write(&c)?;
            }

            param.get_timeout().unwrap_or(60)
        };

        Ok(Box::new(move || {
            // dbg!(timeout);
            param.wait_fn::<Arc<Notify<Parameter>>, (), _>(
                Duration::from_secs(timeout.into()),
                |param_lock| {
                    // dbg!(param);
                    if *param_lock.get_state() == PropertyState::Alert {
                        return Err(());
                    }
                    if values.try_eq(&param_lock)? {
                        Ok(notify::Status::Complete(param.clone()))
                    } else {
                        Ok(notify::Status::Pending)
                    }
                },
            )
        }))
    }

    pub fn enable_blob(
        &self,
        name: Option<&str>,
        enabled: BlobEnable,
    ) -> Result<(), notify::Error<()>> {
        // Wait for device and paramater to exist
        if let Some(name) = name {
            let _ = self.get_parameter(name)?;
        }
        let device_name = self.device.lock().name.clone();

        self.client
            .connection
            .write(&Command::EnableBlob(EnableBlob {
                device: device_name,
                name: name.map(|x| String::from(x)),
                enabled: enabled,
            }))
            .expect("Unable to write command");
        Ok(())
    }

    pub fn capture_image(&self, exposure: f64) -> Result<ndarray::ArrayD<u16>, notify::Error<()>> {
        // Set imge format to something we can work with.
        batch(vec![
            self.change("CCD_CAPTURE_FORMAT", vec![("ASI_IMG_RAW16", true)])
                .unwrap(),
            self.change(
                "CCD_TRANSFER_FORMAT",
                vec![
                    //            ( "FORMAT_NATIVE", true ),
                    ("FORMAT_FITS", true),
                ],
            )
            .unwrap(),
        ])?;

        let image_param = self.get_parameter("CCD1")?;

        // Record image generation before starting exposure
        let image_gen = image_param.lock().gen();

        let exposure_param = self
            .change("CCD_EXPOSURE", vec![("CCD_EXPOSURE_VALUE", exposure)])
            .unwrap()()?;

        let mut previous_exposure_secs = exposure;
        let mut previous_tick = Instant::now();

        exposure_param.wait_fn::<(), (), _>(
            Duration::from_secs(exposure.ceil() as u64 + 10),
            |exposure_param| {
                // Exposure goes to idle when canceled
                if *exposure_param.get_state() == PropertyState::Idle {
                    return Err(());
                }
                let remaining_exposure = exposure_param
                    .get_values::<HashMap<String, Number>>()?
                    .get("CCD_EXPOSURE_VALUE")
                    .and_then(|x| Some(x.value))
                    .expect("Missing CCD_EXPOSURE_VALUE from CCD_EXPOSURE parameter");

                // Image is done exposing, new image data should be sent very soon
                if remaining_exposure == 0.0 {
                    return Ok(notify::Status::Complete(()));
                }

                // remaining exposure didn't change, nothing to check
                if previous_exposure_secs == remaining_exposure {
                    return Ok(notify::Status::Pending);
                }

                // Make sure exposure changed by a reasonable amount.
                // If another exposure is started before our exposure is finished
                //  there is a good chance the remaining exposure won't have changed
                //  by the amount of time since the last tick.
                let now = Instant::now();
                let exposure_change = Duration::from_millis(
                    ((previous_exposure_secs - remaining_exposure).abs() * 1000.0) as u64,
                );
                let time_change = now - previous_tick;

                if exposure_change > time_change + Duration::from_millis(1100) {
                    return Err(());
                }
                previous_tick = now;
                previous_exposure_secs = remaining_exposure;

                // Nothing funky happened, so we're still waiting for the
                // exposure to finish.
                Ok(notify::Status::Pending)
            },
        )?;

        // Wait for the image data to come in.
        let image_data = image_param.wait_fn(
            Duration::from_secs((exposure.ceil() as u64 + 60).into()),
            |ccd| {
                // We've been called before the next image has come in.
                if ccd.gen() == image_gen {
                    return Ok(notify::Status::Pending);
                };

                if let Some(image_data) = ccd.get_values::<HashMap<String, Blob>>()?.get("CCD1") {
                    if let Some(bytes) = &image_data.value {
                        Ok(notify::Status::Complete(Device::image_from_fits(bytes)))
                        // let mut ptr_size = bytes.capacity();
                        // let mut ptr = bytes.as_ptr();

                        // // now we have a pointer to the data, let's open this in `fitsio_sys`
                        // let mut fptr = std::ptr::null_mut();
                        // let mut status = 0;

                        // let c_filename = std::ffi::CString::new("memory.fits").expect("creating c string");
                        // unsafe {
                        //     fitsio::sys::ffomem(
                        //         &mut fptr as *mut *mut _,
                        //         c_filename.as_ptr(),
                        //         fitsio::sys::READONLY as _,
                        //         &mut ptr as *const _ as *mut *mut libc::c_void,
                        //         &mut ptr_size as *mut _,
                        //         0,
                        //         None,
                        //         &mut status,
                        //     );
                        // }

                        // fitsio::errors::check_status(status).expect("checking internal fitsio status");

                        // let mut f = unsafe { FitsFile::from_raw(fptr, fitsio::FileOpenMode::READONLY) }.expect("Creating a FitsFile");

                        // let hdu = f.primary_hdu().expect("Getting primary image");
                        // let i = hdu.read_image(&mut f).expect("reading image from in memory fits file");

                        // Ok(notify::Status::Complete(i))
                    } else {
                        Err(())
                    }
                } else {
                    dbg!("Missing CCD1");
                    Err(())
                }
            },
        )?;
        Ok(image_data)
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use std::ops::Deref;

    use super::*;
    use crate::*;

    #[test]
    fn test_update_switch() {
        let mut device = Device::new(String::from("CCD Simulator"));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);
        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::SwitchVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &SwitchVector {
                        gen: Wrapping(0),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        rule: SwitchRule::AtMostOne,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
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
        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::SwitchVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &SwitchVector {
                        gen: Wrapping(1),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        rule: SwitchRule::AtMostOne,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
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

    #[test]
    fn test_update_number() {
        let mut device = client::device::Device::new(String::from("CCD Simulator"));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

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
                value: 13.3,
            }],
        };
        assert_eq!(device.get_parameters().len(), 0);
        device
            .update(serialization::Command::DefNumberVector(def_number))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::NumberVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &NumberVector {
                        gen: Wrapping(0),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Number {
                                label: Some(String::from_str("asdf").unwrap()),
                                format: String::from_str("%4.0f").unwrap(),
                                min: 0.0,
                                max: 100.0,
                                step: 1.0,
                                value: 13.3,
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }

        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
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
                value: 5.0,
            }],
        };
        assert_eq!(device.get_parameters().len(), 1);
        device
            .update(serialization::Command::SetNumberVector(set_number))
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::NumberVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &NumberVector {
                        gen: Wrapping(1),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
                        values: HashMap::from([(
                            String::from_str("seconds").unwrap(),
                            Number {
                                label: Some(String::from_str("asdf").unwrap()),
                                format: String::from_str("%4.0f").unwrap(),
                                min: 0.0,
                                max: 100.0,
                                step: 1.0,
                                value: 5.0
                            }
                        )])
                    }
                );
            } else {
                panic!("Unexpected");
            }
        }
    }

    #[test]
    fn test_update_text() {
        let mut device = Device::new(String::from("CCD Simulator"));
        let timestamp = DateTime::from_str("2022-10-13T07:41:56.301Z").unwrap();

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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::TextVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &TextVector {
                        gen: Wrapping(0),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
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
        let timestamp = DateTime::from_str("2022-10-13T08:41:56.301Z").unwrap();
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device.get_parameters().get("Exposure").unwrap().lock();
            if let Parameter::TextVector(stored) = param.deref() {
                assert_eq!(
                    stored,
                    &TextVector {
                        gen: Wrapping(1),
                        name: String::from_str("Exposure").unwrap(),
                        group: Some(String::from_str("group").unwrap()),
                        label: Some(String::from_str("thingo").unwrap()),
                        state: PropertyState::Ok,
                        perm: PropertyPerm::RW,
                        timeout: Some(60),
                        timestamp: Some(timestamp),
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
