use glob::glob;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::num::ParseFloatError;
use std::path::{Path, PathBuf};

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
    dst: String,
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
            let new_path = Path::new(&self.dst).join(path.file_name().unwrap());
            fs::rename(old_path, new_path)?;
        }
        Ok(())
    }
}

fn main() {
    let mut files_to_ignore = HashSet::new();

    let filter = FileFilter {
        re: Regex::new(r"([\d\.]+)pixels").unwrap(),
        fwhm_threshold: 3.0,
        dst: "test/keeps".to_string(),
    };

    fs::create_dir_all("test/keeps").unwrap();
    loop {
        for entry in glob("test/*.fit").expect("Invalid glob") {
            match entry {
                Ok(path) => {
                    if !files_to_ignore.contains(&path) {
                        if let Err(e) = filter.handle_file(&path) {
                            println!("{}: Error -> {:?}", path.display(), e);
                            files_to_ignore.insert(path);
                        }
                    }
                }
                Err(e) => println!("Error: {:?}", e),
            }
        }
        thread::sleep(time::Duration::from_millis(100));
    }
}
