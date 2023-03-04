use std::{
    mem,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::ChangeError;

use super::device::{ActiveDevice, FitsImage};

pub enum ActionError<E> {
    Change(ChangeError<E>),
    AlreadyComplete,
}

impl<E> From<ChangeError<E>> for ActionError<E> {
    fn from(value: ChangeError<E>) -> Self {
        ActionError::Change(value)
    }
}

pub struct CaptureImage {
    started_at: Instant,
    exposure: Duration,
    camera: Arc<ActiveDevice>,
    // current_subscription: Subscription<>,
    join: Option<JoinHandle<Result<FitsImage, ActionError<()>>>>,
}

impl CaptureImage {
    pub fn start(camera: ActiveDevice, exposure: Duration) -> CaptureImage {
        let camera = Arc::new(camera);
        let exposure_secs = exposure.as_secs_f64();
        CaptureImage {
            started_at: Instant::now(),
            camera: camera.clone(),
            exposure: exposure.clone(),
            join: Some(thread::spawn(
                move || -> Result<FitsImage, ActionError<()>> {
                    let image = camera.capture_image(exposure_secs)?;
                    Ok(image)
                },
            )),
        }
    }
    pub fn remaining(&self) -> Duration {
        let elapsed = self.started_at.elapsed();
        if elapsed > self.exposure {
            return Duration::from_secs(0);
        } else {
            elapsed - self.exposure
        }
    }

    pub fn cancel(&self) -> Result<(), ActionError<()>> {
        self.camera
            .change("CCD_ABORT_EXPOSURE", vec![("CCD_ABORT_EXPOSURE", true)])?;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        match &self.join {
            Some(join) => !join.is_finished(),
            None => false,
        }
    }

    pub fn wait(&mut self) -> Result<FitsImage, ActionError<()>> {
        if let Some(join) = mem::replace(&mut self.join, None) {
            join.join().unwrap()
        } else {
            Err(ActionError::AlreadyComplete)
        }
    }
}
