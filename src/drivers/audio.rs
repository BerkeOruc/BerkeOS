use crate::drivers::pcspeaker;

pub struct WavPlayer {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub data: [u8; 256000],
    pub data_len: usize,
    pub playing: bool,
}

impl WavPlayer {
    pub fn load(data: &[u8]) -> Option<Self> {
        if data.len() < 44 {
            return None;
        }

        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return None;
        }

        let mut fmt_offset = 0;
        let mut data_offset = 0;
        let mut data_size = 0;

        let mut i = 12;
        while i < data.len() - 8 {
            let chunk_id = &data[i..i + 4];
            let chunk_size =
                u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;

            if chunk_id == b"fmt " {
                fmt_offset = i + 8;
            } else if chunk_id == b"data" {
                data_offset = i + 8;
                data_size = chunk_size;
                break;
            }

            i += 8 + chunk_size;
        }

        if fmt_offset == 0 || data_offset == 0 {
            return None;
        }

        let audio_format = u16::from_le_bytes([data[fmt_offset], data[fmt_offset + 1]]);
        if audio_format != 1 && audio_format != 3 {
            return None;
        }

        let channels = u16::from_le_bytes([data[fmt_offset + 2], data[fmt_offset + 3]]);
        let sample_rate = u32::from_le_bytes([
            data[fmt_offset + 4],
            data[fmt_offset + 5],
            data[fmt_offset + 6],
            data[fmt_offset + 7],
        ]);
        let bits_per_sample = u16::from_le_bytes([data[fmt_offset + 14], data[fmt_offset + 15]]);

        let copy_size = data_size.min(256000);

        let mut player = WavPlayer {
            sample_rate,
            channels,
            bits_per_sample,
            data: [0u8; 256000],
            data_len: copy_size,
            playing: false,
        };

        for i in 0..copy_size {
            player.data[i] = data[data_offset + i];
        }

        Some(player)
    }

    pub fn play(&self) {
        if self.bits_per_sample == 8 && self.sample_rate == 8000 {
            for i in 0..self.data_len {
                if i % 2 == 0 {
                    let sample = self.data[i] as u16;
                    if sample > 128 {
                        unsafe {
                            pcspeaker::speaker_beep(440, 1);
                        }
                    }
                }
            }
        }
    }
}

pub fn play_wav(data: &[u8]) -> bool {
    if let Some(player) = WavPlayer::load(data) {
        player.play();
        true
    } else {
        false
    }
}
