//! Callback-safe preview output for the original frontend's mixer.
#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use unwritten_studio_ui_egui::audio::{AudioDevice, AudioDeviceFactory, AudioFeed, AudioOutput};

pub const IOS_AUDIO_RATE: u32 = 48_000;
const MAX_PULL_SAMPLES: usize = IOS_AUDIO_RATE as usize * 2;

struct AudioState {
    feed: Mutex<Option<(u64, Arc<AudioFeed>)>>,
    next_device: AtomicU64,
}

impl AudioState {
    fn new() -> Self {
        Self { feed: Mutex::new(None), next_device: AtomicU64::new(1) }
    }

    fn start(&self, owner: u64, feed: Arc<AudioFeed>) -> Result<(), String> {
        let mut current = self.feed.lock().map_err(|_| "iOS audio state is unavailable".to_string())?;
        *current = Some((owner, feed));
        Ok(())
    }

    fn stop(&self, owner: u64) {
        if let Ok(mut current) = self.feed.lock()
            && current.as_ref().is_some_and(|(id, _)| *id == owner)
        {
            *current = None;
        }
    }

    fn pull(&self, output: &mut [f32]) -> bool {
        // Do not hold the device-state lock while the feed's queue is drained.
        let feed = self.feed.try_lock().ok().and_then(|current| current.as_ref().map(|(_, feed)| Arc::clone(feed)));
        if let Some(feed) = feed {
            feed.try_pull(output);
            true
        } else {
            output.fill(0.0);
            false
        }
    }
}

/// Only this shared audio state crosses into AVAudioSourceNode's callback.
/// The callback never reads the thread-confined NativeUi.
pub struct NativeAudioBridge {
    state: Arc<AudioState>,
}

impl NativeAudioBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { state: Arc::new(AudioState::new()) })
    }

    pub fn factory(self: &Arc<Self>) -> AudioDeviceFactory {
        let state = Arc::clone(&self.state);
        Box::new(move |_output: &AudioOutput| {
            let owner = state.next_device.fetch_add(1, Ordering::Relaxed);
            Some(Box::new(NativeAudioDevice { state: Arc::clone(&state), owner }) as Box<dyn AudioDevice>)
        })
    }

    /// The caller releases this independent owned handle after callbacks stop.
    pub fn callback_handle(&self) -> *mut UnwrittenStudioAudio {
        Box::into_raw(Box::new(UnwrittenStudioAudio { state: Arc::clone(&self.state) }))
    }
}

struct NativeAudioDevice {
    state: Arc<AudioState>,
    owner: u64,
}

impl AudioDevice for NativeAudioDevice {
    fn sample_rate(&self) -> u32 {
        IOS_AUDIO_RATE
    }
    fn start(&mut self, feed: Arc<AudioFeed>) -> Result<(), String> {
        self.state.start(self.owner, feed)
    }
    fn stop(&mut self) {
        self.state.stop(self.owner);
    }
}

impl Drop for NativeAudioDevice {
    fn drop(&mut self) {
        self.stop();
    }
}

/// An owned callback handle, independent of the editor's lifetime.
#[repr(C)]
pub struct UnwrittenStudioAudio {
    state: Arc<AudioState>,
}

/// Pull interleaved stereo Float32. Invalid arguments are rejected untouched;
/// a valid stopped handle fills silence and returns zero.
///
/// # Safety
/// `handle` must be live until this call returns; `output` must address writable
/// storage for `sample_count` samples, at most 96,000. Destroy only after all
/// callbacks stop. The allocation may not alias the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_audio_pull(handle: *const UnwrittenStudioAudio, output: *mut f32, sample_count: usize) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() || output.is_null() || sample_count == 0 || sample_count > MAX_PULL_SAMPLES {
            return 0;
        }
        // SAFETY: live handle and bounded writable buffer are the caller's contract.
        let state = unsafe { &*handle };
        let output = unsafe { std::slice::from_raw_parts_mut(output, sample_count) };
        if state.state.pull(output) { sample_count } else { 0 }
    }))
    .unwrap_or(0)
}

/// Pull stereo into two non-interleaved channels without heap allocation.
///
/// # Safety
/// `handle` must be live, and `left`/`right` must be disjoint writable buffers
/// for `frames` f32 values each (1..=48,000). No buffer may alias the handle.
/// Destroy the handle only after every callback has returned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_audio_pull_planar(handle: *const UnwrittenStudioAudio, left: *mut f32, right: *mut f32, frames: usize) -> usize {
    catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() || left.is_null() || right.is_null() || frames == 0 || frames > IOS_AUDIO_RATE as usize {
            return 0;
        }
        // SAFETY: these non-aliasing live allocations are required by the contract.
        let state = unsafe { &*handle };
        let left = unsafe { std::slice::from_raw_parts_mut(left, frames) };
        let right = unsafe { std::slice::from_raw_parts_mut(right, frames) };
        let mut scratch = [0.0_f32; 2048];
        let mut active_frames = 0;
        for (left, right) in left.chunks_mut(1024).zip(right.chunks_mut(1024)) {
            let count = left.len();
            if state.state.pull(&mut scratch[..count * 2]) {
                active_frames += count;
            }
            for ((left, right), stereo) in left.iter_mut().zip(right.iter_mut()).zip(scratch.chunks_exact(2)) {
                *left = stereo[0];
                *right = stereo[1];
            }
        }
        active_frames
    }))
    .unwrap_or(0)
}

/// # Safety
/// `handle` must be null or one live owned handle from `callback_handle`.
/// Stop and join all callbacks before releasing it exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_audio_destroy(handle: *mut UnwrittenStudioAudio) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !handle.is_null() {
            // SAFETY: caller gives up its unique owned allocation.
            drop(unsafe { Box::from_raw(handle) });
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn feed_is_consumed_and_stopped_is_silence() {
        let state = AudioState::new();
        let feed = Arc::new(AudioFeed::default());
        feed.push(&[0.25, -0.5, 0.75, -1.0]);
        state.start(1, feed.clone()).unwrap();
        let mut out = [0.0; 4];
        assert!(state.pull(&mut out));
        assert_eq!(out, [0.25, -0.5, 0.75, -1.0]);
        assert_eq!(feed.consumed(), 2);
        state.stop(1);
        out.fill(9.0);
        assert!(!state.pull(&mut out));
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn old_device_stop_does_not_silence_new_owner() {
        let state = AudioState::new();
        state.start(1, Arc::new(AudioFeed::default())).unwrap();
        let feed = Arc::new(AudioFeed::default());
        feed.push(&[0.25, -0.5]);
        state.start(2, feed).unwrap();
        state.stop(1);
        let mut out = [0.0; 2];
        assert!(state.pull(&mut out));
        assert_eq!(out, [0.25, -0.5]);
    }

    #[test]
    fn planar_callback_outlives_bridge_and_deinterleaves_full_buffer() {
        let bridge = NativeAudioBridge::new();
        let feed = Arc::new(AudioFeed::default());
        let input: Vec<f32> = (0..6000).map(|i| i as f32 / 6000.0).collect();
        feed.push(&input);
        bridge.state.start(1, feed.clone()).unwrap();
        let handle = bridge.callback_handle();
        drop(bridge);
        let mut left = vec![0.0; 3000];
        let mut right = vec![0.0; 3000];
        assert_eq!(unsafe { unwritten_studio_audio_pull_planar(handle, left.as_mut_ptr(), right.as_mut_ptr(), 3000) }, 3000);
        for (i, (&left, &right)) in left.iter().zip(&right).enumerate() {
            assert_eq!(left, input[i * 2]);
            assert_eq!(right, input[i * 2 + 1]);
        }
        assert_eq!(feed.consumed(), 3000);
        unsafe { unwritten_studio_audio_destroy(handle) };
    }

    #[test]
    fn pull_rejects_unbounded_input_without_touching_pointer() {
        assert_eq!(unsafe { unwritten_studio_audio_pull(ptr::null(), ptr::null_mut(), MAX_PULL_SAMPLES + 1) }, 0);
        assert_eq!(unsafe { unwritten_studio_audio_pull_planar(ptr::null(), ptr::null_mut(), ptr::null_mut(), 48001) }, 0);
    }
}
