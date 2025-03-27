#[cfg(feature = "fitsio")]
use std::{
    collections::HashMap,
    fs::{create_dir_all, File},
    io::Write,
    ops::Deref,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(not(feature = "fitsio"))]
use std::{collections::HashMap, ops::Deref, sync::Arc, time::Duration};

#[cfg(feature = "fitsio")]
use fitsio::{headers::ReadsKey, FitsFile};
use twinkle_client::notify::{self, wait_fn, Notify};

use crate::{
    device::Device, serialization, Command, EnableBlob, Number, Parameter, PropertyState, Text,
    ToCommand, TryEq,
};

use super::{active_parameter::ActiveParameter, ChangeError};

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

#[cfg(feature = "fitsio")]
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

#[derive(Debug)]
pub enum SendError<T> {
    Disconnected,
    SendError(tokio::sync::mpsc::error::SendError<T>),
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for SendError<T> {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        SendError::SendError(value)
    }
}

/// Object representing a device connected to an INDI server.
#[derive(Clone)]
pub struct ActiveDevice {
    name: String,
    device: Arc<Notify<Device<Notify<Parameter>>>>,
    command_sender: Option<tokio::sync::mpsc::UnboundedSender<serialization::Command>>,
}

impl ActiveDevice {
    pub fn new(
        name: String,
        device: Arc<Notify<Device<Notify<Parameter>>>>,
        command_sender: Option<tokio::sync::mpsc::UnboundedSender<serialization::Command>>,
    ) -> ActiveDevice {
        ActiveDevice {
            name,
            device,
            command_sender,
        }
    }

    /// Returns the sender used to send commands
    ///  to the associated INDI server connection.
    pub fn send(&self, c: Command) -> Result<(), SendError<Command>> {
        if let Some(command_sender) = &self.command_sender {
            command_sender.send(c)?;
        }
        Ok(())
    }
}

impl Deref for ActiveDevice {
    type Target = Arc<Notify<Device<Notify<Parameter>>>>;

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
        let subs = self.device.subscribe().await;
        wait_fn(subs, Duration::from_secs(1), |device| {
            Ok(match device.get_parameters().get(param_name) {
                Some(param) => notify::Status::Complete(param.clone()),
                None => notify::Status::Pending,
            })
        })
        .await
    }

    pub async fn parameter(&self, param_name: &str) -> Option<ActiveParameter> {
        Some(ActiveParameter::new(
            self.clone(),
            self.device
                .read()
                .await
                .get_parameters()
                .get(param_name)?
                .clone(),
        ))
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
    /// use indi::client::active_device::ActiveDevice;
    /// async fn change_usage_example(filter_wheel: ActiveDevice) {
    ///     filter_wheel.change(
    ///         "FILTER_SLOT",
    ///         vec![("FILTER_SLOT_VALUE", 5.0)],
    ///     ).await.expect("Changing filter");
    /// }
    /// ```
    pub async fn change<'a, P: Clone + TryEq<Parameter> + ToCommand<P>>(
        &'a self,
        param_name: &'a str,
        values: P,
    ) -> Result<notify::NotifyArc<Parameter>, ChangeError<Command>> {
        let device_name = self.name.clone();

        let (subscription, timeout) = {
            let param = self.get_parameter(param_name).await?;
            let subscription = param.subscribe().await;
            let timeout = {
                let param = param.read().await;

                if !values.try_eq(&param)? {
                    let c = values
                        .clone()
                        .to_command(device_name, String::from(param_name));
                    self.send(c)?;
                }

                param.get_timeout().unwrap_or(60)
            }
            .max(1);
            (subscription, timeout)
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
    /// use indi::client::active_device::ActiveDevice;
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
        let device_name = self.device.read().await.get_name().clone();
        if let Err(_) = self.send(Command::EnableBlob(EnableBlob {
            device: device_name,
            name: name.map(|x| String::from(x)),
            enabled,
        })) {
            return Err(notify::Error::Canceled);
        };
        Ok(())
    }

    #[cfg(feature = "fitsio")]
    /// Returns a [FitsImage] after exposing the camera device for `exposure` seconds.
    ///   Currently this method is only tested on the ZWO ASI 294MM Pro.  `enable_blob` must be
    ///   called against the `"CCD1"` parameter prior to the usage of this method.
    /// # Arguments
    /// * `exposure` - How long to expose the camera in seconds.
    /// # Example
    /// ```no_run
    /// use std::time::Duration;
    /// use indi::client::active_device::ActiveDevice;
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

    #[cfg(feature = "fitsio")]
    /// Waits for and returns the next image from the given parameter.
    pub async fn next_image(
        &self,
        image_param: &Notify<Parameter>,
    ) -> Result<FitsImage, ChangeError<Command>> {
        let sub = image_param.changes();

        Ok(wait_fn(sub, Duration::from_secs(60), move |ccd| {
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

    #[cfg(feature = "fitsio")]
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
    /// use indi::client::active_device::{ActiveDevice, FitsImage};
    /// use indi::*;
    /// async fn capture_image_from_param_usage_example(client: Client, blob_client: Client) {
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
        use twinkle_client::OnDropFutureExt;

        let exposure = exposure.as_secs_f64();
        let exposure_param = self.get_parameter("CCD_EXPOSURE").await?;
        let device_name = self.name.clone();

        let image_changes = image_param.changes();
        let exposure_changes = exposure_param.changes();

        let c = vec![("CCD_EXPOSURE_VALUE", exposure)]
            .to_command(device_name.clone(), String::from("CCD_EXPOSURE"));
        self.send(c)?;

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
                if let Err(e) = self.send(c) {
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
            let ccd_binning_lock = ccd_binning.read().await;
            ccd_binning_lock
                .get_values::<HashMap<String, Number>>()
                .unwrap()
                .get("HOR_BIN")
                .unwrap()
                .value
                .into()
        };
        let pixel_scale = {
            let ccd_info_lock = ccd_info.read().await;
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
            let l = filter_names_param.read().await;
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
