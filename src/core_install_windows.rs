//! Consumer adapter for the shared zsys Windows private file contract.
/// Read existing security without taking ownership or rewriting the DACL.
pub fn check_private(file: &std::fs::File) -> Result<(), String> {
    zsys::windows::privacy::check_private(file).map_err(|error| error.to_string())
}

/// The caller must have exclusively created this object, before payload writes.
pub fn protect_created(file: &std::fs::File) -> Result<(), String> {
    zsys::windows::privacy::protect_created(
        file,
        zsys::windows::privacy::DirectoryInheritance::ObjectOnly,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent native fixture construction and snapshots retained from
    // the pre-migration consumer. No production permission checker is copied.
    use std::{ffi::c_void, os::windows::io::AsRawHandle, ptr};
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(process: *mut c_void, access: u32, token: *mut *mut c_void) -> i32;
        fn GetTokenInformation(
            token: *mut c_void,
            class: u32,
            buffer: *mut c_void,
            size: u32,
            needed: *mut u32,
        ) -> i32;
        fn ConvertSidToStringSidW(sid: *mut c_void, text: *mut *mut u16) -> i32;
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
        fn GetSecurityDescriptorDacl(
            descriptor: *mut c_void,
            present: *mut i32,
            acl: *mut *mut c_void,
            defaulted: *mut i32,
        ) -> i32;
        fn SetSecurityInfo(
            handle: *mut c_void,
            kind: u32,
            information: u32,
            owner: *mut c_void,
            group: *mut c_void,
            dacl: *mut c_void,
            sacl: *mut c_void,
        ) -> u32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn GetSecurityInfo(
            handle: *mut c_void,
            kind: u32,
            information: u32,
            owner: *mut *mut c_void,
            group: *mut *mut c_void,
            dacl: *mut *mut c_void,
            sacl: *mut *mut c_void,
            descriptor: *mut *mut c_void,
        ) -> u32;
    }
    struct Token(*mut c_void);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Local(*mut c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn private_user() -> Result<Vec<usize>, String> {
        let error = || "cannot inspect the current Windows account".to_string();
        let mut token = ptr::null_mut();
        // SAFETY: all output buffers are owned and sized according to Win32 results.
        unsafe {
            if OpenProcessToken(GetCurrentProcess(), 8, &mut token) == 0 {
                return Err(error());
            }
            let token = Token(token);
            let mut size = 0;
            GetTokenInformation(token.0, 1, ptr::null_mut(), 0, &mut size);
            if size == 0 || size > 65536 {
                return Err(error());
            }
            let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(token.0, 1, storage.as_mut_ptr().cast(), size, &mut size) == 0 {
                return Err(error());
            }
            Ok(storage)
        }
    }

    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Seek, SeekFrom, Write},
        os::windows::fs::OpenOptionsExt,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor: *const c_void,
            revision: u32,
            information: u32,
            text: *mut *mut u16,
            length: *mut u32,
        ) -> i32;
    }

    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Temporary(PathBuf);
    impl Temporary {
        fn new() -> (Self, File) {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "zcall-gui-native-security-{}-{stamp}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .access_mode(0x80000000 | 0x40000000 | 0x00040000 | 0x00080000)
                .share_mode(0x1 | 0x2)
                .open(root.join("private"))
                .unwrap();
            (Self(root), file)
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn current_sid_text() -> String {
        let user = private_user().unwrap();
        let sid = unsafe { *(user.as_ptr().cast::<*mut c_void>()) };
        let mut text = ptr::null_mut();
        // SAFETY: user owns the TOKEN_USER storage and Local releases the native string.
        assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut text) }, 0);
        let memory = Local(text.cast());
        let mut length = 0;
        while length < 256 && unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        assert!(length < 256);
        let value =
            String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).unwrap();
        drop(memory);
        value
    }

    fn replace_test_dacl(file: &File, foreign: bool, protected: bool) {
        replace_test_dacl_flags(file, foreign, protected, "");
    }

    fn replace_test_dacl_flags(file: &File, foreign: bool, protected: bool, flags: &str) {
        let sid = current_sid_text();
        let sddl = format!(
            "D:{}(A;{flags};FA;;;{sid})(A;{flags};FA;;;SY){}",
            if protected { "P" } else { "" },
            if foreign { "(A;;FR;;;WD)" } else { "" }
        )
        .encode_utf16()
        .chain([0])
        .collect::<Vec<_>>();
        let mut descriptor = ptr::null_mut();
        // SAFETY: these tests only mutate their exclusively created temporary file.
        unsafe {
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut descriptor,
                    ptr::null_mut()
                ),
                0
            );
            let descriptor = Local(descriptor);
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = ptr::null_mut();
            assert_ne!(
                GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted),
                0
            );
            assert_ne!(present, 0);
            assert!(!acl.is_null());
            assert_eq!(
                SetSecurityInfo(
                    file.as_raw_handle(),
                    1,
                    4 | if protected { 0x80000000 } else { 0x20000000 },
                    ptr::null_mut(),
                    ptr::null_mut(),
                    acl,
                    ptr::null_mut()
                ),
                0
            );
        }
    }

    fn security_snapshot(file: &File) -> Vec<u16> {
        let mut descriptor = ptr::null_mut();
        let mut text = ptr::null_mut();
        let mut length = 0;
        // SAFETY: both native allocations are guarded until their contents are copied.
        unsafe {
            assert_eq!(
                GetSecurityInfo(
                    file.as_raw_handle(),
                    1,
                    1 | 4,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut descriptor
                ),
                0
            );
            let descriptor = Local(descriptor);
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor.0,
                    1,
                    1 | 4,
                    &mut text,
                    &mut length
                ),
                0
            );
            let text_memory = Local(text.cast());
            assert!((1..=65536).contains(&length));
            let copy = std::slice::from_raw_parts(text, length as usize).to_vec();
            drop(text_memory);
            copy
        }
    }

    #[test]
    fn creation_protects_owner_and_current_account_system_access_before_data() {
        let (_temporary, mut file) = Temporary::new();
        protect_created(&file).unwrap();
        check_private(&file).unwrap();
        file.write_all(b"private payload").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"private payload");
    }

    #[test]
    fn existing_foreign_read_access_is_rejected_without_rewriting_security_or_data() {
        let (_temporary, mut file) = Temporary::new();
        protect_created(&file).unwrap();
        file.write_all(b"existing payload").unwrap();
        replace_test_dacl(&file, true, true);
        let before = security_snapshot(&file);
        assert!(
            check_private(&file).is_err(),
            "Everyone read access must be rejected"
        );
        assert!(
            before == security_snapshot(&file),
            "read-only validation changed the existing ACL"
        );
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"existing payload");
    }

    #[test]
    fn existing_unprotected_dacl_is_rejected_without_repair() {
        let (_temporary, file) = Temporary::new();
        protect_created(&file).unwrap();
        replace_test_dacl(&file, false, false);
        let before = security_snapshot(&file);
        assert!(
            check_private(&file).is_err(),
            "inheritable security must be rejected"
        );
        assert!(
            before == security_snapshot(&file),
            "validation must not protect an existing DACL"
        );
    }

    #[test]
    fn core_installer_directories_are_read_without_changing_their_private_acl() {
        let (temporary, file) = Temporary::new();
        drop(file);
        let path = temporary.0.join("transactions");
        fs::create_dir(&path).unwrap();
        let directory = OpenOptions::new()
            .access_mode(0x0002_0000 | 0x0004_0000 | 0x0008_0000)
            .share_mode(0x1 | 0x2)
            .custom_flags(0x0200_0000 | 0x0020_0000)
            .open(&path)
            .unwrap();
        protect_created(&directory).unwrap();
        // Match the GUI's existing-directory reader: READ_CONTROL only, no repair.
        let reader = OpenOptions::new()
            .access_mode(0x0002_0000)
            .share_mode(0x1 | 0x2)
            .custom_flags(0x0200_0000 | 0x0020_0000)
            .open(&path)
            .unwrap();
        for flags in ["", "OICI"] {
            replace_test_dacl_flags(&directory, false, true, flags);
            let before = security_snapshot(&reader);
            check_private(&reader).unwrap();
            // Session::find_transaction uses this same existing-directory path.
            super::super::directory(&path).unwrap();
            assert_eq!(security_snapshot(&reader), before);
        }
        for flags in ["OI", "CI", "OICINP", "OICIIO"] {
            replace_test_dacl_flags(&directory, false, true, flags);
            let before = security_snapshot(&reader);
            assert!(check_private(&reader).is_err(), "accepted {flags}");
            assert!(super::super::directory(&path).is_err());
            assert_eq!(security_snapshot(&reader), before);
        }
        replace_test_dacl_flags(&directory, true, true, "OICI");
        let before = security_snapshot(&reader);
        assert!(check_private(&reader).is_err());
        assert_eq!(security_snapshot(&reader), before);
        replace_test_dacl_flags(&directory, false, true, "OICI");
    }
}
