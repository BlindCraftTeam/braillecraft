//! Where an embossing job's bytes go (re/spec/25-emboss.md section 5): a file, a port (parallel, serial or USB
//! printer device), or a printer queue that takes the bytes as they are. The file holds exactly the bytes a port
//! would get. The reference program runs on Windows only; how ports and printer queues are reached on Linux and
//! macOS is BrailleCraft's own choice (spec 25 section 6).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Serial port settings of an embosser setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serial {
    pub baud: u32,
    /// 0 none, 1 even, 2 odd, 3 mark.
    pub parity: usize,
    pub data_bits: u32,
    pub stop_bits: u32,
    /// 0 hardware (RTS/CTS), 1 software (XON/XOFF), 2 none.
    pub handshaking: usize,
}

/// Where the bytes go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    File(PathBuf),
    /// A port by the name the setup lists (lpt1, com1, usb); serial settings for a com port when the setup
    /// initializes it.
    Port { name: String, serial: Option<Serial> },
    /// A printer queue by name; empty for the system's default printer.
    Printer(String),
}

/// Why the bytes could not be sent; each has the reference program's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The output file cannot be created (message 2380).
    File,
    /// The port cannot be opened (message 2384).
    Port,
    /// The embosser could not be reached through the printer queue (message 2382); the system's own text.
    Printer(String),
}

impl Failure {
    pub fn message(&self) -> String {
        match self {
            Failure::File => "Embossing failed because the output file cannot be created. Please check to be sure that the path to the file exists and that you have permission to write to it.".into(),
            Failure::Port => "Embossing failed because the port cannot be opened. Please check the the port exists and is not in use by other software.".into(),
            // The reference ends with the source line and file of the failure; here the system's own text stands there.
            Failure::Printer(detail) => format!(
                "BrailleCraft is unable to communicate with your braille embosser. Please check that it is correctly installed. For example, you my be attempting to output to a USB port that is not valid. In the event that there is another cause of this problem, you can report that embossing failed due to an error: {detail}"
            ),
        }
    }
}

/// Sends `bytes` (a job titled `title`) to `target`.
pub fn send(target: &Target, title: &str, bytes: &[u8]) -> Result<(), Failure> {
    match target {
        Target::File(path) => std::fs::write(path, bytes).map_err(|_| Failure::File),
        Target::Port { name, serial } => {
            let device = port_device(name).ok_or(Failure::Port)?;
            if let Some(s) = serial.as_ref().filter(|_| name.to_ascii_lowercase().starts_with("com")) {
                set_serial(&device, s)?;
            }
            let mut f = std::fs::OpenOptions::new().write(true).open(&device).map_err(|_| Failure::Port)?;
            f.write_all(bytes).and_then(|_| f.flush()).map_err(|_| Failure::Port)
        }
        Target::Printer(name) => send_to_printer(name, title, bytes),
    }
}

/// The device a port name stands for. On Windows the names are the system's own; elsewhere lptN is the Nth parallel
/// printer device, comN the Nth serial device and usb the first USB printer device.
pub fn port_device(name: &str) -> Option<PathBuf> {
    let lower = name.trim().to_ascii_lowercase();
    let number = |prefix: &str| lower.strip_prefix(prefix).and_then(|n| n.parse::<u32>().ok()).filter(|n| *n >= 1);
    if cfg!(windows) {
        return if lower.starts_with("com") || lower.starts_with("lpt") {
            Some(PathBuf::from(format!(r"\\.\{}", lower.to_ascii_uppercase())))
        } else {
            None
        };
    }
    if let Some(n) = number("lpt") {
        Some(PathBuf::from(format!("/dev/lp{}", n - 1)))
    } else if let Some(n) = number("com") {
        let base = if cfg!(target_os = "macos") { "/dev/cu.serial" } else { "/dev/ttyS" };
        Some(PathBuf::from(format!("{base}{}", n - 1)))
    } else if lower == "usb" {
        Some(PathBuf::from(if cfg!(target_os = "macos") { "/dev/cu.usbserial" } else { "/dev/usb/lp0" }))
    } else {
        None
    }
}

/// Sets a serial device's speed, framing and handshaking with the system's own tool.
fn set_serial(device: &Path, s: &Serial) -> Result<(), Failure> {
    let run = |cmd: &mut Command| cmd.stdout(Stdio::null()).stderr(Stdio::null()).status().map(|st| st.success()).unwrap_or(false);
    let ok = if cfg!(windows) {
        let port = device.to_string_lossy().trim_start_matches(r"\\.\").to_string();
        let parity = ["n", "e", "o", "m"][s.parity.min(3)];
        let (xon, rts) = match s.handshaking {
            0 => ("off", "hs"),
            1 => ("on", "on"),
            _ => ("off", "on"),
        };
        run(Command::new("mode").args([
            format!("{port}:"),
            format!("baud={}", s.baud),
            format!("parity={parity}"),
            format!("data={}", s.data_bits),
            format!("stop={}", s.stop_bits),
            format!("xon={xon}"),
            format!("rts={rts}"),
        ]))
    } else {
        let flag = if cfg!(target_os = "macos") { "-f" } else { "-F" };
        let mut args: Vec<String> = vec![flag.into(), device.to_string_lossy().into(), s.baud.to_string(), "raw".into(), format!("cs{}", s.data_bits)];
        args.push(if s.stop_bits == 2 { "cstopb".into() } else { "-cstopb".into() });
        match s.parity {
            1 => args.extend(["parenb".into(), "-parodd".into()]),
            2 => args.extend(["parenb".into(), "parodd".into()]),
            // Mark parity has no setting of its own here; the line runs without parity.
            _ => args.push("-parenb".into()),
        }
        match s.handshaking {
            0 => args.extend(["crtscts".into(), "-ixon".into(), "-ixoff".into()]),
            1 => args.extend(["-crtscts".into(), "ixon".into(), "ixoff".into()]),
            _ => args.extend(["-crtscts".into(), "-ixon".into(), "-ixoff".into()]),
        }
        run(Command::new("stty").args(&args))
    };
    if ok { Ok(()) } else { Err(Failure::Port) }
}

/// Sends the bytes to a printer queue as raw data: through the spooler with the data type RAW on Windows, and with
/// lp's raw option elsewhere.
#[cfg(not(windows))]
fn send_to_printer(name: &str, title: &str, bytes: &[u8]) -> Result<(), Failure> {
    let mut cmd = Command::new("lp");
    cmd.args(["-o", "raw", "-t", title]);
    if !name.trim().is_empty() {
        cmd.args(["-d", name.trim()]);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Failure::Printer(e.to_string()))?;
    if let Some(mut input) = child.stdin.take() {
        input.write_all(bytes).map_err(|e| Failure::Printer(e.to_string()))?;
    }
    let out = child.wait_with_output().map_err(|e| Failure::Printer(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Failure::Printer(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

#[cfg(windows)]
fn send_to_printer(name: &str, title: &str, bytes: &[u8]) -> Result<(), Failure> {
    use windows_sys::Win32::Graphics::Printing::{
        ClosePrinter, EndDocPrinter, EndPagePrinter, GetDefaultPrinterW, OpenPrinterW, StartDocPrinterW, StartPagePrinter,
        WritePrinter, DOC_INFO_1W,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let fail = || Failure::Printer(std::io::Error::last_os_error().to_string());
    let mut printer = if name.trim().is_empty() {
        let mut size: u32 = 0;
        unsafe { GetDefaultPrinterW(std::ptr::null_mut(), &mut size) };
        if size == 0 {
            return Err(fail());
        }
        let mut buf = vec![0u16; size as usize];
        if unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut size) } == 0 {
            return Err(fail());
        }
        buf
    } else {
        wide(name.trim())
    };
    let mut doc_name = wide(title);
    let mut data_type = wide("RAW");
    unsafe {
        let mut handle = std::mem::zeroed();
        if OpenPrinterW(printer.as_mut_ptr(), &mut handle, std::ptr::null()) == 0 {
            return Err(fail());
        }
        let info = DOC_INFO_1W { pDocName: doc_name.as_mut_ptr(), pOutputFile: std::ptr::null_mut(), pDatatype: data_type.as_mut_ptr() };
        let mut result = Err(fail());
        if StartDocPrinterW(handle, 1, &info) != 0 {
            if StartPagePrinter(handle) != 0 {
                let mut written: u32 = 0;
                let ok = WritePrinter(handle, bytes.as_ptr().cast(), bytes.len() as u32, &mut written) != 0 && written as usize == bytes.len();
                result = if ok { Ok(()) } else { Err(fail()) };
                EndPagePrinter(handle);
            }
            EndDocPrinter(handle);
        }
        ClosePrinter(handle);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_output_holds_the_bytes() {
        let path = std::env::temp_dir().join(format!("bm-emboss-out-{}.brl", std::process::id()));
        send(&Target::File(path.clone()), "t", b"ab\r\n\x0c").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"ab\r\n\x0c".to_vec());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(send(&Target::File(PathBuf::from("/no/such/folder/x.brl")), "t", b"a"), Err(Failure::File));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn port_names() {
        assert_eq!(port_device("lpt1"), Some(PathBuf::from("/dev/lp0")));
        assert_eq!(port_device("COM2"), Some(PathBuf::from("/dev/ttyS1")));
        assert_eq!(port_device("usb"), Some(PathBuf::from("/dev/usb/lp0")));
        assert_eq!(port_device("lpt0"), None);
        assert_eq!(send(&Target::Port { name: "lpt9".into(), serial: None }, "t", b"a"), Err(Failure::Port));
    }
}
