use glob::glob;
use std::path::{Path, PathBuf};
use regex::Regex;

use std::fs;

fn handle_file(path: &mut PathBuf, re: &Regex) {
    if !path.is_file() {
        return;
    }

    let cap = re.captures(path.file_name().unwrap().to_str().unwrap());
    match cap {
        Some(cap) => {
            match cap.get(1) {
                Some(m) => {
                    let fwhm = m.as_str().parse::<f32>().unwrap();
                    if fwhm > 3.0 {
                        fs::remove_file(path);
                    } else {
                        let old_path = path.to_owned();
                        let new_path = Path::new("test/keeps").join(path.file_name().unwrap());
                        fs::rename(old_path, new_path).unwrap();
                    }
                    println!("foo: {}", fwhm);
                },
                _ => {return}
            }
        },
        _ => {return}
    }
}

fn main() {
    let re = Regex::new(r"([\d\.]+)pixels").unwrap();
    fs::create_dir_all("test/keeps").unwrap();
    for entry in glob("test/*.fit").expect("Failed to find files") {
        match entry {
            Ok(mut path) => handle_file(&mut path, &re),
            Err(e) => println!("Error: {:?}", e)
        }
    }

}
