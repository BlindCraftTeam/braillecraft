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

/// A two-way connection to an embosser that answers, such as an FTDI device (spec 25 21.1).
pub trait Link {
    /// Writes all of `bytes`.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Failure>;
    /// Reads one byte, waiting for it.
    fn read_byte(&mut self) -> Result<u8, Failure>;
}

const ACK: u8 = 0x06;
const NAK: u8 = 0x15;

/// Sends the Typewriter driver's messages (emboss::typewriter_job) over `link` as the reference's writer does (spec
/// 25 21.2): before each message the status check (command 7; the embosser answers ACK and two bytes, asked again
/// until both are 0; any other first byte ends the job), then the message, then its answer (bytes read until ACK; NAK
/// ends the job). A failed answer gives the port message.
pub fn typewriter_send(link: &mut dyn Link, messages: &[Vec<u8>]) -> Result<(), Failure> {
    for m in messages {
        loop {
            link.write_all(&[0x00, 0x07, 0xFF])?;
            if link.read_byte()? != ACK {
                return Err(Failure::Port);
            }
            let (a, b) = (link.read_byte()?, link.read_byte()?);
            if a == 0 && b == 0 {
                break;
            }
        }
        link.write_all(m)?;
        loop {
            match link.read_byte()? {
                ACK => break,
                NAK => return Err(Failure::Port),
                _ => {}
            }
        }
    }
    Ok(())
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

/// A printer the system has: its name and its driver. On Windows the driver is the printer's driver name, as the
/// reference reads it (spec 25 14.1); elsewhere it is the printer's make and model as CUPS reports it, up to the
/// " - " CUPS puts before the driver's own name ("Epson LQ-590 - CUPS+Gutenprint v5.3.4" is "Epson LQ-590"), which
/// is BrailleCraft's stand-in for the driver name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Printer {
    pub name: String,
    pub driver: String,
}

/// The system's local and connected printers in the order the system gives them (spec 25 14.1 and 14.6: the
/// reference lists EnumPrinters local and connections, level 2).
#[cfg(windows)]
pub fn printers() -> Vec<Printer> {
    use windows_sys::Win32::Graphics::Printing::{EnumPrintersW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W};
    fn text(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut n = 0;
        unsafe {
            while *p.add(n) != 0 {
                n += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
        }
    }
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut needed, mut count) = (0u32, 0u32);
    unsafe { EnumPrintersW(flags, std::ptr::null(), 2, std::ptr::null_mut(), 0, &mut needed, &mut count) };
    if needed == 0 {
        return Vec::new();
    }
    // A buffer of PRINTER_INFO_2W records followed by their strings, aligned for the records.
    let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
    if unsafe { EnumPrintersW(flags, std::ptr::null(), 2, buf.as_mut_ptr().cast(), needed, &mut needed, &mut count) } == 0 {
        return Vec::new();
    }
    let infos = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<PRINTER_INFO_2W>(), count as usize) };
    infos.iter().map(|i| Printer { name: text(i.pPrinterName), driver: text(i.pDriverName) }).collect()
}

/// The system's printers as CUPS lists them (lpstat -e), each with its make and model (lpoptions -p).
#[cfg(not(windows))]
pub fn printers() -> Vec<Printer> {
    let Ok(out) = Command::new("lpstat").arg("-e").stderr(Stdio::null()).output() else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|name| {
            let options = Command::new("lpoptions").args(["-p", name]).stderr(Stdio::null()).output();
            let model = options.ok().filter(|o| o.status.success()).map(|o| make_and_model(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
            let driver = model.split(" - ").next().unwrap_or("").trim().to_string();
            Printer { name: name.to_string(), driver }
        })
        .collect()
}

/// The printer-make-and-model value in lpoptions output (name=value pairs separated by spaces, a value with spaces
/// in single quotes), or "" when it is not there.
pub fn make_and_model(options: &str) -> String {
    let key = "printer-make-and-model=";
    let Some(at) = options.find(key) else { return String::new() };
    let rest = &options[at + key.len()..];
    match rest.strip_prefix('\'') {
        Some(quoted) => quoted.split('\'').next().unwrap_or("").to_string(),
        None => rest.split_whitespace().next().unwrap_or("").replace("\\ ", " "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An embosser that answers from a script of bytes and records what it was sent.
    struct Scripted {
        answers: std::collections::VecDeque<u8>,
        sent: Vec<u8>,
    }
    impl Link for Scripted {
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), Failure> {
            self.sent.extend_from_slice(bytes);
            Ok(())
        }
        fn read_byte(&mut self) -> Result<u8, Failure> {
            self.answers.pop_front().ok_or(Failure::Port)
        }
    }

    #[test]
    fn typewriter_handshake() {
        // Busy once (ACK 0 1), then ready (ACK 0 0), the line, then a stray byte and ACK.
        let mut e = Scripted { answers: [6, 0, 1, 6, 0, 0, 9, 6].into_iter().collect(), sent: Vec::new() };
        let line = vec![0x00, 0x08, 0x01, 0xFF];
        typewriter_send(&mut e, &[line.clone()]).unwrap();
        let status = [0x00, 0x07, 0xFF];
        assert_eq!(e.sent, [&status[..], &status[..], &line[..]].concat());
        // A first status byte that is not ACK ends the job; so does a NAK answer.
        let mut e = Scripted { answers: [0x15].into_iter().collect(), sent: Vec::new() };
        assert_eq!(typewriter_send(&mut e, &[line.clone()]), Err(Failure::Port));
        let mut e = Scripted { answers: [6, 0, 0, 0x15].into_iter().collect(), sent: Vec::new() };
        assert_eq!(typewriter_send(&mut e, &[line]), Err(Failure::Port));
    }

    #[test]
    fn make_and_model_from_lpoptions() {
        let out = "copies=1 device-uri=usb://EPSON/LQ-590 printer-info='LQ 590' printer-make-and-model='Epson LQ-590 - CUPS+Gutenprint v5.3.4' printer-state=3";
        assert_eq!(make_and_model(out), "Epson LQ-590 - CUPS+Gutenprint v5.3.4");
        assert_eq!(make_and_model("printer-make-and-model=Generic printer-state=3"), "Generic");
        assert_eq!(make_and_model("copies=1 printer-state=3"), "");
    }

    #[test]
    fn file_output_holds_the_bytes() {
        let path = std::env::temp_dir().join(format!("bc-emboss-out-{}.brl", std::process::id()));
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
