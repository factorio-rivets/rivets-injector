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

        let entry_point: libloading::Symbol<unsafe extern "C" fn()> =
            match lib.get(b"rivets_entry_point") {
                Ok(entry_point) => entry_point,
                Err(e) => {
                    eprintln!("Failed to get rivets entry point: {e}");
                    return;
                }
            };

        entry_point();
    }
}
