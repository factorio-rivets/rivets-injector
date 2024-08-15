//! A small windows application to inject the Rivets DLL into Factorio.

use crate::common;
use anyhow::{anyhow, bail, Context, Result};
use dll_syringe::process::{BorrowedProcess, ProcessModule};
use dll_syringe::{process::OwnedProcess, Syringe};
use mod_util::mod_list::ModList;
use mod_util::mod_loader::ModError;
use rivets::SymbolCache;
use std::ffi::CString;
use std::io;
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use windows::core::{PCSTR, PSTR};
use windows::Win32::Foundation::{
    CloseHandle, SetHandleInformation, BOOL, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessA, ResumeThread, TerminateProcess, CREATE_SUSPENDED, PROCESS_INFORMATION,
    STARTUPINFOA,
};

fn get_syringe() -> Result<Syringe> {
    let Some(process) = OwnedProcess::find_first_by_name("factorio") else {
        bail!("Factorio process not found.");
    };

    Ok(Syringe::for_process(process))
}

fn inject_dll(
    syringe: &Syringe,
    dll_path: impl AsRef<Path>,
) -> Result<ProcessModule<BorrowedProcess<'_>>> {
    syringe
        .inject(dll_path)
        .map_err(|e| anyhow!("Failed to inject DLL: {e}"))
}

fn rpc(
    syringe: &Syringe,
    module: ProcessModule<BorrowedProcess>,
    symbol_cache: &SymbolCache,
) -> Result<()> {
    type Rpc = fn(symbol_cache: SymbolCache) -> Option<String>;

    let rpc = unsafe { syringe.get_payload_procedure::<Rpc>(module, "rivets_finalize") }
        .context("Failed to get RPC procedure")?
        .ok_or_else(|| {
            anyhow!("Failed to get RPC procedure. rivets_finalize does not exist in rivets.dll")
        })?;

    println!("RPC procedure address located.");

    match rpc.call(symbol_cache).context("RPC paniced")? {
        Some(err) => bail!(format!("Failed to preform RPC: {err}")),
        None => Ok(()),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn create_pipe() -> Result<(HANDLE, HANDLE)> {
    let mut stdout_read = INVALID_HANDLE_VALUE;
    let mut stdout_write = INVALID_HANDLE_VALUE;
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: BOOL(1), // The child process needs to inherit the handle
        lpSecurityDescriptor: std::ptr::null_mut(),
    };
    unsafe {
        CreatePipe(&mut stdout_read, &mut stdout_write, Some(&sa), 0)?;
        SetHandleInformation(stdout_read, 0, HANDLE_FLAG_INHERIT)?;
    }

    Ok((stdout_read, stdout_write))
}

fn start_factorio(factorio_path: PCSTR) -> Result<PROCESS_INFORMATION> {
    let mut startup_info: STARTUPINFOA = unsafe { std::mem::zeroed() };
    startup_info.cb = std::mem::size_of::<STARTUPINFOA>().try_into()?;
    let mut factorio_process_information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let process_result = unsafe {
        CreateProcessA(
            factorio_path,
            PSTR::null(),
            None,
            None,
            false,
            CREATE_SUSPENDED,
            None,
            PCSTR::null(),
            &startup_info,
            &mut factorio_process_information,
        )
    };

    if let Err(err) = process_result {
        bail!("Failed to create Factorio process: {err}");
    }

    Ok(factorio_process_information)
}

fn extract_all_mods_libs(
    read_data: impl AsRef<Path>,
    write_data: impl AsRef<Path>,
) -> Result<Vec<(String, PathBuf)>> {
    #[cfg(target_os = "linux")]
    static DYNAMIC_LIBRARY_SUFFIX: &str = ".so";
    #[cfg(target_os = "linux")]
    static RIVETS_LIB: &str = "rivets.so";
    #[cfg(target_os = "windows")]
    static DYNAMIC_LIBRARY_SUFFIX: &str = ".dll";
    #[cfg(target_os = "windows")]
    static RIVETS_LIB: &str = "rivets.dll";

    let mut result = vec![];
    let mut mod_list = ModList::generate_custom(&read_data, &write_data)
        .context("Failed to find your Factorio mods directory.")?;
    mod_list.load().context(
        "Failed to parse mods-list.json file. Please ensure this file exists and is well-formated.",
    )?;

    let (all_active_mods, mod_load_order) = mod_list.active_with_order();
    for mod_name in mod_load_order {
        let current_mod = all_active_mods
            .get(&mod_name)
            .expect("The list of active mods contains all mods in the load order");

        let lib = match current_mod.get_file(RIVETS_LIB) {
            Err(ModError::PathDoesNotExist(_)) => continue,
            Err(ModError::ZipError(e))
                if e.to_string() == "specified file not found in archive" =>
            {
                continue
            }
            Ok(lib) => lib,
            Err(e) => return Err(anyhow!(e).context(format!("Detected corrupted mod file! {mod_name} does not have proper zip encodings. Please try to reinstall this mod."))),
        };

        std::fs::create_dir_all(write_data.as_ref().join("temp/rivets")).context(
            "Could not find the factorio/temp directory. Please manually create this directory.",
        )?;

        let extracted_lib_name = format!("{mod_name}{DYNAMIC_LIBRARY_SUFFIX}");
        let lib_path = write_data
            .as_ref()
            .join("temp/rivets")
            .join(extracted_lib_name);
        std::fs::write(&lib_path, lib)
            .context("Failed to write the extracted rivets .dll library to the temp directory.")?;

        result.push((mod_name, lib_path));
    }

    Ok(result)
}

pub fn run() -> Result<()> {
    println!("Starting Rivets injector...");
    let bin_path = common::get_bin_folder().context("Failed to find factorio.exe. You must run this program from the Factorio installation directory.")?;
    let (read_path, write_path) = common::get_data_dirs(&bin_path).context("Failed to parse Factorio's config-path.cfg and config.ini files. Please ensure these files actually exist. If not, reinstall Factorio.")?;

    println!("Searching for Rivets mod .dll libraries...");
    let all_mods = extract_all_mods_libs(read_path, write_path)?;
    println!(
        "Mods list loaded successfully. Found {} enabled rivets mods.",
        all_mods.len()
    );

    let (stdout_read, _) = create_pipe().context(
        "Failed to create operating system pipes. Try running rivets with elevated permissions.",
    )?;
    let mut reader = unsafe { std::fs::File::from_raw_handle(stdout_read.0) };

    let factorio_path = bin_path.join("factorio.exe");
    let pdb_path = bin_path.join("factorio.pdb");

    let factorio_path = CString::new(factorio_path.as_os_str().to_string_lossy().into_owned()).context("Failed to convert Factorio path to CString. Do you have any invalid chars in your Factorio path? Please report this on the Rivets Github.")?;
    println!("Factorio path: {factorio_path:?}");
    let factorio_path = PCSTR(factorio_path.as_ptr().cast());

    let factorio_process_information: PROCESS_INFORMATION = start_factorio(factorio_path)?;
    println!("Factorio process started.");
    let syringe = get_syringe().inspect_err(|_| {
        attempt_kill_factorio(factorio_process_information);
    })?;

    let symbol_cache = SymbolCache::new(pdb_path, "factorio.exe").inspect_err(|_| {
        attempt_kill_factorio(factorio_process_information);
    }).context("Failed to create symbol cache.")?;

    for (mod_name, dll_path) in all_mods {
        println!("Discovered rivets mod: {mod_name}");
        println!("Injecting DLL into Factorio process...");

        let module = inject_dll(&syringe, &dll_path)
            .inspect_err(|_| {
                attempt_kill_factorio(factorio_process_information);
            })
            .context(format!("DLL injection failed for {mod_name}"))?;

        println!("DLL injected successfully. Performing DLL initialization RPC...");

        rpc(&syringe, module, &symbol_cache)
            .inspect_err(|_| {
                attempt_kill_factorio(factorio_process_information);
            })
            .context(format!("Function detouring failed for {mod_name}"))?;

        println!("RPC completed successfully.");
    }

    println!("Returning control back into Factorio process...");

    unsafe {
        ResumeThread(factorio_process_information.hThread);
        CloseHandle(factorio_process_information.hThread).ok();
        CloseHandle(factorio_process_information.hProcess).ok();
    }

    // Duplicate the factorio stdout stream onto our own stdout using OS pipes.
    io::copy(&mut reader, &mut io::stdout())?;

    Ok(())
}

fn attempt_kill_factorio(factorio_process_information: PROCESS_INFORMATION) {
    let _ = unsafe { TerminateProcess(factorio_process_information.hProcess, 0) };
}
