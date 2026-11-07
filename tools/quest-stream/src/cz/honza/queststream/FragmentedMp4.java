package cz.honza.queststream;

import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Minimal single-track fragmented AVC MP4. Input must contain no B frames.
 * Each fragment contains one complete access unit. Instances are not thread safe.
 */
public final class FragmentedMp4 {
    private static final int TIMESCALE = 1_000_000;
    private final int width, height, durationUs;
    private final byte[] sps, pps;
    private long sequence = 1, firstPts = -1, lastDecodeTime = -1;

    public FragmentedMp4(int width, int height, byte[] sps, byte[] pps) {
        this(width, height, sps, pps, 33_333);
    }

    public FragmentedMp4(int width, int height, byte[] sps, byte[] pps, int durationUs) {
        if (width < 1 || width > 65535 || height < 1 || height > 65535 || durationUs < 1)
            throw new IllegalArgumentException("Invalid dimensions or sample duration");
        this.width = width;
        this.height = height;
        this.durationUs = durationUs;
        this.sps = parameterSet(sps, 7);
        this.pps = parameterSet(pps, 8);
        if (this.sps.length < 4) throw new IllegalArgumentException("SPS is too short");
    }

    public byte[] initSegment() {
        byte[] ftyp = box("ftyp", new Bytes().ascii("isom").u32(512)
                .ascii("isomiso6avc1mp41").array());
        byte[] mvhd = full("mvhd", 0, 0, new Bytes().u32(0).u32(0).u32(TIMESCALE)
                .u32(0).u32(0x10000).u16(0x100).zeros(10).matrix().zeros(24).u32(2).array());
        byte[] tkhd = full("tkhd", 0, 7, new Bytes().u32(0).u32(0).u32(1).u32(0)
                .u32(0).zeros(8).u16(0).u16(0).u16(0).u16(0).matrix()
                .u32((long) width << 16).u32((long) height << 16).array());
        byte[] mdhd = full("mdhd", 0, 0, new Bytes().u32(0).u32(0).u32(TIMESCALE)
                .u32(0).u16(0x55c4).u16(0).array());
        byte[] hdlr = full("hdlr", 0, 0, new Bytes().u32(0).ascii("vide").zeros(12)
                .ascii("Quest AVC").u8(0).array());
        byte[] avcc = box("avcC", new Bytes().u8(1).u8(sps[1]).u8(sps[2]).u8(sps[3])
                .u8(255).u8(225).u16(sps.length).bytes(sps).u8(1).u16(pps.length).bytes(pps).array());
        byte[] avc1 = box("avc1", new Bytes().zeros(6).u16(1).zeros(16).u16(width).u16(height)
                .u32(0x480000).u32(0x480000).u32(0).u16(1).zeros(32).u16(24).u16(65535)
                .bytes(avcc).array());
        byte[] stbl = box("stbl", join(
                full("stsd", 0, 0, join(new Bytes().u32(1).array(), avc1)),
                full("stts", 0, 0, new Bytes().u32(0).array()),
                full("stsc", 0, 0, new Bytes().u32(0).array()),
                full("stsz", 0, 0, new Bytes().u32(0).u32(0).array()),
                full("stco", 0, 0, new Bytes().u32(0).array())));
        byte[] dinf = box("dinf", full("dref", 0, 0,
                join(new Bytes().u32(1).array(), full("url ", 0, 1, new byte[0]))));
        byte[] minf = box("minf", join(full("vmhd", 0, 1, new byte[8]), dinf, stbl));
        byte[] trak = box("trak", join(tkhd, box("mdia", join(mdhd, hdlr, minf))));
        byte[] trex = full("trex", 0, 0, new Bytes().u32(1).u32(1).u32(durationUs)
                .u32(0).u32(0x01010000).array());
        return join(ftyp, box("moov", join(mvhd, trak, box("mvex", trex))));
    }

    /** PTS is normalized to zero; duplicate/backward timestamps advance by one sample.
     * Parameter sets and access-unit delimiters are stored in the init segment only.
     * Empty or parameter-set-only input is rejected without advancing the timeline.
     */
    public byte[] fragment(byte[] annexBOrAvccAccessUnit, long ptsUs, boolean keyFrame) {
        if (ptsUs < 0) throw new IllegalArgumentException("Negative PTS");
        Bytes sample = new Bytes();
        boolean hasSlice = false;
        for (byte[] nal : nals(annexBOrAvccAccessUnit, false)) {
            int type = nal[0] & 31;
            if (type == 7 || type == 8 || type == 9) continue;
            if (type >= 1 && type <= 5) hasSlice = true;
            if (type == 5) keyFrame = true;
            sample.u32(nal.length).bytes(nal);
        }
        if (!hasSlice) throw new IllegalArgumentException("Access unit contains no AVC slice");
        long origin = firstPts < 0 ? ptsUs : firstPts;
        long decodeTime = Math.max(ptsUs - origin, lastDecodeTime < 0 ? 0 : lastDecodeTime + durationUs);
        byte[] data = sample.array();
        byte[] moof = moof(decodeTime, data.length, keyFrame, 0);
        moof = moof(decodeTime, data.length, keyFrame, moof.length + 8);
        firstPts = origin;
        lastDecodeTime = decodeTime;
        sequence++;
        return join(moof, box("mdat", data));
    }

    private byte[] moof(long decodeTime, int size, boolean keyFrame, int offset) {
        return box("moof", join(full("mfhd", 0, 0, new Bytes().u32(sequence).array()),
                box("traf", join(full("tfhd", 0, 0x020000, new Bytes().u32(1).array()),
                        full("tfdt", 1, 0, new Bytes().u64(decodeTime).array()),
                        full("trun", 0, 0x000701, new Bytes().u32(1).u32(offset)
                                .u32(durationUs).u32(size).u32(keyFrame ? 0x02000000 : 0x01010000).array())))));
    }

    private static byte[] parameterSet(byte[] source, int type) {
        for (byte[] nal : nals(source, true)) {
            if ((nal[0] & 31) == type) {
                if (nal.length > 65535) throw new IllegalArgumentException("Parameter set too large");
                return nal;
            }
        }
        throw new IllegalArgumentException("Missing AVC parameter set " + type);
    }

    private static List<byte[]> nals(byte[] data, boolean allowRaw) {
        if (data == null || data.length == 0) throw new IllegalArgumentException("Empty AVC data");
        List<byte[]> result = new ArrayList<>();
        // Validate the complete length-prefixed form first: a legal length can resemble a start code.
        int pos = 0;
        while (pos + 4 <= data.length) {
            long size = ((long)(data[pos] & 255) << 24) | ((long)(data[pos+1] & 255) << 16)
                    | ((data[pos+2] & 255) << 8) | (data[pos+3] & 255);
            if (size == 0 || size > data.length - pos - 4) break;
            result.add(Arrays.copyOfRange(data, pos + 4, pos + 4 + (int)size));
            pos += 4 + (int)size;
        }
        if (pos == data.length && !result.isEmpty()) return result;
        result.clear();
        int first = start(data, 0);
        if (first >= 0) {
            for (int i = 0; i < first; i++)
                if (data[i] != 0) throw new IllegalArgumentException("Invalid Annex B prefix");
            pos = first;
            while (pos >= 0) {
                int begin = pos + (data[pos+2] == 1 ? 3 : 4);
                int next = start(data, begin);
                int end = next < 0 ? data.length : next;
                while (end > begin && data[end-1] == 0) end--;
                if (end <= begin) throw new IllegalArgumentException("Empty Annex B NAL");
                result.add(Arrays.copyOfRange(data, begin, end));
                pos = next;
            }
            return result;
        }
        if (allowRaw) {
            result.add(data.clone());
            return result;
        }
        throw new IllegalArgumentException("Expected Annex B or four-byte AVCC lengths");
    }

    private static int start(byte[] data, int offset) {
        for (int i = offset; i + 2 < data.length; i++) {
            if (data[i] == 0 && data[i+1] == 0 &&
                    (data[i+2] == 1 || (i + 3 < data.length && data[i+2] == 0 && data[i+3] == 1))) return i;
        }
        return -1;
    }

    private static byte[] full(String type, int version, int flags, byte[] payload) {
        return box(type, new Bytes().u8(version).u8(flags >> 16).u16(flags).bytes(payload).array());
    }
    private static byte[] box(String type, byte[] payload) {
        return new Bytes().u32(payload.length + 8L).ascii(type).bytes(payload).array();
    }
    private static byte[] join(byte[]... arrays) {
        Bytes out = new Bytes();
        for (byte[] array : arrays) out.bytes(array);
        return out.array();
    }
    private static final class Bytes {
        private final ByteArrayOutputStream out = new ByteArrayOutputStream();
        Bytes u8(int value) { out.write(value); return this; }
        Bytes u16(int value) { return u8(value >> 8).u8(value); }
        Bytes u32(long value) { return u16((int)(value >> 16)).u16((int)value); }
        Bytes u64(long value) { return u32(value >> 32).u32(value); }
        Bytes bytes(byte[] value) { out.write(value, 0, value.length); return this; }
        Bytes zeros(int count) { return bytes(new byte[count]); }
        Bytes ascii(String value) { for (int i=0; i<value.length(); i++) u8(value.charAt(i)); return this; }
        Bytes matrix() { return u32(0x10000).u32(0).u32(0).u32(0).u32(0x10000).u32(0).u32(0).u32(0).u32(0x40000000); }
        byte[] array() { return out.toByteArray(); }
    }
}
