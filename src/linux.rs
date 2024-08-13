use abi_stable::std_types::{ROption, RString};

use crate::common;

pub fn run() {
    let bin_folder = match common::get_bin_folder() {
        Ok(folder) => folder,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    let (read_data, write_data) = match common::get_data_dirs(bin_folder) {
        Ok(dirs) => dirs,
        Err(e) => {
            eprintln!("Failed to get data directories: {e}");
            return;
        }
    };

    let rivets_lib = match common::extract_rivets_lib(&read_data, &write_data) {
        Ok(lib) => lib,
        Err(e) => {
            eprintln!("Failed to extract rivets library: {e}");
            return;
        }
    };

    unsafe {
        let lib = match libloading::Library::new(rivets_lib) {
            Ok(lib) => lib,
            Err(e) => {
                eprintln!("Failed to load rivets library: {e}");
                return;
            }
        };

        let setup: libloading::Symbol<extern "C" fn(RString, RString) -> ROption<RString>> =
            match lib.get(b"rivetslib_setup\0") {
                Ok(setup) => setup,
                Err(e) => {
                    eprintln!("Failed to get rivetslib entry point: {e}");
                    return;
                }
            };

        if let ROption::RSome(err) = setup(
            read_data.to_string_lossy().into(),
            write_data.to_string_lossy().into(),
        ) {
            eprintln!("{err}");
        }
    }
}
