//! FTDI devices through the FTDI library (ftd2xx) where it is installed, as the reference loads it at run time
//! (re/spec/25-emboss.md sections 16.2 and 21.1): the devices are listed by their descriptions for models with
//! YesFTDI, opened by description at 38400 baud, 8 data bits, 1 stop bit, no parity, and written and read through the
//! library. Without the library the list is empty and nothing is opened.

use bc_core::emboss_out::{Failure, Link};
use libloading::Library;
use std::cell::RefCell;
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_ulong};
use std::sync::OnceLock;

type Handle = *mut c_void;
type CreateList = unsafe extern "system" fn(*mut u32) -> u32;
type Detail = unsafe extern "system" fn(u32, *mut u32, *mut u32, *mut u32, *mut u32, *mut c_char, *mut c_char, *mut Handle) -> u32;
type OpenEx = unsafe extern "system" fn(*mut c_void, u32, *mut Handle) -> u32;
type Close = unsafe extern "system" fn(Handle) -> u32;
type ReadWrite = unsafe extern "system" fn(Handle, *mut c_void, u32, *mut u32) -> u32;
type SetBaud = unsafe extern "system" fn(Handle, c_ulong) -> u32;
type SetData = unsafe extern "system" fn(Handle, u8, u8, u8) -> u32;

const OK: u32 = 0;
const OPEN_BY_DESCRIPTION: u32 = 2;

/// The library's file names, the system's own search first; the FTDI installers put it in the system folder on
/// Windows and in /usr/local/lib on Linux and macOS.
const NAMES: &[&str] = if cfg!(windows) {
    &["ftd2xx.dll"]
} else if cfg!(target_os = "macos") {
    &["libftd2xx.dylib", "/usr/local/lib/libftd2xx.dylib"]
} else {
    &["libftd2xx.so", "/usr/local/lib/libftd2xx.so", "libftd2xx.so.1"]
};

fn library() -> Option<&'static Library> {
    static LIB: OnceLock<Option<Library>> = OnceLock::new();
    LIB.get_or_init(|| NAMES.iter().find_map(|n| unsafe { Library::new(n) }.ok())).as_ref()
}

/// A function of the library by name.
fn func<T: Copy>(name: &[u8]) -> Option<T> {
    library().and_then(|l| unsafe { l.get::<T>(name) }.ok().map(|s| *s))
}

thread_local! {
    /// The devices as last listed: the reference lists them once when the setup sheet opens (spec 25 16.1).
    static DEVICES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Lists the FTDI devices again by their descriptions (FT_CreateDeviceInfoList, then each device's description;
/// the reference reads the same descriptions with FT_GetDeviceInfoList). Called when the setup sheet opens.
pub fn refresh() -> Vec<String> {
    let list = list();
    DEVICES.with(|d| *d.borrow_mut() = list.clone());
    list
}

/// The devices as last listed, listing them on first use.
pub fn devices() -> Vec<String> {
    let known = DEVICES.with(|d| d.borrow().clone());
    if known.is_empty() {
        refresh()
    } else {
        known
    }
}

fn list() -> Vec<String> {
    let (Some(create), Some(detail)) = (func::<CreateList>(b"FT_CreateDeviceInfoList\0"), func::<Detail>(b"FT_GetDeviceInfoDetail\0")) else {
        return Vec::new();
    };
    let mut n = 0u32;
    if unsafe { create(&mut n) } != OK {
        return Vec::new();
    }
    (0..n)
        .filter_map(|i| {
            let (mut flags, mut kind, mut id, mut loc) = (0u32, 0u32, 0u32, 0u32);
            let mut serial = [0 as c_char; 16];
            let mut description = [0 as c_char; 64];
            let mut handle: Handle = std::ptr::null_mut();
            let status = unsafe {
                detail(i, &mut flags, &mut kind, &mut id, &mut loc, serial.as_mut_ptr(), description.as_mut_ptr(), &mut handle)
            };
            description[63] = 0;
            (status == OK).then(|| unsafe { CStr::from_ptr(description.as_ptr()) }.to_string_lossy().into_owned())
        })
        .filter(|d| !d.is_empty())
        .collect()
}

/// Whether `name` is one of the listed FTDI devices.
pub fn is_device(name: &str) -> bool {
    bc_core::emboss_setup::is_ftdi(name, &devices())
}

/// An open FTDI device.
pub struct Device {
    handle: Handle,
    write: ReadWrite,
    read: ReadWrite,
    close: Close,
}

impl Device {
    /// Opens the device described `description` and sets its line (spec 25 21.1); any failure gives the port
    /// message, as the reference reports an open or line-setting error.
    pub fn open(description: &str) -> Result<Device, Failure> {
        let (Some(open), Some(write), Some(read), Some(close), Some(baud), Some(data)) = (
            func::<OpenEx>(b"FT_OpenEx\0"),
            func::<ReadWrite>(b"FT_Write\0"),
            func::<ReadWrite>(b"FT_Read\0"),
            func::<Close>(b"FT_Close\0"),
            func::<SetBaud>(b"FT_SetBaudRate\0"),
            func::<SetData>(b"FT_SetDataCharacteristics\0"),
        ) else {
            return Err(Failure::Port);
        };
        let name = CString::new(description.trim()).map_err(|_| Failure::Port)?;
        let mut handle: Handle = std::ptr::null_mut();
        if unsafe { open(name.as_ptr() as *mut c_void, OPEN_BY_DESCRIPTION, &mut handle) } != OK || handle.is_null() {
            return Err(Failure::Port);
        }
        let device = Device { handle, write, read, close };
        // 38400 baud; 8 data bits, 1 stop bit (FT_STOP_BITS_1 = 0), no parity (FT_PARITY_NONE = 0).
        if unsafe { baud(handle, 38400) } != OK || unsafe { data(handle, 8, 0, 0) } != OK {
            return Err(Failure::Port);
        }
        Ok(device)
    }
}

impl Link for Device {
    /// FT_Write until every byte is written. The reference tries again after a failed call; BrailleCraft ends the
    /// job there, so an unplugged embosser gives the port message rather than a program that waits for ever.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        let mut done = 0usize;
        while done < bytes.len() {
            let mut written = 0u32;
            let rest = &bytes[done..];
            let status = unsafe { (self.write)(self.handle, rest.as_ptr() as *mut c_void, rest.len() as u32, &mut written) };
            if status != OK {
                return Err(Failure::Port);
            }
            done += written as usize;
        }
        Ok(())
    }

    /// FT_Read of one byte, asked again until one arrives, as the reference does; a failed call ends the job.
    fn read_byte(&mut self) -> Result<u8, Failure> {
        loop {
            let mut b = 0u8;
            let mut got = 0u32;
            if unsafe { (self.read)(self.handle, &mut b as *mut u8 as *mut c_void, 1, &mut got) } != OK {
                return Err(Failure::Port);
            }
            if got == 1 {
                return Ok(b);
            }
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { (self.close)(self.handle) };
    }
}

/// Sends a byte job to the device described `description`, in writes of at most 512 bytes as the reference's writer
/// sends them (spec 25 21.2).
pub fn send(description: &str, bytes: &[u8]) -> Result<(), Failure> {
    let mut d = Device::open(description)?;
    bytes.chunks(512).try_for_each(|c| d.write_all(c))
}

/// Sends the Typewriter driver's messages to the device described `description`, with the embosser's status check
/// and answer around each (spec 25 21.2).
pub fn send_typewriter(description: &str, messages: &[Vec<u8>]) -> Result<(), Failure> {
    let mut d = Device::open(description)?;
    bc_core::emboss_out::typewriter_send(&mut d, messages)
}
