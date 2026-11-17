using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.Web.Script.Serialization;

// The Windows test owner is the only reader of commands and writer of public frames.
public sealed class KilogramTestMailbox
{
    const int MaxCommand = 4096, MaxFrame = 16384, MaxSequence = 192, MaxCursor = 512;
    static readonly UTF8Encoding Utf8 = new UTF8Encoding(false, true);
    static readonly Regex Generation = new Regex(@"\A[0-9a-f]{32}\z");
    static readonly Regex Digest = new Regex(@"\A[0-9a-f]{64}\z");
    static readonly Regex Id = new Regex(@"\A[a-zA-Z0-9_-]{1,64}\z");
    [DllImport("kernel32.dll")] static extern ulong GetTickCount64();

    readonly string path;
    readonly long workTick;
    readonly object gate = new object();
    int sequence, cursor;
    string pending;
    bool pendingQuit;
    volatile bool quitAcknowledged;

    static JavaScriptSerializer Parser(int cap) {
        return new JavaScriptSerializer { MaxJsonLength = cap, RecursionLimit = 24 };
    }

    static void Require(bool condition) {
        if (!condition) throw new InvalidDataException("Mailbox frame or state rejected");
    }

    static void Guard(string path) {
        for (var dir = new DirectoryInfo(path); dir != null; dir = dir.Parent)
            Require(dir.Exists && (dir.Attributes & FileAttributes.ReparsePoint) == 0);
    }

    static bool PlainFile(string path) {
        try {
            FileAttributes attr = File.GetAttributes(path);
            Require((attr & (FileAttributes.ReparsePoint | FileAttributes.Directory)) == 0);
            return true;
        } catch (FileNotFoundException) { return false; }
    }

    static byte[] Read(string path, int cap) {
        Require(PlainFile(path));
        using (var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read)) {
            Require(file.Length > 0 && file.Length <= cap);
            var bytes = new byte[(int)file.Length];
            for (int position = 0; position < bytes.Length;) {
                int count = file.Read(bytes, position, bytes.Length - position);
                Require(count > 0);
                position += count;
            }
            Require(file.ReadByte() == -1);
            return bytes;
        }
    }

    // A same-directory rename publishes a complete frame without replacing existing evidence.
    static bool Publish(string path, byte[] bytes) {
        var temp = Path.Combine(Path.GetDirectoryName(path), "partial-" + Guid.NewGuid().ToString("N"));
        try {
            using (var file = new FileStream(temp, FileMode.CreateNew, FileAccess.Write, FileShare.None)) {
                file.Write(bytes, 0, bytes.Length);
                file.Flush(true);
            }
            try { File.Move(temp, path); return true; }
            catch (IOException) { if (!PlainFile(path)) throw; return false; }
        } finally { if (File.Exists(temp)) File.Delete(temp); }
    }

    static string Numbered(string root, string kind, int index) {
        return Path.Combine(root, kind + "-" + index.ToString("D4") + ".json");
    }

    static Dictionary<string, object> Json(byte[] bytes, int cap) {
        var row = Parser(cap).DeserializeObject(Utf8.GetString(bytes)) as Dictionary<string, object>;
        Require(row != null && row.ContainsKey("v") && object.Equals(row["v"], 1));
        return row;
    }

    static Dictionary<string, object> Command(byte[] bytes) {
        Require(bytes.Length <= MaxCommand && !bytes.Any(b => b == 0 || b == 10 || b == 13));
        var row = Json(bytes, MaxCommand);
        Require(row.Count == 4 && row["id"] is string && Id.IsMatch((string)row["id"]) &&
                row["op"] is string && row["args"] is Dictionary<string, object>);
        return row;
    }

    public KilogramTestMailbox(string root, string generation, long workTick) {
        Require(Generation.IsMatch(generation));
        Guard(Path.GetDirectoryName(root));
        Directory.CreateDirectory(root);
        Guard(root);
        // Generation is fixed by the first owner. A restarted owner cannot replay this directory.
        Require(Publish(Path.Combine(root, "owner"), Utf8.GetBytes(generation + "\n" + workTick)));
        path = root;
        this.workTick = workTick;
    }

    public static string Exchange(string root, string generation, string op, int seq, string digest,
                                  int after, TextReader input) {
        Require(Generation.IsMatch(generation) && after >= 0 && after <= MaxCursor);
        Guard(root);
        string[] owner = Utf8.GetString(Read(Path.Combine(root, "owner"), 60)).Split('\n');
        long cutoff;
        Require(owner.Length == 2 && owner[0] == generation && long.TryParse(owner[1], out cutoff));
        cutoff = long.Parse(owner[1]);
        string terminal = Path.Combine(root, "terminal");
        bool closed = PlainFile(terminal);
        bool expired = (long)GetTickCount64() >= cutoff;
        bool stopped = PlainFile(Path.Combine(root, "stop"));
        string state = null;
        if (op == "submit") {
            Require(seq > 0 && seq <= MaxSequence && Digest.IsMatch(digest));
            var encoded = new StringBuilder();
            for (int b; (b = input.Read()) != '\n';) {
                Require(b >= 0 && encoded.Length < ((MaxCommand + 2) / 3) * 4 + 1);
                if (b != '\r') encoded.Append((char)b);
            }
            Require(input.Read() == -1);
            byte[] bytes = Convert.FromBase64String(encoded.ToString());
            Command(bytes);
            string sha;
            using (var h = SHA256.Create()) sha = BitConverter.ToString(h.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
            Require(sha == digest);
            string command = Numbered(root, "command", seq);
            if (!PlainFile(command)) {
                Require((seq == 1 || PlainFile(Numbered(root, "done", seq - 1))) &&
                        !closed && !expired && !stopped && !PlainFile(Path.Combine(root, "closing")));
                Publish(command, bytes);
            }
            Require(Read(command, MaxCommand).SequenceEqual(bytes));
            state = PlainFile(Numbered(root, "done", seq)) ? "done" : (closed || expired || stopped ? "unknown" : "pending");
        } else if (op == "stop") {
            if (!closed && !expired) Publish(Path.Combine(root, "stop"), new byte[] { 1 });
        } else Require(op == "read");

        var records = new List<string>();
        for (int i = after + 1; i <= MaxCursor && records.Count < 8; i++) {
            string next = Numbered(root, "public", i);
            if (!PlainFile(next)) break;
            records.Add(Convert.ToBase64String(Read(next, MaxFrame)));
        }
        return Parser(256 * 1024).Serialize(new {
            state, cursor = after + records.Count, records, expired, stopped,
            terminal = closed ? Utf8.GetString(Read(terminal, 16)) : null
        });
    }

    public bool Stopped {
        get { return PlainFile(Path.Combine(path, "stop")) || (long)GetTickCount64() >= workTick; }
    }

    public bool QuitAcknowledged { get { return quitAcknowledged; } }

    byte[] Next(Process child = null) {
        while (!Stopped && !QuitAcknowledged && (child == null || !child.HasExited)) {
            lock (gate) {
                if (pending == null && cursor > 0 && sequence < MaxSequence) {
                    string command = Numbered(path, "command", sequence + 1);
                    if (PlainFile(command)) {
                        var bytes = Read(command, MaxCommand);
                        var row = Command(bytes);
                        pending = (string)row["id"];
                        pendingQuit = object.Equals(row["op"], "quit");
                        sequence++;
                        return bytes;
                    }
                }
            }
            Thread.Sleep(40);
        }
        return null;
    }

    void PublicFrame(string line, bool setup) {
        var bytes = Utf8.GetBytes(line);
        Require(bytes.Length <= MaxFrame);
        var row = Json(bytes, MaxFrame);
        string kind = row["type"] as string;
        lock (gate) {
            Require(cursor < MaxCursor);
            if (cursor == 0) Require(kind == "ready");
            else if (kind == "result") Require(pending != null && object.Equals(row["id"], pending));
            else Require(!setup && kind == "event" && object.Equals(row["event"], "incoming") && !row.ContainsKey("id"));
            Require(Publish(Numbered(path, "public", ++cursor), bytes));
            if (kind == "result") {
                if (pendingQuit && object.Equals(row["ok"], true)) {
                    Require(Publish(Path.Combine(path, "closing"), new byte[] { 1 }));
                    quitAcknowledged = true;
                }
                Require(Publish(Numbered(path, "done", sequence), new byte[] { 1 }));
                pending = null;
            }
        }
    }

    public Stream SetupInput() { return new CommandStream(this); }
    public TextWriter SetupOutput() { return new Projection(this); }
    public void Finish(bool clean) {
        Require(Publish(Path.Combine(path, "terminal"), Utf8.GetBytes(clean ? "clean" : "failed")));
    }

    public Task<bool> ForwardOutput(Stream from) {
        return Task.Run(delegate {
            using (var buffer = new MemoryStream())
            using (var source = new BufferedStream(from, 4096)) {
                for (int b; (b = source.ReadByte()) >= 0;) {
                    if (b != 10) {
                        Require(buffer.Length < MaxFrame);
                        buffer.WriteByte((byte)b);
                        continue;
                    }
                    PublicFrame(Utf8.GetString(buffer.ToArray()).TrimEnd('\r'), false);
                    buffer.SetLength(0);
                }
                Require(buffer.Length == 0);
            }
            return true;
        });
    }

    public Task FeedInput(Stream to, Process child) {
        return Task.Run(delegate {
            using (to) {
                byte[] bytes;
                while ((bytes = Next(child)) != null) {
                    to.Write(bytes, 0, bytes.Length);
                    to.WriteByte(10);
                    to.Flush();
                }
            }
        });
    }

    public Task<bool> EmptyError(Stream from) {
        return Task.Run(delegate {
            bool empty = from.ReadByte() == -1;
            if (!empty) from.CopyTo(Stream.Null, 4096);
            return empty;
        });
    }

    // The writer is handed only to the current allowlisted Setup projection.
    sealed class Projection : TextWriter {
        readonly KilogramTestMailbox owner;
        public Projection(KilogramTestMailbox owner) { this.owner = owner; }
        public override Encoding Encoding { get { return Utf8; } }
        public override void WriteLine(string value) { owner.PublicFrame(value, true); }
    }

    sealed class CommandStream : Stream {
        readonly KilogramTestMailbox owner;
        byte[] frame;
        int position;
        public CommandStream(KilogramTestMailbox owner) { this.owner = owner; }
        public override int ReadByte() {
            if (frame == null) {
                frame = owner.Next();
                position = 0;
                if (frame == null) return -1;
            }
            if (position < frame.Length) return frame[position++];
            frame = null;
            return 10;
        }
        public override int Read(byte[] buffer, int offset, int count) {
            if (count == 0) return 0;
            int b = ReadByte();
            if (b < 0) return 0;
            buffer[offset] = (byte)b;
            return 1;
        }
        public override bool CanRead { get { return true; } }
        public override bool CanSeek { get { return false; } }
        public override bool CanWrite { get { return false; } }
        public override long Length { get { throw new NotSupportedException(); } }
        public override long Position { get { throw new NotSupportedException(); } set { throw new NotSupportedException(); } }
        public override void Flush() { }
        public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
        public override void SetLength(long value) { throw new NotSupportedException(); }
        public override void Write(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
    }
}
