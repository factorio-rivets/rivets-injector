//! A small windows application to inject the Rivets DLL into Factorio.

use crate::common;
use anyhow::{anyhow, bail, Context, Result};
use dll_syringe::process::{BorrowedProcess, ProcessModule};
use dll_syringe::{process::OwnedProcess, Syringe};
use std::ffi::CString;
use std::fs;
use std::fs::File;
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
    println!("Injecting DLL into Factorio process...");
    println!("\t{}", dll_path.as_ref().display());

    syringe
        .inject(dll_path)
        .map_err(|e| anyhow!("Failed to inject DLL: {e}"))
}

fn rpc(
    syringe: &Syringe,
    module: ProcessModule<BorrowedProcess>,
    read_path: impl AsRef<Path>,
    write_path: impl AsRef<Path>,
) -> Result<()> {
    let rpc = unsafe {
        syringe.get_payload_procedure::<fn(PathBuf, PathBuf) -> Option<String>>(
            module,
            "payload_procedure",
        )
    }?
    .ok_or(anyhow!("Failed to get RPC procedure"))?;
    match rpc.call(
        &read_path.as_ref().to_path_buf(),
        &write_path.as_ref().to_path_buf(),
    )? {
        Some(err) => bail!(err),
        None => Ok(()),
    }
}

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

pub fn run() -> Result<()> {
    let bin_path = common::get_bin_folder()?;
    let (read_path, write_path) = common::get_data_dirs(&bin_path)?;

    let (stdout_read, _) = create_pipe()?;
    let mut reader = unsafe { std::fs::File::from_raw_handle(stdout_read.0) };

    let factorio_path = bin_path.join("factorio.exe");

    let factorio_path = CString::new(factorio_path.as_os_str().to_string_lossy().into_owned())?;
    println!("Factorio path: {factorio_path:?}");
    let factorio_path = PCSTR(factorio_path.as_ptr().cast());

    let dll_path = common::extract_rivets_lib(&read_path, &write_path)?;

    let factorio_process_information: PROCESS_INFORMATION = start_factorio(factorio_path)?;
    println!("Factorio process started.");
    let syringe = get_syringe().inspect_err(|_| {
        attempt_kill_factorio(factorio_process_information);
    })?;
    let module = inject_dll(&syringe, &dll_path).inspect_err(|_| {
        attempt_kill_factorio(factorio_process_information);
    })?;
    rpc(&syringe, module, read_path, write_path).inspect_err(|_| {
        attempt_kill_factorio(factorio_process_information);
    })?;
    println!("DLL injected successfully.");

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
    if let Err(e) = unsafe { TerminateProcess(factorio_process_information.hProcess, 0) } {
        eprintln!("Failed to terminate Factorio process after experiencing a rivets error: {e}");
        eprintln!("You likely have a ghost Factorio process running. Please kill it manually via task manager.");
    }
}
