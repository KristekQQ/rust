package cz.honza.queststream;

import java.io.ByteArrayOutputStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Run with assertions enabled. Optional input/output paths mux an AUD-delimited AVC fixture. */
public final class FragmentedMp4Test {
    private static final byte[] SPS = {0x67, 0x42, 0, 0x1e, (byte)0x95};
    private static final byte[] PPS = {0x68, (byte)0xce, 6};
    public static void main(String[] args) throws Exception {
        FragmentedMp4 mux = new FragmentedMp4(320, 240, annex(SPS), annex(PPS));
        byte[] init = mux.initSegment();
        assert Arrays.equals(init, mux.initSegment());
        int avcc = find(init, "avcC");
        assert init[avcc+8] == 1 && (init[avcc+12] & 255) == 255;
        assert init[avcc+9] == 0x42 && init[avcc+11] == 0x1e;
        byte[] idr = {0x65, 0x11, 0x22};
        byte[] access = join(annex(new byte[]{9, (byte)0xf0}), annex(SPS), annex(PPS), annex(idr));
        byte[] first = mux.fragment(access, 987654321, false);
        int moofSize = u32(first, 0), mdat = moofSize;
        assert u32(first, mdat) == 15;
        assert Arrays.equals(Arrays.copyOfRange(first, mdat+8, first.length), new byte[]{0,0,0,3,0x65,0x11,0x22});
        int trun = find(first, "trun"), tfdt = find(first, "tfdt");
        assert u32(first, trun+16) == moofSize+8;
        assert u32(first, trun+20) == 33333;
        assert u32(first, trun+24) == 7;
        assert u32(first, trun+28) == 0x02000000;
        assert u64(first, tfdt+12) == 0;
        byte[] second = mux.fragment(new byte[]{0,0,0,2,0x41,0x33}, 987654320, false);
        assert u64(second, find(second,"tfdt")+12) == 33333;
        assert u32(second, find(second,"mfhd")+12) == 2;
        assert u32(second, find(second,"trun")+28) == 0x01010000;
        reject(() -> mux.fragment(annex(SPS), 1, true));
        reject(() -> mux.fragment(new byte[]{0,0,0,10,0x65}, 1, true));
        reject(() -> mux.fragment(annex(idr), -1, true));
        reject(() -> new FragmentedMp4(0, 240, SPS, PPS));
        // A valid four-byte AVCC length 0x00000101 must not be mistaken for Annex B.
        byte[] large = new byte[261]; large[2] = 1; large[3] = 1; large[4] = 0x41;
        byte[] third = mux.fragment(large, 987754321, false);
        assert u32(third, find(third,"trun")+24) == 261;
        assert u64(third, find(third,"tfdt")+12) == 100000;
        // Mixed start-code lengths and multiple slices preserve slice boundaries.
        byte[] mixed = join(new byte[]{0,0,1,0x41,0x11}, new byte[]{0,0,0,1,0x41,0x22});
        byte[] fourth = mux.fragment(mixed, 987854321, false);
        int fourthMdat = u32(fourth, 0);
        assert Arrays.equals(Arrays.copyOfRange(fourth, fourthMdat+8, fourth.length),
                new byte[]{0,0,0,2,0x41,0x11,0,0,0,2,0x41,0x22});
        assert u32(fourth, find(fourth,"mfhd")+12) == 4;
        long farPts = 987654321L + (1L << 33);
        byte[] fifth = mux.fragment(annex(idr), farPts, true);
        assert u64(fifth, find(fifth,"tfdt")+12) == (1L << 33);
        FragmentedMp4 custom = new FragmentedMp4(320,240,SPS,PPS,50000);
        byte[] customFragment = custom.fragment(annex(idr),0,true);
        assert u32(customFragment, find(customFragment,"trun")+20) == 50000;
        FragmentedMp4 av = new FragmentedMp4(320,240,SPS,PPS,33333,9000000L,new byte[]{0x11,(byte)0x90},48000,2);
        byte[] avVideo = av.fragment(annex(idr),9100000L,true);
        byte[] avAudio = av.audioFragment(new byte[]{1,2},9120000L);
        assert u64(avVideo,find(avVideo,"tfdt")+12) == 100000 : "video preserves shared origin";
        assert u64(avAudio,find(avAudio,"tfdt")+12) == 5760 : "audio preserves 20ms A/V offset";
        assert u32(avAudio,find(avAudio,"tfhd")+12) == 2;
        assert u32(avAudio,find(avAudio,"trun")+20) == 1024;
        reject(() -> av.audioFragment(new byte[]{1},8999999L));
        if (args.length >= 2) fixture(args[0], args[1], args.length >= 3 ? Integer.parseInt(args[2]) : 60,
                args.length >= 4 ? args[3] : null);
        System.out.println("PASS FragmentedMp4 deterministic boxes, offsets, flags, timestamps and input formats");
    }

    private static void fixture(String input, String output, int expectedFrames, String audioFile) throws Exception {
        List<byte[]> nals = split(Files.readAllBytes(Paths.get(input)));
        byte[] sps=null, pps=null;
        List<byte[]> units = new ArrayList<>();
        ByteArrayOutputStream current = new ByteArrayOutputStream();
        boolean slice=false;
        for (byte[] nal : nals) {
            int type=nal[0]&31;
            if (type==7) sps=nal;
            if (type==8) pps=nal;
            if (type==9 && slice) { units.add(current.toByteArray()); current.reset(); slice=false; }
            byte[] encoded=annex(nal); current.write(encoded);
            if (type>=1 && type<=5) slice=true;
        }
        if (slice) units.add(current.toByteArray());
        if (sps==null || pps==null || units.size()!=expectedFrames) throw new AssertionError("Expected " + expectedFrames + " AVC frames");
        List<byte[]> audio = new ArrayList<>();
        if (audioFile != null) {
            byte[] adts = Files.readAllBytes(Paths.get(audioFile));
            for (int i=0;i<adts.length;) {
                if ((adts[i]&255)!=255 || (adts[i+1]&0xf6)!=0xf0) throw new AssertionError("Invalid ADTS");
                int size=((adts[i+3]&3)<<11)|((adts[i+4]&255)<<3)|((adts[i+5]&0xe0)>>5);
                int header=(adts[i+1]&1)==1?7:9;
                if (((adts[i+2]>>2)&15)!=3 || (((adts[i+2]&1)<<2)|((adts[i+3]>>6)&3))!=2)
                    throw new AssertionError("Expected 48k stereo ADTS");
                audio.add(Arrays.copyOfRange(adts,i+header,i+size)); i+=size;
            }
        }
        long origin = audioFile == null ? 0 : 9000000L;
        FragmentedMp4 mux = audioFile == null ? new FragmentedMp4(320,240,sps,pps) :
            new FragmentedMp4(320,240,sps,pps,33333,origin,new byte[]{0x11,(byte)0x90},48000,2);
        ByteArrayOutputStream out = new ByteArrayOutputStream(); out.write(mux.initSegment());
        int a=0;
        for (int i=0;i<units.size();i++) {
            long videoPts = i*1000000L/30;
            while (a<audio.size() && 120000L+a*1024000000L/48000 <= videoPts) {
                out.write(mux.audioFragment(audio.get(a), origin+120000L+a*1024000000L/48000)); a++;
            }
            out.write(mux.fragment(units.get(i), origin+videoPts, false));
        }
        while(a<audio.size()) { out.write(mux.audioFragment(audio.get(a),origin+120000L+a*1024000000L/48000)); a++; }
        Files.write(Paths.get(output),out.toByteArray());
        System.out.println("PASS muxed " + expectedFrames + " generated AVC frames");
    }
    private static List<byte[]> split(byte[] data) {
        List<byte[]> out=new ArrayList<>(); int begin=-1;
        for (int i=0;i+2<data.length;i++) {
            if (data[i]==0 && data[i+1]==0 && (data[i+2]==1 || (i+3<data.length && data[i+2]==0 && data[i+3]==1))) {
                if (begin>=0) out.add(Arrays.copyOfRange(data,begin,i));
                int length=data[i+2]==1?3:4; begin=i+length; i=begin-1;
            }
        }
        if(begin>=0) out.add(Arrays.copyOfRange(data,begin,data.length)); return out;
    }
    private static void reject(Runnable work) {
        try { work.run(); throw new AssertionError("Expected rejection"); } catch (IllegalArgumentException expected) { }
    }
    private static int find(byte[] data,String type) {
        for(int i=4;i+4<=data.length;i++) {
            boolean match=true; for(int j=0;j<4;j++) if(data[i+j]!=(byte)type.charAt(j)) match=false;
            if(match) return i-4;
        }
        throw new AssertionError("Missing "+type);
    }
    private static int u32(byte[] b,int p) { return (b[p]&255)<<24|(b[p+1]&255)<<16|(b[p+2]&255)<<8|(b[p+3]&255); }
    private static long u64(byte[] b,int p) { return ((long)u32(b,p)<<32)|(u32(b,p+4)&0xffffffffL); }
    private static byte[] annex(byte[] nal) { return join(new byte[]{0,0,0,1},nal); }
    private static byte[] join(byte[]... arrays) {
        ByteArrayOutputStream out=new ByteArrayOutputStream(); for(byte[] a:arrays) out.write(a,0,a.length); return out.toByteArray();
    }
}
