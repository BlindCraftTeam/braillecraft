//! The margin bell (Global, View Preferences, spec 24 2d.2): a short tone when typing brings the caret to the bell
//! position before the end of the line. The reference program plays a sound file it ships, and when that is missing
//! it builds the tone itself: 100 ms of a 4000 Hz sine, 8-bit mono at 11025 samples a second; failing that, the
//! system beep. BrailleCraft ships no sound file, so it always builds the tone.

use std::cell::RefCell;

use wxdragon_sys as ffi;

const RATE: u32 = 11025;
const FREQUENCY: f64 = 4000.0;
const MILLISECONDS: u32 = 100;

/// The tone as a WAV file: a RIFF header, then 8-bit samples centred on 128.
pub fn tone_wav() -> Vec<u8> {
    let n = MILLISECONDS * RATE / 1000;
    let mut w = Vec::with_capacity(44 + n as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + n).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // one channel
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&RATE.to_le_bytes()); // bytes a second
    w.extend_from_slice(&1u16.to_le_bytes()); // bytes a sample
    w.extend_from_slice(&8u16.to_le_bytes()); // bits a sample
    w.extend_from_slice(b"data");
    w.extend_from_slice(&n.to_le_bytes());
    for i in 0..n {
        let v = ((2.0 * std::f64::consts::PI * FREQUENCY * f64::from(i) / f64::from(RATE)).sin() + 1.0) * 128.0;
        w.push(v.min(255.0) as u8);
    }
    w
}

thread_local! {
    /// The tone, built once and kept, as the reference keeps its sound after the first ring.
    static TONE: RefCell<Option<(Vec<u8>, *mut ffi::wxd_Sound_t)>> = const { RefCell::new(None) };
}

/// Rings the margin bell without waiting for it to finish.
pub fn ring() {
    let played = TONE.with(|t| {
        let mut t = t.borrow_mut();
        if t.is_none() {
            let data = tone_wav();
            let sound = unsafe { ffi::wxd_Sound_CreateFromData(data.as_ptr(), data.len()) };
            *t = Some((data, sound));
        }
        let (_, sound) = t.as_ref().unwrap();
        !sound.is_null()
            && unsafe { ffi::wxd_Sound_IsOk(*sound) }
            // 1 = WXD_SOUND_ASYNC (wxd_sound.h).
            && unsafe { ffi::wxd_Sound_Play(*sound, 1) }
    });
    if !played {
        unsafe { ffi::wxd_Bell() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_is_a_100_ms_4000_hz_wav() {
        let w = tone_wav();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 11025);
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 1102);
        assert_eq!(w.len(), 44 + 1102);
        // The first sample is the middle (sine 0), the next is above it (a rising sine).
        assert_eq!(w[44], 128);
        assert!(w[45] > 128);
    }
}
