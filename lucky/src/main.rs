use glob::glob;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::num::ParseFloatError;
use std::path::{Path, PathBuf};

use dialoguer::{theme::ColorfulTheme, Input};
use std::io::Write;
use std::{thread, time};

#[derive(Debug)]
enum FilterError {
    FloatError(ParseFloatError),
    IoError(std::io::Error),
    MisingFwhm(),
}

impl From<ParseFloatError> for FilterError {
    fn from(err: ParseFloatError) -> Self {
        FilterError::FloatError(err)
    }
}

impl From<std::io::Error> for FilterError {
    fn from(err: std::io::Error) -> Self {
        FilterError::IoError(err)
    }
}

struct FileFilter {
    re: Regex,
    dst: PathBuf,
    fwhm_threshold: f32,
}

impl FileFilter {
    fn extract_fwhm(&self, path: &PathBuf) -> Result<f32, FilterError> {
        let filename = path.file_name().unwrap().to_string_lossy();
        if let Some(cap) = self.re.captures(&filename) {
            if let Some(m) = cap.get(1) {
                return Ok(m.as_str().parse::<f32>()?);
            }
        }

        return Err(FilterError::MisingFwhm());
    }

    fn handle_file(&self, path: &PathBuf) -> Result<(), FilterError> {
        if !path.is_file() {
            return Ok(());
        }
        let fwhm = self.extract_fwhm(&path)?;
        if fwhm > self.fwhm_threshold {
            fs::remove_file(path)?;
        } else {
            let old_path = path.to_owned();
            let new_path = self.dst.join(path.file_name().unwrap());
            fs::rename(old_path, new_path)?;
        }
        Ok(())
    }
}

fn main() {
    println!(
        r"This script will automatically delete image files if they exceed a certain full width half maximum"
    );
    println!(r"It runs as long as nina is open, and stops monitoring once NINA is closed");
    println!(
        r"In NINA under options, find image file pattern.  Add '\$$FWHM$$pixels' at the start of the filename"
    );
    println!(
        r"If the filename does not include 'pixels', the program will throw an error about converting string to float"
    );
    println!(
        r"For example, $$TARGETNAME$$\$$DATEMINUS12$$\$$IMAGETYPE$$\$$FILTER$$\$$EXPOSURETIME$$\$$FWHM$$pixels_$$DATETIME$$_$$FILTER$$_$$EXPOSURETIME$$s_$$FRAMENR$$"
    );
    println!(
        r"This uses Hocus Focus for FWHM calculation, if not using this plug in then $$HFR$$pixels can be used instead based on half-flux radius"
    );
    println!(r"This is set up for fits files only");
    // perW=float(input("Fraction of the width to keep (0 - 1, where 0 crops all, 1 keeps full image), ex: 70%=0.7: "))
    // perH=float(input("Fraction of the height to keep (0 - 1, where 0 crops all, 1 keeps full image),, ex: 70%=0.7: "))
    let source_dir: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(r"Enter Filepath for monitoring images (e.g. C:\Astrophotography\M31 )")
        .interact()
        .unwrap();
    let mut glob_pattern = PathBuf::from(&source_dir);
    glob_pattern.push("*.fit");
    let mut destination = PathBuf::from(&source_dir);
    destination.push("keepers");

    println!(r"Next select the FWHM or half-flux radius over which images will be deleted");
    println!(r"Note that this is not the FWHM in arc seconds, but in pixels");
    println!(
        r"To convert, take the intended maximum FWHM in arc seconds, and multiply by the focal length (mm) and divide by the pixel size (um) times 206.25"
    );
    println!(
        r"e.g. 3.0 arcsecs threshold*1200mm/3.75um/206.25 yields a threshold FWHM in pixels of 4.8 pixels"
    );
    println!(
        r"HFR is correlated to FWHM but doesn't have a direct conversion.  To use assess the variability of values and select an appropriate threshold"
    );

    let fwhm_threshold: f32 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(
            r"Rejection threshold (FWHM in pixels, or HFR in pixels) above which to delete files",
        )
        .interact_text()
        .unwrap();

    let filter = FileFilter {
        re: Regex::new(r"([\d\.]+)pixels").unwrap(),
        fwhm_threshold: fwhm_threshold,
        dst: destination,
    };

    let mut files_to_ignore = HashSet::new();
    fs::create_dir_all(&filter.dst).unwrap();
    loop {
        for entry in glob(glob_pattern.as_os_str().to_str().unwrap()).expect("Invalid glob") {
            match entry {
                Ok(path) => {
                    if !files_to_ignore.contains(&path) {
                        match filter.handle_file(&path) {
                            Err(MisingFwhm) => {
                                println!("{}: missing fwhm in filename", path.display());
                                files_to_ignore.insert(path);
                            }
                            Err(e) => println!("{}: Error -> {:?}", path.display(), e),
                            _ => (),
                        }
                    }
                }
                Err(e) => println!("Error: {:?}", e),
            }
        }
        thread::sleep(time::Duration::from_millis(100));
    }
}
