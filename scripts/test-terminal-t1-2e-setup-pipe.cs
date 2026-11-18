using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Web.Script.Serialization;

// Local-only test bridge. The child never inherits the controller's streams.
public sealed class KilogramSetupPipe
{
    private const int Limit = 16384;
    private static readonly UTF8Encoding Utf8 = new UTF8Encoding(false, true);
    private static readonly Regex Hex = new Regex(@"\A[0-9a-f]{64}\z");
    private static readonly Regex LanId = new Regex(@"\A(?!0{32}\z)[0-9a-f]{32}\z");
    private static readonly Regex Tag = new Regex(@"\A[a-zA-Z0-9_-]{1,64}\z");
    private static readonly Regex Label = new Regex(@"\A[a-z_]{1,48}\z");
    private static readonly Regex Recovery = new Regex(@"\A[a-z]{1,16}(?: [a-z]{1,16}){23}\z");
    private static readonly HashSet<string> Allowed = new HashSet<string> {
        "begin", "open", "create", "status", "quit",
        "invite_export", "invite_inspect", "invite_join",
        "reply_export", "reply_inspect", "reply_accept",
        "consent_propose", "consent_export", "consent_inspect", "consent_accept",
        "consent_cosign", "consent_import", "route_lan", "route_lan_auto"
    };

    private readonly JavaScriptSerializer json = new JavaScriptSerializer {
        MaxJsonLength = Limit, RecursionLimit = 16
    };
    private readonly string setup;

    private KilogramSetupPipe(string setup) { this.setup = setup; }

    public static void Redirect(ProcessStartInfo child)
    {
        child.UseShellExecute = false;
        child.RedirectStandardInput = true;
        child.RedirectStandardOutput = true;
        child.RedirectStandardError = true;
        child.CreateNoWindow = true;
    }

    public static Task Run(Process child, Stream input, TextWriter output, string setup)
    {
        return Task.Run(delegate {
            try { new KilogramSetupPipe(setup).Exchange(child, input, output); }
            catch { throw new InvalidDataException("Protected Setup exchange failed"); }
        });
    }

    private static void Require(bool condition)
    {
        if (!condition) throw new InvalidDataException("Invalid Setup frame");
    }

    private Dictionary<string, object> Read(Stream source)
    {
        byte[] bytes = new byte[Limit];
        try {
            int count = 0;
            while (true) {
                int next = source.ReadByte();
                if (next < 0) {
                    Require(count == 0);
                    return null;
                }
                if (next == 10) {
                    return Object(json.DeserializeObject(Utf8.GetString(bytes, 0, count)));
                }
                Require(count < bytes.Length && next != 0);
                bytes[count++] = (byte)next;
            }
        } finally {
            Array.Clear(bytes, 0, bytes.Length);
        }
    }

    private static Dictionary<string, object> Object(object value)
    {
        var result = value as Dictionary<string, object>;
        Require(result != null);
        return result;
    }

    private static string Field(Dictionary<string, object> value, string name, Regex format)
    {
        var field = value[name] as string;
        Require(field != null && format.IsMatch(field));
        return field;
    }

    private static string Ip(object input)
    {
        IPAddress ipv4;
        string text = input as string;
        Require(text != null && IPAddress.TryParse(text, out ipv4) && ipv4.AddressFamily == AddressFamily.InterNetwork
                && text == ipv4.ToString());
        return text;
    }

    private static Dictionary<string, object> Fields(params object[] values)
    {
        var result = new Dictionary<string, object>();
        for (int i = 0; i < values.Length; i += 2) result.Add((string)values[i], values[i + 1]);
        return result;
    }

    private static object Identity(Dictionary<string, object> value)
    {
        return Fields("account_id", Field(value, "account_id", Hex), "device_id", Field(value, "device_id", Hex));
    }

    private static object Contexts(object value)
    {
        var items = value as object[];
        Require(items != null && items.Length == 2);
        foreach (object item in items) Require(item is string && Hex.IsMatch((string)item));
        return items;
    }

    private static Dictionary<string, object> LanDiscovery(Dictionary<string, object> discovery, string routeMode, string peer)
    {
        string mode = Field(discovery, "mode", Label);
        if (mode == "manual") {
            Require(discovery.Count == 1);
            return Fields("mode", mode);
        }
        Require(mode == "auto" && discovery.Count == 4 && routeMode == "direct_lan" && peer != null);
        object prefix = discovery["prefix"];
        Require(prefix is int && (int)prefix >= 1 && (int)prefix <= 30);
        return Fields("mode", mode, "adapter_id", Field(discovery, "adapter_id", LanId),
            "network_id", Field(discovery, "network_id", LanId), "prefix", prefix);
    }

    private static Dictionary<string, object> Status(Dictionary<string, object> value)
    {
        var result = Fields("state", Field(value, "state", Label));
        foreach (string key in new[] {"account_id", "device_id"})
            result[key] = value[key] == null ? null : Field(value, key, Hex);
        result["contexts"] = value["contexts"] == null ? null : Contexts(value["contexts"]);
        if (value.ContainsKey("peer_selection")) {
            var peer = Object(value["peer_selection"]);
            result["peer_selection"] = Fields("role", Field(peer, "role", Label),
                "invitation_id", Field(peer, "invitation_id", Hex), "peer", Identity(Object(peer["peer"])));
        }
        if (value.ContainsKey("consent")) {
            var consent = Object(value["consent"]);
            var slots = consent["slots"] as object[];
            Require(slots != null && slots.Length == 2);
            var replies = new object[2];
            for (int i = 0; i < 2; i++) {
                var slot = Object(slots[i]);
                replies[i] = Fields("context_id", Field(slot, "context_id", Hex), "importing", Field(slot, "importing", Label));
            }
            result["consent"] = Fields("role", Field(consent, "role", Label), "stage", Field(consent, "stage", Label),
                "local_installation", Field(consent, "local_installation", Label), "slots", replies);
        }
        if (value.ContainsKey("connection")) {
            var route = Object(value["connection"]);
            string mode = Field(route, "mode", Label);
            string peer = route["peer_ipv4"] == null ? null : Ip(route["peer_ipv4"]);
            var connection = Fields("mode", mode, "local_ipv4", Ip(route["local_ipv4"]),
                "peer_ipv4", peer, "profile_state", Field(route, "profile_state", Label));
            if (route.ContainsKey("lan_discovery"))
                connection["lan_discovery"] = LanDiscovery(Object(route["lan_discovery"]), mode, peer);
            result["connection"] = connection;
        }
        return result;
    }

    private Dictionary<string, object> Artifact(Dictionary<string, object> value, bool export)
    {
        string kind = Field(value, "kind", Label);
        Require(kind == "invite" || kind == "reply" || kind == "proposal" || kind == "final");
        var result = Fields("kind", kind, "invitation_id", Field(value, "invitation_id", Hex),
            "contexts", Contexts(value["contexts"]), "sender", Identity(Object(value["sender"])),
            "recipient", value["recipient"] == null ? null : Identity(Object(value["recipient"])));
        if (export) {
            string expected = Path.Combine(setup, "exchange", kind + ".kilogram");
            string actual = value["path"] as string;
            Require(actual != null);
            if (actual.StartsWith(@"\\?\", StringComparison.Ordinal)) actual = actual.Substring(4);
            Require(string.Equals(actual, expected, StringComparison.OrdinalIgnoreCase));
            result["path"] = expected;
        } else {
            result["preview_id"] = Field(value, "preview_id", Hex);
        }
        if (value.ContainsKey("artifact_digest")) result["artifact_digest"] = Field(value, "artifact_digest", Hex);
        return result;
    }

    private static void Response(Dictionary<string, object> message, string id)
    {
        Require(message != null && object.Equals(message["v"], 1)
            && object.Equals(message["type"], "result") && object.Equals(message["id"], id)
            && message["ok"] is bool && message.Count == 5);
    }

    private Dictionary<string, object> Call(Process child, Stream input, string id, string op, object args)
    {
        child.StandardInput.WriteLine(json.Serialize(Fields("v", 1, "id", id, "op", op, "args", args)));
        child.StandardInput.Flush();
        var result = Read(input);
        Response(result, id);
        return result;
    }

    private void ReadPrivateRecovery(Process child, Stream input)
    {
        var reply = Call(child, input, "host-recovery", "recovery_show", Fields());
        Require(object.Equals(reply["ok"], true));
        var value = Object(reply["value"]);
        Require(value.Count == 2 && object.Equals(value["recovery_scope"], "root-key-only-authority-history-required"));
        Field(value, "recovery_words", Recovery);
        // The UTF-8 buffer is cleared in Read. Managed JSON strings cannot be reliably erased.
        value.Clear();
        reply.Clear();
    }

    private void Exchange(Process child, Stream input, TextWriter output)
    {
        Task stderr = child.StandardError.BaseStream.CopyToAsync(Stream.Null, 4096);
        Stream receive = child.StandardOutput.BaseStream;
        var ready = Read(receive);
        Require(ready != null && ready.Count == 2 && object.Equals(ready["v"], 1) && object.Equals(ready["type"], "ready"));
        Write(output, Fields("v", 1, "type", "ready"));
        bool recover = false;
        while (true) {
            var command = Read(input);
            Require(command != null && command.Count == 4 && object.Equals(command["v"], 1));
            string id = Field(command, "id", Tag);
            string op = Field(command, "op", Label);
            var args = Object(command["args"]);
            bool localRecovery = op == "save_test_recovery";
            if (localRecovery) {
                Require(recover && args.Count == 0);
                recover = false;
                ReadPrivateRecovery(child, receive);
                op = "recovery_ack";
                args = Fields("saved", true);
            } else {
                Require(Allowed.Contains(op));
                if (op == "route_lan_auto") {
                    Require(args.Count == 2);
                    args = Fields("local_ipv4", Ip(args["local_ipv4"]), "peer_ipv4", Ip(args["peer_ipv4"]));
                }
            }
            var reply = Call(child, receive, id, op, args);
            if (!object.Equals(reply["ok"], true)) {
                Require(!localRecovery);
                Write(output, Fields("v", 1, "type", "result", "id", id, "ok", false,
                    "code", Field(reply, "code", Label)));
                continue;
            }
            var value = Object(reply["value"]);
            object safe;
            if (op.EndsWith("_inspect", StringComparison.Ordinal) || op.EndsWith("_export", StringComparison.Ordinal))
                safe = Artifact(value, op.EndsWith("_export", StringComparison.Ordinal));
            else if (op == "quit") {
                Require(value.Count == 1 && object.Equals(value["closed"], true));
                safe = Fields("closed", true);
            } else {
                safe = Status(value);
                if (op == "create") recover = object.Equals(value["state"], "recovery_available");
                if (localRecovery) Require(object.Equals(value["state"], "recovery_saved"));
            }
            Write(output, Fields("v", 1, "type", "result", "id", id, "ok", true, "value", safe));
            if (op == "quit") {
                child.StandardInput.Close();
                Require(Read(receive) == null);
                stderr.GetAwaiter().GetResult();
                return;
            }
        }
    }

    private void Write(TextWriter output, object value)
    {
        output.WriteLine(json.Serialize(value));
        output.Flush();
    }
}
