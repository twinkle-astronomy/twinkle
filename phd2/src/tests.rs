use super::*;

use tokio::fs::File;

#[tokio::test]
async fn test_read_session() {
    let file: Phd2Connection<File> = File::open("./src/test_data/session.log")
        .await
        .unwrap()
        .into();
    let mut sub = file.subscribe().await.unwrap();
    while let Ok(event) = sub.recv().await {
        dbg!(event);
    }
}


#[cfg(feature = "test_phd2_simulator")]
mod integration {
    use crate::serialization::Event;
    use std::io::Write;
    use tokio::net::TcpListener;
    
    fn verbose_log(prefix: &str, buf: &[u8]) {
        std::io::stdout().write(prefix.as_bytes()).unwrap();
        std::io::stdout()
            .write(format!("{:?}", std::str::from_utf8(buf).unwrap()).as_bytes())
            .unwrap();
        std::io::stdout().write(&[b'\n']).unwrap();
    }

    #[tokio::test]
    async fn test_integration_phd2_simulator() -> Result<(), ClientError> {
        println!("Starting phd2");
    
        let mut phd2_instance = tokio::process::Command::new("phd2").spawn().unwrap();
    
        let mut phd2: Phd2Connection<_> = loop {
            println!("Connecting to phd2");
            let connection = TcpStream::connect("localhost:4400").await;
            match connection {
                Ok(connection) => {
                    break connection
                        .inspect_read(move |buf: &[u8]| verbose_log("-> ", buf))
                        .inspect_write(move |buf: &[u8]| verbose_log("<- ", buf))
                        .into()
                }
                Err(e) => {
                    dbg!(e);
                    println!("Waiting 1s before trying again");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        };
    
        phd2.stop_capture().await?;
        phd2.set_connected(false).await?;
    
        let profiles = phd2.get_profiles().await?;
        let simulator_profile = profiles
            .iter()
            .find(|item| item.name == "Simulator")
            .expect("Finding 'Simulator' profile.");
        phd2.set_profile(simulator_profile.id).await?;
    
        assert_eq!(phd2.get_profile().await?.name, String::from("Simulator"));
    
        phd2.set_connected(true).await?;
        assert!(phd2.get_connected().await?);
    
        assert_eq!(phd2.get_app_state().await?, State::Stopped);
    
        phd2.clear_calibration(ClearCalibrationParam::Both).await?;
        assert!(!phd2.get_calibrated().await?);
    
        phd2.get_camera_frame_size().await?;
        phd2.get_current_equipment().await?;
    
        phd2.get_cooler_status().await?;
    
        phd2.set_dec_guide_mode(DecGuideMode::Auto).await?;
        assert_eq!(phd2.get_dec_guide_mode().await?, DecGuideMode::Auto);
    
        let exp = phd2.get_exposure_durations().await?[0];
        phd2.set_exposure(exp).await?;
        assert_eq!(phd2.get_exposure().await?, exp);
    
        phd2.set_guide_output_enabled(true).await?;
        assert!(phd2.get_guide_output_enabled().await?);
    
        phd2.get_lock_shift_params().await?;
        phd2.set_lock_shift_params([1.0, 1.0], "arcsec/hr", "RA/Dec")
            .await?;
        phd2.set_lock_shift_enabled(false).await?;
        assert!(!phd2.get_lock_shift_enabled().await?);
    
        let param = &phd2.get_algo_param_names(Axis::Dec).await?[1];
        let value = phd2.get_algo_param(Axis::Dec, param).await?;
        phd2.set_algo_param(Axis::Dec, param, value).await?;
    
        phd2.get_pixel_scale().await?;
        phd2.get_search_region().await?;
        phd2.get_ccd_temperature().await?;
        phd2.get_use_subframes().await?;
    
        // Start doing frame-things
        phd2.capture_single_frame(Duration::from_secs(1), None)
            .await?;
    
        phd2.set_exposure(Duration::from_secs(1)).await?;
        {
            println!("Starting looping");
            let mut events = phd2.subscribe().await.expect("Getting events");
            phd2.loop_().await?;
    
            let mut frame_count = 0;
            loop {
                dbg!(frame_count);
                let event = events.recv().await.unwrap();
                if let Event::LoopingExposures(_) = &event.event {
                    frame_count += 1;
                    if frame_count > 5 {
                        break;
                    }
                }
            }
            phd2.guide_pulse(10, PulseDirection::E, None).await?;
            phd2.find_star(Some([621, 356, 50, 50])).await?;
        }
        {
            println!("Starting guiding");
            let settle = Settle::new(1.5, Duration::from_secs(1), Duration::from_secs(60));
            let mut events = phd2.subscribe().await.expect("Getting events");
            phd2.guide(settle, Some(true), None).await?;
    
            loop {
                let event = events.recv().await.unwrap();
                if let Event::SettleDone(event) = &event.event {
                    assert_eq!(event.status, 0);
                    break;
                }
            }
    
            assert!(phd2.get_calibrated().await?);
            phd2.get_calibration_data(WhichDevice::Mount).await?;
            phd2.set_guide_output_enabled(false).await?;
            assert!(!phd2.get_guide_output_enabled().await?);
            phd2.set_guide_output_enabled(true).await?;
    
            phd2.get_lock_position().await?;
            phd2.get_star_image().await?;
    
            println!("Dither!");
            phd2.dither(10.0, false, settle).await?;
    
            loop {
                let event = events.recv().await.unwrap();
                if let Event::SettleDone(event) = &event.event {
                    assert_eq!(event.status, 0);
                    break;
                }
            }
            phd2.flip_calibration().await?;
            let pos = phd2.get_lock_position().await?.unwrap();
            phd2.set_lock_position(pos[0], pos[1], None).await?;
            phd2.set_paused(true, true).await?;
            assert!(phd2.get_paused().await?);
    
            phd2.stop_capture().await?;
        }
        phd2.shutdown().await?;
    
        let shutdown = tokio::time::timeout(Duration::from_secs(5), phd2_instance.wait()).await;
    
        if let Ok(Ok(status)) = shutdown {
            assert!(status.success());
        } else {
            dbg!(&shutdown);
            phd2_instance.kill().await.expect("Killing phd2");
            panic!("Shutting down phd2 didn't work");
        }
        Ok(())
    }
}
