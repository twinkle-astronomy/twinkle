extern crate fitsio;

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    force: bool,

    filenames: Vec<String>,
    #[arg(short, long)]
    name: String,
    #[arg(short, long)]
    value: String,
}

fn main() {
    let args = Args::parse();

    let filenames = args.filenames;

    for filename in filenames {
        let mut fptr = fitsio::FitsFile::edit(&filename).expect("Opening fits file");
        let hdu = fptr.primary_hdu().expect("Getting primary HDU");

        match hdu.read_key::<String>(&mut fptr, &args.name) {
            Ok(value) => match args.force {
                true => {
                    println!(
                        "{}/{}: Replacing '{}' with '{}'.",
                        &filename, &args.name, &value, &args.value
                    );
                    hdu.write_key(&mut fptr, &args.name, args.value.clone())
                        .expect("Writing header");
                }
                false => {
                    println!(
                        "{}/{}: Already has value '{}'.  Skipping.",
                        &filename, &args.name, &value
                    );
                }
            },
            Err(e) => {
                if let fitsio::errors::Error::Fits(e) = e {
                    if e.status as u32 == fitsio::sys::KEY_NO_EXIST {
                        println!("{}/{}: Adding '{}'.", &filename, &args.name, &args.value);
                        hdu.write_key(&mut fptr, &args.name, args.value.clone())
                            .expect("Writing header");
                    } else {
                        eprintln!("{}/{}: Error '{}'.", &filename, &args.name, e.message);
                    }
                } else {
                    eprintln!("{}/{}: Unexpected error '{}'.", &filename, &args.name, e);
                }
            }
        }
    }
}
