using System.Buffers.Binary;
using System.Runtime.InteropServices;
using System.Text;
namespace Shanjie_UI.Interop;
internal sealed record CandidateFrame(int Sequence, bool Visible, int Layout, int Selected, int First, int Total, int Left, int Bottom, int Top, int Dpi, int Theme, int FontSize, int Columns, bool Shutdown, nint Callback, string[] Words, bool Associated = false, string Reading = "", bool SpacePages = true);
internal sealed class CandidateChannel : IDisposable
{
    private const int PacketSize = 80 + 82 * 256;
    private readonly nint mapping, view, parent, signal;
    private volatile bool stopping;
    private readonly object lifetime = new();
    private int sequence = -1;
    public CandidateChannel(nint mapping, nint signal)
    {
        this.mapping = mapping;
        this.signal = signal;
        view = Native.MapViewOfFile(mapping, 4, 0, 0, PacketSize);
        if (view == 0) throw new System.ComponentModel.Win32Exception();
        if (Marshal.ReadInt32(view, 4) != 2) throw new InvalidDataException("Unknown candidate protocol");
        parent = Native.OpenProcess(0x100000, false, (uint)Marshal.ReadInt32(view, 72));
    }
    public bool Wait() => !stopping && parent != 0 && Native.WaitForMultipleObjects(2, [signal, parent], false, uint.MaxValue) == 0 && !stopping;
    public void Stop() { lock (lifetime) { if (stopping) return; stopping = true; Native.SetEvent(signal); } }
    public CandidateFrame? Read()
    {
        int before = Marshal.ReadInt32(view);
        if (before == sequence || (before & 1) != 0) return null;
        var bytes = new byte[PacketSize]; Marshal.Copy(view, bytes, 0, bytes.Length);
        Thread.MemoryBarrier();
        if (before != Marshal.ReadInt32(view)) return null;
        sequence = before;
        int Get(int offset) => BinaryPrimitives.ReadInt32LittleEndian(bytes.AsSpan(offset, 4));
        int count = Get(20); if (count is < 0 or > 81) throw new InvalidDataException("Invalid candidate count");
        var words = new string[count];
        for (int i = 0; i < count; i++) words[i] = Encoding.Unicode.GetString(bytes, 80 + i * 256, 256).TrimEnd('\0');
        return new(before, Get(8) != 0, Get(12), Get(16), Get(24), Get(28), Get(32), Get(36), Get(40), Get(44), Get(48), Get(52), Get(56), Get(60) != 0, (nint)BinaryPrimitives.ReadInt64LittleEndian(bytes.AsSpan(64, 8)), words, (Get(76) & 1) != 0, Encoding.Unicode.GetString(bytes, 80 + 81 * 256, 256).TrimEnd('\0'), (Get(76) & 2) != 0);
    }
    public void Dispose() { lock (lifetime) { stopping = true; Native.UnmapViewOfFile(view); Native.CloseHandle(mapping); Native.CloseHandle(signal); if (parent != 0) Native.CloseHandle(parent); } }
}
