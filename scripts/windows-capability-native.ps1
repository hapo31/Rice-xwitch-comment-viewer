param([Parameter(Mandatory = $true)][int]$RicePid, [Parameter(Mandatory = $true)][string]$FixtureRoot, [switch]$ValidateOnly)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or -not $env:RUNNER_TEMP) { throw 'Native UI probes require a disposable GitHub-hosted Windows runner' }
$fixture = [IO.Path]::GetFullPath($FixtureRoot)
if (-not $fixture.StartsWith([IO.Path]::GetFullPath($env:RUNNER_TEMP) + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Fixtures must be below RUNNER_TEMP' }
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -ReferencedAssemblies System.dll, System.Core.dll, System.Windows.Forms.dll, System.Drawing.dll -TypeDefinition @'
using System;
using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;
public static class RiceNativeProbe {
  [UnmanagedFunctionPointer(CallingConvention.Winapi)] public delegate bool EnumerateWindow(IntPtr window,IntPtr argument);
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int dx, dy; public uint data, flags, time; public UIntPtr extra; }
  [StructLayout(LayoutKind.Sequential)] public struct Keyboard { public ushort key, scan; public uint flags, time; public UIntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size=32)] public struct Union { [FieldOffset(0)] public Mouse mouse; [FieldOffset(0)] public Keyboard keyboard; }
  [StructLayout(LayoutKind.Sequential)] public struct Input { public uint type; public Union value; }
  [StructLayout(LayoutKind.Sequential)] public struct GuiThread { public uint size, flags; public IntPtr active, focus, capture, menuOwner, moveSize, caret; public Rect caretRect; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, Input[] inputs, int size);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern bool GetCursorPos(out Point p);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point point);
  [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr window,uint flags);
  [DllImport("user32.dll", EntryPoint="GetWindowLongW")] static extern int GetWindowLong(IntPtr window,int index);
  [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint thread, ref GuiThread info);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref Point p);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int command);
  [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int width,int height,uint flags);
  [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumerateWindow callback, IntPtr arg);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder text, int max);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder text, int max);
  [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h,uint message,IntPtr wparam,IntPtr lparam);
  [DllImport("user32.dll")] static extern IntPtr GetDlgItem(IntPtr dialog,int id);
  [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,EnumerateWindow callback,IntPtr argument);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr SendMessage(IntPtr window,uint message,IntPtr parameter,StringBuilder text);
  public static IntPtr Window(int pid) {
    using (var process = Process.GetProcessById(pid)) {
      if (process.HasExited || !String.Equals(process.ProcessName,"rice",StringComparison.OrdinalIgnoreCase)) throw new Exception("Not the owned live Rice process");
      // Process.MainWindowHandle switches to the 16px single-instance helper
      // when the real window is minimized. Bind to the exact owned UI instead.
      IntPtr window=IntPtr.Zero; int matches=0;
      EnumWindows((h,arg)=> {
        uint owner; GetWindowThreadProcessId(h,out owner);
        var title=new StringBuilder(128); GetWindowText(h,title,128);
        if(owner==pid && title.ToString()=="Rice") { window=h; matches++; }
        return true;
      },IntPtr.Zero);
      if(matches!=1) throw new Exception("Missing or ambiguous owned Rice UI HWND");
      return window;
    }
  }
  public static object State(int pid) {
    var h=Window(pid); Rect r; if(!GetWindowRect(h,out r)) throw new Exception("GetWindowRect failed");
    var name=new StringBuilder(128); var title=new StringBuilder(128); GetClassName(h,name,128); GetWindowText(h,title,128);
    var foreground=GetForegroundWindow(); uint foregroundPid; GetWindowThreadProcessId(foreground,out foregroundPid); Point cursor; GetCursorPos(out cursor);
    var foregroundClass=new StringBuilder(128); GetClassName(foreground,foregroundClass,128);
    return new { hwnd=h.ToInt64(), windowClass=name.ToString(), title=title.ToString(), left=r.Left, top=r.Top, width=r.Right-r.Left, height=r.Bottom-r.Top, visible=IsWindowVisible(h), enabled=IsWindowEnabled(h), style=GetWindowLong(h,-16), extendedStyle=GetWindowLong(h,-20), minimized=IsIconic(h), maximized=IsZoomed(h), foregroundHwnd=foreground.ToInt64(), foregroundPid=foregroundPid, foregroundClass=foregroundClass.ToString(), cursorX=cursor.X, cursorY=cursor.Y };
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
  public static void Focus(int pid,int x,int y) {
    var h=Window(pid);
    // Windows can deny SetForegroundWindow after NSIS ran in another process.
    // Expose only the owned fixture while physically activating it. Preserve
    // its original topmost style in finally; no UAC/global focus-policy change.
    bool wasTopmost=(GetWindowLong(h,-20)&8)!=0;
    try {
      if(!SetWindowPos(h,new IntPtr(-1),0,0,0,0,0x0013)) throw new Exception("Cannot expose owned UI for focus");
      var p=Screen(pid,x,y); var hit=IntPtr.Zero; var ready=Stopwatch.StartNew();
      // IsIconic/IsZoomed flags can update before the compositor completes
      // rapid maximize/restore/minimize transitions. Wait for the actual OS
      // hit-test to expose our window, not merely for a fixed short delay.
      while(ready.ElapsedMilliseconds<5000) {
        hit=GetAncestor(WindowFromPoint(p),2); if(hit==h) break; Thread.Sleep(100);
      }
      if(hit!=h) {
        uint owner; GetWindowThreadProcessId(hit,out owner); var name=new StringBuilder(128); GetClassName(hit,name,128);
        throw new Exception("Owned UI focus point ("+p.X+","+p.Y+") is occluded by HWND "+hit.ToInt64()+", PID "+owner+", class "+name+"; owned="+State(pid));
      }
      SetCursorPos(p.X,p.Y); Button(2); Thread.Sleep(200); Button(4);
    } finally {
      if(!SetWindowPos(h,new IntPtr(wasTopmost?-1:-2),0,0,0,0,0x0013)) throw new Exception("Cannot restore owned UI Z-order style");
    }
    Thread.Sleep(750); // Separate the setup click from a titlebar double click.
    if(GetForegroundWindow()!=h) throw new Exception("Physical activation did not focus the owned Rice UI");
  }
  public static void NativeClose(int pid) { if(!PostMessage(Window(pid),0x0010,IntPtr.Zero,IntPtr.Zero)) throw new Exception("Owned Rice WM_CLOSE failed"); }
  static Point Screen(int pid, int x, int y) {
    var p=new Point { X=x, Y=y }; if(!ClientToScreen(Window(pid),ref p)) throw new Exception("ClientToScreen failed"); return p;
  }
  static void Button(uint flags) {
    var input=new Input { type=0, value=new Union { mouse=new Mouse { flags=flags } } };
    if(SendInput(1,new [] {input},Marshal.SizeOf(typeof(Input))) != 1) throw new Exception("SendInput failed");
  }
  static Input Key(ushort key,ushort scan,uint flags) { return new Input { type=1,value=new Union {keyboard=new Keyboard {key=key,scan=scan,flags=flags}} }; }
  static void Keys(Input[] keys) {
    if(SendInput((uint)keys.Length,keys,Marshal.SizeOf(typeof(Input)))!=(uint)keys.Length) throw new Exception("Native keyboard input failed");
  }
  static void RequireDialogFocus(int pid,IntPtr dialog,IntPtr edit) {
    uint owner; var foreground=GetForegroundWindow(); var thread=GetWindowThreadProcessId(foreground,out owner);
    var info=new GuiThread {size=(uint)Marshal.SizeOf(typeof(GuiThread))};
    if(foreground!=dialog || owner!=pid || !GetGUIThreadInfo(thread,ref info) || info.focus!=edit) throw new Exception("Owned filename Edit is not the focused native control");
  }
  static void Move(Point from, Point to) {
    for(int step=1;step<=20;step++) { SetCursorPos(from.X+(to.X-from.X)*step/20,from.Y+(to.Y-from.Y)*step/20); Thread.Sleep(40); }
  }
  public static void Drag(int pid,int x,int y,int dx,int dy) {
    SetForegroundWindow(Window(pid)); Thread.Sleep(200);
    var from=Screen(pid,x,y); var to=new Point {X=from.X+dx,Y=from.Y+dy};
    SetCursorPos(from.X,from.Y); Button(2);
    // The physical mouse down reaches WebView/React before its asynchronous
    // startDragging IPC enters the native move loop. Allow that round trip to
    // settle before moving the pointer; never replace it with direct IPC.
    try { Thread.Sleep(750); Move(from,to); } finally { Button(4); }
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
    EnumWindows((h,arg)=> { uint owner; GetWindowThreadProcessId(h,out owner); var name=new StringBuilder(128); GetClassName(h,name,128); if(owner==pid && name.ToString()=="#32770" && GetDlgItem(h,1148)!=IntPtr.Zero && GetDlgItem(h,1)!=IntPtr.Zero) found=h; return true; },IntPtr.Zero);
    return found;
  }
  public static void SetDialogFiles(int pid,string text) {
    var dialog=Dialog(pid); var host=GetDlgItem(dialog,1148);
    if(dialog==IntPtr.Zero || host==IntPtr.Zero) throw new Exception("Missing owned filename host");
    IntPtr edit=IntPtr.Zero; int matches=0;
    EnumChildWindows(host,(h,arg)=> {
      uint owner; GetWindowThreadProcessId(h,out owner); var name=new StringBuilder(128); GetClassName(h,name,128);
      if(owner==pid && name.ToString()=="Edit") { edit=h; matches++; } return true;
    },IntPtr.Zero);
    if(matches!=1) throw new Exception("Missing or ambiguous owned native filename Edit");
    // Type into the actual focused native Edit, including normal EN_CHANGE/
    // dialog validation, rather than bypassing it with WM_SETTEXT or UIA.
    SetForegroundWindow(dialog); Thread.Sleep(250); Rect r;
    if(!GetWindowRect(edit,out r) || r.Right<=r.Left || r.Bottom<=r.Top) throw new Exception("Invalid native filename Edit bounds");
    SetCursorPos(r.Left+(r.Right-r.Left)/2,r.Top+(r.Bottom-r.Top)/2); Button(2); Button(4); Thread.Sleep(200);
    RequireDialogFocus(pid,dialog,edit);
    Keys(new [] {Key(0x11,0,0),Key(0x41,0,0),Key(0x41,0,2),Key(0x11,0,2)}); Thread.Sleep(100);
    var keys=new Input[text.Length*2];
    for(int n=0;n<text.Length;n++) { keys[n*2]=Key(0,text[n],4); keys[n*2+1]=Key(0,text[n],6); }
    Keys(keys);
    string actual=null; var watch=Stopwatch.StartNew();
    while(watch.ElapsedMilliseconds<5000) {
      var buffer=new StringBuilder(text.Length+2); SendMessage(edit,0x000D,new IntPtr(buffer.Capacity),buffer); actual=buffer.ToString();
      if(actual==text) break; Thread.Sleep(100);
    }
    if(actual!=text) throw new Exception("Native filename input mismatch: "+actual);
    RequireDialogFocus(pid,dialog,edit);
    Keys(new [] {Key(0x0D,0,0),Key(0x0D,0,2)});
  }
}
'@
if ($ValidateOnly) {
    # Exercise callback marshaling too; compilation alone cannot detect an
    # invalid generic P/Invoke delegate. PID -1 never matches a real process.
    if ([RiceNativeProbe]::Dialog(-1) -ne [IntPtr]::Zero) { throw 'Unexpected HWND for impossible PID' }
    Write-Output 'Native capability helper compiled and HWND enumeration validated'
    exit 0
}
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
            'focus' { [RiceNativeProbe]::Focus($RicePid, $request.x, $request.y); $value = [RiceNativeProbe]::State($RicePid) }
            'drag' { [RiceNativeProbe]::Drag($RicePid, $request.x, $request.y, $request.dx, $request.dy); $value = [RiceNativeProbe]::State($RicePid) }
            'click' { [RiceNativeProbe]::Click($RicePid, $request.x, $request.y); $value = $true }
            'close-native' { [RiceNativeProbe]::NativeClose($RicePid); $value = $true }
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
                $fileText = ($files | ForEach-Object { '"' + $_ + '"' }) -join ' '
                # Wait for the actual compound native controls, then enter and
                # accept the real file selection through native keyboard input.
                [RiceNativeProbe]::SetDialogFiles($RicePid, $fileText)
                $value = [ordered]@{ selectedCount = $files.Count; ownerPid = $RicePid; nativeDialog = $true }
            }
            default { throw 'Unknown native UI action' }
        }
        [Console]::WriteLine((@{ id = $request.id; value = $value } | ConvertTo-Json -Compress -Depth 8))
    } catch {
        [Console]::WriteLine((@{ id = $request.id; error = $_.Exception.Message } | ConvertTo-Json -Compress))
    }
}
