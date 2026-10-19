param(
    [Parameter(Mandatory)][string]$Application,
    [Parameter(Mandatory)][string]$Report
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$applicationPath = if ([IO.Path]::IsPathRooted($Application)) { [IO.Path]::GetFullPath($Application) } else { [IO.Path]::GetFullPath((Join-Path $repository $Application)) }
$reportPath = if ([IO.Path]::IsPathRooted($Report)) { [IO.Path]::GetFullPath($Report) } else { [IO.Path]::GetFullPath((Join-Path $repository $Report)) }
$buildRoot = [IO.Path]::GetFullPath((Join-Path $repository 'build')) + [IO.Path]::DirectorySeparatorChar
if (-not $reportPath.StartsWith($buildRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'GUI reports must stay inside this repository build directory' }
$node = (Get-Command node -ErrorAction Stop).Source
$driver = Join-Path $PSScriptRoot 'tauri_windows_smoke.mjs'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    & $node $driver $applicationPath $reportPath
    exit $LASTEXITCODE
}

# Keep the same user/profile while removing administrator rights for desktop UI.
$parent = Split-Path -Parent $reportPath
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$acl = Get-Acl -LiteralPath $parent
$rule = [Security.AccessControl.FileSystemAccessRule]::new(
    $identity.User, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$acl.AddAccessRule($rule)
Set-Acl -LiteralPath $parent -AclObject $acl
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
public static class ShuGuiUser {
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    public struct Startup {
        public int cb;
        public string reserved, desktop, title;
        public uint x,y,xSize,ySize,xChars,yChars,fill,flags;
        public ushort show,reservedSize;
        public IntPtr reservedData,input,output,error;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
    [StructLayout(LayoutKind.Sequential)]
    public struct SidAndAttributes { public IntPtr sid; public uint attributes; }
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll")] static extern IntPtr GetStdHandle(int kind);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool DuplicateHandle(IntPtr sourceProcess,IntPtr source,IntPtr targetProcess,out IntPtr target,uint access,bool inherit,uint options);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr pointer);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool SaferCreateLevel(uint scope,uint level,uint flags,out IntPtr handle,IntPtr reserved);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool SaferComputeTokenFromLevel(IntPtr level,IntPtr original,out IntPtr token,uint flags,IntPtr reserved);
    [DllImport("advapi32.dll")] static extern bool SaferCloseLevel(IntPtr level);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool ConvertStringSidToSid(string text,out IntPtr sid);
    [DllImport("advapi32.dll")] static extern uint GetLengthSid(IntPtr sid);
    [DllImport("advapi32.dll", SetLastError=true)] static extern bool SetTokenInformation(IntPtr token,int kind,IntPtr value,uint length);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool CreateProcessAsUser(IntPtr token,string application,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string directory,ref Startup startup,out ProcessInfo process);
    static void Check(bool result) { if (!result) throw new Win32Exception(Marshal.GetLastWin32Error()); }
    static string Quote(string value) {
        var result=new StringBuilder("\""); int slashes=0;
        foreach(char c in value) {
            if(c=='\\') { slashes++; continue; }
            if(c=='"') result.Append('\\',slashes*2+1).Append(c);
            else result.Append('\\',slashes).Append(c);
            slashes=0;
        }
        return result.Append('\\',slashes*2).Append('"').ToString();
    }
    static IntPtr Standard(int kind) {
        var source=GetStdHandle(kind);
        if(source==IntPtr.Zero || source==new IntPtr(-1)) return IntPtr.Zero;
        IntPtr result;
        Check(DuplicateHandle(GetCurrentProcess(),source,GetCurrentProcess(),out result,0,true,2));
        return result;
    }
    public static int Run(string node,string driver,string application,string report,string directory) {
        IntPtr original=IntPtr.Zero,level=IntPtr.Zero,token=IntPtr.Zero,sid=IntPtr.Zero,integrityBuffer=IntPtr.Zero;
        var startup=new Startup(); var process=new ProcessInfo();
        try {
            Check(OpenProcessToken(GetCurrentProcess(),0x000A,out original));
            Check(SaferCreateLevel(2,0x20000,1,out level,IntPtr.Zero));
            Check(SaferComputeTokenFromLevel(level,original,out token,0,IntPtr.Zero));
            Check(ConvertStringSidToSid("S-1-16-8192",out sid));
            int labelSize=Marshal.SizeOf(typeof(SidAndAttributes));
            var sidBytes=new byte[GetLengthSid(sid)];
            Marshal.Copy(sid,sidBytes,0,sidBytes.Length);
            integrityBuffer=Marshal.AllocHGlobal(labelSize+sidBytes.Length);
            var integrity=new SidAndAttributes { sid=IntPtr.Add(integrityBuffer,labelSize),attributes=0x20 };
            Marshal.StructureToPtr(integrity,integrityBuffer,false);
            Marshal.Copy(sidBytes,0,integrity.sid,sidBytes.Length);
            Check(SetTokenInformation(token,25,integrityBuffer,(uint)(labelSize+sidBytes.Length)));
            using(var identity=new WindowsIdentity(token)) {
                if(new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator))
                    throw new InvalidOperationException("GUI token still has administrator rights");
            }
            startup.cb=Marshal.SizeOf(typeof(Startup));
            startup.desktop="winsta0\\default"; startup.flags=0x101; startup.show=0;
            startup.input=Standard(-10); startup.output=Standard(-11); startup.error=Standard(-12);
            var command=new StringBuilder(Quote(node)+" "+Quote(driver)+" "+Quote(application)+" "+Quote(report));
            Check(CreateProcessAsUser(token,node,command,IntPtr.Zero,IntPtr.Zero,true,0x08000400,IntPtr.Zero,directory,ref startup,out process));
            Console.WriteLine("GUI suite running without administrator rights, PID="+process.pid);
            uint wait;
            while((wait=WaitForSingleObject(process.process,1000))==258) {}
            if(wait!=0) throw new Win32Exception(Marshal.GetLastWin32Error());
            uint code; Check(GetExitCodeProcess(process.process,out code)); return (int)code;
        } finally {
            foreach(var handle in new[]{process.thread,process.process,startup.input,startup.output,startup.error,token,original})
                if(handle!=IntPtr.Zero) CloseHandle(handle);
            if(sid!=IntPtr.Zero) LocalFree(sid);
            if(integrityBuffer!=IntPtr.Zero) Marshal.FreeHGlobal(integrityBuffer);
            if(level!=IntPtr.Zero) SaferCloseLevel(level);
        }
    }
}
'@
exit [ShuGuiUser]::Run($node, $driver, $applicationPath, $reportPath, $repository)
