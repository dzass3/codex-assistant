use std::path::Path;

#[cfg(not(windows))]
use std::fs;

#[cfg(not(windows))]
pub(crate) fn protect_owned_path(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if path.is_dir() { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|_| "Theme state permissions could not be set".to_owned())
}

#[cfg(windows)]
pub(crate) fn protect_owned_path(path: &Path) -> Result<(), String> {
    windows_acl::protect_current_user(path)
}

#[cfg(not(windows))]
pub(crate) fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(temporary, destination).map_err(|_| "Theme state could not be replaced".to_owned())
}

#[cfg(windows)]
pub(crate) fn replace_existing(temporary: &Path, destination: &Path) -> Result<(), String> {
    windows_acl::move_file(temporary, destination, true)
}

#[cfg(windows)]
mod windows_acl {
    use std::{ffi::c_void, path::Path};

    use windows_sys::{
        core::PWSTR,
        Win32::{
            Foundation::{CloseHandle, LocalFree},
            Security::{
                Authorization::{
                    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                },
                GetTokenInformation, SetFileSecurityW, TokenUser, DACL_SECURITY_INFORMATION,
                PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
            },
            Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    pub fn protect_current_user(path: &Path) -> Result<(), String> {
        let sid = current_user_sid()?;
        let descriptor = wide(&format!("D:P(A;;FA;;;{sid})"));
        let mut security_descriptor = std::ptr::null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                descriptor.as_ptr(),
                1,
                &mut security_descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err("Theme state permissions could not be set".to_owned());
        }
        let path = wide_path(path)?;
        let result = unsafe {
            SetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                security_descriptor,
            )
        };
        unsafe { LocalFree(security_descriptor) };
        if result == 0 {
            Err("Theme state permissions could not be set".to_owned())
        } else {
            Ok(())
        }
    }

    pub fn move_file(source: &Path, destination: &Path, replace: bool) -> Result<(), String> {
        let source = wide_path(source)?;
        let destination = wide_path(destination)?;
        let mut flags = MOVEFILE_WRITE_THROUGH;
        if replace {
            flags |= MOVEFILE_REPLACE_EXISTING;
        }
        let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
        if result == 0 {
            Err("Theme state could not be replaced".to_owned())
        } else {
            Ok(())
        }
    }

    fn current_user_sid() -> Result<String, String> {
        let mut token = std::ptr::null_mut();
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        if opened == 0 {
            return Err("Theme state permissions could not be set".to_owned());
        }
        let mut length = 0;
        unsafe {
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut length);
        }
        if length == 0 {
            unsafe { CloseHandle(token) };
            return Err("Theme state permissions could not be set".to_owned());
        }
        let mut bytes = vec![0_u8; length as usize];
        let read = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                bytes.as_mut_ptr() as *mut c_void,
                length,
                &mut length,
            )
        };
        unsafe { CloseHandle(token) };
        if read == 0 {
            return Err("Theme state permissions could not be set".to_owned());
        }
        let token_user = bytes.as_ptr() as *const TOKEN_USER;
        let mut sid: PWSTR = std::ptr::null_mut();
        let converted = unsafe { ConvertSidToStringSidW((*token_user).User.Sid, &mut sid) };
        if converted == 0 || sid.is_null() {
            return Err("Theme state permissions could not be set".to_owned());
        }
        let result = unsafe {
            let length = (0..).take_while(|index| *sid.add(*index) != 0).count();
            String::from_utf16_lossy(std::slice::from_raw_parts(sid, length))
        };
        unsafe { LocalFree(sid as *mut c_void) };
        Ok(result)
    }

    fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
        path.to_str()
            .map(wide)
            .ok_or_else(|| "Theme state path is invalid".to_owned())
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
