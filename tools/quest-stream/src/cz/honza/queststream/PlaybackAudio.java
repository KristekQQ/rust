package cz.honza.queststream;

import android.media.*;
import android.media.projection.MediaProjection;
import java.nio.ByteBuffer;

/** Playback only (never a microphone). Pumped on the encoder worker; no unbounded PCM queue. */
final class PlaybackAudio implements AutoCloseable {
    static final int RATE = 48000, CHANNELS = 2;
    private AudioRecord record;
    private MediaCodec codec;
    private long framesRead, startUs;
    private final ByteBuffer pcm = ByteBuffer.allocateDirect(4096);
    private final MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
    private byte[] config;
    interface Sample { void accept(byte[] data, long ptsUs); }
    PlaybackAudio(MediaProjection projection) throws Exception {
        try {
            AudioPlaybackCaptureConfiguration capture = new AudioPlaybackCaptureConfiguration.Builder(projection)
                .addMatchingUsage(AudioAttributes.USAGE_GAME).addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN).build();
            int minimum = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT);
            if (minimum <= 0) throw new IllegalStateException("Nepodporovaný zvukový formát");
            record = new AudioRecord.Builder().setAudioPlaybackCaptureConfig(capture)
                .setAudioFormat(new AudioFormat.Builder().setSampleRate(RATE).setChannelMask(AudioFormat.CHANNEL_IN_STEREO)
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT).build())
                .setBufferSizeInBytes(Math.max(minimum * 2, 32768)).build();
            if (record.getState() != AudioRecord.STATE_INITIALIZED) throw new IllegalStateException("Zachytávání zvuku není dostupné");
            MediaFormat format = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AAC, RATE, CHANNELS);
            format.setInteger(MediaFormat.KEY_AAC_PROFILE, MediaCodecInfo.CodecProfileLevel.AACObjectLC);
            format.setInteger(MediaFormat.KEY_BIT_RATE, 128000);
            format.setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, 4096);
            codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_AUDIO_AAC);
            codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE);
            codec.start();
            startUs = System.nanoTime() / 1000;
            record.startRecording();
            if (record.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) throw new IllegalStateException("Zvuk nelze spustit");
        } catch (Exception e) { close(); throw e; }
    }
    byte[] config() { return config; }
    void pump(Sample sample) {
        // Drain at most eight codec buffers per video iteration, independent of socket readers.
        for (int n = 0; n < 8; n++) {
            int output = codec.dequeueOutputBuffer(info, 0);
            if (output == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                MediaFormat actual = codec.getOutputFormat();
                if (actual.getInteger(MediaFormat.KEY_SAMPLE_RATE) != RATE || actual.getInteger(MediaFormat.KEY_CHANNEL_COUNT) != CHANNELS)
                    throw new IllegalStateException("Enkodér změnil zvukový formát");
                ByteBuffer b = actual.getByteBuffer("csd-0");
                if (b == null) throw new IllegalStateException("Chybí AAC konfigurace");
                config = new byte[b.remaining()]; b.duplicate().get(config);
                if (config.length < 2 || config[0] != 0x11 || (config[1] & 0xf8) != 0x90)
                    throw new IllegalStateException("Nepodporovaná AAC-LC konfigurace");
            } else if (output >= 0) {
                try {
                    if (info.size > 0 && (info.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) == 0) {
                        ByteBuffer b = codec.getOutputBuffer(output).duplicate(); b.position(info.offset); b.limit(info.offset + info.size);
                        byte[] data = new byte[info.size]; b.get(data); sample.accept(data, info.presentationTimeUs);
                    }
                } finally { codec.releaseOutputBuffer(output, false); }
            } else break;
        }
        // Only take PCM when the codec can accept it. READ_NON_BLOCKING prevents video starvation.
        int input = codec.dequeueInputBuffer(0);
        if (input < 0) return;
        ByteBuffer target = codec.getInputBuffer(input); target.clear(); pcm.clear();
        int size = record.read(pcm, Math.min(pcm.capacity(), target.remaining()) & ~3, AudioRecord.READ_NON_BLOCKING);
        if (size < 0) throw new IllegalStateException("Čtení zvuku selhalo: " + size);
        long pts = startUs + framesRead * 1000000L / RATE;
        AudioTimestamp timestamp = new AudioTimestamp();
        if (record.getTimestamp(timestamp, AudioTimestamp.TIMEBASE_MONOTONIC) == AudioRecord.SUCCESS)
            pts = timestamp.nanoTime / 1000 + (framesRead - timestamp.framePosition) * 1000000L / RATE;
        pcm.position(0); pcm.limit(size); target.put(pcm);
        codec.queueInputBuffer(input, 0, size, Math.max(0, pts), 0);
        framesRead += size / 4;
    }
    public void close() {
        if (record != null) { try { record.stop(); } catch (Exception ignored) {} record.release(); record = null; }
        if (codec != null) { try { codec.stop(); } catch (Exception ignored) {} codec.release(); codec = null; }
    }
}
