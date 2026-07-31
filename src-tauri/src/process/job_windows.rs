#![cfg(windows)]

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject, TerminateJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

/// Windows Job Object that owns a process tree.
/// When dropped (with KILL_ON_JOB_CLOSE), remaining processes are terminated.
pub struct JobObject {
    handle: HANDLE,
}

unsafe impl Send for JobObject {}

impl JobObject {
    pub fn new() -> Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(None, None).context("CreateJobObjectW failed")?;

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&info) as u32,
            )
            .context("SetInformationJobObject failed")?;

            Ok(Self { handle })
        }
    }

    pub fn assign_pid(&self, pid: u32) -> Result<()> {
        unsafe {
            let process =
                OpenProcess(PROCESS_ALL_ACCESS, false, pid).context("OpenProcess failed")?;
            let result = AssignProcessToJobObject(self.handle, process);
            let _ = CloseHandle(process);
            result.context("AssignProcessToJobObject failed")?;
        }
        Ok(())
    }

    pub fn terminate(&self) -> Result<()> {
        unsafe {
            TerminateJobObject(self.handle, 1).context("TerminateJobObject failed")?;
        }
        Ok(())
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
