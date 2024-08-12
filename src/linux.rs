use crate::common;

pub fn run() {
    let Ok(bin_path) = std::env::current_exe() else {
        eprintln!("Failed to get binary path");
        return;
    };

    let Some(bin_folder) = bin_path.parent() else {
        eprintln!("Failed to get binary folder");
        return;
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

    // TODO: load rivets library
}
