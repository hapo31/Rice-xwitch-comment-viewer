param([Parameter(Mandatory = $true)][int]$RicePid, [Parameter(Mandatory = $true)][string]$FixtureRoot, [switch]$ValidateOnly)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not $env:RUNNER_TEMP) { throw 'Native UI probes require a disposable GitHub-hosted Windows runner' }
$fixture = [IO.Path]::GetFullPath($FixtureRoot)
if (-not $fixture.StartsWith([IO.Path]::GetFullPath($env:RUNNER_TEMP) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixtures must be below RUNNER_TEMP' }
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type -ReferencedAssemblies System.dll, System.Core.dll, System.Windows.Forms.dll, System.Drawing.dll -TypeDefinition @'
using System;
using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
public static class RiceNativeProbe {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int dx, dy; public uint data, flags, time; public UIntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size=32)] public struct Union { [FieldOffset(0)] public Mouse mouse; }
  [StructLayout(LayoutKind.Sequential)] public struct Input { public uint type; public Union value; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, Input[] inputs, int size);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref Point p);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int command);
  [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int width,int height,uint flags);
  [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] static extern bool EnumWindows(Func<IntPtr,IntPtr,bool> callback, IntPtr arg);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder text, int max);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder text, int max);
  public static IntPtr Window(int pid) {
    using (var process = Process.GetProcessById(pid)) {
      if (process.HasExited || !String.Equals(process.ProcessName,"rice",StringComparison.OrdinalIgnoreCase)) throw new Exception("Not the owned live Rice process");
      var window = process.MainWindowHandle;
      uint owner; GetWindowThreadProcessId(window, out owner);
      if (window == IntPtr.Zero || owner != pid) throw new Exception("Missing owned Rice HWND");
      return window;
    }
  }
  public static object State(int pid) {
    var h=Window(pid); Rect r; if(!GetWindowRect(h,out r)) throw new Exception("GetWindowRect failed");
    var name=new StringBuilder(128); var title=new StringBuilder(128); GetClassName(h,name,128); GetWindowText(h,title,128);
    return new { hwnd=h.ToInt64(), windowClass=name.ToString(), title=title.ToString(), left=r.Left, top=r.Top, width=r.Right-r.Left, height=r.Bottom-r.Top, minimized=IsIconic(h), maximized=IsZoomed(h) };
  }
  public static void Restore(int pid) { var h=Window(pid); ShowWindow(h,9); SetForegroundWindow(h); Thread.Sleep(250); }
  public static void Prepare(int pid) {
    // Set up a deterministic on-screen fixture, not proof of UI resizing. The
    // actual titlebar/resize-handle tests separately measure physical input.
    var area=System.Windows.Forms.Screen.PrimaryScreen.WorkingArea;
    int width=Math.Min(1000,area.Width-100), height=Math.Min(680,area.Height-100);
    if(width<920 || height<560) throw new Exception("Desktop too small for the production window minimum");
    var h=Window(pid); ShowWindow(h,9);
    if(!SetWindowPos(h,IntPtr.Zero,area.Left+20,area.Top+30,width,height,0x0044)) throw new Exception("Cannot prepare owned on-screen window");
    SetForegroundWindow(h); Thread.Sleep(250);
  }
  public static void Click(int pid,int x,int y) {
    SetForegroundWindow(Window(pid)); Thread.Sleep(200); var p=Screen(pid,x,y); SetCursorPos(p.X,p.Y); Button(2); Button(4);
  }
  static Point Screen(int pid, int x, int y) {
    var p=new Point { X=x, Y=y }; if(!ClientToScreen(Window(pid),ref p)) throw new Exception("ClientToScreen failed"); return p;
  }
  static void Button(uint flags) {
    var input=new Input { type=0, value=new Union { mouse=new Mouse { flags=flags } } };
    if(SendInput(1,new [] {input},Marshal.SizeOf(typeof(Input))) != 1) throw new Exception("SendInput failed");
  }
  static void Move(Point from, Point to) {
    for(int step=1;step<=20;step++) { SetCursorPos(from.X+(to.X-from.X)*step/20,from.Y+(to.Y-from.Y)*step/20); Thread.Sleep(40); }
  }
  public static void Drag(int pid,int x,int y,int dx,int dy) {
    SetForegroundWindow(Window(pid)); Thread.Sleep(200);
    var from=Screen(pid,x,y); var to=new Point {X=from.X+dx,Y=from.Y+dy};
    SetCursorPos(from.X,from.Y); Button(2);
    try { Thread.Sleep(250); Move(from,to); } finally { Button(4); }
    Thread.Sleep(250);
  }
  public static string Drop(int pid,int x,int y,string[] files) {
    var target=Screen(pid,x,y); string result=null; Exception failure=null;
    using(var form=new Form()) {
      form.Text="Rice CI FileDrop"; form.StartPosition=FormStartPosition.Manual; form.Location=new System.Drawing.Point(10,10); form.ClientSize=new Size(140,60); form.TopMost=true;
      var label=new Label { Dock=DockStyle.Fill, Text="Owned CI file", TextAlign=ContentAlignment.MiddleCenter }; form.Controls.Add(label);
      label.MouseDown += (sender,args) => {
        try { var data=new DataObject(); data.SetData(DataFormats.FileDrop,files); result=label.DoDragDrop(data,DragDropEffects.Copy).ToString(); }
        catch(Exception e) { failure=e; }
        finally { form.BeginInvoke(new Action(form.Close)); }
      };
      var mover=new Thread(() => {
        try {
          Thread.Sleep(700); var from=new Point {X=65,Y=60}; SetCursorPos(from.X,from.Y); Button(2);
          try { Thread.Sleep(300); Move(from,target); Thread.Sleep(400); } finally { Button(4); }
        } catch(Exception e) { failure=e; }
      }); mover.IsBackground=true;
      using(var timeout=new System.Windows.Forms.Timer()) {
        timeout.Interval=15000; timeout.Tick+=(sender,args)=> { SendKeys.SendWait("{ESC}"); form.Close(); }; timeout.Start();
        form.Shown+=(sender,args)=>mover.Start(); Application.Run(form); timeout.Stop();
      }
      if(!mover.Join(3000)) throw new Exception("FileDrop input thread did not finish");
    }
    if(failure!=null) throw failure;
    if(result!="Copy") throw new Exception("Native FileDrop did not complete: "+result);
    return result;
  }
  public static IntPtr Dialog(int pid) {
    IntPtr found=IntPtr.Zero;
    EnumWindows((h,arg)=> { uint owner; GetWindowThreadProcessId(h,out owner); var name=new StringBuilder(128); GetClassName(h,name,128); if(owner==pid && name.ToString()=="#32770") found=h; return true; },IntPtr.Zero);
    return found;
  }
}
'@
if ($ValidateOnly) { Write-Output 'Native capability helper compiled successfully'; exit 0 }
function Owned-Files($Paths) {
    $result = @()
    foreach ($path in $Paths) {
        $full = [IO.Path]::GetFullPath([string]$path)
        if (-not $full.StartsWith($fixture + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetExtension($full) -ne '.exe' -or -not (Test-Path -LiteralPath $full -PathType Leaf)) { throw 'Only owned fixture executables may be selected or dropped' }
        $result += $full
    }
    if ($result.Count -lt 1 -or $result.Count -gt 3) { throw 'Unexpected fixture count' }
    return ,$result
}
while ($null -ne ($line = [Console]::ReadLine())) {
    $request = $line | ConvertFrom-Json
    try {
        switch ($request.action) {
            'state' { $value = [RiceNativeProbe]::State($RicePid) }
            'prepare' { [RiceNativeProbe]::Prepare($RicePid); $value = [RiceNativeProbe]::State($RicePid) }
            'restore' { [RiceNativeProbe]::Restore($RicePid); $value = [RiceNativeProbe]::State($RicePid) }
            'drag' { [RiceNativeProbe]::Drag($RicePid, $request.x, $request.y, $request.dx, $request.dy); $value = [RiceNativeProbe]::State($RicePid) }
            'click' { [RiceNativeProbe]::Click($RicePid, $request.x, $request.y); $value = $true }
            'drop' { $files = Owned-Files $request.paths; $value = [RiceNativeProbe]::Drop($RicePid, $request.x, $request.y, [string[]]$files) }
            'dialog' {
                $files = Owned-Files $request.paths
                $watch = [Diagnostics.Stopwatch]::StartNew()
                $dialog = [IntPtr]::Zero
                while ($watch.Elapsed.TotalSeconds -lt 20) {
                    $null = [RiceNativeProbe]::Window($RicePid)
                    $dialog = [RiceNativeProbe]::Dialog($RicePid)
                    if ($dialog -ne [IntPtr]::Zero) { break }
                    Start-Sleep -Milliseconds 100
                }
                if ($dialog -eq [IntPtr]::Zero) { throw 'Owned native file-open dialog did not appear' }
                $root = [Windows.Automation.AutomationElement]::FromHandle($dialog)
                $idProperty = [Windows.Automation.AutomationElement]::AutomationIdProperty
                $filenameHost = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new($idProperty, '1148'))
                if ($null -eq $filenameHost) { throw 'Missing native filename control (1148)' }
                $edit = $filenameHost.FindFirst([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, [Windows.Automation.ControlType]::Edit))
                if ($null -eq $edit) { $edit = $filenameHost }
                $pattern = $edit.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
                $pattern.SetValue(($files | ForEach-Object { '"' + $_ + '"' }) -join ' ')
                $open = $root.FindFirst([Windows.Automation.TreeScope]::Descendants, [Windows.Automation.PropertyCondition]::new($idProperty, '1'))
                if ($null -eq $open) { throw 'Missing owned file dialog Open button' }
                $open.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
                $value = [ordered]@{ selectedCount = $files.Count; ownerPid = $RicePid; nativeDialog = $true }
            }
            default { throw 'Unknown native UI action' }
        }
        [Console]::WriteLine((@{ id = $request.id; value = $value } | ConvertTo-Json -Compress -Depth 8))
    } catch {
        [Console]::WriteLine((@{ id = $request.id; error = $_.Exception.Message } | ConvertTo-Json -Compress))
    }
}
