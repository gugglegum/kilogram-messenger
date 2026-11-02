param([Parameter(Mandatory)][ValidateSet('valid','pressure','malformed','invalid-utf8','nonzero','stdout-overflow','stderr-overflow','stall')][string]$Mode)
$ErrorActionPreference='Stop'
# Closed benign modes, no child process, profile/store access or network.
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
public static class S54CaptureFixture {
    static readonly string json="{\"keys\":{\"public\":\"S54_STATUS_SENTINEL\"},\"nested\":[{\"keys\":[\"S54_STATUS_SENTINEL\"]}],\"ok\":true}";
    static void Write(Stream stream, byte[] bytes) {
        for(int i=0;i<bytes.Length;i+=8192) stream.Write(bytes,i,Math.Min(8192,bytes.Length-i));
        stream.Flush();
    }
    public static int Run(string mode) {
        try {
            var output=Console.OpenStandardOutput(); var error=Console.OpenStandardError();
            byte[] raw=Encoding.UTF8.GetBytes(json);
            if(mode=="pressure") {
                raw=Encoding.UTF8.GetBytes(json.Substring(0,json.Length-1)+",\"padding\":\""+new string('x',524288)+"\"}");
                var a=Task.Run(()=>Write(output,raw));
                var b=Task.Run(()=>Write(error,Encoding.UTF8.GetBytes("S54_STDERR_SENTINEL"+new string('y',524288))));
                Task.WaitAll(a,b); return 0;
            }
            Write(error,Encoding.UTF8.GetBytes("S54_STDERR_SENTINEL"));
            if(mode=="stall") { Thread.Sleep(30000); return 0; }
            if(mode=="stdout-overflow") { Write(output,new byte[2097152]); return 0; }
            if(mode=="stderr-overflow") { Write(error,new byte[2097152]); return 0; }
            if(mode=="malformed") { Write(output,Encoding.UTF8.GetBytes("{\"keys\":\"S54_STATUS_SENTINEL\"")); return 0; }
            if(mode=="invalid-utf8") { Write(output,new byte[]{255,254,253}); return 0; }
            Write(output,raw); return mode=="nonzero"?17:0;
        } catch { return 23; }
    }
}
'@
exit ([S54CaptureFixture]::Run($Mode))
