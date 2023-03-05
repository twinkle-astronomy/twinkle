use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::Write,
    num::Wrapping,
    ops::Deref,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fitsio::FitsFile;

use crate::*;

use super::{notify::{self, Notify, Subscription}, PendingChangeImpl, ChangeError, Pending, batch};

#[derive(Debug, Clone)]
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
}

pub struct FitsImage {
    raw_data: Arc<Vec<u8>>,
}

impl FitsImage {
    pub fn new(data: Arc<Vec<u8>>) -> FitsImage {
        FitsImage { raw_data: data }
    }

    pub fn read_image(&self) -> fitsio::errors::Result<ndarray::ArrayD<u16>> {
        let mut ptr_size = self.raw_data.capacity();
        let mut ptr = self.raw_data.as_ptr();

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
        fitsio::errors::check_status(status)?;

        let mut f = unsafe { FitsFile::from_raw(fptr, fitsio::FileOpenMode::READONLY) }?;

        let hdu = f.primary_hdu()?;
        hdu.read_image(&mut f)
    }

    pub fn save<T: AsRef<Path>>(&self, path: T) -> Result<(), std::io::Error> {
        if let Some(dir) = path.as_ref().parent() {
            create_dir_all(dir)?;
        }
        let mut f = File::create(path)?;
        f.write_all(&self.raw_data)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ActiveDevice {
    device: Arc<Notify<Device>>,
    command_sender: crossbeam_channel::Sender<Command>,
}
impl ActiveDevice {
    pub fn new(
        device: Arc<Notify<Device>>,
        command_sender: crossbeam_channel::Sender<Command>,
    ) -> ActiveDevice {
        ActiveDevice {
            device,
            command_sender,
        }
    }

    pub fn sender(&self) -> &crossbeam_channel::Sender<Command> {
        &self.command_sender
    }
}

impl Deref for ActiveDevice {
    type Target = Arc<Notify<Device>>;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl ActiveDevice {
    pub fn get_parameter(
        &self,
        param_name: &str,
    ) -> Result<Arc<Notify<Parameter>>, notify::Error<Command>> {
        self.device
            .subscribe()
            .wait_fn(Duration::from_secs(1), |device| {
                Ok(match device.get_parameters().get(param_name) {
                    Some(param) => notify::Status::Complete(param.clone()),
                    None => notify::Status::Pending,
                })
            })
    }

    pub fn change<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static>(
        &self,
        param_name: &str,
        values: P,
    ) -> Result<PendingChangeImpl<P>, ChangeError<Command>> {
        let (device_name, param) =
            self.device
                .subscribe()
                .wait_fn::<_, Command, _>(Duration::from_secs(1), |device| {
                    Ok(match device.get_parameters().get(param_name) {
                        Some(param) => {
                            notify::Status::Complete((device.name.clone(), param.clone()))
                        }
                        None => notify::Status::Pending,
                    })
                })?;
        let subscription = param.subscribe();
        let timeout = {
            let param = param.lock();

            if !values.try_eq(&param)? {
                let c = values
                    .clone()
                    .to_command(device_name, String::from(param_name));
                self.sender().send(c)?;
            }

            param.get_timeout().unwrap_or(60)
        };
        Ok(PendingChangeImpl {
            subscription,
            param,
            deadline: Instant::now() + Duration::from_secs(timeout.into()),
            values,
        })
    }

    pub fn enable_blob(
        &self,
        name: Option<&str>,
        enabled: BlobEnable,
    ) -> Result<(), notify::Error<Command>> {
        // Wait for device and paramater to exist
        if let Some(name) = name {
            let _ = self.get_parameter(name)?;
        }
        let device_name = self.device.lock().name.clone();
        self.sender().send(Command::EnableBlob(EnableBlob {
            device: device_name,
            name: name.map(|x| String::from(x)),
            enabled,
        }))?;
        Ok(())
    }

    pub fn capture_image(&self, exposure: f64) -> Result<FitsImage, ChangeError<Command>> {
        let image_param = self.get_parameter("CCD1")?;
        let image_changes = image_param.changes();
        let exposure_param = self.get_parameter("CCD_EXPOSURE")?;
        let exposure_changes = exposure_param.changes();

        let device_name = self.lock().name.clone();
        let c = vec![("CCD_EXPOSURE_VALUE", exposure)]
            .to_command(device_name, String::from("CCD_EXPOSURE"));
        self.sender().send(c)?;

        let previous_exposure_secs = exposure;
        let previous_tick = Instant::now();

        let pe = PendingExposure {
            camera: self.clone(),
            exposure_param,
            changes: exposure_changes,
            deadline: Instant::now() + Duration::from_secs(exposure.ceil() as u64 + 10),
            state: Mutex::new(PendingExposureState {
                previous_exposure_secs,
                previous_tick,
            })
        };
        let pi = PendingImage {
            image_param,
            changes: image_changes,
            deadline: Instant::now() + Duration::from_secs(60)
        };
        batch(vec![pe])?;

        batch(vec![pi])
    }
}

struct PendingImage {
    image_param: Arc<Notify<Parameter>>,
    changes: Subscription<Parameter>,
    deadline: Instant,
}

impl Pending for PendingImage {
    type Item = Arc<Parameter>;
    type Result = FitsImage;

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn receiver(&self) -> &crossbeam_channel::Receiver<Self::Item> {
        &self.changes
    }

    fn tick(&self, ccd: Self::Item) -> Result<notify::Status<Self::Result>, ChangeError<Command>> {
        // We've been called before the next image has come in.
        if let Some(image_data) = ccd.get_values::<HashMap<String, Blob>>()?.get("CCD1") {
            if let Some(bytes) = &image_data.value {
                Ok(notify::Status::Complete(FitsImage {
                    raw_data: bytes.clone(),
                }))
            } else {
                Err(ChangeError::PropertyError)
            }
        } else {
            dbg!("Missing CCD1");
            Err(ChangeError::PropertyError)
        }
    }

    fn abort(&self) {
        self.image_param.cancel(&self.changes);
    }
}


struct PendingExposureState {
    previous_exposure_secs: f64,
    previous_tick: Instant,

}
struct PendingExposure {
    camera: ActiveDevice,
    exposure_param: Arc<Notify<Parameter>>,
    changes: Subscription<Parameter>,
    deadline: Instant,
    state: Mutex<PendingExposureState>
}

impl Pending for PendingExposure {
    type Item = Arc<Parameter>;
    type Result = Arc<Parameter>;

    fn deadline(&self) -> Instant {
        self.deadline.clone()
    }

    fn receiver(&self) -> &crossbeam_channel::Receiver<Self::Item> {
        self.changes.deref()
    }

    fn tick(&self, exposure_param: Self::Item) -> Result<notify::Status<Self::Item>, ChangeError<Command>> {
        // Exposure goes to idle when canceled
        if *exposure_param.get_state() == PropertyState::Idle {
            return Err(ChangeError::Abort);
        }
        
        let remaining_exposure = exposure_param
            .get_values::<HashMap<String, Number>>()?
            .get("CCD_EXPOSURE_VALUE")
            .and_then(|x| Some(x.value))
            .expect("Missing CCD_EXPOSURE_VALUE from CCD_EXPOSURE parameter");
        dbg!(&remaining_exposure);
        // Image is done exposing, new image data should be sent very soon
        if remaining_exposure == 0.0 {
            return Ok(notify::Status::Complete(exposure_param));
        }
        let mut state = self.state.lock().unwrap();

        // remaining exposure didn't change, nothing to check
        if state.previous_exposure_secs == remaining_exposure {
            return Ok(notify::Status::Pending);
        }

        // Make sure exposure changed by a reasonable amount.
        // If another exposure is started before our exposure is finished
        //  there is a good chance the remaining exposure won't have changed
        //  by the amount of time since the last tick.
        let now = Instant::now();
        let exposure_change = Duration::from_millis(
            ((state.previous_exposure_secs - remaining_exposure).abs() * 1000.0) as u64,
        );
        let time_change = now - state.previous_tick;

        if exposure_change > time_change + Duration::from_millis(1100) {
            return Err(ChangeError::Abort);
        }
        state.previous_tick = now;
        state.previous_exposure_secs = remaining_exposure;

        // Nothing funky happened, so we're still waiting for the
        // exposure to finish.
        Ok(notify::Status::Pending)
    }

    fn abort(&self) {
        let device_name = self.camera.lock().name.clone();
        let c = vec![("CCD_ABORT_EXPOSURE", true)]
            .to_command(device_name, String::from("CCD_ABORT_EXPOSURE"));
        if let Err(e) = self.camera.sender().send(c) {
            dbg!(e);
        }
        self.exposure_param.cancel(&self.changes);
    }
}
#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use std::ops::Deref;

    use super::*;

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
