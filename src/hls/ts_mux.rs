//! Joins a video-only and an audio-only MPEG-TS stream into one TS, with no
//! re-encoding.
//!
//! Each input segment is its own little program (PAT, PMT and one elementary
//! stream). ToonPlay's video and audio segments even use the same PIDs
//! (PMT `0x1000`, stream `0x100`). The output has one PAT and one PMT that
//! list both streams on separate PIDs; the input tables are dropped, the
//! packets of each stream are re-stamped with their new PID and a continuous
//! continuity counter, and whole PES units are interleaved in decode-time
//! order so a player never has to buffer far ahead in either stream.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::HlsError;

pub const PACKET: usize = 188;
const SYNC: u8 = 0x47;
const PAT_PID: u16 = 0x0000;
pub const PMT_PID: u16 = 0x1000;
pub const VIDEO_PID: u16 = 0x0100;
pub const AUDIO_PID: u16 = 0x0101;
const PROGRAM: u16 = 1;
const STREAM_TYPE_H264: u8 = 0x1b;
const STREAM_TYPE_AAC: u8 = 0x0f;

type Packet = [u8; PACKET];

/// The stream a segment's PMT describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elementary {
    pub stream_type: u8,
    pub es_info: Vec<u8>,
}

/// One PES packet's worth of TS packets, with its decode time (90 kHz).
#[derive(Debug, Clone, PartialEq)]
pub struct PesUnit {
    packets: Vec<Packet>,
    pub dts: Option<u64>,
}

/// A segment split into the units of its (single) elementary stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DemuxedSegment {
    pub units: Vec<PesUnit>,
    pub stream: Option<Elementary>,
}

impl DemuxedSegment {
    fn last_dts(&self) -> Option<u64> {
        self.units.iter().filter_map(|unit| unit.dts).max()
    }
}

/// Splits one segment into PES units of its first elementary stream.
pub fn demux(data: &[u8]) -> Result<DemuxedSegment, HlsError> {
    let start = find_sync(data)
        .ok_or_else(|| HlsError::Segment("the segment is not MPEG-TS".to_string()))?;
    let mut pmt_pid = None;
    let mut es_pid = None;
    let mut segment = DemuxedSegment::default();

    for chunk in data[start..].chunks_exact(PACKET) {
        if chunk[0] != SYNC {
            return Err(HlsError::Segment("lost MPEG-TS sync".to_string()));
        }
        let pid = pid_of(chunk);
        let pusi = chunk[1] & 0x40 != 0;
        let Some(payload) = payload_of(chunk) else {
            if Some(pid) == es_pid {
                // Adaptation only (a PCR, stuffing): keep it with its unit.
                push_continuation(&mut segment, chunk);
            }
            continue;
        };
        if pid == PAT_PID && pusi {
            pmt_pid = pmt_pid.or_else(|| parse_pat(payload));
        } else if Some(pid) == pmt_pid && pusi {
            if es_pid.is_none()
                && let Some((pid, stream)) = parse_pmt(payload)
            {
                es_pid = Some(pid);
                segment.stream = Some(stream);
            }
        } else if Some(pid) == es_pid {
            let mut packet = [0u8; PACKET];
            packet.copy_from_slice(chunk);
            if pusi {
                segment.units.push(PesUnit {
                    packets: vec![packet],
                    dts: pes_dts(payload),
                });
            } else {
                push_continuation(&mut segment, chunk);
            }
        }
    }
    Ok(segment)
}

fn push_continuation(segment: &mut DemuxedSegment, chunk: &[u8]) {
    let mut packet = [0u8; PACKET];
    packet.copy_from_slice(chunk);
    match segment.units.last_mut() {
        Some(unit) => unit.packets.push(packet),
        // The tail of a PES that began in the previous segment.
        None => segment.units.push(PesUnit {
            packets: vec![packet],
            dts: None,
        }),
    }
}

fn find_sync(data: &[u8]) -> Option<usize> {
    (0..data.len().min(PACKET)).find(|&i| {
        data.get(i) == Some(&SYNC)
            && data.get(i + PACKET).is_none_or(|b| *b == SYNC)
            && data.get(i + 2 * PACKET).is_none_or(|b| *b == SYNC)
    })
}

fn pid_of(packet: &[u8]) -> u16 {
    (u16::from(packet[1] & 0x1f) << 8) | u16::from(packet[2])
}

fn payload_of(packet: &[u8]) -> Option<&[u8]> {
    let control = (packet[3] >> 4) & 0x3;
    if control & 0x1 == 0 {
        return None;
    }
    let offset = if control & 0x2 != 0 {
        5 + usize::from(packet[4])
    } else {
        4
    };
    packet.get(offset..).filter(|payload| !payload.is_empty())
}

/// A PSI section from a payload that starts one (pointer field first).
fn section(payload: &[u8]) -> Option<&[u8]> {
    let pointer = usize::from(*payload.first()?);
    let body = payload.get(1 + pointer..)?;
    let length = (usize::from(*body.get(1)? & 0x0f) << 8) | usize::from(*body.get(2)?);
    body.get(..3 + length)
}

fn parse_pat(payload: &[u8]) -> Option<u16> {
    let table = section(payload)?;
    if table[0] != 0x00 {
        return None;
    }
    // Program loop after the 8-byte header, minus the CRC.
    table
        .get(8..table.len().checked_sub(4)?)?
        .chunks_exact(4)
        .find(|entry| u16::from_be_bytes([entry[0], entry[1]]) != 0)
        .map(|entry| (u16::from(entry[2] & 0x1f) << 8) | u16::from(entry[3]))
}

fn parse_pmt(payload: &[u8]) -> Option<(u16, Elementary)> {
    let table = section(payload)?;
    if table[0] != 0x02 {
        return None;
    }
    let program_info = (usize::from(*table.get(10)? & 0x0f) << 8) | usize::from(*table.get(11)?);
    let end = table.len().checked_sub(4)?;
    let mut at = 12 + program_info;
    while at + 5 <= end {
        let stream_type = table[at];
        let pid = (u16::from(table[at + 1] & 0x1f) << 8) | u16::from(table[at + 2]);
        let info_len = (usize::from(table[at + 3] & 0x0f) << 8) | usize::from(table[at + 4]);
        let es_info = table.get(at + 5..at + 5 + info_len)?.to_vec();
        if matches!(
            stream_type,
            0x01..=0x04 | 0x0f | 0x10 | 0x11 | 0x1b | 0x24 | 0x81 | 0x87
        ) {
            return Some((
                pid,
                Elementary {
                    stream_type,
                    es_info,
                },
            ));
        }
        at += 5 + info_len;
    }
    None
}

/// The DTS of a PES header, or its PTS when it has no separate DTS.
fn pes_dts(payload: &[u8]) -> Option<u64> {
    if payload.len() < 14 || payload[..3] != [0, 0, 1] {
        return None;
    }
    let flags = payload[7] >> 6;
    let at = match flags {
        0b11 if payload.len() >= 19 => 14,
        0b10 | 0b11 => 9,
        _ => return None,
    };
    let t = &payload[at..at + 5];
    Some(
        (u64::from((t[0] >> 1) & 0x07) << 30)
            | (u64::from(t[1]) << 22)
            | (u64::from(t[2] >> 1) << 15)
            | (u64::from(t[3]) << 7)
            | u64::from(t[4] >> 1),
    )
}

/// Continuity counters of the output PIDs, kept across a pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Counters {
    pub pat: u8,
    pub pmt: u8,
    pub video: u8,
    pub audio: u8,
}

/// Writes the output program: tables and re-stamped PES units.
#[derive(Debug, Clone)]
pub struct Muxer {
    counters: Counters,
    video: Elementary,
    audio: Elementary,
}

impl Muxer {
    pub fn new(video: Option<Elementary>, audio: Option<Elementary>, counters: Counters) -> Self {
        Self {
            counters,
            video: video.unwrap_or(Elementary {
                stream_type: STREAM_TYPE_H264,
                es_info: Vec::new(),
            }),
            audio: audio.unwrap_or(Elementary {
                stream_type: STREAM_TYPE_AAC,
                es_info: Vec::new(),
            }),
        }
    }

    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// The PAT and PMT, written at the start and before every video segment.
    pub fn write_tables(&mut self, out: &mut Vec<u8>) {
        let mut pat = vec![0x00, 0xb0, 0x00, 0x00, 0x01, 0xc1, 0x00, 0x00];
        pat.extend_from_slice(&PROGRAM.to_be_bytes());
        pat.extend_from_slice(&(0xe000 | PMT_PID).to_be_bytes());
        out.extend_from_slice(&psi_packet(PAT_PID, &mut self.counters.pat, pat));

        let mut pmt = vec![0x02, 0xb0, 0x00];
        pmt.extend_from_slice(&PROGRAM.to_be_bytes());
        pmt.extend_from_slice(&[0xc1, 0x00, 0x00]);
        pmt.extend_from_slice(&(0xe000 | VIDEO_PID).to_be_bytes()); // PCR PID
        pmt.extend_from_slice(&[0xf0, 0x00]);
        for (pid, stream) in [(VIDEO_PID, &self.video), (AUDIO_PID, &self.audio)] {
            pmt.push(stream.stream_type);
            pmt.extend_from_slice(&(0xe000 | pid).to_be_bytes());
            let len = stream.es_info.len().min(0x3ff) as u16;
            pmt.extend_from_slice(&(0xf000 | len).to_be_bytes());
            pmt.extend_from_slice(&stream.es_info[..usize::from(len)]);
        }
        out.extend_from_slice(&psi_packet(PMT_PID, &mut self.counters.pmt, pmt));
    }

    pub fn write_unit(&mut self, unit: &PesUnit, pid: u16, out: &mut Vec<u8>) {
        let counter = if pid == AUDIO_PID {
            &mut self.counters.audio
        } else {
            &mut self.counters.video
        };
        for packet in &unit.packets {
            let mut packet = *packet;
            packet[1] = (packet[1] & 0xe0) | ((pid >> 8) as u8 & 0x1f);
            packet[2] = pid as u8;
            if (packet[3] >> 4) & 0x1 != 0 {
                packet[3] = (packet[3] & 0xf0) | (*counter & 0x0f);
                *counter = (*counter + 1) & 0x0f;
            } else {
                // A packet without payload repeats the previous counter.
                packet[3] = (packet[3] & 0xf0) | (counter.wrapping_sub(1) & 0x0f);
            }
            out.extend_from_slice(&packet);
        }
    }
}

/// One PSI section in one packet: `body` is the section without its length
/// bytes filled in and without the CRC.
fn psi_packet(pid: u16, counter: &mut u8, mut body: Vec<u8>) -> Packet {
    let length = body.len() + 4 - 3;
    body[1] = 0xb0 | ((length >> 8) as u8 & 0x0f);
    body[2] = length as u8;
    let crc = crc32_mpeg(&body);
    body.extend_from_slice(&crc.to_be_bytes());

    let mut packet = [0xffu8; PACKET];
    packet[0] = SYNC;
    packet[1] = 0x40 | ((pid >> 8) as u8 & 0x1f);
    packet[2] = pid as u8;
    packet[3] = 0x10 | (*counter & 0x0f);
    *counter = (*counter + 1) & 0x0f;
    packet[4] = 0; // pointer field
    let len = body.len().min(PACKET - 5);
    packet[5..5 + len].copy_from_slice(&body[..len]);
    packet
}

pub fn crc32_mpeg(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Where an interleave stands, so a paused download continues exactly there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InterleaveState {
    pub counters: Counters,
    /// The audio segment the first queued unit came from.
    pub audio_front: usize,
    /// How many units of [`Self::audio_front`] are already written.
    pub audio_skip: usize,
    /// The next audio segment to fetch.
    pub audio_next: usize,
}

/// Merges whole video segments with the audio that plays alongside them.
#[derive(Debug)]
pub struct Interleaver {
    muxer: Option<Muxer>,
    counters: Counters,
    audio_stream: Option<Elementary>,
    queue: VecDeque<(usize, PesUnit)>,
    front_written: usize,
    audio_next: usize,
}

impl Interleaver {
    pub fn new(state: InterleaveState) -> Self {
        Self {
            muxer: None,
            counters: state.counters,
            audio_stream: None,
            queue: VecDeque::new(),
            front_written: state.audio_skip,
            audio_next: state.audio_front,
        }
    }

    /// The next audio segment index to hand to [`Self::push_audio`].
    pub fn audio_next(&self) -> usize {
        self.audio_next
    }

    /// True while the queued audio does not reach `until` (a video DTS).
    pub fn needs_audio(&self, until: Option<u64>) -> bool {
        match (self.queue.back().and_then(|(_, unit)| unit.dts), until) {
            (_, None) => self.queue.is_empty(),
            (None, Some(_)) => true,
            (Some(last), Some(until)) => last < until,
        }
    }

    /// Queues audio segment `index`. On a resume the first one pushed is
    /// [`InterleaveState::audio_front`], whose written units are skipped.
    pub fn push_audio(&mut self, index: usize, segment: DemuxedSegment) {
        if self.audio_stream.is_none() {
            self.audio_stream = segment.stream.clone();
        }
        // Only the segment that becomes the front can have written units: on a
        // resume, the one the pause was in.
        let skip = if self.queue.is_empty() {
            let skip = if index == self.audio_next {
                self.front_written
            } else {
                0
            };
            self.front_written = skip;
            skip
        } else {
            0
        };
        let skipped = skip.min(segment.units.len());
        self.queue.extend(
            segment
                .units
                .into_iter()
                .skip(skipped)
                .map(|unit| (index, unit)),
        );
        if self.queue.is_empty() {
            // Everything in it was written already: nothing to skip in the next.
            self.front_written = 0;
        }
        self.audio_next = index + 1;
    }

    /// Writes one video segment and the audio due before each of its units.
    pub fn write_video(&mut self, segment: DemuxedSegment, out: &mut Vec<u8>) {
        if self.muxer.is_none() {
            self.muxer = Some(Muxer::new(
                segment.stream.clone(),
                self.audio_stream.clone(),
                self.counters,
            ));
        }
        let Some(mut muxer) = self.muxer.take() else {
            return;
        };
        muxer.write_tables(out);
        for unit in &segment.units {
            if let Some(dts) = unit.dts {
                self.write_audio_until(&mut muxer, Some(dts), out);
            }
            muxer.write_unit(unit, VIDEO_PID, out);
        }
        self.muxer = Some(muxer);
    }

    /// Writes the audio left after the last video segment.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if let Some(mut muxer) = self.muxer.take() {
            self.write_audio_until(&mut muxer, None, out);
            self.muxer = Some(muxer);
        }
    }

    fn write_audio_until(&mut self, muxer: &mut Muxer, until: Option<u64>, out: &mut Vec<u8>) {
        while let Some((index, unit)) = self.queue.front() {
            let due = match (unit.dts, until) {
                (Some(dts), Some(until)) => dts <= until,
                _ => true,
            };
            if !due {
                break;
            }
            let index = *index;
            muxer.write_unit(unit, AUDIO_PID, out);
            self.queue.pop_front();
            let next_index = self.queue.front().map(|(i, _)| *i);
            if next_index == Some(index) {
                self.front_written += 1;
            } else {
                self.front_written = 0;
            }
        }
    }

    pub fn state(&self) -> InterleaveState {
        let counters = self.muxer.as_ref().map_or(self.counters, Muxer::counters);
        match self.queue.front() {
            Some((index, _)) => InterleaveState {
                counters,
                audio_front: *index,
                audio_skip: self.front_written,
                audio_next: self.audio_next,
            },
            None => InterleaveState {
                counters,
                audio_front: self.audio_next,
                audio_skip: 0,
                audio_next: self.audio_next,
            },
        }
    }

    /// The last decode time a video segment reaches.
    pub fn video_until(segment: &DemuxedSegment) -> Option<u64> {
        segment.last_dts()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a segment as ffmpeg's HLS muxer writes it: PAT, PMT on 0x1000,
    /// one stream on 0x100, each unit a PES with a PTS (and a DTS for video).
    pub(crate) fn segment(stream_type: u8, es_info: &[u8], units: &[(u64, usize)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut counters = (0u8, 0u8, 0u8);
        let mut pat = vec![0x00, 0xb0, 0x00, 0x00, 0x01, 0xc1, 0x00, 0x00, 0x00, 0x01];
        pat.extend_from_slice(&(0xe000 | PMT_PID).to_be_bytes());
        out.extend_from_slice(&psi_packet(PAT_PID, &mut counters.0, pat));
        let mut pmt = vec![
            0x02, 0xb0, 0x00, 0x00, 0x01, 0xc1, 0x00, 0x00, 0xe1, 0x00, 0xf0, 0x00,
        ];
        pmt.push(stream_type);
        pmt.extend_from_slice(&[0xe1, 0x00, 0xf0, es_info.len() as u8]);
        pmt.extend_from_slice(es_info);
        out.extend_from_slice(&psi_packet(PMT_PID, &mut counters.1, pmt));

        for (dts, size) in units {
            let mut pes = vec![0, 0, 1, 0xe0, 0, 0, 0x80, 0xc0, 10];
            pes.extend_from_slice(&timestamp(0x3, *dts + 3000));
            pes.extend_from_slice(&timestamp(0x1, *dts));
            pes.extend(std::iter::repeat_n(0xabu8, *size));
            let mut first = true;
            for chunk in pes.chunks(PACKET - 4) {
                let mut packet = [0xffu8; PACKET];
                packet[0] = SYNC;
                packet[1] = if first { 0x41 } else { 0x01 };
                packet[2] = 0x00;
                if chunk.len() == PACKET - 4 {
                    packet[3] = 0x10 | counters.2;
                    packet[4..].copy_from_slice(chunk);
                } else {
                    // Stuffed through the adaptation field, as muxers do.
                    let stuffing = PACKET - 4 - chunk.len();
                    packet[3] = 0x30 | counters.2;
                    packet[4] = (stuffing - 1) as u8;
                    if stuffing > 1 {
                        packet[5] = 0x00;
                    }
                    packet[4 + stuffing..].copy_from_slice(chunk);
                }
                counters.2 = (counters.2 + 1) & 0xf;
                first = false;
                out.extend_from_slice(&packet);
            }
        }
        out
    }

    fn timestamp(prefix: u8, t: u64) -> [u8; 5] {
        [
            (prefix << 4) | (((t >> 30) as u8 & 0x07) << 1) | 1,
            (t >> 22) as u8,
            (((t >> 15) as u8) << 1) | 1,
            (t >> 7) as u8,
            ((t as u8) << 1) | 1,
        ]
    }

    const ENGLISH: &[u8] = &[0x0a, 0x04, b'e', b'n', b'g', 0x00];

    fn video(start: u64) -> Vec<u8> {
        // 4 frames of 3750 ticks: 1/24 s; frames of 400 bytes span 3 packets.
        segment(
            STREAM_TYPE_H264,
            &[],
            &(0..4).map(|i| (start + i * 3750, 400)).collect::<Vec<_>>(),
        )
    }

    fn audio(start: u64) -> Vec<u8> {
        // 8 frames of 1920 ticks; small enough for one packet each.
        segment(
            STREAM_TYPE_AAC,
            ENGLISH,
            &(0..8).map(|i| (start + i * 1920, 100)).collect::<Vec<_>>(),
        )
    }

    struct Parsed {
        pid: u16,
        cc: u8,
        has_payload: bool,
        dts: Option<u64>,
    }

    fn parse_output(data: &[u8]) -> Vec<Parsed> {
        assert_eq!(data.len() % PACKET, 0);
        data.chunks_exact(PACKET)
            .map(|p| {
                assert_eq!(p[0], SYNC);
                let payload = payload_of(p);
                Parsed {
                    pid: pid_of(p),
                    cc: p[3] & 0x0f,
                    has_payload: payload.is_some(),
                    dts: payload.filter(|_| p[1] & 0x40 != 0).and_then(pes_dts),
                }
            })
            .collect()
    }

    #[test]
    fn demuxes_units_and_the_stream_description() {
        let segment = demux(&audio(0)).unwrap();
        assert_eq!(segment.units.len(), 8);
        assert_eq!(segment.units[1].dts, Some(1920));
        assert_eq!(
            segment.stream,
            Some(Elementary {
                stream_type: STREAM_TYPE_AAC,
                es_info: ENGLISH.to_vec()
            })
        );
        let video = demux(&video(0)).unwrap();
        assert_eq!(video.units.len(), 4);
        assert_eq!(video.units[0].packets.len(), 3);
        assert_eq!(Interleaver::video_until(&video), Some(3 * 3750));
    }

    #[test]
    fn refuses_a_body_that_is_not_ts() {
        assert!(demux(b"#EXTM3U\n#EXT-X-VERSION:3\n").is_err());
    }

    #[test]
    fn writes_one_program_with_both_streams() {
        let mut interleaver = Interleaver::new(InterleaveState::default());
        let mut out = Vec::new();
        interleaver.push_audio(0, demux(&audio(0)).unwrap());
        interleaver.write_video(demux(&video(0)).unwrap(), &mut out);
        interleaver.finish(&mut out);

        let packets = parse_output(&out);
        // Only the output PIDs.
        assert!(
            packets
                .iter()
                .all(|p| [PAT_PID, PMT_PID, VIDEO_PID, AUDIO_PID].contains(&p.pid))
        );
        assert_eq!(packets.iter().filter(|p| p.pid == PMT_PID).count(), 1);
        assert_eq!(
            packets
                .iter()
                .filter(|p| p.pid == AUDIO_PID && p.dts.is_some())
                .count(),
            8
        );
        assert_eq!(
            packets
                .iter()
                .filter(|p| p.pid == VIDEO_PID && p.dts.is_some())
                .count(),
            4
        );

        // The PMT lists video then audio, with the audio's language kept.
        let pmt = &out[PACKET..2 * PACKET];
        let table = section(payload_of(pmt).unwrap()).unwrap();
        let crc = u32::from_be_bytes(table[table.len() - 4..].try_into().unwrap());
        assert_eq!(crc, crc32_mpeg(&table[..table.len() - 4]));
        let (pid, stream) = parse_pmt(payload_of(pmt).unwrap()).unwrap();
        assert_eq!((pid, stream.stream_type), (VIDEO_PID, STREAM_TYPE_H264));
        assert!(table.windows(ENGLISH.len()).any(|w| w == ENGLISH));
        let pat = &out[..PACKET];
        assert_eq!(parse_pat(payload_of(pat).unwrap()), Some(PMT_PID));
    }

    #[test]
    fn interleaves_by_decode_time_with_continuous_counters() {
        let mut interleaver = Interleaver::new(InterleaveState::default());
        let mut out = Vec::new();
        // Audio segment 0 covers 0..15360, video segments 0..15000 and 15000..30000.
        interleaver.push_audio(0, demux(&audio(0)).unwrap());
        let first = demux(&video(0)).unwrap();
        assert!(!interleaver.needs_audio(Interleaver::video_until(&first)));
        interleaver.write_video(first, &mut out);
        let second = demux(&video(15_000)).unwrap();
        assert!(interleaver.needs_audio(Interleaver::video_until(&second)));
        interleaver.push_audio(1, demux(&audio(15_360)).unwrap());
        interleaver.write_video(second, &mut out);
        interleaver.finish(&mut out);

        let packets = parse_output(&out);
        // Every audio unit comes before the first video unit that decodes after it.
        let mut last_video = None;
        for p in packets.iter().filter(|p| p.dts.is_some()) {
            if p.pid == VIDEO_PID {
                last_video = p.dts;
            } else if let (Some(video), Some(audio)) = (last_video, p.dts) {
                assert!(audio > video, "audio {audio} written after video {video}");
            }
        }
        for pid in [PAT_PID, PMT_PID, VIDEO_PID, AUDIO_PID] {
            let ccs: Vec<u8> = packets
                .iter()
                .filter(|p| p.pid == pid && p.has_payload)
                .map(|p| p.cc)
                .collect();
            for pair in ccs.windows(2) {
                assert_eq!(pair[1], (pair[0] + 1) & 0x0f, "pid {pid:#x} counter jumps");
            }
        }
        assert_eq!(
            packets
                .iter()
                .filter(|p| p.pid == AUDIO_PID && p.dts.is_some())
                .count(),
            16
        );
    }

    #[test]
    fn a_resumed_interleave_continues_byte_for_byte() {
        let audio_segments = [audio(0), audio(15_360)];
        let videos = [video(0), video(15_000), video(30_000)];

        let mut whole = Vec::new();
        let mut full = Interleaver::new(InterleaveState::default());
        let mut paused_at = (0, InterleaveState::default());
        for (i, segment) in videos.iter().enumerate() {
            let segment = demux(segment).unwrap();
            while full.needs_audio(Interleaver::video_until(&segment)) && full.audio_next() < 2 {
                let index = full.audio_next();
                full.push_audio(index, demux(&audio_segments[index]).unwrap());
            }
            full.write_video(segment, &mut whole);
            if i == 0 {
                paused_at = (whole.len(), full.state());
            }
        }
        full.finish(&mut whole);

        let (bytes, state) = paused_at;
        assert!(
            state.audio_skip > 0,
            "the pause falls inside an audio segment"
        );
        let mut resumed = whole[..bytes].to_vec();
        let mut again = Interleaver::new(state);
        for (index, segment) in audio_segments
            .iter()
            .enumerate()
            .take(state.audio_next)
            .skip(state.audio_front)
        {
            again.push_audio(index, demux(segment).unwrap());
        }
        for segment in &videos[1..] {
            let segment = demux(segment).unwrap();
            while again.needs_audio(Interleaver::video_until(&segment)) && again.audio_next() < 2 {
                let index = again.audio_next();
                again.push_audio(index, demux(&audio_segments[index]).unwrap());
            }
            again.write_video(segment, &mut resumed);
        }
        again.finish(&mut resumed);
        assert_eq!(resumed, whole);
    }

    #[test]
    fn crc_matches_the_mpeg_reference() {
        // The PAT ffmpeg writes for program 1 on PMT 0x1000.
        let pat = [
            0x00, 0xb0, 0x0d, 0x00, 0x01, 0xc1, 0x00, 0x00, 0x00, 0x01, 0xf0, 0x00,
        ];
        assert_eq!(crc32_mpeg(&pat), 0x2ab1_04b2);
    }
}
