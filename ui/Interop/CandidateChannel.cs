using System.Buffers.Binary;
using System.Runtime.InteropServices;
using System.Text;
namespace Shanjie_UI.Interop;
internal sealed record CandidateFrame(int Sequence, bool Visible, int Layout, int Selected, int First, int Total, int Left, int Bottom, int Top, int Dpi, int Theme, int FontSize, int Columns, bool Shutdown, nint Callback, string[] Words);
internal sealed class CandidateChannel : IDisposable
{
    private const int PacketSize = 80 + 81 * 256;
    private readonly nint mapping, view, parent;
    private int sequence = -1;
    public CandidateChannel(nint mapping)
    {
        this.mapping = mapping;
        view = Native.MapViewOfFile(mapping, 4, 0, 0, PacketSize);
        if (view == 0) throw new System.ComponentModel.Win32Exception();
        if (Marshal.ReadInt32(view, 4) != 1) throw new InvalidDataException("Unknown candidate protocol");
        parent = Native.OpenProcess(0x100000, false, (uint)Marshal.ReadInt32(view, 72));
    }
    public bool ParentExited => parent == 0 || Native.WaitForSingleObject(parent, 0) == 0;
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
        return new(before, Get(8) != 0, Get(12), Get(16), Get(24), Get(28), Get(32), Get(36), Get(40), Get(44), Get(48), Get(52), Get(56), Get(60) != 0, (nint)BinaryPrimitives.ReadInt64LittleEndian(bytes.AsSpan(64, 8)), words);
    }
    public void Dispose() { Native.UnmapViewOfFile(view); Native.CloseHandle(mapping); if (parent != 0) Native.CloseHandle(parent); }
}
