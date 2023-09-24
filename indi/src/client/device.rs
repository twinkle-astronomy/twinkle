use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::Write,
    num::Wrapping,
    ops::Deref,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use fitsio::{headers::ReadsKey, FitsFile};

use super::ChangeError;
use crate::*;
use ::client::{
    notify::{self, wait_fn, Notify},
    OnDropFutureExt,
};

/// Internal representation of a device.
#[derive(Debug, Clone)]
pub struct Device {
    name: String,
    parameters: HashMap<String, Arc<Notify<Parameter>>>,
    names: Vec<String>,
    groups: Vec<Option<String>>,
}

impl Device {
    /// Creates a new device named `name` with no parameters.
    pub fn new(name: String) -> Device {
        Device {
            name,
            parameters: HashMap::new(),
            names: vec![],
            groups: vec![],
        }
    }

    /// Updates the current device based on `command`.
    pub fn update<'a>(
        &'a mut self,
        command: serialization::Command,
    ) -> Result<ParamUpdateResult<'a>, UpdateError> {
        match command {
            Command::Message(_) => Ok(ParamUpdateResult::NoUpdate),
            Command::GetProperties(_) => Ok(ParamUpdateResult::NoUpdate),
            Command::DefSwitchVector(command) => self.new_param(command),
            Command::SetSwitchVector(command) => self.update_param(command),
            Command::NewSwitchVector(_) => Ok(ParamUpdateResult::NoUpdate),
            Command::DefNumberVector(command) => self.new_param(command),
            Command::SetNumberVector(command) => self.update_param(command),
            Command::NewNumberVector(_) => Ok(ParamUpdateResult::NoUpdate),
            Command::DefTextVector(command) => self.new_param(command),
            Command::SetTextVector(command) => self.update_param(command),
            Command::NewTextVector(_) => Ok(ParamUpdateResult::NoUpdate),
            Command::DefBlobVector(command) => self.new_param(command),
            Command::SetBlobVector(command) => self.update_param(command),
            Command::DefLightVector(command) => self.new_param(command),
            Command::SetLightVector(command) => self.update_param(command),
            Command::DelProperty(command) => self.delete_param(command.name),
            Command::EnableBlob(_) => Ok(ParamUpdateResult::NoUpdate),
        }
    }

    /// Returns a `&Vec<String>` of all currently know parameter names.
    pub fn parameter_names(&self) -> &Vec<String> {
        return &self.names;
    }

    /// Returns a `&Vec<Option<String>>` of all currently know parameter groups.
    pub fn parameter_groups(&self) -> &Vec<Option<String>> {
        return &self.groups;
    }

    /// Returns a `&Vec<String>` of all currently parameters.
    pub fn get_parameters(&self) -> &HashMap<String, Arc<Notify<Parameter>>> {
        return &self.parameters;
    }

    fn new_param<'a, T: CommandtoParam + std::fmt::Debug>(
        &'a mut self,
        def: T,
    ) -> Result<ParamUpdateResult<'a>, UpdateError> {
        let name = def.get_name().clone();

        self.names.push(name.clone());
        if let None = self.groups.iter().find(|&x| x == def.get_group()) {
            self.groups.push(def.get_group().clone());
        }

        if !self.parameters.contains_key(&name) {
            let param = def.to_param(Wrapping(0));
            self.parameters
                .insert(name.clone(), Arc::new(Notify::new(param)));
        }
        let param = self.parameters.get(&name).unwrap();
        Ok(ParamUpdateResult::ExistingParam(param.lock().unwrap()))
    }

    fn update_param<'a, T: CommandToUpdate>(
        &'a mut self,
        new_command: T,
    ) -> Result<ParamUpdateResult<'a>, UpdateError> {
        match self.parameters.get_mut(&new_command.get_name().clone()) {
            Some(param) => {
                let mut param = param.lock()?;
                *param.gen_mut() += Wrapping(1);
                new_command.update_param(&mut param)?;
                Ok(ParamUpdateResult::ExistingParam(param))
            }
            None => Err(UpdateError::ParameterMissing(
                new_command.get_name().clone(),
            )),
        }
    }

    fn delete_param(&mut self, name: Option<String>) -> Result<ParamUpdateResult<'_>, UpdateError> {
        Ok(ParamUpdateResult::DeletedParams(match name {
            Some(name) => {
                self.names.retain(|n| *n != name);
                let removed = self.parameters.remove(&name);
                if let Some(removed) = removed {
                    vec![removed]
                } else {
                    vec![]
                }
            }
            None => {
                self.names.clear();
                self.parameters.drain().map(|(_, v)| v).collect()
            }
        }))
    }
}

pub enum ParamUpdateResult<'a> {
    NoUpdate,
    ExistingParam(notify::NotifyMutexGuard<'a, Parameter>),
    DeletedParams(Vec<Arc<Notify<Parameter>>>),
}

/// A struct wrapping the raw bytes of a FitsImage.
pub struct FitsImage {
    raw_data: Arc<Vec<u8>>,
}

impl std::fmt::Debug for FitsImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FitsImage")
            .field("raw_data", &self.raw_data.len())
            .finish()
    }
}

impl FitsImage {
    /// Returns a new FitsImage from the given raw data
    pub fn new(data: Arc<Vec<u8>>) -> FitsImage {
        FitsImage { raw_data: data }
    }

    pub fn read_header<T: ReadsKey>(&self, name: &str) -> fitsio::errors::Result<T> {
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

        hdu.read_key(&mut f, name)
    }

    /// Returns an `ndarray::ArrayD<u16>` of the image data contained within `self`.  Currently only supports
    ///   single-channel 16bit images.
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

    /// Saves the FitsImage as a fits file at the given path.  Will create all
    ///  necessary directories if they do not exist.
    pub fn save<T: AsRef<Path>>(&self, path: T) -> Result<(), std::io::Error> {
        if let Some(dir) = path.as_ref().parent() {
            create_dir_all(dir)?;
        }
        let mut f = File::create(path)?;
        f.write_all(&self.raw_data)?;
        Ok(())
    }
}

/// Object representing a device connected to an INDI server.
#[derive(Clone)]
pub struct ActiveDevice {
    name: String,
    device: Arc<Notify<Device>>,
    command_sender: crossbeam_channel::Sender<Command>,
}

impl ActiveDevice {
    pub fn new(
        name: String,
        device: Arc<Notify<Device>>,
        command_sender: crossbeam_channel::Sender<Command>,
    ) -> ActiveDevice {
        ActiveDevice {
            name,
            device,
            command_sender,
        }
    }

    /// Returns the sender used to send commands
    ///  to the associated INDI server connection.
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
    /// Returns the requested parameter, waiting up to 1 second for it to be defined
    ///  by the connected INDI server.  
    pub async fn get_parameter(
        &self,
        param_name: &str,
    ) -> Result<Arc<Notify<Parameter>>, notify::Error<Command>> {
        let subs = self.device.subscribe()?;
        wait_fn(subs, Duration::from_secs(1), |device| {
            Ok(match device.get_parameters().get(param_name) {
                Some(param) => notify::Status::Complete(param.clone()),
                None => notify::Status::Pending,
            })
        })
        .await
    }

    /// Ensures that the parameter named `param_name` has the given value with the INDI server.
    /// If the INDI server's value does not match the `values` given, it will send the
    /// INDI server commands necessary to change values, and wait for the server
    /// to confirm the desired values.  This method will wait for the parameter's
    /// `timeout` (or 60 seconds if not defined by the server) for the parameter value to match
    ///  the desired value before timing out.
    /// # Arguments
    /// * `param_name` - The name of the parameter you wish to change.  If the parameter does not exist,
    ///                  This method will wait up to 1 second for it to exist before timing out.
    /// * `values` - The target values of the named parameter.  This argument must be of a type that
    ///              can be compared to the named parameter, and converted into an INDI command if nessesary.
    ///              See [crate::TryEq] and [crate::ToCommand] for type conversions.  If the given values do not
    ///              match the parameter types nothing be communicated to the server and aa [ChangeError::TypeMismatch]
    ///              will be returned.
    /// # Example
    /// ```no_run
    /// use indi::*;
    /// use indi::client::device::ActiveDevice;
    /// async fn change_usage_example(filter_wheel: ActiveDevice) {
    ///     filter_wheel.change(
    ///         "FILTER_SLOT",
    ///         vec![("FILTER_SLOT_VALUE", 5.0)],
    ///     ).await.expect("Changing filter");
    /// }
    /// ```
    pub async fn change<P: Clone + TryEq<Parameter> + ToCommand<P> + 'static>(
        &self,
        param_name: &str,
        values: P,
    ) -> Result<Arc<Parameter>, ChangeError<Command>> {
        let device_name = self.name.clone();

        let param = self.get_parameter(param_name).await?;

        let subscription = param.subscribe()?;
        let timeout = {
            let param = param.lock()?;

            if !values.try_eq(&param)? {
                let c = values
                    .clone()
                    .to_command(device_name, String::from(param_name));
                self.sender().send(c)?;
            }

            param.get_timeout().unwrap_or(60)
        };

        let res = wait_fn::<_, ChangeError<Command>, _, _>(
            subscription,
            Duration::from_secs(timeout.into()),
            move |next| {
                if *next.get_state() == PropertyState::Alert {
                    return Err(ChangeError::PropertyError);
                }
                if values.try_eq(&next)? {
                    Ok(notify::Status::Complete(next.clone()))
                } else {
                    Ok(notify::Status::Pending)
                }
            },
        )
        .await?;

        Ok(res)
    }

    /// Sends an `EnableBlob` command to the connected INDI server for the named parameter.  This must be called
    ///  on a Blob parameter with a value of either [crate::BlobEnable::Only] or [crate::BlobEnable::Also] for
    ///  the server to send image data.
    /// # Arguments
    /// * `param_name` - The optional name of the blob parameter to configure.  If `Some(param_name)` is provided
    ///                  and the parameter does not exist, this method will wait up to 1 second for it to exist
    ///                  before timing out.
    /// * `enabled` - The [crate::BlobEnable] value you wish to send to the server.
    /// # Example
    /// ```no_run
    /// use indi::client::device::ActiveDevice;
    /// use indi::BlobEnable;
    /// async fn enable_blob_usage_example(camera: ActiveDevice) {
    ///     // Instruct server to send blob data along with regular parameter updates
    ///     camera.enable_blob(
    ///         Some("CCD1"), BlobEnable::Also,
    ///     ).await.expect("Enabling blobs");
    /// }
    /// ```
    pub async fn enable_blob(
        &self,
        name: Option<&str>,
        enabled: crate::BlobEnable,
    ) -> Result<(), notify::Error<Command>> {
        // Wait for device and paramater to exist
        if let Some(name) = name {
            let _ = self.get_parameter(name).await?;
        }
        let device_name = self.device.lock()?.name.clone();
        if let Err(_) = self.sender().send(Command::EnableBlob(EnableBlob {
            device: device_name,
            name: name.map(|x| String::from(x)),
            enabled,
        })) {
            return Err(notify::Error::Canceled);
        };
        Ok(())
    }

    /// Returns a [FitsImage] after exposing the camera device for `exposure` seconds.
    ///   Currently this method is only tested on the ZWO ASI 294MM Pro.  `enable_blob` must be
    ///   called against the `"CCD1"` parameter prior to the usage of this method.
    /// # Arguments
    /// * `exposure` - How long to expose the camera in seconds.
    /// # Example
    /// ```no_run
    /// use std::time::Duration;
    /// use indi::client::device::ActiveDevice;
    /// use indi::*;
    /// async fn capture_image_usage_example(camera: ActiveDevice) {
    ///     let image = camera.capture_image(Duration::from_secs(30)).await.expect("Capturing an image");
    /// }
    /// ```
    pub async fn capture_image(
        &self,
        exposure: Duration,
    ) -> Result<FitsImage, ChangeError<Command>> {
        let image_param = self.get_parameter("CCD1").await?;
        self.capture_image_from_param(exposure, &image_param).await
    }

    /// Returns a [FitsImage] after exposing the camera device for `exposure` seconds.
    ///   Currently this method is only tested on the ZWO ASI 294MM Pro.  `enable_blob` must be
    ///   called against the `"CCD1"` parameter prior to the usage of this method.
    /// # Arguments
    /// * `exposure` - How long to expose the camera in seconds.
    /// * `image_param` - The parameter to read the fits data from.  This does not need to be
    ///                   from the same client connection, enabling you to use a dedicated client
    ///                   connection for retrieving images.
    /// # Example
    /// ```no_run
    /// use std::time::Duration;
    /// use std::net::TcpStream;
    /// use indi::client::Client;
    /// use indi::client::device::{ActiveDevice, FitsImage};
    /// use indi::*;
    /// async fn capture_image_from_param_usage_example(client: Client<TcpStream>, blob_client: Client<TcpStream>) {
    ///     // Get the camera device from the client dedicated to transfering blob data.
    ///     let blob_camera = blob_client.get_device::<()>("ZWO CCD ASI294MM Pro").await.unwrap();
    ///     // Enable blobs
    ///     blob_camera.enable_blob(Some("CCD1"), indi::BlobEnable::Only).await.unwrap();
    ///     // Get the parameter used to transfer images from the camera after an exposure
    ///     let ccd_blob_param = blob_camera.get_parameter("CCD1").await.unwrap();
    ///
    ///     // Use the non-blob client to get the device used to control the camera
    ///     let camera = client.get_device::<()>("ZWO CCD ASI294MM Pro").await.unwrap();
    ///     // Capture an image, getting the blob data from a client connection dedicated
    ///     //  to transfering blob data.
    ///     let image: FitsImage = camera.capture_image_from_param(Duration::from_secs(30), &ccd_blob_param).await.unwrap();
    /// }
    /// ```
    pub async fn capture_image_from_param(
        &self,
        exposure: Duration,
        image_param: &Notify<Parameter>,
    ) -> Result<FitsImage, ChangeError<Command>> {
        let exposure = exposure.as_secs_f64();
        let exposure_param = self.get_parameter("CCD_EXPOSURE").await?;
        let device_name = self.name.clone();

        let image_changes = image_param.changes();
        let exposure_changes = exposure_param.changes();

        let c = vec![("CCD_EXPOSURE_VALUE", exposure)]
            .to_command(device_name.clone(), String::from("CCD_EXPOSURE"));
        self.sender().send(c)?;

        let mut previous_exposure_secs = exposure;

        let exposing = Arc::new(Mutex::new(true));
        let exposing_ondrop = exposing.clone();
        // Wait for exposure to run out
        wait_fn(
            exposure_changes,
            Duration::from_secs(exposure.ceil() as u64 + 10),
            move |exposure_param| {
                // Exposure goes to idle when canceled
                if *exposure_param.get_state() == PropertyState::Idle {
                    dbg!("Exposure was canceled");
                    return Err(ChangeError::<Command>::Canceled);
                }
                let remaining_exposure: f64 = exposure_param
                    .get_values::<HashMap<String, Number>>()?
                    .get("CCD_EXPOSURE_VALUE")
                    .and_then(|x| Some(x.value.into()))
                    .expect("Missing CCD_EXPOSURE_VALUE from CCD_EXPOSURE parameter");
                // Image is done exposing, new image data should be sent very soon
                if remaining_exposure == 0.0 {
                    *exposing.lock().unwrap() = false;
                    return Ok(notify::Status::Complete(exposure_param));
                }
                // remaining exposure didn't change, nothing to check
                if previous_exposure_secs == remaining_exposure {
                    return Ok(notify::Status::Pending);
                }
                // Make sure exposure changed by a reasonable amount.
                // If another exposure is started before our exposure is finished
                //  there is a good chance the remaining exposure won't have changed
                //  by the amount of time since the last tick.
                let exposure_change = Duration::from_millis(
                    ((previous_exposure_secs - remaining_exposure).abs() * 1000.0) as u64,
                );
                if exposure_change > Duration::from_millis(1100) {
                    return Err(ChangeError::Canceled);
                }
                previous_exposure_secs = remaining_exposure;

                // Nothing funky happened, so we're still waiting for the
                // exposure to finish.
                Ok(notify::Status::Pending)
            },
        )
        .on_drop(|| {
            if *exposing_ondrop.lock().unwrap() {
                let c = vec![("CCD_ABORT_EXPOSURE", true)]
                    .to_command(device_name.clone(), String::from("CCD_ABORT_EXPOSURE"));
                if let Err(e) = self.sender().send(c) {
                    dbg!(e);
                }
            }
        })
        .await?;

        Ok(wait_fn(image_changes, Duration::from_secs(60), move |ccd| {
            // We've been called before the next image has come in.
            if let Some(image_data) = ccd
                .get_values::<HashMap<String, crate::Blob>>()?
                .get("CCD1")
            {
                if let Some(bytes) = &image_data.value {
                    Ok(notify::Status::Complete(FitsImage::new(bytes.clone())))
                } else {
                    dbg!("No image data");
                    Err(ChangeError::PropertyError)
                }
            } else {
                dbg!("Missing CCD1");
                Err(ChangeError::PropertyError)
            }
        })
        .await?)
    }

    pub async fn pixel_scale(&self) -> f64 {
        let ccd_info = self.get_parameter("CCD_INFO").await.unwrap();

        let ccd_binning = self.get_parameter("CCD_BINNING").await.unwrap();

        let binning: f64 = {
            let ccd_binning_lock = ccd_binning.lock().unwrap();
            ccd_binning_lock
                .get_values::<HashMap<String, Number>>()
                .unwrap()
                .get("HOR_BIN")
                .unwrap()
                .value
                .into()
        };
        let pixel_scale = {
            let ccd_info_lock = ccd_info.lock().unwrap();
            let ccd_pixel_size: f64 = ccd_info_lock
                .get_values::<HashMap<String, Number>>()
                .unwrap()
                .get("CCD_PIXEL_SIZE")
                .unwrap()
                .value
                .into();
            binning * ccd_pixel_size / 800.0 * 180.0 / std::f64::consts::PI * 3.6
        };

        pixel_scale
    }

    pub async fn filter_names(&self) -> Result<HashMap<String, usize>, ChangeError<Command>> {
        let filter_names: HashMap<String, usize> = {
            let filter_names_param = self.get_parameter("FILTER_NAME").await?;
            let l = filter_names_param.lock().unwrap();
            l.get_values::<HashMap<String, Text>>()?
                .iter()
                .map(|(slot, name)| {
                    let slot = slot
                        .split("_")
                        .last()
                        .map(|x| x.parse::<usize>().unwrap())
                        .unwrap();
                    (name.value.clone(), slot)
                })
                .collect()
        };
        Ok(filter_names)
    }

    pub async fn change_filter(&self, filter_name: &str) -> Result<(), ChangeError<Command>> {
        let filter_names: HashMap<String, usize> = self.filter_names().await?;
        match filter_names.get(filter_name) {
            Some(slot) => {
                self.change("FILTER_SLOT", vec![("FILTER_SLOT_VALUE", *slot as f64)])
                    .await?;
                Ok(())
            }
            None => Err(ChangeError::PropertyError),
        }
    }
}
#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use std::ops::Deref;

    use crate::client::device;

    use super::*;

    #[test]
    fn test_update_switch() {
        let mut device = Device::new(String::from("CCD Simulator"));
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);
        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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

    #[test]
    fn test_update_number() {
        let mut device = device::Device::new(String::from("CCD Simulator"));
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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

    #[test]
    fn test_update_text() {
        let mut device = Device::new(String::from("CCD Simulator"));
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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
            .unwrap();
        assert_eq!(device.get_parameters().len(), 1);

        {
            let param = device
                .get_parameters()
                .get("Exposure")
                .unwrap()
                .lock()
                .unwrap();
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
